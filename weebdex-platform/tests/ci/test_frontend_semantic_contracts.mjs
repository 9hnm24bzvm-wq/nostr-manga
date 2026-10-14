#!/usr/bin/env node
// Finite actual-overlay and helper regression seams. No browser, services,
// network, real IndexedDB or package installation. UI runes are stand-ins.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import vm from 'node:vm';
import { fileURLToPath } from 'node:url';
import { stripTypeScriptTypes } from 'node:module';
import { createHash } from 'node:crypto';
import { createFrontendOverlays } from '../../platform-integrations/frontend-overlays.mjs';
import { createReaderOverlays } from '../../platform-integrations/frontend-reader-overlays.mjs';
import { createLocalLibraryOverlays } from '../../platform-integrations/local-library/overlays.mjs';
if (!vm.SourceTextModule) throw Error('Run with node --experimental-vm-modules');
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..'), frontend = path.join(root, 'frontend');
const plugins = [createFrontendOverlays(frontend), createReaderOverlays(frontend), createLocalLibraryOverlays(frontend)];
const compose = relative => { const file = path.join(frontend, relative); let code = fs.readFileSync(file, 'utf8'); for (const plugin of plugins) code = plugin.transform.handler(code, file)?.code ?? code; return code; };
const context = vm.createContext({ console, URLSearchParams, URL, TextEncoder, Uint8Array, ArrayBuffer, SharedArrayBuffer, structuredClone });
const modules = new Map();
async function load(relative) {
  const entry = path.join(root, relative);
  function moduleFor(file) {
    if (modules.has(file)) return modules.get(file);
    let code = fs.readFileSync(file, 'utf8');
    if (file.endsWith('.ts')) code = stripTypeScriptTypes(code, { mode: 'transform' });
    const module = new vm.SourceTextModule(code, { context, identifier: file }); modules.set(file, module); return module;
  }
  const module = moduleFor(entry);
  await module.link((specifier, importer) => {
    const target = specifier.startsWith('$lib/') ? path.join(frontend, 'src/lib', specifier.slice(5)) : path.resolve(path.dirname(importer.identifier), specifier);
    return moduleFor(fs.existsSync(target) ? target : target + '.ts');
  });
  await module.evaluate(); return module.namespace;
}
const query = await load('platform-integrations/frontend-behavior/manga-search-query.ts');
const prompt = await load('platform-integrations/frontend-behavior/install-prompt.ts');
const transfer = await load('platform-integrations/local-library/graph-transfer.ts');
const receipt = await load('platform-integrations/local-library/protocol-receipt.ts');
const Manga = modules.get(path.join(frontend, 'src/lib/constants/Manga.ts')).namespace;
const MangaFollow = modules.get(path.join(frontend, 'src/lib/constants/MangaFollow.ts')).namespace;
const ErrorNamespace = modules.get(path.join(frontend, 'src/lib/constants/Error.ts')).namespace;
const defaults = { limit: 20, sort: Manga.SortBy.LastUploadedChapterAt, order: 'desc', hasChapters: false, contentRating: ['safe'], tmod: '0', txmod: '0' };
const parse = (text, library = false) => query.parseMangaSearchQuery({ ...defaults, sort: library ? MangaFollow.SortBy.FollowedAt : defaults.sort }, new URLSearchParams(text), library);
const clean = value => JSON.parse(JSON.stringify(value));
let cases = 0;
function check(name, operation) { operation(); cases++; }
check('all exact retained sources remain byte-identical', () => {
  const parity = JSON.parse(fs.readFileSync(path.join(root, 'test-evidence/frontend/source-parity.json')));
  assert.equal(parity.files.length, 676);
  for (const file of parity.files) { const relative = file.path ?? file.relative_path; const expected = file.source_sha256; assert.equal(createHash('sha256').update(fs.readFileSync(path.join(frontend, relative))).digest('hex'), expected, relative); }
});
check('backend language vocabulary equals retained source vocabulary', () => {
  const backend = fs.readFileSync(path.join(root, 'backend/src/domains/catalog/languages.rs'), 'utf8');
  const expected = [...backend.matchAll(/"([a-z-]+)"/g)].map(match => match[1]).sort();
  const actual = [...query.searchLanguages].sort(); assert.deepEqual(actual, expected);
});
check('repeated/bracketed arrays and creator distinctions', () => {
  const value = parse('lang[]=en&lang=ja&demographic[]=shounen&status=hiatus&author=a&artist=b&artists=c&authorOrArtist=d&id=m');
  assert.equal(value.issues.length, 0); assert.deepEqual(clean(value.search.lang), ['en', 'ja']);
  for (const [key, expected] of [['author','a'],['artist','b'],['artists','c'],['authorOrArtist','d'],['id','m']]) assert.deepEqual(clean(value.search[key]), [expected]);
});
for (const value of ['false','0','true','1']) check('real boolean ' + value, () => {
  const parsed = parse('hasChapters=' + value), encoded = query.serializeMangaSearchQuery(parsed.search, parsed.passthrough, parsed.explicitHasChapters);
  assert.equal(parsed.search.hasChapters, value === 'true' || value === '1'); assert.equal(encoded.get('hasChapters'), String(parsed.search.hasChapters));
});
check('absent hasChapters remains absent', () => { const value = parse(''); assert.equal(query.serializeMangaSearchQuery(value.search, value.passthrough, value.explicitHasChapters).has('hasChapters'), false); });
for (const value of ['0','100','+0','00020','-0']) check('accepted integer spelling ' + value, () => { const parsed = parse('limit=' + encodeURIComponent(value)); assert.equal(parsed.issues.length, 0); assert.equal(parsed.search.limit, Number(value)); });
for (const value of ['101','-1','1.5','1e2','','NaN','Infinity','9007199254740993']) check('invalid limit remains visible ' + value, () => { const parsed = parse('limit=' + value); assert.ok(query.searchValidationError(parsed.issues)); });
check('inclusive year boundaries', () => { const parsed = parse('yearFrom=0&yearTo=9999'); assert.equal(parsed.issues.length, 0); assert.equal(parsed.search.yearFrom, 0); assert.equal(parsed.search.yearTo, 9999); });
for (const value of ['yearFrom=10000','yearTo=-1','yearFrom=2024&yearTo=2023','limit=10&limit[]=20','order=up','contentRating=__none__&contentRating=safe','contentRating=bogus','status=bogus','demographic=bogus','hasChapters=yes','sort=followedAt','lang=bad','tag=bad!']) check('malformed explicit filters ' + value, () => { assert.ok(query.validateMangaSearchQuery(new URLSearchParams(value), false)); });
check('empty rating sentinel stays empty', () => { assert.deepEqual(clean(parse('contentRating=__none__').search.contentRating), []); });
check('list is preserved as one exact library identifier', () => { const value = parse('list=' + 'a'.repeat(32), true); assert.equal(value.issues.length, 0); assert.equal(query.serializeMangaSearchQuery(value.search, value.passthrough, value.explicitHasChapters).get('list'), 'a'.repeat(32)); assert.ok(parse('list=bad', true).issues.length); });
for (const key of ['tmod','txmod']) for (const value of ['0','1','AND','OR','and','or']) check('mode radio/wire roundtrip ' + key + '=' + value, () => {
  const parsed = parse('tag=a&tagx=b&' + key + '=' + value); assert.equal(parsed.issues.length, 0);
  const radio = parsed.search[key], expected = key === 'tmod' ? radio === '0' ? 'AND' : 'OR' : radio === '0' ? 'OR' : 'AND';
  const encoded = query.serializeMangaSearchQuery(parsed.search, parsed.passthrough, false); assert.equal(encoded.get(key), expected); assert.equal(parse(encoded.toString()).search[key], radio);
});
check('mode mapping matches actual backend and retained radios', () => {
  const backend = fs.readFileSync(path.join(root, 'backend/src/domains/catalog/query.rs'), 'utf8'); assert.ok(backend.includes('if exclude { "OR" } else { "AND" }')); assert.ok(backend.includes('"AND" | "and" => true')); assert.ok(backend.includes('"OR" | "or" => false'));
  const retained = fs.readFileSync(path.join(frontend, 'src/lib/components/MangaSearchFilter.svelte'), 'utf8'); for (const pair of [['0','inc_and'],['1','inc_or'],['0','exc_or'],['1','exc_and']]) assert.ok(retained.includes('value="' + pair[0] + '" id="' + pair[1] + '"'));
});
check('passthrough cannot override validated fields or restore page', () => { const value = parse('future=a&future=b&page=9&yearFrom=0'); const encoded = query.serializeMangaSearchQuery(value.search, new URLSearchParams('future=a&future=b&page[]=8&limit[]=100&hasChapters[]=1'), false); assert.deepEqual(encoded.getAll('future'), ['a','b']); assert.equal(encoded.has('page[]'), false); assert.equal(encoded.has('limit[]'), false); assert.equal(encoded.has('hasChapters[]'), false); assert.equal(encoded.get('limit'), '20'); });
check('invalid fields can be corrected without Reset', () => { const value = parse('yearFrom=bogus&order=bogus'); assert.equal(query.unresolvedSearchIssues(value.issues, value.search, { ...value.search, yearFrom: 2000 }).length, 1); assert.equal(query.unresolvedSearchIssues(value.issues, value.search, { ...value.search, yearFrom: 2000, order: 'asc' }).length, 0); assert.equal(query.unresolvedSearchIssues(value.issues, value.search, { ...value.search, title: 'unrelated' }).length, 2); });
check('unsupported prompts are not retained', () => { assert.equal(prompt.isInstallPromptEvent({}), false); assert.equal(prompt.isInstallPromptEvent({ prompt: 2 }), false); assert.equal(prompt.isInstallPromptEvent({ prompt() {} }), true); assert.equal(prompt.isStandaloneNavigator({}), false); assert.equal(prompt.isStandaloneNavigator({ standalone: true }), true); assert.equal(prompt.isStandaloneNavigator({ standalone: 'true' }), false); });
check('graph preserves legitimate views and deduplicates owned backing buffers', () => { const buffer = new ArrayBuffer(16), first = new Uint8Array(buffer, 2, 3), second = new Uint8Array(buffer, 8, 4); const graph = { objects: [{ data: first }, { data: second }] }; const buffers = transfer.graphTransferBuffers(graph); assert.equal(buffers.length, 1); assert.equal(buffers[0], buffer); assert.equal(first.byteOffset, 2); assert.equal(second.byteLength, 4); });
for (const data of [null, new Uint8Array(0), new Uint8Array(new SharedArrayBuffer(4)), new DataView(new ArrayBuffer(4))]) check('non-owned or non-transferable graph rejected', () => { assert.throws(() => transfer.graphTransferBuffers({ objects: [{ data }] }), /transferable bytes/); });

// Execute the retained layout's exact query-string prefix extension. This is
// part of production navigation, rather than inventing a '?' in the test.
const layout = fs.readFileSync(path.join(frontend, 'src/routes/+layout.ts'), 'utf8');
vm.runInContext(stripTypeScriptTypes(layout.slice(layout.indexOf('const _toString ='), layout.indexOf('Number.prototype.short ='))), context);

for (const value of [{ id: 'a'.repeat(64) }, { id: 'a'.repeat(64), projected: true }, { id: 'a'.repeat(64), superseded: true, projected: false }]) check('exact valid event receipt', () => { assert.equal(receipt.isPublicActionReceipt(value), true); });
for (const value of [null, [], {}, { id: 'wrong' }, { id: 'a'.repeat(64), projected: 'true' }, { id: 'a'.repeat(64), superseded: 1 }]) check('malformed event receipt is never accepted', () => { assert.equal(receipt.isPublicActionReceipt(value), false); });

function script(relative, exports, supplied = {}) {
  const code = [...compose(relative).matchAll(/<script\b[^>]*>([\s\S]*?)<\/script>/g)].map(match => match[1]).join('\n').replace(/^\s*import[^;]+;\s*$/gm, '').replace(/^\s*export\s+/gm, '');
  const effects = [], mounts = [], state = Object.assign(value => value, { snapshot: structuredClone });
  const sandbox = vm.createContext({ console, URLSearchParams, URL, TextEncoder, Uint8Array, ArrayBuffer, structuredClone, Promise,
    Constants: { Manga, MangaFollow, Error: ErrorNamespace }, ...query, ...prompt,
    $bindable: value => value, $state: state, $derived: value => value, $props: () => ({ feed: undefined, isShowFilter: true }), $effect: callback => effects.push(callback), onMount: callback => mounts.push(callback), onDestroy() {},
    $mangaView: 'grid', $prefs: { items_per_page: '20', show_safe: true, languages: [], tags: [] },
    window: { location: { pathname: '/search', search: '', href: 'http://localhost/search' }, clearTimeout() {}, setTimeout: () => 1 }, ...supplied,
  });
  const transformed = stripTypeScriptTypes(code, { mode: 'transform' });
  const expose = Object.entries(exports).map(([key, expression]) => key + ': ' + expression).join(',');
  const api = vm.runInContext('(function(){' + transformed + '\nreturn {' + expose + '};})()', sandbox); return { api, effects, mounts, sandbox };
}
check('actual filter query apply normalizes false, zero, and tag modes', () => {
  let navigated;
  const window = { location: { pathname: '/search', search: '?hasChapters=0&yearFrom=0&limit=0&tag=a&tmod=OR&tagx=b&txmod=AND' } };
  const value = script('src/lib/components/MangaSearchFilter.svelte', { submit: 'onSubmit', search: '() => search' }, { window, goto: target => { navigated = target; }, toast: { error() { assert.fail('valid filter rejected'); } } });
  value.api.submit(); const target = new URL(navigated, 'http://localhost'); assert.equal(target.searchParams.get('hasChapters'), 'false'); assert.equal(target.searchParams.get('limit'), '0'); assert.equal(target.searchParams.get('yearFrom'), '0'); assert.equal(target.searchParams.get('tmod'), 'OR'); assert.equal(target.searchParams.get('txmod'), 'AND');
});
check('actual filter invalid state blocks Apply and accepts a targeted correction', () => {
  const errors = [], targets = []; const window = { location: { pathname: '/search', search: '?yearFrom=bad' } };
  const value = script('src/lib/components/MangaSearchFilter.svelte', { submit: 'onSubmit', correct: 'value => { search.yearFrom = value; }' }, { window, goto: target => targets.push(target), toast: { error: (...args) => errors.push(args) } });
  value.api.submit(); assert.equal(errors.length, 1); assert.equal(targets.length, 0); value.api.correct(2000); value.api.submit(); assert.equal(targets.length, 1);
});
check('actual install listener retains only supported prompts and clears on installation', () => {
  const listeners = {}, navigator = { userAgent: 'desktop' }, window = { navigator, addEventListener: (name, callback) => { listeners[name] = callback; }, matchMedia: () => ({ matches: false }) };
  const value = script('src/lib/components/AppHeader.svelte', { current: '() => deferredPrompt' }, { window, navigator, useSidebar: () => ({}) });
  const unsupported = { preventDefault() {}, prompt: 4 }; listeners.beforeinstallprompt(unsupported); assert.equal(value.api.current(), undefined);
  const supported = { preventDefault() {}, prompt() {} }; listeners.beforeinstallprompt(supported); assert.equal(value.api.current(), supported);
  listeners.appinstalled(); assert.equal(value.api.current(), undefined); navigator.standalone = true; listeners.beforeinstallprompt(supported); assert.equal(value.api.current(), undefined);
});
check('exact Lightbox extension preserves its existing callable browser actions', () => {
  const window = {};
  script('src/lib/components/Lightbox.svelte', { current: '() => isOpen' }, { window });
  assert.deepEqual(Object.keys(window._lightbox_), ['open','close','prev','next']);
  for (const action of Object.values(window._lightbox_)) assert.equal(typeof action, 'function');
});
const bad = script('src/lib/components/MangaSearch.svelte', { current: '() => ({ error, feed, isMounted })' }, { page: { state: { _$MangaSearch_feed: { data: ['stale'] } } }, $props: () => ({ feed: { data: ['stale'] }, onSearch: () => { assert.fail('invalid query read issued'); } }), window: { location: { pathname: '/search', search: '?limit=101' } } });
await bad.mounts[0](); check('actual feed existing error state wins over cached stale results', () => { assert.equal(bad.api.current().error.title, 'Invalid search filters'); assert.equal(bad.api.current().feed, undefined); assert.equal(bad.api.current().isMounted, true); });
// Use the exact composed preference method, so literal-key default behavior is
// exercised against actual canonical route queries rather than a permissive stub.
const preferenceSource = compose('src/lib/stores/prefs.ts');
const preferenceStart = preferenceSource.indexOf('applyMangaPreferences('), preferenceEnd = preferenceSource.indexOf('applyChapterPreferences(', preferenceStart);
const preferenceMethod = preferenceSource.slice(preferenceStart, preferenceEnd).trim().replace(/^applyMangaPreferences/, 'function applyMangaPreferences').replace(/,$/, '');
const preferenceValues = { show_safe: true, show_suggestive: false, show_erotic: false, show_hentai: false, languages: ['en'], tags: [] };
const preferenceContext = vm.createContext({ URLSearchParams, get: () => preferenceValues, store: {}, Constants: { Manga } });
const applyMangaPreferences = vm.runInContext(stripTypeScriptTypes(preferenceMethod) + ';applyMangaPreferences;', preferenceContext);
for (const [library, input, expected] of [
  [false, '?tag=a&tmod=1&tagx=b&txmod=1', { tmod: 'OR', txmod: 'AND' }],
  [false, '?contentRating[]=pornographic&limit[]=0&page[]=2', { contentRating: 'pornographic', limit: '0', page: '2' }],
  [false, '?page[]=1000000&lang[]=mx', { page: '1000000', lang: 'mx' }],
  [true, '?reading_status[]=completed&list[]=' + 'a'.repeat(32) + '&id[]=native', { reading_status: 'completed', list: 'a'.repeat(32), id: 'native' }],
  [true, '?contentRating[]=__none__&hasChapters[]=0', { contentRating: '__none__', hasChapters: 'false' }],
]) {
  const pathname = library ? '/library' : '/search', window = { location: { pathname, search: input, href: 'http://localhost' + pathname + input } }; let issued;
  const request = async value => { issued = value; return { status: 200, body: { data: [], total: 0 } }; };
  const route = script('src/routes/(app)' + pathname + '/+page.svelte', { read: 'onSearch' }, { window, prefs: { applyMangaPreferences }, API: { Manga: { Search: request }, MangaFollow: { Search: request } }, StatusCodes: { OK: 200, TOO_MANY_REQUESTS: 429 }, page: { state: {} }, replaceState() {} });
  await route.api.read();
  check('actual initial read canonicalizes before preferences ' + pathname + input, () => { for (const [key, value] of Object.entries(expected)) { assert.deepEqual(issued.getAll(key), [value]); assert.equal(issued.has(key + '[]'), false); } });
}
for (const input of ['?page=0','?page=1000001','?page=1&page[]=2','?page=bad','?tlang=bad','?manga=bad!','?group[]=bad!','?name=x&name[]=y','?rank=x&rank=y','?time=x&time=y','?draft=x&draft=y']) {
  const window = { location: { pathname: '/search', search: input, href: 'http://localhost/search' + input } };
  const cached = script('src/lib/components/MangaSearch.svelte', { current: '() => ({ error, feed })' }, { window, page: { state: { _$MangaSearch_feed: { data: ['stale'] } } }, $props: () => ({ feed: { data: ['stale'] }, onSearch: () => { assert.fail('invalid initial query was read'); } }) });
  await cached.mounts[0]();
  check('shared transport validation wins before cache/read ' + input, () => { assert.equal(cached.api.current().error.title, 'Invalid search filters'); assert.equal(cached.api.current().feed, undefined); });
}
check('valid pagination is retained for reads and excluded from Apply state', () => { const result = query.mangaSearchReadQuery(new URLSearchParams('page[]=2&limit[]=0'), false); assert.equal(result.error, undefined); assert.equal(result.query.get('page'), '2'); assert.equal(result.query.get('limit'), '0'); const parsed = parse('page=2'); assert.equal(Object.hasOwn(parsed.search, 'page'), false); assert.equal(parsed.passthrough.has('page'), false); });
check('library username singleton does not narrow public unowned fields', () => { assert.ok(query.validateMangaSearchQuery(new URLSearchParams('username=a&username[]=b'), true)); assert.equal(query.validateMangaSearchQuery(new URLSearchParams('username=a&username=b'), false), undefined); });
check('unowned future fields stay separate and survive initial read normalization', () => { const result = query.mangaSearchReadQuery(new URLSearchParams('future[]=one&future[]=two&tlang[]=mx&manga[]=native'), false); assert.equal(result.error, undefined); assert.deepEqual(result.query.getAll('future[]'), ['one','two']); assert.deepEqual(result.query.getAll('tlang'), ['mx']); assert.deepEqual(result.query.getAll('manga'), ['native']); });
for (const [field, input, corrected] of [['title', 'title=' + 'a'.repeat(1001), 'short title'], ['lang', Array.from({ length: 101 }, () => 'lang=en').join('&'), ['en']]]) {
  const errors = [], targets = [], window = { location: { pathname: '/search', search: '?' + input } };
  const value = script('src/lib/components/MangaSearchFilter.svelte', { submit: 'onSubmit', edit: '(field, value) => { search[field] = value; }' }, { window, goto: target => targets.push(target), toast: { error: (...args) => errors.push(args) } });
  value.api.submit(); assert.equal(targets.length, 0); value.api.edit(field, corrected); value.api.submit();
  check('owned field budget error clears only after a valid correction ' + field, () => { assert.equal(errors.length, 1); assert.equal(targets.length, 1); });
}
const longTags = Array.from({ length: 100 }, (_, i) => 'a'.repeat(125) + String(i).padStart(3, '0'));
const multiBudget = new URLSearchParams(); longTags.forEach(value => { multiBudget.append('tag', value); multiBudget.append('tagx', value); }); multiBudget.set('title', 'a');
check('total budget is owned by all present editable fields', () => { const parsed = parse(multiBudget.toString()); const issue = parsed.issues.find(issue => issue.message === 'Query is too long'); assert.deepEqual([...issue.fields].sort(), ['tag','tagx','title']); });
check('correcting one total-budget field cannot forgive remaining excess', () => {
  const errors = [], targets = [], window = { location: { pathname: '/search', search: multiBudget.toString() } };
  const value = script('src/lib/components/MangaSearchFilter.svelte', { submit: 'onSubmit', edit: '(field, value) => { search[field] = value; }' }, { window, goto: target => targets.push(target), toast: { error: (...args) => errors.push(args) } });
  value.api.edit('title', 'b'); value.api.submit(); assert.equal(targets.length, 0); assert.equal(errors.length, 1);
  value.api.edit('tagx', []); value.api.submit(); assert.equal(targets.length, 1);
});
check('valid owned-field edits do not discard over-budget unknown passthrough', () => {
  const input = new URLSearchParams(); for (let i = 0; i < 30; i++) input.append('future', 'a'.repeat(700)); input.set('title', 'a');
  const errors = [], targets = [], window = { location: { pathname: '/search', search: input.toString() } };
  const value = script('src/lib/components/MangaSearchFilter.svelte', { submit: 'onSubmit', edit: 'value => { search.title = value; }' }, { window, goto: target => targets.push(target), toast: { error: (...args) => errors.push(args) } });
  value.api.edit('b'); value.api.submit(); assert.equal(targets.length, 0); assert.equal(errors.length, 1);
});
check('actual library Apply preserves explicit empty rating and resets page', () => { let target; const window = { location: { pathname: '/library', search: '?contentRating=__none__&page=2' } }; const value = script('src/lib/components/MangaSearchFilter.svelte', { submit: 'onSubmit' }, { window, goto: value => { target = value; }, toast: { error() { assert.fail('valid explicit empty selection rejected'); } } }); value.api.submit(); const next = new URL(target, 'http://localhost'); assert.equal(next.searchParams.get('contentRating'), '__none__'); assert.equal(next.searchParams.has('page'), false); });
check('explicit library Reset clears its empty-rating selection deliberately', () => { let target; const window = { location: { pathname: '/library', search: '?contentRating=__none__&page=2' } }; const value = script('src/lib/components/MangaSearchFilter.svelte', { reset: 'onReset' }, { window, goto: value => { target = value; }, toast: { error() { assert.fail('explicit Reset was blocked'); } } }); value.api.reset(); assert.equal(target, '/library'); });
const wire = value => value.toString().replace(/^\?/, '');
function boundedQuery(seed, length) {
  const result = new URLSearchParams(seed);
  while (length - wire(result).length > 1008) result.append('future', 'a'.repeat(1000));
  const remaining = length - wire(result).length;
  assert.ok(remaining >= 8); result.append('future', 'a'.repeat(remaining - 8));
  assert.equal(wire(result).length, length); return result;
}
const noRatings = { items_per_page: '20', show_safe: false, show_suggestive: false, show_erotic: false, show_hentai: false, languages: [], tags: [] };
const selected = 'limit=0&sort=title&order=asc&tag=a&tmod=OR&tagx=b&txmod=AND';
for (const [length, accepted] of [[16375, false], [16360, true]]) {
  const input = boundedQuery(selected, length), errors = [], targets = [];
  assert.equal(query.validateMangaSearchQuery(input, false), undefined);
  const value = script('src/lib/components/MangaSearchFilter.svelte', { submit: 'onSubmit' }, { $prefs: noRatings, window: { location: { pathname: '/search', search: input.toString() } }, goto: target => targets.push(target), toast: { error: (...args) => errors.push(args) } });
  value.api.submit();
  check('actual Apply admits final sentinel wire only within budget ' + length, () => {
    assert.equal(targets.length, accepted ? 1 : 0); assert.equal(errors.length, accepted ? 0 : 1);
    if (accepted) { const final = new URL(targets[0], 'http://localhost').searchParams; assert.equal(final.get('contentRating'), '__none__'); for (const [key, expected] of new URLSearchParams(selected)) assert.equal(final.get(key), expected); assert.equal(query.validateMangaSearchQuery(final, false), undefined); }
  });
}
const savedPreferences = { ...preferenceValues };
Object.assign(preferenceValues, noRatings);
for (const [library, seed, length, accepted] of [[false, selected, 16375, false], [false, selected, 16360, true], [true, 'sort=title&order=asc', 16380, false], [true, 'sort=title&order=asc', 16370, true]]) {
  const input = boundedQuery(seed, length), pathname = library ? '/library' : '/search';
  const window = { location: { pathname, search: input.toString(), href: 'http://localhost' + pathname + input.toString() } };
  assert.equal(query.validateMangaSearchQuery(input, library), undefined);
  let issued;
  const request = async value => { issued = value; return { status: 200, body: { data: [], total: 0 } }; };
  const supplied = { window, $prefs: noRatings, prefs: { applyMangaPreferences }, API: { Manga: { Search: request }, MangaFollow: { Search: request } }, StatusCodes: { OK: 200, TOO_MANY_REQUESTS: 429 }, page: { state: {} }, replaceState() {} };
  const route = script('src/routes/(app)' + pathname + '/+page.svelte', { read: 'onSearch' }, supplied);
  let rejected; try { await route.api.read(); } catch (error) { rejected = error; }
  check('actual read validates preference and default additions ' + pathname + '/' + length, () => { assert.equal(Boolean(issued), accepted); if (accepted) assert.equal(query.validateMangaSearchQuery(issued, library), undefined); else assert.equal(rejected.title, 'Invalid search filters'); });
  let cacheReads = 0;
  const cached = script('src/lib/components/MangaSearch.svelte', { current: '() => ({ error, feed })' }, { ...supplied, page: { state: { _$MangaSearch_feed: { data: ['cached'] } } }, $props: () => ({ feed: { data: ['cached'] }, onSearch: async () => { cacheReads++; await route.api.read(); } }) });
  await cached.mounts[0]();
  check('cache admission validates identical effective query ' + pathname + '/' + length, () => { assert.equal(cacheReads, accepted && library ? 1 : 0); if (accepted) { assert.equal(cached.api.current().error, undefined); if (!library) assert.deepEqual(clean(cached.api.current().feed.data), ['cached']); } else { assert.equal(cached.api.current().error.title, 'Invalid search filters'); assert.equal(cached.api.current().feed, undefined); } });
}
for (const [field, values] of [['languages', Array(101).fill('en')], ['tags', Array(101).fill('native')]]) {
  Object.assign(preferenceValues, noRatings, { [field]: values });
  check('effective query rejects excessive preference rows ' + field, () => { const result = query.effectiveMangaSearchReadQuery(new URLSearchParams(), false, { applyMangaPreferences }, '20'); assert.equal(result.error.title, 'Invalid search filters'); });
}
Object.assign(preferenceValues, savedPreferences);
check('effective query validates an invalid preference default limit', () => { assert.equal(query.effectiveMangaSearchReadQuery(new URLSearchParams(), true, { applyMangaPreferences }, '101').error.title, 'Invalid search filters'); });
const flush = async () => { for (let i = 0; i < 8; i++) await Promise.resolve(); };
for (const relative of ['src/routes/(app)/chapter/[id]/edit/+page.svelte','src/routes/(app)/title/[id]/upload/+page.svelte']) {
  for (const mode of ['undefined','throw','success','stale']) {
    const data = { chapter: { id: 'chapter', relationships: {} }, manga: { id: 'manga' } }, actor = { identity: { id: 'actor' } };
    let resolve; const response = mode === 'stale' ? new Promise(done => { resolve = done; }) : mode === 'success' ? Promise.resolve({ status: 200, body: { id: 'actor' } }) : undefined;
    const value = script(relative, { current: '() => ({ user, error, isMounted })' }, { $props: () => ({ data }), $Session: actor, StatusCodes: { OK: 200, TOO_MANY_REQUESTS: 429 }, API: { User: { Get: () => { if (mode === 'throw') throw Error('read failed'); return response; } } } });
    value.effects[0](); if (mode === 'stale') { actor.identity.id = 'new'; resolve({ status: 200, body: { id: 'actor' } }); } await flush();
    check('actual current actor read ' + path.basename(path.dirname(relative)) + '/' + mode, () => { if (mode === 'success') { assert.equal(value.api.current().user.id, 'actor'); assert.equal(value.api.current().isMounted, true); } else { assert.equal(value.api.current().user, undefined); if (mode !== 'stale') assert.ok(value.api.current().error); } });
  }
}
for (const mode of ['undefined','throw','success','stale']) {
  let resolve; const errors = [];
  const response = mode === 'stale' ? new Promise(done => { resolve = done; }) : mode === 'success' ? Promise.resolve({ status: 200, body: [{ id: 'reason' }] }) : undefined;
  const value = script('src/lib/components/ReportDialog.svelte', { start: '() => { entity = { id: "subject" }; entityType = "manga"; isOpen = true; }', close: '() => { isOpen = false; }', current: '() => reasons' }, { page: { url: { href: 'http://localhost/search' } }, Mutex: class {}, createMutationOwner: () => ({ invalidate() {} }), StatusCodes: { OK: 200, FORBIDDEN: 403, TOO_MANY_REQUESTS: 429 }, API: { Report: { Reasons: () => { if (mode === 'throw') throw Error('read failed'); return response; } } }, toast: { error: (...args) => errors.push(args) } });
  value.api.start(); value.effects[1](); if (mode === 'stale') { value.api.close(); resolve({ status: 200, body: [{ id: 'reason' }] }); } await flush();
  check('actual report current-context read ' + mode, () => { if (mode === 'success') assert.equal(value.api.current()[0].id, 'reason'); else { assert.equal(value.api.current(), undefined); assert.equal(errors.length, mode === 'stale' ? 0 : 1); } });
}
const publicSource = fs.readFileSync(path.join(root, 'platform-integrations/local-library/protocol-identity.ts'), 'utf8');
const publicFunction = publicSource.slice(publicSource.indexOf('export async function deliverPublicAction('), publicSource.indexOf('export async function cancelPublicAction(')).replace('export ', '');
for (const [status, body, expected] of [[200,{ id: 'a'.repeat(64), projected: true },'accepted'],[202,{ id: 'a'.repeat(64), projected: false },'accepted'],[409,{ id: 'a'.repeat(64), superseded: true },'superseded'],[200,null,'outcome_unknown'],[200,{ id: 'a'.repeat(64), projected: 'yes' },'outcome_unknown'],[409,{ id: 'a'.repeat(64), superseded: false },'outcome_unknown'],[403,{},'refused']]) {
  const id = 'a'.repeat(64), record = { event: { id, pubkey: 'owner' }, state: 'prepared', relay: { origin: 'http://localhost', serviceKey: 'service' }, path: '/metadata' }, updates = [];
  const sandbox = vm.createContext({ Error, navigator: { locks: { request: async (_, __, operation) => operation({}) } }, publicActionRecords: async () => [record], selectedSigner: () => ({ owner: 'owner', check() {} }), capabilities: async () => ({}), assertAllowedRelay() {}, proveService: async () => {}, signedRequest: async () => ({ status, data: body }), isPublicActionReceipt: receipt.isPublicActionReceipt, updatePublicAction: async (_, update) => { updates.push(update); Object.assign(record, update); } });
  const deliver = vm.runInContext(stripTypeScriptTypes(publicFunction) + ';deliverPublicAction;', sandbox);
  await deliver(id).catch(() => {});
  check('actual public action receipt path ' + status + '/' + expected, () => { assert.equal(record.state, expected); if (expected === 'accepted' || expected === 'superseded' || expected === 'refused') assert.equal(record.receipt.data, body); if (expected === 'outcome_unknown') assert.ok(updates.some(row => row.state === 'outcome_unknown')); });
}
for (const relative of ['src/lib/components/MostReadCarousel.svelte','src/lib/components/RecentlyAddedCarousel.svelte']) check('actual carousel callback always returns a Promise', () => { assert.match(compose(relative), /}, async q => API\.Manga\.(Trending|Search)\(q\)\);/); });
console.log(JSON.stringify({ status: 'passed', cases, boundary: 'Actual anchored overlays and pure helper/script seams, using explicit UI/browser stand-ins; no real browser, network, database, service or package install.' }));
