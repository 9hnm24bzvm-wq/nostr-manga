import {assertCataloguePreviewLibraryReady,publicUuidLibraryEnabled} from '../public-api/library-staging';
import { PUBLIC_API_URL } from '$env/static/public';
import { HTTPError } from '$lib/HTTP';
import type { Constants } from '$lib';
import { apiRead, accountRequest, captureLibraryView, assertLibraryView, currentDeviceSnapshot, isCurrentCatalogueId, isAccountMode, saveSeries, removeSeries, params, applyCatalogueRecords, captureImportContext, assertImportContext, type LibraryImportContext, openLibrary, captureLibraryContext, assertLibraryContext } from './library';
import { filteredCatalog } from './catalog';
import { normalizeDevice } from './device-store';
import {publicImportMappings} from '../public-api/import-mappings';
import { openDeviceUpdatesFeed } from './updates-feed';
export { openDeviceUpdatesFeed } from './updates-feed';
import { openDeviceTitleFeed } from './title-feed';
export { openDeviceTitleFeed } from './title-feed';
export async function Search(searchParams: URLSearchParams): Promise<{ status: number; body: MangaFollowFeedResponse }> { assertCataloguePreviewLibraryReady();
  await openLibrary();
  const q = params(searchParams), context = captureLibraryContext(), account = context.mode === 'account', view = captureLibraryView();
  const list = q.get('list');
  if (q.has('list') && (q.getAll('list').length !== 1 || !list || !/^[0-9a-f]{32}$/.test(list))) throw new HTTPError(400, 'Choose one existing custom device list', 'Invalid device list');
  if (account && list) throw new HTTPError(409, 'This link selects a custom device list. Choose Use device library, then reopen this list. Account results are not a substitute for its saved members.', 'Device library selection required');
  if (account) { const body = await apiRead<MangaFollowFeedResponse>('/user/follows/manga', q, true); assertLibraryView(view); return { status: 200, body }; }
  // The mounted device component owns incremental indexing. The compatibility
  // endpoint can return a complete one-batch result, never a false total/page.
  const pager = await openDeviceTitleFeed(q);
  try {
    const result = await pager.load(Number(q.get('page') ?? 1)); assertLibraryView(view);
    if (!result.ready || result.total === null) throw new HTTPError(409, 'This device title library needs bounded indexing. Open My Library and use Continue indexing.', 'Title continuation required');
    const data = result.data;
    // The pager attaches saved follow metadata. Narrow its generic Manga rows
    // only after checking the required retained response fields, never a cast.
    if (!data.every((row): row is MangaFollow => 'reading_status' in row && typeof row.reading_status === 'string' && ['reading', 'plan_to_read', 'completed', 'on_hold', 're_reading', 'dropped'].includes(row.reading_status) && 'followed_at' in row && typeof row.followed_at === 'string')) throw new HTTPError(502, 'Saved title follow metadata is invalid', 'Invalid device title response');
    return { status: 200, body: { total: result.total, limit: result.limit, page: result.page, data } };
  } finally { await pager.dispose(); }
}
export async function LatestFeed(searchParams: URLSearchParams) { assertCataloguePreviewLibraryReady(); const q = params(searchParams); q.delete('tagx'); q.delete('txmod'); return localUpdates(q, '/user/follows/chapter/feed', false); }
export async function LatestUpdates(searchParams: URLSearchParams) { assertCataloguePreviewLibraryReady(); const q = params(searchParams); q.delete('tagx'); q.delete('txmod'); return localUpdates(q, '/user/follows/chapter/updates', true); }
async function localUpdates(searchParams: URLSearchParams, accountPath: string, latestOnly: boolean) {
  const q = params(searchParams), account = await isAccountMode(), view = captureLibraryView();
  if (account) { const body = await apiRead<MappedChapterFeedResponse>(accountPath, q, true); assertLibraryView(view); return { status: 200, body }; }
  const pager = await openDeviceUpdatesFeed(q,latestOnly);
  try {
    const result = await pager.load(Number(q.get('page') ?? 1)); assertLibraryView(view);
    if (!result.ready || result.total === null) throw new HTTPError(409, 'Device updates require bounded indexing. Open My Updates and use Continue indexing.', 'Update continuation required');
    return { status: 200, body: { total: result.total, limit: result.limit, page: result.page, data: result.data, map: { manga: Object.fromEntries(result.data.map(row => [row.relationships.manga.id,row.relationships.manga])) } } };
  } finally { await pager.dispose(); }
}
interface UpdateMangaFollowRequest { status: Constants.Manga.ReadingStatus; notification?: boolean }
export async function Update(id: string, payload: UpdateMangaFollowRequest, admitted?: { context: ReturnType<typeof captureLibraryContext>; assert(): void }) { assertCataloguePreviewLibraryReady();
  admitted?.assert(); await openLibrary(); admitted?.assert(); const context = admitted?.context ?? captureLibraryContext();
  const assert = () => { admitted?.assert(); assertLibraryContext(context); }; assert();
  if (context.mode === 'account') return accountRequest(`/user/follows/manga/${id}`, 'POST', payload, context.accountId!, assert);
  await saveSeries(id, payload.status, payload.notification ?? true, assert); assert(); return { status: 204 };
}
export async function Unfollow(id: string, admitted?: { context: ReturnType<typeof captureLibraryContext>; assert(): void }) { assertCataloguePreviewLibraryReady();
  admitted?.assert(); await openLibrary(); admitted?.assert(); const context = admitted?.context ?? captureLibraryContext();
  const assert = () => { admitted?.assert(); assertLibraryContext(context); }; assert();
  if (context.mode === 'account') return accountRequest(`/user/follows/manga/${id}`, 'DELETE', undefined, context.accountId!, assert);
  await removeSeries(id, assert); assert(); return { status: 204 };
}
export async function Import(payload: { weebdex?: Record<string, Constants.Manga.ReadingStatus>; myanimelist?: Record<string, Constants.Manga.ReadingStatus> }, frozenContext?: LibraryImportContext) { assertCataloguePreviewLibraryReady();
  const context = frozenContext ?? await captureImportContext(); assertImportContext(context);
  if (context.mode === 'account') return accountRequest('/user/follows/manga/import', 'POST', payload, context.accountId!, () => assertImportContext(context));
  const current = await currentDeviceSnapshot(); assertImportContext(context, current.scope);
  const selected = new Map(Object.entries(payload.weebdex ?? {}));
  const titles = new Map<string, string>();
  let missing: string[] = [], ambiguous: string[] = [];
  if (payload.myanimelist) {
    const ids = Object.keys(payload.myanimelist);
    if (!ids.length || ids.length > 400 || ids.some(id => !/^[0-9]{1,20}$/.test(id))) throw new HTTPError(400, 'Import at most 400 valid MyAnimeList identifiers at once', 'Invalid import');
    const response = await fetch(PUBLIC_API_URL.replace(/\/$/, '') + '/manga/import-mappings', { method: 'POST', credentials: 'omit', cache: 'no-store', redirect: 'error', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ myanimelist: ids }), signal: AbortSignal.timeout(30000) });
    if (!response.ok) throw new HTTPError(response.status, 'The instance could not resolve this MyAnimeList import', 'Import mapping failed');
    const rawMappings = await response.json(); assertImportContext(context);
    const mappings = publicUuidLibraryEnabled ? await publicImportMappings(rawMappings,ids,current.scope.catalogNamespace!,()=>assertImportContext(context)) : rawMappings;
    if (!Array.isArray(mappings.data) || !Array.isArray(mappings.missing) || !Array.isArray(mappings.ambiguous)) throw new HTTPError(502, 'Invalid catalogue import mapping', 'Import mapping failed');
    missing = mappings.missing; ambiguous = mappings.ambiguous;
    for (const row of mappings.data) { if (!(row.myanimelist in payload.myanimelist)) throw Error('Unexpected import mapping'); if(selected.has(row.id)&&selected.get(row.id)!==payload.myanimelist[row.myanimelist])throw new HTTPError(400,'The import gives different reading states to the same title. Review those entries before importing.','Conflicting import'); selected.set(row.id, payload.myanimelist[row.myanimelist]); titles.set(row.id, row.title); }
  }
  if (selected.size > 400) throw new HTTPError(400, 'Import at most 400 entries at once', 'Invalid import');
  const visibilityQuery = new URLSearchParams();
  for (const value of ['safe', 'suggestive', 'erotica', 'pornographic']) visibilityQuery.append('contentRating', value);
  const found = await filteredCatalog<Manga>('/manga', 'id', [...selected.keys()], visibilityQuery); assertImportContext(context);
  const visible = new Set(found.map(row => row.id)); for (const row of found) titles.set(row.id, row.title);
  for (const id of selected.keys()) if (!visible.has(id)) { missing.push(id); selected.delete(id); }
  const records = [...selected].map(([id, status]) => normalizeDevice({ family: 'series', key: id, id, status, notification: false, followedAt: new Date().toISOString(), updated_at: Math.floor(Date.now() / 1000) }));
  if (!records.length) throw new HTTPError(400, `No visible unambiguous entries found. Missing: ${missing.length}; ambiguous: ${ambiguous.length}.`, 'Import unavailable');
  const collisions = records.filter(row => current.records.some(old => old.family === row.family && old.key === row.key)).length;
  const sample = [...titles].slice(0, 12).map(([id, title]) => id + ': ' + title).join('\n');
  if (!confirm(`Interpret these IDs in the current instance ${current.scope.origin}, namespace ${current.scope.catalogNamespace ?? 'origin-only'}?\nMerge ${records.length} entries, ${collisions} overlapping keys. Missing ${missing.length}, ambiguous ${ambiguous.length} are excluded.\n${sample}\nNewer local records win. Nothing will be published.`)) throw new HTTPError(409, 'No device records were imported', 'Import cancelled');
  assertImportContext(context); await applyCatalogueRecords(records,current.revision,current.scope,()=>assertImportContext(context)); return { status: 204 };
}
