import { apiRead, accountRequest, captureLibraryView, assertLibraryView, currentDeviceSnapshot, isAccountMode, updateChapters, params, assertCatalogueScope, openLibrary, captureLibraryContext, assertLibraryContext } from './library';
import { HTTPError } from '$lib/HTTP';
import { filteredCatalog, pageRows, compareValue } from './catalog';
export interface UpdateChapterHistoryBatchRequest { read_ids?: string[]; unread_ids?: string[] }
async function localHistory(ids?: string[]) {
  await assertCatalogueScope();
  const snapshot = await currentDeviceSnapshot(), result: Record<string, Record<string, null>> = {};
  if (snapshot.migration_reason) throw new HTTPError(409, snapshot.migration_reason, 'Catalogue scope changed');
  for (const row of snapshot.records) if (row.family === 'chapters' && row.completed && (!ids || ids.includes(row.mangaId!))) (result[row.mangaId!] ??= {})[row.id!] = null;
  return result;
}
export async function Search(searchParams: URLSearchParams) {
  const q = params(searchParams), account = await isAccountMode(), view = captureLibraryView();
  const ids = q.getAll('id'), body = account ? await apiRead<Record<string, Record<string, null>>>('/user/history/manga', q, true) : await localHistory(ids.length ? ids : undefined);
  assertLibraryView(view); return { status: 200, body };
}
export async function List(mangaId: string) {
  const account = await isAccountMode(), view = captureLibraryView();
  const body = account ? await apiRead<Record<string, Record<string, null>>>(`/user/history/manga/${mangaId}`, undefined, true) : await localHistory([mangaId]);
  assertLibraryView(view); return { status: 200, body };
}
export async function Feed(searchParams: URLSearchParams) {
  const q = params(searchParams), account = await isAccountMode(), view = captureLibraryView();
  if (account) { const body = await apiRead<MappedChapterHistoryFeedResponse>('/user/history/manga/feed', q, true); assertLibraryView(view); return { status: 200, body }; }
  const snapshot = await currentDeviceSnapshot(), history = new Map(snapshot.records.filter(row => row.family === 'chapters' && row.completed).map(row => [row.id!, row]));
  const rows = await filteredCatalog<Chapter>('/chapter/feed', 'id', [...history.keys()], q);
  const chapters: ChapterHistory[] = rows.filter(row => history.has(row.id)).map(row => ({ ...row, read_at: history.get(row.id)!.readAt ?? new Date(history.get(row.id)!.updated_at * 1000).toISOString() }));
  const direction = q.get('order') === 'asc' ? 1 : -1;
  chapters.sort((a, b) => direction * compareValue(a.read_at, b.read_at) || compareValue(a.id, b.id));
  assertLibraryView(view); return { status: 200, body: { ...pageRows(chapters, q), map: { manga: Object.fromEntries(chapters.map(row => [row.relationships.manga.id, row.relationships.manga])) } } };
}
export async function UpdateBatch(mangaId: string, payload: UpdateChapterHistoryBatchRequest, admitted?: { context: ReturnType<typeof captureLibraryContext>; assert(): void }) {
  admitted?.assert(); await openLibrary(); admitted?.assert();
  const context = admitted?.context ?? captureLibraryContext();
  const assert = () => { admitted?.assert(); assertLibraryContext(context); };
  assert();
  if (context.mode === 'account') return accountRequest<UpdateChaptersHistoryBatchResponse>(`/user/history/manga/${mangaId}`, 'POST', payload, context.accountId!, assert);
  const body = await updateChapters(mangaId, payload.read_ids, payload.unread_ids, assert);
  assert(); return { status: 200, body };
}
