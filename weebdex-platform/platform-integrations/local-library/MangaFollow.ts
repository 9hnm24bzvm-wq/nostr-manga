import {assertCataloguePreviewLibraryReady} from '../public-api/library-staging';
import { PUBLIC_API_URL } from '$env/static/public';
import { HTTPError } from '$lib/HTTP';
import type { Constants } from '$lib';
import { apiRead, accountRequest, captureLibraryView, assertLibraryView, currentDeviceSnapshot, isAccountMode, saveSeries, removeSeries, params, applyImport, type ImportReview, captureImportContext, assertImportContext, type LibraryImportContext, openLibrary, captureLibraryContext, assertLibraryContext } from './library';
import { filteredCatalog, pageRows, compareValue, compareCatalogueRows } from './catalog';
import { normalizeDevice } from './device-store';
export async function Search(searchParams: URLSearchParams) { assertCataloguePreviewLibraryReady();
  await openLibrary();
  const q = params(searchParams), context = captureLibraryContext(), account = context.mode === 'account', view = captureLibraryView();
  const list = q.get('list');
  if (q.has('list') && (q.getAll('list').length !== 1 || !list || !/^[0-9a-f]{32}$/.test(list))) throw new HTTPError(400, 'Choose one existing custom device list', 'Invalid device list');
  if (account && list) throw new HTTPError(409, 'This link selects a custom device list. Choose Use device library, then reopen this list. Account results are not a substitute for its saved members.', 'Device library selection required');
  if (account) { const body = await apiRead<MangaFollowFeedResponse>('/user/follows/manga', q, true); assertLibraryView(view); return { status: 200, body }; }
  const snapshot = await currentDeviceSnapshot(), wanted = q.getAll('reading_status');
  assertLibraryContext(context);
  if (list && !snapshot.records.some(row => row.family === 'lists' && row.id === list)) throw new HTTPError(404, 'This list is not present in the current saved-device scope', 'Device list unavailable');
  const members = list ? new Set(snapshot.records.filter(row => row.family === 'members' && row.list === list).map(row => row.series)) : null;
  const records = snapshot.records.filter(row => row.family === 'series' && (!wanted.length || wanted.includes(row.status!)) && (!members || members.has(row.id)));
  const allowed = q.getAll('id');
  const rows = records.filter(row => !allowed.length || allowed.includes(row.id!));
  const follows = new Map(rows.map(row => [row.id!, row]));
  const catalogueQuery = params(q); catalogueQuery.delete('reading_status'); catalogueQuery.delete('list');
  if (!catalogueQuery.has('contentRating')) for (const value of ['safe', 'suggestive', 'erotica', 'pornographic']) catalogueQuery.append('contentRating', value);
  if (catalogueQuery.get('sort') === 'followedAt') catalogueQuery.set('sort', 'createdAt');
  const mangas = await filteredCatalog<Manga>('/manga', 'id', rows.map(row => row.id!), catalogueQuery);
  const data = mangas.filter(row => follows.has(row.id)).map(row => ({ ...row, reading_status: follows.get(row.id)!.status as Constants.Manga.ReadingStatus, followed_at: follows.get(row.id)!.followedAt! }));
  const sort = q.get('sort') ?? 'followedAt', direction = q.get('order') === 'asc' ? 1 : -1;
  data.sort((a, b) => sort === 'followedAt'
    ? direction * compareValue(a.followed_at, b.followed_at) || compareValue(a.id, b.id)
    : compareCatalogueRows(a, b, direction));
  assertLibraryView(view); return { status: 200, body: pageRows(data, q) };
}
export async function LatestFeed(searchParams: URLSearchParams) { assertCataloguePreviewLibraryReady(); const q = params(searchParams); q.delete('tagx'); q.delete('txmod'); return localUpdates(q, '/user/follows/chapter/feed', false); }
export async function LatestUpdates(searchParams: URLSearchParams) { assertCataloguePreviewLibraryReady(); const q = params(searchParams); q.delete('tagx'); q.delete('txmod'); return localUpdates(q, '/user/follows/chapter/updates', true); }
async function localUpdates(searchParams: URLSearchParams, accountPath: string, latestOnly: boolean) {
  const q = params(searchParams), account = await isAccountMode(), view = captureLibraryView();
  if (account) { const body = await apiRead<MappedChapterFeedResponse>(accountPath, q, true); assertLibraryView(view); return { status: 200, body }; }
  const snapshot = await currentDeviceSnapshot(), ids = snapshot.records.filter(row => row.family === 'series' && row.notification).map(row => row.id!);
  let chapters = await filteredCatalog<Chapter & { _sort_value?: unknown; _sort_text?: string }>('/chapter/feed', 'manga', ids, q);
  if (latestOnly) {
    const newest = new Map<string, Chapter & { _sort_value?: unknown; _sort_text?: string }>();
    for (const row of chapters) { const id = row.relationships.manga.id, previous = newest.get(id); if (!previous || compareValue(row.published_at, previous.published_at) > 0 || (row.published_at === previous.published_at && row.id < previous.id)) newest.set(id, row); }
    chapters = [...newest.values()];
  }
  const direction = q.get('order') === 'asc' ? 1 : -1;
  chapters.sort((a, b) => compareCatalogueRows(a, b, direction));
  const result = pageRows(chapters, q);
  const map = { manga: Object.fromEntries(chapters.map(row => [row.relationships.manga.id, row.relationships.manga])) };
  assertLibraryView(view); return { status: 200, body: { ...result, map } };
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
    const mappings = await response.json(); assertImportContext(context);
    if (!Array.isArray(mappings.data) || !Array.isArray(mappings.missing) || !Array.isArray(mappings.ambiguous)) throw new HTTPError(502, 'Invalid catalogue import mapping', 'Import mapping failed');
    missing = mappings.missing; ambiguous = mappings.ambiguous;
    for (const row of mappings.data) { if (!(row.myanimelist in payload.myanimelist)) throw Error('Unexpected import mapping'); selected.set(row.id, payload.myanimelist[row.myanimelist]); titles.set(row.id, row.title); }
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
  const review: ImportReview = { source: 'Reviewed catalogue import', expectedRevision: current.revision, snapshot: { scope: current.scope, format: 'weebdex-device-library', version: 1, revision: 0, exported_at: new Date().toISOString(), records } };
  assertImportContext(context); await applyImport(review, 'merge', scope => assertImportContext(context, scope)); return { status: 204 };
}
