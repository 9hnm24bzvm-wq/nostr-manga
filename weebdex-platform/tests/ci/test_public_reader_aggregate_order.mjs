// Real retained catalogue client, normalization and exact-number comparator;
// only the public transport is a finite stand-in. No browser/server is started.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import vm from 'node:vm';
import { stripTypeScriptTypes } from 'node:module';
import test from 'node:test';
import { createFrontendOverlays } from '../../platform-integrations/frontend-overlays.mjs';

const root = path.resolve(new URL('../../', import.meta.url).pathname), directory = path.join(root, 'platform-integrations/public-api');
const context = vm.createContext({ URL, URLSearchParams, AbortController, TextEncoder, console, fetch() { throw Error('Unexpected real network request'); } }), modules = new Map();
async function load(file) {
  if (!modules.has(file)) {
    const module = new vm.SourceTextModule(stripTypeScriptTypes(fs.readFileSync(file, 'utf8'), { mode: 'transform' }), { context, identifier: file });
    modules.set(file, module);
    await module.link((specifier, importer) => load(specifier === '$lib/constants/Error' ? path.join(root, 'frontend/src/lib/constants/Error.ts') : path.resolve(path.dirname(importer.identifier), specifier + '.ts')));
  }
  return modules.get(file);
}
// Link common leaves first because SourceTextModule's linker runs sibling
// requests concurrently, including shared dependencies of the site bridge.
for (const name of ['validation', 'query', 'normalize']) { const module = await load(path.join(directory, name + '.ts')); await module.evaluate(); }
const client = await load(path.join(directory, 'client.ts')); await client.evaluate();
const order = await load(path.join(directory, 'reader-order.ts')); await order.evaluate();
const site = await load(path.join(directory, 'site-client.ts')); await site.evaluate();
const uuid = value => '00000000-0000-8000-8000-' + String(value).padStart(12, '0');
const mangaId = uuid(1), groupId = uuid(2), otherGroupId = uuid(3);
const manga = { id: mangaId, type: 'manga', attributes: { title: { en: 'Actual title' }, originalLanguage: 'ja', status: 'ongoing', contentRating: 'safe', tags: [] }, relationships: [] };
function chapter(index, volume, number, language = 'en', group = groupId) {
  return { id: uuid(index + 100), type: 'chapter', attributes: { title: null, volume, chapter: number, translatedLanguage: language, pages: 1, publishAt: '2026-10-19T00:00:00Z' },
    relationships: [{ id: mangaId, type: 'manga' }, { id: group, type: 'scanlation_group', attributes: { name: group === groupId ? 'Current group' : 'Other group' } }] };
}
function fixture(rows) {
  const requests = [];
  const api = new client.namespace.PublicApiClient(async request => {
    const url = new URL('https://fixture.invalid' + request.path); requests.push({ url, signal: request.signal });
    const collection = (data, total = data.length, offset = 0) => ({ status: 200, body: { result: 'ok', response: 'collection', data, limit: 100, offset, total } });
    if (url.pathname.endsWith('/aggregate')) return { status: 200, body: { result: 'ok', volumes: {} } };
    if (url.pathname === '/chapter') { const offset = Number(url.searchParams.get('offset')); return collection(rows.slice(offset, offset + 100), rows.length, offset); }
    if (url.pathname === '/manga') return collection([manga]);
    if (url.pathname === '/nostr/native-forum/contexts') return { status: 200, body: { result: 'ok', data: request.body.resources.map(row => ({ ...row, thread: null })), unavailable: [] } };
    throw Error('Unexpected transport route ' + request.path);
  });
  return { requests, read: options => new site.namespace.RetainedCatalogueClient(api).read('chapter.aggregate', { id: mangaId, ...options }) };
}
const positions = body => JSON.parse(JSON.stringify(body.chapters.map(row => [row.volume, row.chapter])));
const titleFile = path.join(root, 'frontend/src/routes/(app)/title/MangaActions.svelte');
const title = createFrontendOverlays(path.join(root, 'frontend')).transform.handler(fs.readFileSync(titleFile, 'utf8'), titleFile).code;
const start = title.indexOf('\t\t\t\t\tif (history) {'), end = title.indexOf('\t\t\t\t\tif (targetAggregatedChapter) {', start);
assert(start > 0 && end > start, 'Actual Continue Reading selection must be present');
const continueAt = new Function('aggregated', 'history', 'console', 'let targetAggregatedChapter;\n' + title.slice(start, end) + '\nreturn targetAggregatedChapter;');
test('actual aggregate uses exact volume/chapter order despite a later upload of an older chapter', async () => {
  const rows = [chapter(1, '1', '2'), chapter(2, '1', '9.99999999999999999999'), chapter(3, '1', '10'), chapter(4, '2', '1'), chapter(5, '1', '2.5')];
  const response = await fixture(rows).read();
  assert.deepEqual(positions(response.body), [['2', '1'], ['1', '10'], ['1', '9.99999999999999999999'], ['1', '2.5'], ['1', '2']]);
});
test('exact volume decimals also survive Number collisions before chapter comparison', async () => {
  const f = fixture([chapter(1, '9.99999999999999999999', '999'), chapter(2, '10', '1')]);
  assert.deepEqual(positions((await f.read()).body), [['10', '1'], ['9.99999999999999999999', '999']]);
});
test('all pages are gathered before aggregate ordering, including a newest position on page two', async () => {
  const rows = Array.from({ length: 101 }, (_, index) => chapter(index, '1', String(index + 1))), f = fixture(rows);
  const response = await f.read(); assert.equal(response.body.chapters.length, 101);
  assert.deepEqual(positions(response.body).slice(0, 2), [['1', '101'], ['1', '100']]);
  assert.deepEqual(f.requests.filter(row => row.url.pathname === '/chapter').map(row => row.url.searchParams.get('offset')), ['0', '100']);
});
test('sorting preserves real alternate editions, language/group indexes and query/signal constraints', async () => {
  const en = chapter(1, '1', '2.5'), fr = chapter(2, '1', '2.5', 'fr', otherGroupId), f = fixture([en, fr, chapter(3, '2', '1')]);
  const signal = new AbortController().signal, response = await f.read({ query: new URLSearchParams({ tlang: 'en', group: groupId }), signal });
  const edition = response.body.chapters.find(row => row.chapter === '2.5');
  assert.deepEqual(Object.keys(edition.entries), [en.id, fr.id]);
  assert.equal(response.body.languages[edition.entries[en.id].language], 'en'); assert.equal(response.body.languages[edition.entries[fr.id].language], 'fr');
  assert.equal(response.body.groups[edition.entries[en.id].groups[0]].id, groupId); assert.equal(response.body.groups[edition.entries[fr.id].groups[0]].id, otherGroupId);
  const selector = f.requests.find(row => row.url.pathname === '/chapter').url;
  assert.equal(selector.searchParams.get('translatedLanguage[]'), 'en'); assert.equal(selector.searchParams.get('groups[]'), groupId); assert.ok(f.requests.every(row => row.signal === signal));
});
test('empty aggregates remain empty, and exact numeric ties retain labels with specials/null last', async () => {
  assert.deepEqual(positions((await fixture([]).read()).body), []);
  const compare = order.namespace.compareReaderChapters, rows = ['2', null, 'special', '02', '2.0'].map(chapter => ({ volume: '1', chapter }));
  assert.deepEqual(rows.sort(compare).map(row => row.chapter), ['2.0', '2', '02', 'special', null]);
});
test('actual Continue Reading selects the last completed aggregate position after old-edition uploads', async () => {
  const old = chapter(1, '1', '2'), decimal = chapter(2, '1', '2.5'), last = chapter(3, '2', '1');
  const { body } = await fixture([old, last, decimal]).read(), history = Object.fromEntries([old, last, decimal].map(row => [row.id, null]));
  const selected = continueAt(body, history, { info() {} }); assert.deepEqual(Object.keys(selected.entries), [last.id]);
});
test('actual Continue Reading selects the next unread decimal position and starts at oldest with no history', async () => {
  const old = chapter(1, '1', '2'), decimal = chapter(2, '1', '2.5'), last = chapter(3, '2', '1');
  const { body } = await fixture([last, old, decimal]).read();
  assert.deepEqual(Object.keys(continueAt(body, { [old.id]: null }, { info() {} }).entries), [decimal.id]);
  assert.deepEqual(Object.keys(continueAt(body, undefined, { info() {} }).entries), [old.id]);
});
