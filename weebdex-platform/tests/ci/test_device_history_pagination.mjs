import { retainedTitleHistorySource } from './retained_title_history_source.mjs';
import fs from 'node:fs';
import path from 'node:path';
import vm from 'node:vm';
import { stripTypeScriptTypes } from 'node:module';
import { fileURLToPath } from 'node:url';
import test from 'node:test';
import assert from 'node:assert/strict';
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const context = vm.createContext({ URLSearchParams, URL, TextEncoder, TextDecoder, AbortController, setTimeout, clearTimeout, console, fetch: (...args) => fetcher(...args) });
let fetcher = () => { throw Error('Unexpected network'); };
const cache = new Map();
async function load(relative) {
  const full = path.resolve(root, relative); if (cache.has(full)) return cache.get(full);
  const source = fs.readFileSync(full, 'utf8');
  const module = new vm.SourceTextModule(full.endsWith('.ts') ? stripTypeScriptTypes(source, { mode: 'transform' }) : source, { context, identifier: full }); cache.set(full, module);
  await module.link(specifier => specifier === '../local-library/protocol/codec.js' ? new vm.SyntheticModule(['hash'], function () { this.setExport('hash', () => { throw Error('Unexpected hashing in record validation'); }); }, { context }) : load(path.relative(root, path.resolve(path.dirname(full), specifier + (path.extname(specifier) ? '' : '.ts'))))); return module;
}
const pagination = await load('platform-integrations/local-library/history-pagination.ts'); await pagination.evaluate();
const { HistoryPager, HistoryBudget, HistoryBudgetStop, HISTORY_LIMITS, historyRetainedSize } = pagination.namespace;
const transport = await load('platform-integrations/local-library/history-transport.ts'); await transport.evaluate();
const publicBatch = await load('platform-integrations/public-api/history-catalogue.ts'); await publicBatch.evaluate();
const { publicHistoryBatch } = publicBatch.namespace;
const uuid = n => 'aaaaaaaa-aaaa-8aaa-aaaa-' + String(n).padStart(12, '0');
const keys = count => Array.from({ length: count }, (_, i) => ({ id: uuid(i + 1), mangaId: uuid(999999), readAt: '2026-01-01T00:00:00.000Z' }));
const row = key => ({ id: key.id, relationships: { manga: { id: key.mangaId } } });
const plain = value => JSON.parse(JSON.stringify(value));
const hydrate = async batch => batch.map(row);
const complete = async (pager, page) => { for (let i = 0; i < 30; i++) { const result = await pager.load(page); if (result.ready) return result; } throw Error('Finite test continuation bound exceeded'); };
for (const size of [10000, 10001]) test(`${size} histories first, next and last are exact with 100-ID requests and unknown totals until completion`, async () => {
  let calls = 0; const pager = new HistoryPager(keys(size), async batch => { assert.ok(batch.length <= 100); calls++; return batch.map(row).reverse(); }, () => {});
  const first = await pager.load(1); assert.equal(first.data.length, 100); assert.equal(first.data[0].id, uuid(1)); assert.equal(first.total, null); assert.equal(calls, 1);
  const next = await pager.load(2); assert.equal(next.data[0].id, uuid(101)); assert.equal(next.total, null); assert.equal(calls, 2);
  const last = await complete(pager, Math.ceil(size / 100)); assert.equal(last.total, size); assert.equal(last.data.length, size === 10001 ? 1 : 100); assert.equal(last.data.at(-1).id, uuid(size)); assert.equal(last.continuation, false);
  const firstAgain = await complete(pager, 1); assert.equal(firstAgain.data[0].id, uuid(1)); pager.dispose();
});
test('stable equal read times tie by ID; both readAt directions retain that tie', async () => {
  for (const direction of ['asc', 'desc']) { const input = keys(3).reverse(); input[0].readAt = '2027-01-01T00:00:00.000Z'; const pager = new HistoryPager(input, hydrate, () => {}, 100, direction); assert.deepEqual(plain((await pager.load()).data.map(row => row.id)), direction === 'asc' ? [uuid(1), uuid(2), uuid(3)] : [uuid(3), uuid(1), uuid(2)]); }
});
test('sparse preferences continue instead of false empty/end and do not slice the first 100 local IDs', async () => {
  let calls = 0; const pager = new HistoryPager(keys(10001), async batch => { calls++; return batch.filter(key => Number(key.id.slice(-12)) > 9999).map(row); }, () => {});
  const initial = await pager.load(); assert.equal(initial.data.length, 0); assert.equal(initial.ready, false); assert.equal(initial.complete, false); assert.equal(initial.total, null); assert.equal(initial.continuation, true); assert.equal(initial.scanned, 1000); assert.equal(calls, 10);
  const final = await complete(pager, 1); assert.deepEqual(plain(final.data.map(row => row.id)), [uuid(10000), uuid(10001)]); assert.equal(final.total, 2);
});
test('hidden/missing scoped IDs do not take page slots across batches', async () => {
  const pager = new HistoryPager(keys(301), async batch => batch.filter(key => Number(key.id.slice(-12)) % 2).map(row), () => {});
  const first = await pager.load(); assert.equal(first.data.length, 100); assert.equal(first.data.at(-1).id, uuid(199)); const next = await pager.load(2); assert.equal(next.total, 151); assert.equal(next.data[0].id, uuid(201)); assert.equal(next.data.at(-1).id, uuid(301));
});
test('small page size does not discard early rows of a larger hydration chunk', async () => { for (const limit of [1, 20, 99]) { const pager = new HistoryPager(keys(301), hydrate, () => {}, limit); assert.equal((await pager.load()).data[0].id, uuid(1)); assert.equal((await pager.load(2)).data[0].id, uuid(limit + 1)); } });
test('transport errors cannot advance the failed batch; retry resumes acknowledged earlier scans', async () => {
  const seen = []; let fail = true;
  const pager = new HistoryPager(keys(201), async batch => { seen.push(batch[0].id); if (batch[0].id === uuid(101) && fail) throw Error('503'); return batch.filter(key => Number(key.id.slice(-12)) > 100).map(row); }, () => {});
  await assert.rejects(pager.load(), /503/); fail = false; const result = await pager.load(); assert.deepEqual(seen, [uuid(1), uuid(101), uuid(101)]); assert.equal(result.data[0].id, uuid(101));
});
test('unrequested, duplicate and wrong-parent responses do not advance any cursor', async () => {
  for (const bad of [batch => [row(keys(200)[199])], batch => [row(batch[0]), row(batch[0])], batch => [{ id: batch[0].id, relationships: { manga: { id: uuid(9) } } }]]) { let first = true, requests = []; const pager = new HistoryPager(keys(1), async batch => { requests.push(batch[0].id); if (first) { first = false; return bad(batch); } return batch.map(row); }, () => {}); await assert.rejects(pager.load(), /unrequested or wrong-parent/); assert.equal((await pager.load()).total, 1); assert.deepEqual(requests, [uuid(1), uuid(1)]); }
});
for (const change of ['scope', 'revision', 'account namespace', 'filter']) test(`${change} race refuses old rows before cursor commit`, async () => {
  let resolve, current = true; const pager = new HistoryPager(keys(1), batch => new Promise(done => { resolve = () => done(batch.map(row)); }), () => { if (!current) throw Error(change + ' changed'); });
  const pending = pager.load(); current = false; resolve(); await assert.rejects(pending, /changed/); await assert.rejects(pager.load(), /changed/); pager.dispose();
});
test('cancellation, concurrent clicks and disposed late success cannot publish or consume a batch', async () => {
  let resolve, captured; const pager = new HistoryPager(keys(1), (batch, budget) => { captured = budget; return new Promise(done => { resolve = () => done(batch.map(row)); }); }, () => {});
  const pending = pager.load(); await assert.rejects(pager.load(), /already in progress/); pager.cancel(); resolve(); await assert.rejects(pending, /cancelled/); assert.equal(captured.signal.aborted, true);
  const late = pager.load(); pager.dispose(); resolve(); await assert.rejects(late, /cancelled/);
});
test('request and byte budgets are shared across all chunks and stop without completing the scan', async () => {
  for (const kind of ['requests', 'bytes']) { let calls = 0; const pager = new HistoryPager(keys(2000), async (batch, budget) => { calls++; if (kind === 'requests') for (let i = 0; i < 9; i++) budget.request(); else budget.consume(3 * 1024 * 1024); return []; }, () => {}); const result = await pager.load(); assert.equal(result.ready, false); assert.equal(result.total, null); assert.equal(result.reason, kind === 'requests' ? 'request budget' : 'byte budget'); assert.equal(result.scanned, kind === 'requests' ? 400 : 200); assert.equal(calls, kind === 'requests' ? 5 : 3); }
});
test('deadline and pre-aborted budgets reject before another request', () => { let time = 0; const budget = new HistoryBudget(() => {}, () => time); time = HISTORY_LIMITS.deadlineMs; assert.throws(() => budget.request(), /deadline/); budget.close(); });
test('older evicted pages honestly rescan while only three full pages are retained', async () => { let calls = 0; const pager = new HistoryPager(keys(700), async batch => { calls++; return batch.map(row); }, () => {}); for (let page = 1; page <= 5; page++) await pager.load(page); assert.equal(calls, 5); await pager.load(4); assert.equal(calls, 5); const first = await pager.load(1); assert.equal(calls, 6); assert.equal(first.data[0].id, uuid(1)); assert.equal(first.total, null); });
test('empty snapshot is complete without a global request', async () => { const pager = new HistoryPager([], () => { throw Error('global'); }, () => {}); const result = await pager.load(); assert.equal(result.total, 0); assert.equal(result.continuation, false); });
const attrs = { title: { en: 'Fixture' }, originalLanguage: 'ja', contentRating: 'safe', status: 'ongoing', state: 'published', version: 1, translatedLanguage: 'en', createdAt: '2026-01-01T00:00:00Z', updatedAt: '2026-01-01T00:00:00Z', publishAt: '2026-01-01T00:00:00Z', tags: [] };
const resource = (id, type, parent) => ({ id, type, attributes: attrs, relationships: parent ? [{ id: parent, type: 'manga' }] : [] });
const envelope = (data, total = data.length, offset = 0) => ({ status: 200, body: { result: 'ok', response: 'collection', data, total, limit: 100, offset } });
test('public title+chapter filters stay scoped to local batch parents and children', async () => {
  const input = keys(3), calls = [], budget = new HistoryBudget(() => {});
  try { const result = await publicHistoryBatch(input, new URLSearchParams({ lang: 'ja', tagx: uuid(55), tlang: 'en', groupx: uuid(77), uploaderx: uuid(88), page: '9', limit: '1', sort: 'readAt' }), budget, async ({ path }) => { calls.push(path); const url = new URL(path, 'https://fixture.test'); if (url.pathname === '/manga') { assert.deepEqual(url.searchParams.getAll('ids[]'), [uuid(999999)]); assert.equal(url.searchParams.get('originalLanguage[]'), 'ja'); assert.equal(url.searchParams.get('excludedTags[]'), uuid(55)); assert.equal(url.searchParams.has('translatedLanguage[]'), false); return envelope([resource(uuid(999999), 'manga')]); } assert.deepEqual(url.searchParams.getAll('ids[]'), input.map(row => row.id)); assert.equal(url.searchParams.get('translatedLanguage[]'), 'en'); assert.equal(url.searchParams.get('excludedGroups[]'), uuid(77)); assert.equal(url.searchParams.get('excludedUploaders[]'), uuid(88)); assert.equal(url.searchParams.has('excludedTags[]'), false); return envelope(input.slice(1).map(key => resource(key.id, 'chapter', key.mangaId))); }); assert.equal(result.length, 2); assert.equal(calls.length, 2); } finally { budget.close(); }
});
test('successfully missing parents skip only their scoped children; 503 and malformed pages are not missing', async () => {
  for (const scenario of ['missing', '503', 'truncated', 'wrongParent', 'unrequested']) { let calls = 0; const budget = new HistoryBudget(() => {}); const pending = publicHistoryBatch(keys(1), new URLSearchParams(), budget, async ({ path }) => { calls++; if (scenario === '503') return { status: 503 }; if (scenario === 'truncated') return envelope([], 1); if (scenario === 'missing') return envelope([]); if (path.startsWith('/manga')) return envelope([resource(uuid(999999), 'manga')]); return envelope([resource(uuid(scenario === 'unrequested' ? 9 : 1), 'chapter', uuid(scenario === 'wrongParent' ? 9 : 999999))]); }); try { if (scenario === 'missing') { assert.equal((await pending).length, 0); assert.equal(calls, 1); } else await assert.rejects(pending); } finally { budget.close(); } }
});
test('public byte-bounded prefix pagination retains exact scopes and changing totals fail closed', async () => {
  for (const changed of [false, true]) { const budget = new HistoryBudget(() => {}); let calls = 0; const pending = publicHistoryBatch(keys(2), new URLSearchParams(), budget, async ({ path }) => { if (path.startsWith('/manga')) return envelope([resource(uuid(999999), 'manga')]); const q = new URL(path, 'https://fixture.test').searchParams; calls++; return envelope([resource(uuid(calls), 'chapter', uuid(999999))], changed && calls > 1 ? 1 : 2, Number(q.get('offset'))); }); try { if (changed) await assert.rejects(pending, /scope or total|declared page bounds/); else { assert.equal((await pending).length, 2); assert.equal(calls, 2); } } finally { budget.close(); } }
});
test('actual streamed transport accounts bytes, propagates cancellation and rejects oversized JSON before parse', async () => {
  const budget = new HistoryBudget(() => {}); let signal;
  try { fetcher = async (url, options) => { signal = options.signal; return new Response(JSON.stringify({ ok: true }), { headers: { 'Content-Type': 'application/json' } }); }; assert.equal((await transport.namespace.historyJson('/fixture', budget)).body.ok, true); assert.equal(budget.requests, 1); assert.equal(budget.bytes, 11); assert.equal(signal.aborted, false); budget.cancel(); assert.equal(signal.aborted, false); budget.close();
  } finally { budget.close(); }
  const oversized = new HistoryBudget(() => {}); try {
    fetcher = async () => new Response(' '.repeat(HISTORY_LIMITS.responseBytes + 1), { headers: { 'Content-Type': 'application/json' } }); await assert.rejects(transport.namespace.historyJson('/fixture', oversized), /byte budget/);
  } finally { oversized.close(); }
});
test('old source counterfactual retains the 10001 pre-pagination cap and all-ID hydration', () => {
  const old = fs.readFileSync(path.join(root, 'tests/fixtures/history-eager-old-source.txt'), 'utf8'); assert.match(old, /filteredCatalog<Chapter>\('\/chapter\/feed', 'id', \[\.\.\.history.keys\(\)\]/); assert.match(old, /unique.length>\(parents\?maxParents:maxRows\)/); assert.match(old, /maxRows=10000/);
  const current = fs.readFileSync(path.join(root, 'platform-integrations/local-library/ChapterHistory.ts'), 'utf8'); assert.doesNotMatch(current, /\[\.\.\.history.keys\(\)\]/); assert.match(current, /openDeviceHistoryFeed/);
});
test('executed eager old-source counterfactual rejects 10001 completed IDs before any hydration', async () => {
  const text = fs.readFileSync(path.join(root, 'tests/fixtures/history-eager-old-source.txt'), 'utf8'), boundary = text.indexOf('// Hydration for a device-owned UUID library.');
  const oldContext = vm.createContext({ URLSearchParams });
  const stub = values => new vm.SyntheticModule(Object.keys(values), function () { for (const [key, value] of Object.entries(values)) this.setExport(key, value); }, { context: oldContext });
  let requests = 0;
  const candidates = new vm.SourceTextModule(stripTypeScriptTypes(text.slice(boundary), { mode: 'transform' }), { context: oldContext });
  await candidates.link(specifier => specifier === './site-client' ? stub({ publicCatalogueRead: async () => { requests++; throw Error('Unexpected hydration'); } }) : specifier === './validation' ? stub({ publicUuid: value => value, PublicApiContractError: class extends Error {}, record: value => value }) : stub({ utcOrderKey: value => value, decimalKey: value => value })); await candidates.evaluate();
  const oldFeed = new vm.SourceTextModule(stripTypeScriptTypes(text.slice(0, boundary), { mode: 'transform' }), { context: oldContext });
  const library = { apiRead() {}, accountRequest() {}, captureLibraryView: () => 'device:1', assertLibraryView() {}, currentDeviceSnapshot: async () => ({ records: keys(10001).map(key => ({ ...key, family: 'chapters', completed: true, updated_at: 1 })) }), isCurrentCatalogueId: () => true, isAccountMode: async () => false, updateChapters() {}, params: input => new URLSearchParams(input), assertCatalogueScope() {}, openLibrary() {}, captureLibraryContext() {}, assertLibraryContext() {} };
  await oldFeed.link(specifier => specifier === './library' ? stub(library) : specifier === '../public-api/library-staging' ? stub({ assertCataloguePreviewLibraryReady() {} }) : specifier === '$lib/HTTP' ? stub({ HTTPError: Error }) : stub({ filteredCatalog: (path, key, ids, query) => candidates.namespace.publicLibraryCandidates(path, key, ids, query), pageRows() { throw Error('Paging must not be reached'); }, compareValue() {} })); await oldFeed.evaluate();
  await assert.rejects(oldFeed.namespace.Feed(new URLSearchParams({ limit: '100' })), /bounded catalogue budget/); assert.equal(requests, 0);
});
test('unchanged global parent-composition counterfactual demonstrates why history requires an exact-parent adapter', async () => {
  const oldComposition = await load('platform-integrations/public-api/author-chapter-feed.ts'); await oldComposition.evaluate();
  const calls = [];
  await oldComposition.namespace.scopedMangaChapterCollection({ collection: async (path, type, query) => { calls.push({ path, query }); return envelope([]); } }, new URLSearchParams({ id: uuid(1), tagx: uuid(55), tlang: 'en' }));
  assert.equal(calls[0].path, '/manga'); assert.equal(calls[0].query.getAll('ids[]').length, 0); assert.equal(calls[0].query.get('excludedTags[]'), uuid(55));
});
test('real public transport shared request budget includes every byte-prefix subpage and continuation reduces an unfinishable batch', async () => {
  const input = keys(100).map((key, i) => ({ ...key, mangaId: uuid(5000 + i) })), parent = new Map(input.map(key => [key.id, key.mangaId])); let requests = 0;
  fetcher = async (url, options) => {
    requests++; assert.equal(options.method, 'GET'); const parsed = new URL(url, 'https://fixture.test'), q = parsed.searchParams, ids = q.getAll('ids[]'), offset = Number(q.get('offset'));
    assert.ok(ids.length >= 1 && ids.length <= 100);
    const type = parsed.pathname.endsWith('/manga') ? 'manga' : 'chapter', id = ids[offset];
    return new Response(JSON.stringify(envelope([resource(id, type, type === 'chapter' ? parent.get(id) : undefined)], ids.length, offset).body), { headers: { 'Content-Type': 'application/json' } });
  };
  const pager = new HistoryPager(input, (batch, budget) => publicHistoryBatch(batch, new URLSearchParams(), budget), () => {});
  const first = await pager.load(); assert.equal(requests, 40); assert.equal(first.scanned, 0); assert.equal(first.total, null); assert.equal(first.reason, 'request budget');
  const final = await complete(pager, 1); assert.equal(final.total, 100); assert.equal(final.data.length, 100); assert.equal(final.data.at(-1).id, uuid(100));
});
test('already-aborted streaming read is cancelled and a malformed JSON response is never a missing chapter', async () => {
  const budget = new HistoryBudget(() => {});
  try { fetcher = async () => new Response('{bad', { headers: { 'Content-Type': 'application/json' } }); await assert.rejects(transport.namespace.historyJson('/fixture', budget), /JSON/); budget.cancel(); let calls = 0; fetcher = async () => { calls++; throw Error('network'); }; await assert.rejects(transport.namespace.historyJson('/fixture', budget), /cancelled/); assert.equal(calls, 0); } finally { budget.close(); }
});
test('native account Feed keeps its original exact numeric-total contract and never opens a device session', async () => {
  const local = vm.createContext({ URLSearchParams });
  const stub = values => new vm.SyntheticModule(Object.keys(values), function () { for (const [key, value] of Object.entries(values)) this.setExport(key, value); }, { context: local });
  let calls = 0, checks = 0;
  const body = { total: 25001, page: 3, limit: 100, data: [], map: { manga: {} } };
  const source = fs.readFileSync(path.join(root, 'platform-integrations/local-library/ChapterHistory.ts'), 'utf8');
  const module = new vm.SourceTextModule(stripTypeScriptTypes(source, { mode: 'transform' }), { context: local });
  await module.link(specifier => specifier === './library' ? stub({ apiRead: async (path, query, account) => { calls++; assert.equal(path, '/user/history/manga/feed'); assert.equal(account, true); assert.equal(query.get('page'), '3'); return body; }, accountRequest() {}, captureLibraryView: () => 'account:selected:0', assertLibraryView: () => checks++, currentDeviceSnapshot() { throw Error('Device snapshot must not open'); }, isCurrentCatalogueId() {}, isAccountMode: async () => true, updateChapters() {}, params: input => new URLSearchParams(input), assertCatalogueScope() {}, openLibrary() {}, captureLibraryContext() {}, assertLibraryContext() {} }) : specifier === '../public-api/library-staging' ? stub({ assertCataloguePreviewLibraryReady() {} }) : specifier === '$lib/HTTP' ? stub({ HTTPError: Error }) : stub({ openDeviceHistoryFeed() { throw Error('Account feed must not use device paging'); } })); await module.evaluate();
  const response = await module.namespace.Feed(new URLSearchParams({ page: '3', limit: '100' })); assert.equal(response.body, body); assert.equal(calls, 1); assert.equal(checks, 1);
});

test('history sorts mixed timestamp precision, equal instants and offsets chronologically without changing read_at', async () => {
  const input = keys(5);
  const dates = ['2026-01-01T00:00:00.900Z', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00.000Z', '2026-01-01T00:30:00+01:00', '2025-12-31T23:30:00-01:00'];
  input.forEach((key, i) => key.readAt = dates[i]);
  for (const [direction, expected] of [['asc', [4, 2, 3, 1, 5]], ['desc', [5, 1, 2, 3, 4]]]) {
    const pager = new HistoryPager(input.slice().reverse(), hydrate, () => {}, 100, direction);
    const result = await pager.load(); assert.deepEqual(plain(result.data.map(row => row.id)), expected.map(uuid));
    for (const row of result.data) assert.equal(row.read_at, dates[Number(row.id.slice(-12)) - 1]);
  }
  for (const readAt of ['bad-date', '2026-13-01T00:00:00Z', '2026-01-01T29:00:00Z', '']) assert.throws(() => new HistoryPager([{ ...input[0], readAt }], hydrate, () => {}), /Invalid device history snapshot/);
});
test('scan allowance clamps non-divisor page and adaptive scopes to exactly 1000 candidates', async () => {
  let scanned = [], requests = 0;
  const pager = new HistoryPager(keys(2001), async (batch, budget) => { budget.request(); requests++; scanned.push(batch.length); return []; }, () => {}, 99);
  const result = await pager.load(); assert.equal(result.scanned, 1000); assert.equal(result.ready, false); assert.deepEqual(scanned, [...Array(10).fill(99), 10]); assert.equal(requests, 11);
  const adaptive = []; let attempts = 0, totalRequests = 0;
  const shrinking = new HistoryPager(keys(2001), async (batch, budget) => {
    adaptive.push(batch.length);
    if (++attempts <= 3) for (let i = 0; i <= 40; i++) { budget.request(); totalRequests++; }
    return [];
  }, () => {});
  for (let i = 0; i < 3; i++) assert.equal((await shrinking.load()).scanned, 0);
  assert.deepEqual(adaptive, [100, 50, 25]); adaptive.length = 0;
  const continuation = await shrinking.load(); assert.equal(continuation.scanned, 1000); assert.equal(continuation.ready, false);
  assert.deepEqual(adaptive, [...Array(83).fill(12), 4]); assert.equal(totalRequests, 120);
});

test('identity-aware retained accounting is iterative, bounded, and does not expand shared parents', () => {
  const parent = { id: 'shared', description: 'd'.repeat(15000) }, rows = Array.from({ length: 100 }, (_, i) => ({ id: String(i), relationships: { manga: parent } }));
  assert.ok(historyRetainedSize([rows]) < 100000); assert.ok(JSON.stringify(rows).length > 1500000);
  const cycle = {}; cycle.self = cycle; assert.ok(Number.isFinite(historyRetainedSize([cycle])));
  let deep = {}; for (let i = 0; i < 20000; i++) deep = { child: deep }; assert.ok(Number.isFinite(historyRetainedSize([deep])));
  assert.equal(historyRetainedSize([Array(70000).fill(null)]), Infinity);
  assert.equal(historyRetainedSize([{ value: 'x'.repeat(HISTORY_LIMITS.cacheBytes) }]), Infinity);
});
test('actual public adapter renders 100 shared-parent chapters with a 15 KB description; rejected source loops on the same batch', async () => {
  const description = 'd'.repeat(15000), input = keys(100); let calls = 0;
  const scoped = async ({ path }) => { calls++; return path.startsWith('/manga') ? envelope([{ ...resource(uuid(999999), 'manga'), attributes: { ...attrs, description: { en: description } } }]) : envelope(input.map(key => resource(key.id, 'chapter', key.mangaId))); };
  const pager = new HistoryPager(input, (batch, budget) => publicHistoryBatch(batch, new URLSearchParams(), budget, scoped), () => {});
  const page = await pager.load(); assert.equal(page.data.length, 100); assert.equal(page.total, 100); assert.equal(calls, 2);
  assert.equal(page.data[0].relationships.manga, page.data[99].relationships.manga); assert.equal(page.data[0].relationships.manga.description, description);
  assert.ok(historyRetainedSize([pager.window, pager.displayed, ...pager.pages.values()]) <= HISTORY_LIMITS.cacheBytes);
  const oldContext = vm.createContext({ TextEncoder, AbortController, setTimeout, clearTimeout });
  const old = new vm.SourceTextModule(stripTypeScriptTypes(fs.readFileSync(path.join(root, 'tests/fixtures/history-pagination-rejected-a147c4f.ts.txt'), 'utf8'), { mode: 'transform' }), { context: oldContext }); await old.link(() => { throw Error('unexpected import'); }); await old.evaluate();
  calls = 0; const rejectedPager = new old.namespace.HistoryPager(input, (batch, budget) => publicHistoryBatch(batch, new URLSearchParams(), budget, scoped), () => {});
  await assert.rejects(rejectedPager.load(), /retained page byte budget/); await assert.rejects(rejectedPager.load(), /retained page byte budget/); assert.equal(calls, 4); assert.equal(rejectedPager.offset, 0);
});
test('many distinct large parents adapt scopes and visible page size, then traverse every ID without skipping', async () => {
  const input = keys(100).map((key, i) => ({ ...key, mangaId: uuid(5000 + i) })), parent = new Map(input.map(key => [key.id, key.mangaId])); let requests = 0;
  fetcher = async (url) => {
    requests++; const parsed = new URL(url, 'https://fixture.test'), q = parsed.searchParams, ids = q.getAll('ids[]'), offset = Number(q.get('offset')), type = parsed.pathname.endsWith('/manga') ? 'manga' : 'chapter';
    const data = ids.slice(offset, offset + (type === 'manga' ? 10 : 100)).map(id => type === 'manga' ? { ...resource(id, type), attributes: { ...attrs, description: { en: 'p'.repeat(15000) } } } : resource(id, type, parent.get(id)));
    const body = JSON.stringify(envelope(data, ids.length, offset).body); assert.ok(new TextEncoder().encode(body).length < HISTORY_LIMITS.responseBytes);
    return new Response(body, { headers: { 'Content-Type': 'application/json' } });
  };
  const pager = new HistoryPager(input, (batch, budget) => publicHistoryBatch(batch, new URLSearchParams(), budget), () => {}), notices = [], delivered = [];
  let target = 1, final = false;
  for (let action = 0; action < 60 && !final; action++) {
    const before = requests, page = await pager.load(target); assert.ok(requests - before <= 40); assert.ok(page.scanned <= 100);
    assert.ok(historyRetainedSize([pager.displayed, pager.window, ...pager.pages.values()]) <= HISTORY_LIMITS.cacheBytes);
    if (page.notice) notices.push(page.notice); target = page.page;
    if (page.ready) { delivered.push(...page.data.map(row => row.id)); if (page.complete && !page.continuation) { final = true; assert.equal(page.total, 100); } else { pager.releaseDisplayedPage(); target++; } }
    else assert.equal(page.total, null);
  }
  assert.equal(final, true); assert.ok(pager.limit < 100); assert.ok(notices.some(text => /Page size reduced.*Restarted at page 1/.test(text))); assert.deepEqual(delivered, input.map(row => row.id));
});
test('displayed DTOs stay accounted after cache eviction; explicit display release permits the next bounded page', async () => {
  const create = () => new HistoryPager(keys(2), async batch => batch.map(key => ({ ...row(key), payload: { text: 'x'.repeat(1100000) } })), () => {}, 1);
  const pinned = create(); const first = await pinned.load(); assert.equal(first.data.length, 1);
  await assert.rejects(pinned.load(2), /Loading is stopped without skipping/); assert.equal(pinned.offset, 1); assert.equal(pinned.displayed[0].id, uuid(1)); assert.equal(pinned.pages.size, 0);
  await assert.rejects(pinned.load(2), /Loading is stopped without skipping/);
  const cleared = create(); await cleared.load(); cleared.releaseDisplayedPage(); const next = await cleared.load(2); assert.equal(next.total, 2); assert.equal(next.data[0].id, uuid(2)); assert.ok(historyRetainedSize([cleared.displayed, cleared.window, ...cleared.pages.values()]) <= HISTORY_LIMITS.cacheBytes);
});
test('one oversized retained record stops without advance or unchanged one-item retries', async () => {
  let calls = 0; const pager = new HistoryPager(keys(1), async batch => { calls++; return batch.map(key => ({ ...row(key), payload: { text: 'x'.repeat(2200000) } })); }, () => {});
  await assert.rejects(pager.load(), /One saved chapter exceeds/); assert.equal(calls, 1); assert.equal(pager.offset, 0); await assert.rejects(pager.load(), /Loading is stopped/); assert.equal(calls, 1);
});

test('a valid small-wire dense resource can exceed retained representation and stops once without losing its ID', async () => {
  let calls = 0;
  fetcher = async url => {
    calls++; const isParent = new URL(url, 'https://fixture.test').pathname.endsWith('/manga');
    const data = isParent ? [{ ...resource(uuid(999999), 'manga'), attributes: { ...attrs, extraObjects: Array.from({ length: 60000 }, () => ({})) } }] : [resource(uuid(1), 'chapter', uuid(999999))];
    const body = JSON.stringify(envelope(data).body); assert.ok(new TextEncoder().encode(body).length < HISTORY_LIMITS.responseBytes); return new Response(body, { headers: { 'Content-Type': 'application/json' } });
  };
  const pager = new HistoryPager(keys(1), (batch, budget) => publicHistoryBatch(batch, new URLSearchParams(), budget), () => {});
  await assert.rejects(pager.load(), /One saved chapter exceeds/); assert.equal(pager.offset, 0); assert.equal(calls, 2); await assert.rejects(pager.load(), /Loading is stopped/); assert.equal(calls, 2);
});

async function previousMillisecondPager() {
 const source=retainedTitleHistorySource('d645756-history-pagination');
 const module=new vm.SourceTextModule(stripTypeScriptTypes(source,{mode:'transform'}),{context});await module.link(()=>{throw Error('Unexpected prior history dependency');});await module.evaluate();return module.namespace.HistoryPager;
}
const chronologyDates=['2026-01-01T01:00:00+02:00','2026-01-01T00:00:00Z','2026-01-01T00:00:00.100Z','2026-01-01T01:00:00.1+01:00','2026-01-01T00:00:00.100000002Z','2026-01-01T00:00:00.100000001Z'];
const chronologyKeys=()=>chronologyDates.map((readAt,i)=>({id:uuid(i+1),mangaId:uuid(999999),readAt}));
for(const direction of ['asc','desc'])test(`history exact fraction chronology keeps display originals and bounded pages: ${direction}`,async()=>{
 const input=chronologyKeys(),expected=(direction==='asc'?[1,2,3,4,6,5]:[5,6,3,4,2,1]).map(uuid),calls=[],pager=new HistoryPager([...input].reverse(),async batch=>{calls.push(batch.length);return batch.map(row).reverse();},()=>{},2,direction);
 try{const data=[];for(let page=1;page<=3;page++){const result=await pager.load(page);assert.equal(result.ready,true);assert.equal(result.total,page===3?6:null);data.push(...result.data);}assert.deepEqual(plain(data.map(row=>row.id)),expected);for(const value of data)assert.equal(value.read_at,input.find(key=>key.id===value.id).readAt);assert.deepEqual(calls,[2,2,2]);assert.deepEqual(plain((await pager.load(1)).data.map(row=>row.id)),expected.slice(0,2));}finally{pager.dispose();}
});
for(const direction of ['asc','desc'])test(`executed prior history already handles zones but collapses distinct ISO sub-ms instants: ${direction}`,async()=>{
 const Old=await previousMillisecondPager(),input=chronologyKeys(),expected=(direction==='asc'?[1,2,3,4,6,5]:[5,6,3,4,2,1]).map(uuid);
 const offsets=new Old(input.slice(0,4),hydrate,()=>{},100,direction),fractional=new Old(input,hydrate,()=>{},100,direction);
 try{assert.deepEqual(plain((await offsets.load()).data.map(row=>row.id)),(direction==='asc'?[1,2,3,4]:[3,4,2,1]).map(uuid));assert.notDeepEqual(plain((await fractional.load()).data.map(row=>row.id)),expected);}finally{offsets.dispose();fractional.dispose();}
});

test('actual saved-record ingestion preserves accepted timestamp spellings and the extracted title helper is equivalent',async()=>{
 const state=await load('platform-integrations/public-api/device-id-state.ts');if(state.status!=='evaluated')await state.evaluate();
 for(const readAt of [...chronologyDates,'Wed, 01 Jan 2025 00:00:00 GMT']){const record={family:'chapters',key:uuid(1),id:uuid(1),mangaId:uuid(999999),completed:true,page:1,readAt,updated_at:1};assert.equal(state.namespace.normalizeMigrationRecord(record).readAt,readAt);}
 const old=retainedTitleHistorySource('d645756-title-order'),start=old.indexOf('// Device ingestion preserves valid date spellings.'),end=old.indexOf('export function followedAtOrderKey',start);
 const extracted=fs.readFileSync(path.join(root,'platform-integrations/local-library/device-time.ts'),'utf8').replaceAll('deviceTimestampInstant','followedAtInstant').replaceAll('Invalid saved-device date','Invalid followed-at date');assert.equal(extracted,old.slice(start,end));
});
