import { get } from 'svelte/store';
import { PUBLIC_API_URL, PUBLIC_MANGADEX_CATALOGUE } from '$env/static/public';
import { HTTPError } from '$lib/HTTP';
import { encodeParams, assertCatalogueScope, assertCurrentCatalogueScopeValue, currentDeviceSnapshot, captureLibraryView, assertLibraryView, captureLibraryContext, assertLibraryContext, localLibraryState, isCurrentCatalogueId } from './library';
import { captureNativeAuthority, sameNativeReadAuthority } from '$platform/mutation-authority';
import { TitlePager, type TitleCandidate } from './title-pagination';
import { TitleIndex } from './title-index';
import { type HistoryBudget } from './history-pagination';
import { historyJson } from './history-transport';
import { publicTitleBatch } from '../public-api/title-catalogue';
export async function openDeviceTitleFeed(input: URLSearchParams, assertOwner: () => void = () => {}): Promise<TitlePager<Manga>> {
  assertOwner(); const authority = captureNativeAuthority(); await assertCatalogueScope(); assertOwner();
  const view = captureLibraryView(), context = captureLibraryContext();
  if (context.mode !== 'device') throw Error('Device titles cannot read an account library');
  const check = () => { assertOwner(); assertLibraryView(view); assertLibraryContext(context); if (!sameNativeReadAuthority(authority)) throw Error('The title account authority changed. Reopen this library view.'); };
  const query = new URLSearchParams(input), list = query.get('list');
  for (const control of ['list', 'limit', 'page', 'sort', 'order']) if (query.getAll(control).length > 1) throw new HTTPError(400, 'Duplicate device title control: ' + control);
  if (query.has('list') && (!list || !/^[0-9a-f]{32}$/.test(list))) throw new HTTPError(400, 'Choose one existing custom device list');
  const limit = Number(query.get('limit') ?? 20), sort = query.get('sort') ?? 'followedAt', direction = query.get('order') ?? 'desc';
  if (!['asc', 'desc'].includes(direction)) throw new HTTPError(400, 'Invalid title ordering');
  const snapshot = await currentDeviceSnapshot(); check(); assertCurrentCatalogueScopeValue(snapshot.scope);
  if (snapshot.migration_reason || snapshot.revision !== get(localLibraryState).revision || snapshot.records.length > 2000000) throw Error('The device title snapshot changed or exceeded its record budget. Reopen this library view.');
  if (list && !snapshot.records.some(row => row.family === 'lists' && row.id === list)) throw new HTTPError(404, 'This list is not present in the current saved-device scope', 'Device list unavailable');
  const wanted = new Set(query.getAll('reading_status')), allowed = new Set(query.getAll('id'));
  const members = list ? new Set(snapshot.records.filter(row => row.family === 'members' && row.list === list).map(row => row.series)) : undefined;
  const candidates: TitleCandidate[] = snapshot.records.filter(row => row.family === 'series' && isCurrentCatalogueId(row.id) && (!wanted.size || wanted.has(row.status!)) && (!allowed.size || allowed.has(row.id!)) && (!members || members.has(row.id))).map(row => ({ id: row.id!, status: row.status!, followedAt: row.followedAt! }));
  for (const field of ['reading_status', 'list', 'page', 'limit', 'id']) query.delete(field);
  if (!query.has('contentRating') && !query.has('contentRating[]')) for (const rating of ['safe', 'suggestive', 'erotica', 'pornographic']) query.append('contentRating', rating);
  query.set('sort', sort === 'followedAt' ? 'createdAt' : sort);
  const scope = { ...snapshot.scope };
  const index = await TitleIndex.create(check);
  try {
    check();
    return new TitlePager(candidates, index, async (batch, budget) => {
      check(); assertCurrentCatalogueScopeValue(scope);
      const rows = PUBLIC_MANGADEX_CATALOGUE === '1' ? await publicTitleBatch(batch.map(row => row.id), query, budget) : await nativeTitleBatch(batch, query, budget);
      check(); assertCurrentCatalogueScopeValue(scope); return rows;
    }, check, limit, sort, direction === 'asc' ? 1 : -1);
  } catch (error) { await index.dispose(); throw error; }
}
async function nativeTitleBatch(batch: readonly TitleCandidate[], input: URLSearchParams, budget: HistoryBudget): Promise<Manga[]> {
  const query = new URLSearchParams(input); for (const row of batch) query.append('id', row.id);
  query.set('page', '1'); query.set('limit', '100');
  const response = await historyJson(PUBLIC_API_URL.replace(/\/$/, '') + '/manga?' + encodeParams(query), budget, true);
  if (response.status < 200 || response.status >= 300) throw Error('Title catalogue lookup failed (' + response.status + '). Retry without skipping this batch.');
  const body = response.body as { total?: unknown; data?: Manga[]; limit?: number; page?: number };
  if (!Number.isSafeInteger(body.total) || Number(body.total) < 0 || Number(body.total) > batch.length || !Array.isArray(body.data) || body.data.length !== body.total || body.limit !== 100 || body.page !== 1) throw Error('Title lookup returned an incomplete or unscoped native page');
  return body.data;
}
