import {assertCataloguePreviewLibraryReady} from '../public-api/library-staging';
import { apiRead, accountRequest, captureLibraryView, assertLibraryView, currentDeviceSnapshot, isCurrentCatalogueId, isAccountMode, updateChapters, params, assertCatalogueScope, openLibrary, captureLibraryContext, assertLibraryContext } from './library';
import { HTTPError } from '$lib/HTTP';
import { openDeviceHistoryFeed } from './history-feed';
export { openDeviceHistoryFeed } from './history-feed';
export interface UpdateChapterHistoryBatchRequest { read_ids?: string[]; unread_ids?: string[] }
async function localHistory(ids?: string[]) {
  await assertCatalogueScope();
  const snapshot = await currentDeviceSnapshot(), result: Record<string, Record<string, null>> = {};
  if (snapshot.migration_reason) throw new HTTPError(409, snapshot.migration_reason, 'Catalogue scope changed');
  for (const row of snapshot.records) if (row.family === 'chapters' && isCurrentCatalogueId(row.id) && isCurrentCatalogueId(row.mangaId) && row.completed && (!ids || ids.includes(row.mangaId!))) (result[row.mangaId!] ??= {})[row.id!] = null;
  return result;
}
export async function Search(searchParams: URLSearchParams) { assertCataloguePreviewLibraryReady();
  const q = params(searchParams), account = await isAccountMode(), view = captureLibraryView();
  const ids = q.getAll('id'), body = account ? await apiRead<Record<string, Record<string, null>>>('/user/history/manga', q, true) : await localHistory(ids.length ? ids : undefined);
  assertLibraryView(view); return { status: 200, body };
}
export async function List(mangaId: string) { assertCataloguePreviewLibraryReady();
  const account = await isAccountMode(), view = captureLibraryView();
  const body = account ? await apiRead<Record<string, Record<string, null>>>(`/user/history/manga/${mangaId}`, undefined, true) : await localHistory([mangaId]);
  assertLibraryView(view); return { status: 200, body };
}
export async function Feed(searchParams: URLSearchParams) { assertCataloguePreviewLibraryReady();
  const q = params(searchParams), account = await isAccountMode(), view = captureLibraryView();
  if (account) { const body = await apiRead<MappedChapterHistoryFeedResponse>('/user/history/manga/feed', q, true); assertLibraryView(view); return { status: 200, body }; }
  // The generic feed requires an exact numeric total. Device history's mounted
  // continuation owner uses openDeviceHistoryFeed instead of inventing that total.
  const pager = await openDeviceHistoryFeed(q);
  try {
    const page = await pager.load(Number(q.get('page') ?? 1)); assertLibraryView(view);
    if (page.total === null) throw new HTTPError(409, 'This device history requires bounded continuation. Open My History and use Continue loading or Continue scanning.', 'History continuation required');
    return { status: 200, body: { total: page.total, page: page.page, limit: page.limit, data: page.data, map: { manga: Object.fromEntries(page.data.map(row => [row.relationships.manga.id, row.relationships.manga])) } } };
  } finally { pager.dispose(); }
}
export async function UpdateBatch(mangaId: string, payload: UpdateChapterHistoryBatchRequest, admitted?: { context: ReturnType<typeof captureLibraryContext>; assert(): void }) { assertCataloguePreviewLibraryReady();
  admitted?.assert(); await openLibrary(); admitted?.assert();
  const context = admitted?.context ?? captureLibraryContext();
  const assert = () => { admitted?.assert(); assertLibraryContext(context); };
  assert();
  if (context.mode === 'account') return accountRequest<UpdateChaptersHistoryBatchResponse>(`/user/history/manga/${mangaId}`, 'POST', payload, context.accountId!, assert);
  const body = await updateChapters(mangaId, payload.read_ids, payload.unread_ids, assert);
  assert(); return { status: 200, body };
}
