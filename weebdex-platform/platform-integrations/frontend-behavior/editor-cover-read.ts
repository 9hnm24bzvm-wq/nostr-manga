// Private draft cover reads have an explicit browser actor and target lifetime.
// Public Cover.List is deliberately unchanged.
import HTTP, { HTTPError } from '$lib/HTTP';
import Session from '$lib/stores/Session';
import { PUBLIC_API_URL, PUBLIC_COVER_URL } from '$env/static/public';
import { get } from 'svelte/store';
import { onDestroy } from 'svelte';
import { privateFeedIdentity } from './private-feed';
import type { MutationIntent } from './authenticated-mutation';

export function createEditorCoverReader(target: () => string) {
  let disposed = false, epoch = 0, identity = get(privateFeedIdentity);
  const active = new Set<AbortController>();
  const unsubscribe = privateFeedIdentity.subscribe(next => {
    if (next === identity) return;
    identity = next; epoch++;
    for (const controller of active) controller.abort();
    active.clear();
  });
  onDestroy(() => { disposed = true; epoch++; unsubscribe(); for (const controller of active) controller.abort(); active.clear(); });
  async function read<T>(path: string, id: string, intent?: MutationIntent): Promise<T> {
    const actor = intent?.actor ?? get(Session)?.identity?.id, captured = epoch, key = identity;
    const controller = new AbortController(); active.add(controller);
    const assert = () => {
      intent?.assert();
      if (!actor || disposed || controller.signal.aborted || target() !== id || epoch !== captured || identity !== key || get(Session)?.identity?.id !== actor)
        throw new HTTPError(409, 'The editor target or browser account changed. Old cover data was discarded.', 'Cover read context changed');
    };
    const http = new HTTP<T>(PUBLIC_API_URL.replace(/\/$/, '') + path);
    http.setWithCredentials(true); http.setRequestHeader('X-WeebDex-Expected-Account', actor ?? '');
    const cancel = () => http.cancel();
    controller.signal.addEventListener('abort', cancel, { once: true });
    intent?.signal.addEventListener('abort', cancel, { once: true });
    try {
      assert(); const result = await http.GET(); assert();
      if (result?.status !== 200 || result.body === undefined || result.body === null)
        throw new HTTPError(result?.status ?? 0, `Current editor data could not be read${result?.status ? ` (HTTP ${result.status})` : ''}. No empty cover list or successful mutation was inferred.`, 'Editor cover read failed');
      return result.body;
    } finally { active.delete(controller); controller.signal.removeEventListener('abort', cancel); intent?.signal.removeEventListener('abort', cancel); }
  }
  const capture = (): MutationIntent => {
    const actor = get(Session)?.identity?.id, captured = epoch, key = identity, id = target(), href = window.location.href;
    if (!actor || disposed) throw new HTTPError(401, 'A current browser account is required to read this editor.');
    const controller = new AbortController(); active.add(controller);
    const assert = () => {
      if (disposed || controller.signal.aborted || captured !== epoch || key !== identity || id !== target() || href !== window.location.href || get(Session)?.identity?.id !== actor)
        throw new HTTPError(409, 'The editor navigation, target or account changed. Old private data was discarded.', 'Editor read context changed');
    };
    return { actor, signal: controller.signal, assert, alive: () => { try { assert(); return true; } catch { return false; } }, release: () => { active.delete(controller); } };
  };
  return {
    capture,
    currentTitle: async (id: string, intent: MutationIntent) => {
      try { return await read<Manga>(`/manga/${encodeURIComponent(id)}`, id, intent); }
      catch (cause) {
        intent.assert();
        if (!(cause instanceof HTTPError) || ![403, 404].includes(cause.code)) throw cause;
        return read<Manga>(`/manga/draft/${encodeURIComponent(id)}`, id, intent);
      }
    },
    covers: async (manga: Manga, intent?: MutationIntent) => {
      if (!manga.relationships || typeof manga.language !== 'string') throw new HTTPError(409, 'This is a minimal publication receipt, not an editable title. No private cover request was made.');
      const result = await read<Cover[]>(`/manga/${encodeURIComponent(manga.id)}/covers${manga.state === 'published' ? '' : '?draft=true'}`, manga.id, intent);
      if (!Array.isArray(result)) throw new HTTPError(502, 'The server did not return a current cover list.');
      return result;
    },
    manga: (manga: Manga, intent?: MutationIntent) => read<Manga>(manga.state === 'published' ? `/manga/${encodeURIComponent(manga.id)}` : `/manga/draft/${encodeURIComponent(manga.id)}`, manga.id, intent)
  };
}

export function savedCoverDraft(manga: Manga, cover: Cover, main = cover.id === manga.relationships.cover?.id): CoverDraft {
  return { ...cover, valid: true, main, url: `${PUBLIC_COVER_URL}/${manga.id}/${cover.id}${cover.ext}`, thumbnail: `${PUBLIC_COVER_URL}/${manga.id}/${cover.id}.512.webp` };
}

// Refresh real IDs/versions without replacing local metadata edits or pending Files.
export function mergeCurrentCoverDrafts(previous: CoverDraft[], edited: CoverDraft[], fresh: CoverDraft[]) {
  const prior = new Map(previous.map(row => [row.id, row])), current = new Map(fresh.map(row => [row.id, row]));
  const merged: CoverDraft[] = [];
  for (const row of edited) {
    if (row.file) { merged.push(row); continue; }
    const saved = current.get(row.id); if (!saved) continue;
    current.delete(row.id);
    const before = prior.get(row.id), next: CoverDraft = { ...saved };
    for (const field of ['volume', 'description', 'language', 'main', 'deleted'] as const)
      if (before && row[field] !== before[field]) (next as any)[field] = row[field];
    next.valid = row.valid; merged.push(next);
  }
  // Real newly observed server rows are not silently equated with unknown uploads.
  return [...merged, ...current.values()];
}
