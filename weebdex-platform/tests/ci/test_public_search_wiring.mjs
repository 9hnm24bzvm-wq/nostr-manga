// Real composed tag loader and retained catalogue client. Only transport,
// store, and Svelte lifecycle boundaries are finite stand-ins; no browser runs.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import vm from 'node:vm';
import { stripTypeScriptTypes } from 'node:module';
import test from 'node:test';
import { createPublicSearchModeFixture } from './public-search-mode-fixture.mjs';
import { createFrontendOverlays } from '../../platform-integrations/frontend-overlays.mjs';
import { createReaderOverlays } from '../../platform-integrations/frontend-reader-overlays.mjs';
import { createLocalLibraryOverlays } from '../../platform-integrations/local-library/overlays.mjs';
import { createPublicApiOverlays } from '../../platform-integrations/public-api/overlays.mjs';
const root = path.resolve(new URL('../../', import.meta.url).pathname), frontend = path.join(root, 'frontend'), directory = path.join(root, 'platform-integrations/public-api');
class BrowserSearchParams extends URLSearchParams {}
const context = vm.createContext({ URL, URLSearchParams: BrowserSearchParams, AbortController, TextEncoder, structuredClone, console, fetch() { throw Error('Unexpected real network request'); } }), modules = new Map();
// Production installs a '?' prefix extension on this constructor. Run the
// exact retained extension on an isolated subclass, preserving the test host.
const layout = fs.readFileSync(path.join(frontend, 'src/routes/+layout.ts'), 'utf8');
vm.runInContext(stripTypeScriptTypes(layout.slice(layout.indexOf('const _toString ='), layout.indexOf('Number.prototype.short ='))), context);
const searchMode = createPublicSearchModeFixture(context);
async function load(file) {
  if (!modules.has(file)) { const module = searchMode.module(file) ?? new vm.SourceTextModule(stripTypeScriptTypes(fs.readFileSync(file, 'utf8'), { mode: 'transform' }), { context, identifier: file }); modules.set(file, module); await module.link((specifier, importer) => load(searchMode.resolve(specifier, importer) ?? (specifier.startsWith('$lib/') ? path.join(frontend, 'src/lib', specifier.slice(5) + '.ts') : path.resolve(path.dirname(importer.identifier), specifier + '.ts')))); }
  return modules.get(file);
}
for (const name of ['validation', 'query', 'normalize', 'client']) { const module = await load(path.join(directory, name + '.ts')); await module.evaluate(); }
const site = await load(path.join(directory, 'site-client.ts')); await site.evaluate();
const searchQuery = await load(path.join(root, 'platform-integrations/public-api/manga-search-query.ts')); await searchQuery.evaluate();
const client = modules.get(path.join(directory, 'client.ts')).namespace;
const uuid = number => '00000000-0000-8000-8000-' + String(number).padStart(12, '0');
const tags = Array.from({ length: 101 }, (_, index) => ({ id: uuid(index + 1), type: 'tag', attributes: { name: { en: 'Actual tag ' + index }, group: index % 2 ? 'genre' : 'format' }, relationships: [] }));
const envelope = data => ({ status: 200, body: { result: 'ok', response: 'collection', data, total: data.length, limit: data.length, offset: 0 } });
function fixture(reply = envelope(tags)) {
  const requests = [], bridge = new site.namespace.RetainedCatalogueClient(new client.PublicApiClient(async request => { requests.push(request); return typeof reply === 'function' ? reply(request) : reply; }));
  return { requests, read: (operation, query = new URLSearchParams(), signal) => bridge.read(operation, { query, signal }) };
}
function composed(relative) { const file = path.join(frontend, relative); let source = fs.readFileSync(file, 'utf8'); for (const plugin of [createFrontendOverlays(frontend), createReaderOverlays(frontend), createLocalLibraryOverlays(frontend), createPublicApiOverlays(frontend, { enabled: true })]) source = plugin.transform.handler(source, file)?.code ?? source; return source; }
function filterQuery(edit) {
  const source = [...composed('src/lib/components/MangaSearchFilter.svelte').matchAll(/<script\b[^>]*>([\s\S]*?)<\/script>/g)].map(match => match[1]).join('\n').replace(/^\s*import[^;]+;\s*$/gm, '');
  let target; const environment = { URLSearchParams: BrowserSearchParams, structuredClone, ...searchQuery.namespace,
    Constants: { Manga: modules.get(path.join(frontend, 'src/lib/constants/Manga.ts')).namespace, MangaFollow: modules.get(path.join(frontend, 'src/lib/constants/MangaFollow.ts')).namespace },
    $state: Object.assign(value => value, { snapshot: structuredClone }), $derived: value => value, $bindable: value => value, $props: () => ({ feed: undefined, isShowFilter: true }),
    $prefs: { items_per_page: '42', show_safe: true, show_suggestive: true, show_erotic: true, show_hentai: false, languages: [], tags: [] },
    window: { location: new URL('https://fixture.invalid/search?title=Actual+query') }, onMount() {}, goto: path => { target = path; }, toast: { error() { assert.fail('Valid composed filter edit was rejected'); } }
  };
  const filter = vm.runInNewContext(stripTypeScriptTypes(source, { mode: 'transform' }) + '\n({submit:onSubmit,search});', environment);
  edit(filter.search); filter.submit(); assert(target); return new URL(target, 'https://fixture.invalid').searchParams;
}
test('actual mounted tag-population function reaches the public client and loads all 101 actual tags', async () => {
  const f = fixture(), source = composed('src/lib/stores/global.ts'), start = source.indexOf('export const populateTagList ='), end = source.indexOf('\nexport const isMobile', start);
  assert(start > 0 && end > start); let grouped;
  const populate = vm.runInNewContext(stripTypeScriptTypes(source.slice(start, end).replace('export const ', 'const '), { mode: 'transform' }) + '\npopulateTagList;', {
    URLSearchParams: BrowserSearchParams, API: { Tag: { Search: query => f.read('tag.search', query) } }, StatusCodes: { OK: 200 }, Constants: { Error: { Unknown: Error('Unexpected tag result') } },
    get: store => store.value, tagList: { value: undefined, set: value => { grouped = value; } }, Utils: { groupTags: rows => Object.groupBy(rows, row => row.group) }
  });
  await populate(); assert.equal(Object.values(grouped).flat().length, 101); assert.equal(new Set(Object.values(grouped).flat().map(row => row.id)).size, 101);
  assert.equal(f.requests.length, 2); for (const request of f.requests) assert.equal(request.path, '/manga/tag');
  assert.match(composed('src/lib/API/Tag.ts'), /publicCatalogueRead<TagFeedResponse>\("tag.search"/);
});
test('tag pagination is applied locally to the complete queryless public catalog', async () => {
  const f = fixture(), response = await f.read('tag.search', new URLSearchParams({ limit: '100', page: '2' }));
  assert.equal(response.body.total, 101); assert.equal(response.body.limit, 100); assert.equal(response.body.offset, 100); assert.equal(response.body.page, 2); assert.deepEqual(Array.from(response.body.data, row => row.id), [tags[100].id]); assert.equal(f.requests[0].path, '/manga/tag');
});
test('direct queryless tag reads retain the complete actual catalog and count-only reads return no rows', async () => {
  const f = fixture(), all = await f.read('tag.search'); assert.equal(all.body.data.length, 101);
  const count = await f.read('tag.search', new URLSearchParams({ limit: '0' })); assert.equal(count.body.total, 101); assert.equal(count.body.data.length, 0); assert.equal(count.body.limit, 0);
});
test('unsupported, duplicate and out-of-bound tag controls fail before transport', async () => {
  for (const input of ['limit=101', 'limit=-1', 'page=0', 'page=1&page=2', 'limit=10&limit=20', 'limit=10&page=2&offset=1', 'name=dropped']) { const f = fixture(); await assert.rejects(() => f.read('tag.search', new URLSearchParams(input))); assert.equal(f.requests.length, 0); }
});
test('a truncated or shifted public tag snapshot cannot populate partial selector options', async () => {
  for (const body of [{ ...envelope(tags).body, data: tags.slice(0, 100) }, { ...envelope(tags).body, offset: 1, data: tags.slice(1) }]) await assert.rejects(() => fixture({ status: 200, body }).read('tag.search', new URLSearchParams({ limit: '100', page: '1' })));
});
test('default title-search sort supports the UI ascending order without an explicit sort key', async () => {
  const f = fixture(envelope([])); await f.read('manga.search', new URLSearchParams({ order: 'asc', limit: '42' }));
  const url = new URL('https://fixture.invalid' + f.requests[0].path); assert.equal(url.searchParams.get('order[latestUploadedChapter]'), 'asc'); assert.equal(url.searchParams.get('limit'), '42');
});
test('explicit title-search sort and its order remain exact', async () => {
  const f = fixture(envelope([])); await f.read('manga.search', new URLSearchParams({ sort: 'title', order: 'asc' })); const url = new URL('https://fixture.invalid' + f.requests[0].path); assert.equal(url.searchParams.get('order[title]'), 'asc'); assert.equal(url.searchParams.has('order[latestUploadedChapter]'), false);
});
test('actual composed filter default-sort ascending edit reaches the real public adapter', async () => {
  const query = filterQuery(search => { search.order = 'asc'; }); assert.equal(query.has('sort'), false); assert.equal(query.get('order'), 'asc');
  const f = fixture(envelope([])); await f.read('manga.search', query); assert.equal(new URL('https://fixture.invalid' + f.requests[0].path).searchParams.get('order[latestUploadedChapter]'), 'asc');
});
test('actual composed filter clearing all rating controls reaches a zero-transport empty search', async () => {
  const query = filterQuery(search => { search.contentRating = []; }); assert.equal(query.get('contentRating'), '__none__');
  const f = fixture(), response = await f.read('manga.search', query); assert.equal(response.body.total, 0); assert.equal(response.body.data.length, 0); assert.equal(f.requests.length, 0);
});
test('clearing every content rating produces a truthful empty search page with zero transport', async () => {
  for (const key of ['contentRating', 'contentRating[]']) { const f = fixture(), response = await f.read('manga.search', new URLSearchParams([[key, '__none__'], ['limit', '42'], ['page', '2'], ['title', 'Actual query']])); assert.equal(response.status, 200); assert.equal(response.body.total, 0); assert.equal(response.body.limit, 42); assert.equal(response.body.offset, 42); assert.equal(response.body.page, 2); assert.equal(response.body.data.length, 0); assert.equal(f.requests.length, 0); }
});
test('empty search selection still rejects mixed ratings or unsupported filters before transport', async () => {
  for (const input of ['contentRating=__none__&contentRating=safe', 'contentRating=__none__&unknown=drop-me', 'contentRating=__none__&limit=101', 'contentRating=__none__&order=up']) { const f = fixture(); await assert.rejects(() => f.read('manga.search', new URLSearchParams(input))); assert.equal(f.requests.length, 0); }
});
test('an already cancelled empty search does not attach a synthetic success', async () => {
  const f = fixture(), controller = new AbortController(); controller.abort(); await assert.rejects(() => f.read('manga.search', new URLSearchParams({ contentRating: '__none__' }), controller.signal), error => error.status === 499); assert.equal(f.requests.length, 0);
});
test('actual tag transport failures retain their status and never fall back to native reads', async () => {
  const denied = { status: 503, body: { result: 'error' } }, f = fixture(denied); assert.equal(await f.read('tag.search', new URLSearchParams({ limit: '100', page: '1' })), denied); assert.equal(f.requests.length, 1); assert.equal(f.requests[0].path, '/manga/tag');
});
