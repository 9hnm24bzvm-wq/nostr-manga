// Private canonical-account library; no signer, protocol publication or device fallback.
import { createNativeAccountScope, nativeAccountError, type NativeAccountDependencies } from '../public-api/native-account-scope';
import { publicUuid, record, resource, PublicApiContractError } from '../public-api/validation';
import { normalizeManga, normalizeChapter } from '../public-api/normalize';
const activeWrites = new Set<string>();
export const accountReadingStatuses = ['reading', 'plan_to_read', 'completed', 'on_hold', 're_reading', 'dropped'] as const;
export type AccountReadingStatus = typeof accountReadingStatuses[number];
export interface AccountFollowState { mangaId: string; saved: boolean; status?: AccountReadingStatus; notification?: boolean; followedAt?: string }
export interface AccountList { id: string; name: string; version: number; memberCount: number; mangaIds?: string[]; updatedAt: string }
export interface AccountProgress { chapterId: string; mangaId: string; page: number | null; completed: boolean; version: number }
function fail(message: string, status = 502): never { throw new PublicApiContractError(message, status); }
function integer(input: unknown, minimum = 0): number { if (!Number.isSafeInteger(input) || Number(input) < minimum) fail('The account library returned an invalid version or count.'); return Number(input); }
function instant(input: unknown): string { if (typeof input !== 'string' || input.length > 40 || !Number.isFinite(Date.parse(input))) fail('The account library returned an invalid date.'); return input; }
function status(input: unknown): AccountReadingStatus { if (!accountReadingStatuses.includes(input as AccountReadingStatus)) fail('The account library returned an invalid reading status.'); return input as AccountReadingStatus; }
function ids(input: readonly string[], limit = 100): string[] { if (input.length > limit) fail('Choose at most ' + limit + ' items at a time.', 422); const values = input.map(publicUuid); if (new Set(values).size !== values.length) fail('Duplicate account-library items are not allowed.', 422); return values; }
function follow(input: unknown): AccountFollowState {
  const row = record(input), mangaId = publicUuid(row.mangaId); if (typeof row.saved !== 'boolean') fail('Invalid saved-title state.');
  if (!row.saved) return { mangaId, saved: false };
  if (typeof row.notification !== 'boolean') fail('Invalid chapter update preference.');
  return { mangaId, saved: true, status: status(row.status), notification: row.notification, followedAt: instant(row.followedAt) };
}
function list(input: unknown, detailed = false): AccountList {
  const row = record(input), id = publicUuid(row.id), name = row.name;
  if (typeof name !== 'string' || !name.trim() || name.length > 200) fail('Invalid account list name.');
  const result: AccountList = { id, name, version: integer(row.version, 1), memberCount: row.memberCount === undefined && Array.isArray(row.mangaIds) ? row.mangaIds.length : integer(row.memberCount), updatedAt: instant(row.updatedAt) };
  if (detailed) { if (!Array.isArray(row.mangaIds) || row.mangaIds.some(id => typeof id !== 'string')) fail('Invalid account list members.'); result.mangaIds = ids(row.mangaIds as string[]); if (result.memberCount !== result.mangaIds.length) fail('Account list membership is incomplete.'); }
  return result;
}
function progress(input: unknown): AccountProgress {
  const row = record(input), page = row.page === null ? null : integer(row.page), version = integer(row.version);
  if (typeof row.completed !== 'boolean' || page === null && version !== 0 || page !== null && version < 1) fail('Invalid account reading position.');
  return { chapterId: publicUuid(row.chapterId), mangaId: publicUuid(row.mangaId), page, completed: row.completed, version };
}
function controls(input: URLSearchParams) {
  const q = new URLSearchParams(input), page = Number(q.get('page') ?? 1), limit = Number(q.get('limit') ?? 20);
  if (q.getAll('page').length > 1 || q.getAll('limit').length > 1 || !Number.isSafeInteger(page) || page < 1 || page > 1000000 || !Number.isSafeInteger(limit) || limit < 1 || limit > 100 || (page - 1) * limit > 10000) fail('Choose a valid library page size and page.', 422);
  q.set('page', String(page)); q.set('limit', String(limit)); return { q, page, limit };
}
export async function createNativeAccountLibrary(nativeAccount: string, assertCurrent: () => void, injected: Partial<NativeAccountDependencies> = {}) {
  const scope = await createNativeAccountScope(nativeAccount, assertCurrent, injected, { requireVerified: false });
  const check = scope.check, accountId = scope.account.id; let writing = false;
  function envelope(input: unknown) { const body = record(input); if (body.result !== 'ok' || body.authentication !== 'native-session' || body.scope !== 'instance-account' || publicUuid(body.accountId) !== accountId) fail('The library belongs to another account. Reload before continuing.', 409); return { body, revision: integer(body.revision) }; }
  const fenceKey = (target: string) => scope.key('library-' + target);
  function clearFence(target: string, observed: string | null = scope.fences.get(fenceKey(target)), acknowledged = false) { check(); if (!acknowledged && activeWrites.has(fenceKey(target))) return; if (scope.fences.get(fenceKey(target)) === observed) scope.fences.delete(fenceKey(target)); }
  function requireConfirmed(target: string) { check(); if (scope.fences.get(fenceKey(target))) fail('The previous save is not confirmed. Reload this item and check it before trying again.', 409); }
  async function send(path: string, method: 'GET' | 'POST' | 'DELETE' = 'GET', payload?: unknown) {
    const response = await scope.send('/library' + path, method, payload); check();
    if (response.status !== 200 && response.status !== 201) throw nativeAccountError(response.status, response.body);
    return envelope(response.body);
  }
  async function write<T>(target: string, path: string, revision: number, payload: Record<string, unknown>, decode: (data: unknown) => T, method: 'POST' | 'DELETE' = 'POST') {
    check(); integer(revision); if (writing) fail('Another library change is still being saved.', 409);
    requireConfirmed(target);
    // Store before dispatch; storage failure must not send an untracked mutation.
    scope.fences.set(fenceKey(target), JSON.stringify({ revision, at: Date.now(), ...(target.startsWith('history-') ? { selected: [...payload.readIds as string[], ...payload.unreadIds as string[]] } : {}) })); writing = true; activeWrites.add(fenceKey(target));
    try {
      const result = await send(path, method, { ...payload, expectedAccountId: accountId, revision });
      if (result.revision < revision) fail('The account library receipt is older than the reviewed state.');
      const data = decode(result.body.data ?? result.body); check(); clearFence(target, scope.fences.get(fenceKey(target)), true); return { revision: result.revision, data };
    } catch (cause) {
      // No retry, optimistic success, device fallback or source-record deletion.
      // A new authoritative read is required before another user-triggered save.
      throw cause;
    } finally { writing = false; activeWrites.delete(fenceKey(target)); }
  }
  function page(input: unknown, expected: { page: number; limit: number }) {
    const result = envelope(input), total = integer(result.body.total);
    if (result.body.limit !== expected.limit || result.body.page !== expected.page || !Array.isArray(result.body.data) || result.body.data.length !== Math.min(expected.limit, Math.max(0, total - (expected.page - 1) * expected.limit))) fail('The account library returned an incomplete page.');
    return { ...result, total, page: expected.page, limit: expected.limit, rows: result.body.data };
  }
  async function collection(path: string, query: URLSearchParams) { const selection = controls(query), response = await scope.send('/library' + path + '?' + selection.q.toString().replace(/^\?/, '')); check(); if (response.status !== 200) throw nativeAccountError(response.status, response.body); return page(response.body, selection); }
  async function followStates(selected: readonly string[]) {
    const wanted = ids(selected), fences = new Map(wanted.map(id => [id, scope.fences.get(fenceKey('follow-' + id))])); if (!wanted.length) fail('Choose at least one title.', 422); const q = new URLSearchParams(); for (const id of wanted) q.append('ids[]', id);
    const { body, revision } = await send('/follows/state?' + q.toString().replace(/^\?/, '')); if (!Array.isArray(body.data) || body.data.length !== wanted.length) fail('The account returned an incomplete saved-title selection.');
    const data = body.data.map(follow), seen = new Set(data.map(row => row.mangaId)); if (seen.size !== wanted.length || wanted.some(id => !seen.has(id))) fail('The saved-title response changed its exact selection.');
    check(); for (const id of wanted) clearFence('follow-' + id, fences.get(id) ?? null); return { revision, data };
  }
  async function titles(query: URLSearchParams) {
    const selected = new URLSearchParams(query), rawList = selected.get('list'), listId = rawList === null ? null : publicUuid(rawList);
    if (listId) { if (selected.getAll('list').length !== 1) fail('Choose one account list.', 422); selected.delete('list'); }
    const result = await collection(listId ? '/lists/' + listId + '/titles' : '/follows', selected);
    if (listId && (result.body.listId !== listId || !Number.isSafeInteger(result.body.listVersion) || Number(result.body.listVersion) < 1)) fail('The list selection changed.');
    const data = result.rows.map(input => {
      const row = record(input), manga = normalizeManga(resource(row.manga, 'manga'));
      if (listId && row.saved === false) { if (row.status !== null || row.notification !== null || row.followedAt !== null) fail('Invalid unsaved list member.'); return { ...manga, saved: false, reading_status: undefined, notification: undefined, followed_at: undefined }; }
      if (listId && row.saved !== true) fail('Invalid list-member save state.');
      if (typeof row.notification !== 'boolean') fail('Invalid saved title update preference.');
      return { ...manga, saved: true, reading_status: status(row.status), notification: row.notification, followed_at: instant(row.followedAt) };
    });
    if (new Set(data.map(row => row.id)).size !== data.length) fail('Duplicate saved titles were returned.');
    return { data, total: result.total, limit: result.limit, page: result.page, revision: result.revision };
  }
  async function chapters(kind: 'updates' | 'history', query: URLSearchParams) {
    const result = await collection(kind === 'updates' ? '/updates' : '/history/feed', query), data = result.rows.map(input => {
      const row = record(input), manga = normalizeManga(resource(row.manga, 'manga')), chapter = normalizeChapter(resource(row.chapter, 'chapter'), manga);
      if (typeof row.completed !== 'boolean') fail('Invalid completed chapter state.');
      return { ...chapter, completed: row.completed, ...(row.readAt === undefined ? {} : { read_at: instant(row.readAt) }) };
    });
    if (new Set(data.map(row => row.id)).size !== data.length) fail('Duplicate chapters were returned.');
    return { data, total: result.total, limit: result.limit, page: result.page, revision: result.revision, map: { manga: Object.fromEntries(data.map(row => [row.relationships.manga.id, row.relationships.manga])) } };
  }
  async function historyState(selectedManga: readonly string[], pageNumber = 1, rawSelectedChapters?: readonly string[]) {
    const selectedChapters = rawSelectedChapters && ids(rawSelectedChapters);
    const wanted = ids(selectedManga), fences = new Map(wanted.map(id => [id, scope.fences.get(fenceKey('history-' + id))])); if (!wanted.length) fail('Choose at least one title.', 422); const q = new URLSearchParams({ page: String(pageNumber), limit: '100' }); if (selectedChapters) { const chapters = ids(selectedChapters); if (!chapters.length) fail('Choose at least one chapter.', 422); for (const id of chapters) q.append('ids[]', id); } else for (const id of wanted) q.append('manga[]', id);
    const result = await collection('/history/state', q);
    if (selectedChapters && (!Array.isArray(result.body.selection) || result.body.selection.length !== selectedChapters.length || result.body.selection.some((id, index) => id !== selectedChapters[index]) || result.total > selectedChapters.length)) fail('Exact account history coverage was not confirmed.');
    const data = result.rows.map(input => { const row = record(input), mangaId = publicUuid(row.mangaId), chapterId = publicUuid(row.chapterId); if (!wanted.includes(mangaId) || selectedChapters && !selectedChapters.includes(chapterId)) fail('History escaped the selected titles or chapters.'); return { mangaId, chapterId, readAt: instant(row.readAt) }; });
    if (new Set(data.map(row => row.chapterId)).size !== data.length) fail('Duplicate history rows.');
    for (const id of wanted) { const fence = fences.get(id) ?? null;
      // An unrelated visible chapter may not resolve an uncertain batch for
      // another chapter of the same title. Only exact complete coverage can.
      let covered = !fence;
      if (fence && selectedChapters && result.total === data.length) { try { const pending = JSON.parse(fence); covered = Array.isArray(pending.selected) && pending.selected.length > 0 && pending.selected.every((chapter: unknown) => typeof chapter === 'string' && selectedChapters.includes(chapter)); } catch { covered = false; } }
      if (covered) clearFence('history-' + id, fence);
    }
    return { data, revision: result.revision, total: result.total, page: result.page, limit: result.limit };
  }
  async function readProgress(id: string) { const chapterId = publicUuid(id), fence = scope.fences.get(fenceKey('progress-' + chapterId)), result = await send('/progress/' + chapterId), data = progress(result.body.data); if (data.chapterId !== chapterId) fail('Progress belongs to a different chapter.'); clearFence('progress-' + chapterId, fence); return { revision: result.revision, data }; }
  async function readList(id: string) { const listId = publicUuid(id), fence = scope.fences.get(fenceKey('list-' + listId)), result = await send('/lists/' + listId), data = list(result.body.data, true); if (data.id !== listId) fail('The account returned a different list.'); clearFence('list-' + listId, fence); return { revision: result.revision, data }; }
  async function lists(query = new URLSearchParams({ limit: '20', page: '1' })) { const fence = scope.fences.get(fenceKey('create-list')), result = await collection('/lists', query), data = result.rows.map(row => list(row)); if (new Set(data.map(row => row.id)).size !== data.length) fail('Duplicate account lists.'); if (result.total === data.length && result.page === 1) clearFence('create-list', fence); return { data, revision: result.revision, total: result.total, page: result.page, limit: result.limit }; }
  async function reconcileList(id: string) {
    const target = publicUuid(id), fence = scope.fences.get(fenceKey('list-' + target));
    const observed = await lists(new URLSearchParams({ page: '1', limit: '100' }));
    if (observed.total !== observed.data.length) fail('Account list reconciliation is incomplete.');
    const found = observed.data.find(row => row.id === target); check(); clearFence('list-' + target, fence);
    return { revision: observed.revision, data: found };
  }
  function listName(name: string) { const clean = name.trim(); if (!clean || clean.length > 200 || clean.includes('\0')) fail('Choose a list name from 1 to 200 characters.', 422); return clean; }
  return {
    account: scope.account, check, requireConfirmed, dispose: scope.dispose, titles, chapters, followStates, historyState, progress: readProgress, lists, list: readList, reconcileList,
    setFollow(id: string, revision: number, readingStatus: AccountReadingStatus, notification: boolean) { const target = publicUuid(id); status(readingStatus); if (typeof notification !== 'boolean') fail('Choose whether to show chapter updates.', 422); return write('follow-' + target, '/follows/' + target, revision, { status: readingStatus, notification }, input => { const value = follow(input); if (value.mangaId !== target || !value.saved || value.status !== readingStatus || value.notification !== notification) fail('Saved-title receipt does not match the request.'); return value; }); },
    removeFollow(id: string, revision: number) { const target = publicUuid(id); return write('follow-' + target, '/follows/' + target, revision, {}, input => { const value = follow(input); if (value.mangaId !== target || value.saved) fail('The title removal was not confirmed.'); return value; }, 'DELETE'); },
    updateHistory(mangaId: string, revision: number, readIds: readonly string[], unreadIds: readonly string[]) { const target = publicUuid(mangaId), reads = ids(readIds), unreads = ids(unreadIds); if (!reads.length && !unreads.length || reads.length + unreads.length > 100 || reads.some(id => unreads.includes(id))) fail('Choose 1 to 100 different chapters and one completion state per chapter.', 422); return write('history-' + target, '/history/' + target, revision, { readIds: reads, unreadIds: unreads }, input => { const row = record(input), readUpdated = integer(row.readUpdated), unreadUpdated = integer(row.unreadUpdated); if (readUpdated > reads.length || unreadUpdated > unreads.length) fail('Invalid history receipt.'); return { read_updated: readUpdated, unread_updated: unreadUpdated }; }); },
    saveProgress(id: string, revision: number, page: number, completed: boolean, progressVersion: number) { const target = publicUuid(id); integer(page); integer(progressVersion); if (typeof completed !== 'boolean') fail('Invalid progress completion state.', 422); return write('progress-' + target, '/progress/' + target, revision, { page, completed, progressVersion }, input => { const value = progress(input); if (value.chapterId !== target || value.page !== page || value.completed !== completed || value.version <= progressVersion) fail('Reading progress was not confirmed.'); return value; }); },
    createList(revision: number, name: string, mangaIds: readonly string[] = []) { const selected = ids(mangaIds), clean = listName(name); return write('create-list', '/lists', revision, { name: clean, mangaIds: selected }, input => { const value = list(input, true); if (value.name !== clean || JSON.stringify(value.mangaIds) !== JSON.stringify(selected)) fail('New list receipt does not match the reviewed list.'); return value; }); },
    updateList(id: string, revision: number, listVersion: number, change: { name?: string; mangaIds?: readonly string[] }) { const target = publicUuid(id); integer(listVersion, 1); if (change.name === undefined && change.mangaIds === undefined) fail('Choose a list change first.', 422); const payload = { listVersion, ...(change.name === undefined ? {} : { name: listName(change.name) }), ...(change.mangaIds === undefined ? {} : { mangaIds: ids(change.mangaIds) }) }; return write('list-' + target, '/lists/' + target, revision, payload, input => { const value = list(input, true); if (value.id !== target || value.version <= listVersion || payload.name !== undefined && value.name !== payload.name || payload.mangaIds !== undefined && JSON.stringify(value.mangaIds) !== JSON.stringify(payload.mangaIds)) fail('List change receipt does not match the review.'); return value; }); },
    removeList(id: string, revision: number, listVersion: number) { const target = publicUuid(id); integer(listVersion, 1); return write('list-' + target, '/lists/' + target, revision, { listVersion }, input => { const row = record(input); if (row.deleted !== true || publicUuid(row.id) !== target) fail('List deletion was not confirmed.'); return { deleted: true, id: target }; }, 'DELETE'); }
  };
}
export type NativeAccountLibrary = Awaited<ReturnType<typeof createNativeAccountLibrary>>;
