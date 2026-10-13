// Browser mutations retain the actor who admitted their intent, including limiter waits.
import HTTP, { HTTPError, type HTTPResponse } from '$lib/HTTP';
import Session from '$lib/stores/Session';
import { PUBLIC_API_URL } from '$env/static/public';
import { get } from 'svelte/store';
import { onDestroy } from 'svelte';
let authorityGeneration = 0, watchingAuthority = false;
function ensureAuthorityWatch() {
  if (watchingAuthority) return;
  watchingAuthority = true;
  // Lazy registration avoids API/Session import-cycle initialization side effects.
  Session.subscribe(() => { authorityGeneration++; });
}
export interface MutationIntent {
  readonly actor: string;
  readonly signal: AbortSignal;
  assert(): void;
  alive(): boolean;
  release(): void;
}
export function createMutationOwner(target: () => string) {
  ensureAuthorityWatch();
  let disposed = false;
  const controllers = new Set<AbortController>();
  const invalidate = () => { for (const controller of controllers) controller.abort(); controllers.clear(); };
  const unsubscribe = Session.subscribe(invalidate);
  onDestroy(() => { disposed = true; invalidate(); unsubscribe(); });
  return { invalidate, capture(): MutationIntent {
    const actor = get(Session)?.identity?.id, generation = authorityGeneration, key = target();
    if (!actor || disposed) throw new HTTPError(401, 'A current browser account is required. No mutation was sent.');
    const controller = new AbortController(); controllers.add(controller);
    const assert = () => { if (disposed || controller.signal.aborted || key !== target() || generation !== authorityGeneration || get(Session)?.identity?.id !== actor) throw new HTTPError(409, 'The actor or target changed. An already sent operation may have succeeded for its original actor. Reload that current state before trying again; no mutation was replayed.', 'Previous mutation context changed'); };
    return { actor, signal: controller.signal, assert, alive: () => { try { assert(); return true; } catch { return false; } }, release: () => { controllers.delete(controller); } };
  } };
}
export async function authenticatedMutation<T>(path: string, method: 'POST' | 'PUT' | 'PATCH' | 'DELETE', payload?: unknown, intent?: MutationIntent): Promise<HTTPResponse<T> | undefined> {
  ensureAuthorityWatch();
  const actor = intent?.actor ?? get(Session)?.identity?.id, generation = authorityGeneration;
  if (!actor || !path.startsWith('/') || path.startsWith('//') || path.includes('://')) throw new HTTPError(401, 'A current browser account and local route are required. No mutation was sent.');
  const assert = () => { intent?.assert(); if (generation !== authorityGeneration || get(Session)?.identity?.id !== actor) throw new HTTPError(409, 'Browser authority changed. Reload the original account state before submitting again; the operation may have succeeded and was not replayed.', 'Previous mutation context changed'); };
  assert();
  const http = new HTTP<T>(PUBLIC_API_URL.replace(/\/$/, '') + path, payload);
  http.setWithCredentials(true); http.setRequestHeader('X-WeebDex-Expected-Account', actor);
  const cancel = () => http.cancel();
  const unsubscribe = Session.subscribe(() => { try { assert(); } catch { cancel(); } });
  intent?.signal.addEventListener('abort', cancel, { once: true });
  try { assert(); const result = await http[method](); assert(); return result; }
  finally { unsubscribe(); intent?.signal.removeEventListener('abort', cancel); }
}
