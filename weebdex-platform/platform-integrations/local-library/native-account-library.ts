import { get, writable } from 'svelte/store';
import { captureNativeAuthority, nativeAuthority, sameNativeReadAuthority } from '../frontend-behavior/mutation-authority';
import { createNativeAccountLibrary, type NativeAccountLibrary, type AccountFollowState, type AccountReadingStatus } from './native-account-library-client';
import { publicUuid } from '../public-api/validation';
// Canonical account metadata is memory-only and reset synchronously on every
// authority epoch change. It is never written into guest/device records.
export const nativeAccountProgressRevision = writable(0);
export const nativeAccountLibraryState = writable<{ owner?: string; epoch: number; follows: Record<string, [string, number]>; known: Record<string, boolean>; pending: boolean; error: string }>({ epoch: 0, follows: {}, known: {}, pending: false, error: '' });
let currentKey = '', generation = 0, access: Promise<NativeAccountLibrary> | undefined, active: NativeAccountLibrary | undefined;
const watchers = new Map<string, number>(), queued = new Set<string>(); let draining = false;
function clear() { generation++; active?.dispose(); active = undefined; access = undefined; queued.clear(); draining = false; const authority = captureNativeAuthority(); nativeAccountLibraryState.set({ owner: authority.actor, epoch: authority.epoch, follows: {}, known: {}, pending: false, error: '' }); }
nativeAuthority.subscribe(authority => { const key = JSON.stringify([authority.actor, authority.epoch]); if (key !== currentKey) { currentKey = key; clear(); if (authority.actor) { for (const id of watchers.keys()) queued.add(id); queue(); } } });
export async function accountLibrary(assertCurrent: () => void = () => {}) {
  const authority = captureNativeAuthority(); assertCurrent();
  if (!authority.actor) throw Error('Sign in to use your account library.');
  const check = () => { assertCurrent(); if (!sameNativeReadAuthority(authority)) throw Error('Your account changed. Reopen this library view.'); };
  if (!access) {
    const owner = generation;
    access = createNativeAccountLibrary(authority.actor, () => { if (generation !== owner || !sameNativeReadAuthority(authority)) throw Error('Your account changed. Reopen this library view.'); });
    void access.then(client => { if (generation === owner) active = client; else client.dispose(); }).catch(() => { if (generation === owner) access = undefined; });
  }
  const client = await access; check(); client.check(); return { client, check };
}
export function applyNativeAccountFollow(row: AccountFollowState) {
  const authority = captureNativeAuthority();
  nativeAccountLibraryState.update(state => {
    if (!authority.actor || state.owner !== authority.actor || state.epoch !== authority.epoch) return state;
    const follows = { ...state.follows }, known = { ...state.known, [row.mangaId]: true };
    if (row.saved) follows[row.mangaId] = [row.status!, row.notification ? 1 : 0]; else delete follows[row.mangaId];
    return { ...state, follows, known, error: '' };
  });
}
function queue() { if (queued.size && !draining) void Promise.resolve().then(drain); }
async function drain() {
  if (draining || !queued.size || !captureNativeAuthority().actor) return; draining = true;
  const owner = generation;
  nativeAccountLibraryState.update(state => ({ ...state, pending: true, error: '' }));
  try {
    const { client, check } = await accountLibrary();
    while (queued.size && generation === owner) {
      const selected = [...queued].slice(0, 50); for (const id of selected) queued.delete(id);
      const result = await client.followStates(selected); check(); if (generation !== owner) return;
      for (const row of result.data) applyNativeAccountFollow(row);
    }
  } catch (cause) { if (generation === owner) nativeAccountLibraryState.update(state => ({ ...state, error: cause instanceof Error ? cause.message : 'Saved-title status could not be loaded. Reload to try again.' })); }
  finally { if (generation === owner) { draining = false; nativeAccountLibraryState.update(state => ({ ...state, pending: false })); } }
}
// Components observe only IDs that they actually render. Microtask batching
// never searches the catalogue or fetches every private saved title.
export function observeAccountLibraryTitle(input: string) {
  const id = publicUuid(input); watchers.set(id, (watchers.get(id) ?? 0) + 1);
  if (captureNativeAuthority().actor && !get(nativeAccountLibraryState).known[id]) { queued.add(id); queue(); }
  return () => { const count = (watchers.get(id) ?? 1) - 1; if (count) watchers.set(id, count); else { watchers.delete(id); queued.delete(id); } };
}
export function refreshAccountLibraryTitles() { for (const id of watchers.keys()) queued.add(id); queue(); }
export async function readNativeLibrary(path: string, query: URLSearchParams, assertCurrent: () => void) {
  const { client, check } = await accountLibrary(assertCurrent); let result: unknown;
  if (path === '/user/follows/manga') {
    const page = await client.titles(query); check();
    for (const row of page.data) applyNativeAccountFollow({ mangaId: row.id, saved: row.saved, status: row.reading_status, notification: row.notification, followedAt: row.followed_at }); result = page;
  } else if (path === '/user/follows/chapter/feed' || path === '/user/follows/chapter/updates') {
    const selected = new URLSearchParams(query); if (path.endsWith('/updates')) selected.set('latest', 'true'); result = await client.chapters('updates', selected);
  } else if (path === '/user/history/manga/feed') result = await client.chapters('history', query);
  else if (path === '/user/history/manga' || /^\/user\/history\/manga\/[^/]+$/.test(path)) {
    throw Error('Choose a visible chapter page before loading account completion. A whole-title history scan was not requested.');
  } else throw Error('This account library operation is not supported.');
  check(); return result;
}
export async function mutateNativeLibrary(path: string, method: string, payload: unknown, assertCurrent: () => void) {
  const { client, check } = await accountLibrary(assertCurrent);
  const follows = /^\/user\/follows\/manga\/([^/]+)$/.exec(path), history = /^\/user\/history\/manga\/([^/]+)$/.exec(path);
  if (follows) {
    const id = publicUuid(follows[1]); client.requireConfirmed('follow-' + id); const observed = await client.followStates([id]); check();
    const command = payload as { status?: AccountReadingStatus; notification?: boolean } | undefined;
    const receipt = method === 'DELETE' ? await client.removeFollow(id, observed.revision) : method === 'POST' && command?.status ? await client.setFollow(id, observed.revision, command.status, command.notification ?? observed.data[0].notification ?? true) : undefined;
    if (!receipt) throw Error('Invalid saved-title command.'); check(); applyNativeAccountFollow(receipt.data as AccountFollowState); return { status: 204 };
  }
  if (history && method === 'POST') {
    const id = publicUuid(history[1]), command = payload as { read_ids?: string[]; unread_ids?: string[] }, selected = [...(command.read_ids ?? []), ...(command.unread_ids ?? [])];
    client.requireConfirmed('history-' + id); const observed = await client.historyState([id], 1, selected); check();
    const receipt = await client.updateHistory(id, observed.revision, command.read_ids ?? [], command.unread_ids ?? []); check(); return { status: 200, body: receipt.data };
  }
  throw Error('This account library change is not supported. No device data was changed.');
}
export async function saveNativeAccountProgress(chapter: string, oneBasedPage: number, completed: boolean, assertCurrent: () => void) {
  const { client, check } = await accountLibrary(assertCurrent); client.requireConfirmed('progress-' + publicUuid(chapter)); const observed = await client.progress(chapter); check();
  completed ||= observed.data.completed; // Re-reading an earlier page never silently marks a completed chapter unread.
  const page = oneBasedPage - 1; if (!Number.isSafeInteger(page) || page < 0) throw Error('Invalid reading page.');
  if (observed.data.page === page && observed.data.completed === completed) return;
  const result = await client.saveProgress(chapter, observed.revision, page, completed, observed.data.version); check(); nativeAccountProgressRevision.update(value => value + 1); return result;
}

// Exact currently rendered chapters only. No whole-title or account-history scan.
export async function readNativeVisibleChapterHistory(chapters: readonly { id: string; relationships: { manga: { id: string } } }[], assertCurrent: () => void) {
  assertCurrent(); if (!chapters.length || chapters.length > 100) throw Error('Read status needs a visible page of 1 to 100 chapters.');
  const selected = chapters.map(row => ({ id: publicUuid(row.id), manga: publicUuid(row.relationships.manga.id) }));
  if (new Set(selected.map(row => row.id)).size !== selected.length) throw Error('Duplicate visible chapter IDs.');
  const parents = [...new Set(selected.map(row => row.manga))], expected = new Map(selected.map(row => [row.id, row.manga]));
  const { client, check } = await accountLibrary(assertCurrent); const result = await client.historyState(parents, 1, selected.map(row => row.id)); check();
  if (result.total !== result.data.length) throw Error('Visible chapter read status is incomplete.');
  const history: Record<string, Record<string, null>> = Object.fromEntries(parents.map(id => [id, {}]));
  for (const row of result.data) { if (expected.get(row.chapterId) !== row.mangaId) throw Error('A completed chapter changed its title.'); history[row.mangaId][row.chapterId] = null; }
  check(); return { status: 200, body: history };
}
// Explicit new-view/retry observation. This never sends a mutation and is the
// only reader initialization step allowed to reconcile an uncertain page save.
export async function checkNativeAccountProgress(chapter: string, assertCurrent: () => void) {
  const { client, check } = await accountLibrary(assertCurrent);
  const id = publicUuid(chapter), result = await client.progress(id); check(); client.requireConfirmed('progress-' + id); return result;
}
