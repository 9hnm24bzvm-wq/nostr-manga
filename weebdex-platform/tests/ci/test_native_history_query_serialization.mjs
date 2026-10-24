import { retainedTitleHistorySource } from './retained_title_history_source.mjs';
// Narrow real-function control for native history URL serialization only.
// No browser, network, server, build or saved-library writes.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import vm from 'node:vm';
import { stripTypeScriptTypes } from 'node:module';
import { fileURLToPath } from 'node:url';
import test from 'node:test';
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const filename = 'weebdex-platform/platform-integrations/local-library/history-feed.ts';
const current = fs.readFileSync(path.join(root, 'platform-integrations/local-library/history-feed.ts'), 'utf8');
const prior = retainedTitleHistorySource('08c3bb9-history-feed');
const library = fs.readFileSync(path.join(root, 'platform-integrations/local-library/library.ts'), 'utf8');
const layout = fs.readFileSync(path.join(root, 'frontend/src/routes/+layout.ts'), 'utf8');
function fixture(source = current, prefixed = true) {
  class Query extends URLSearchParams {}
  const calls = [], budget = { exact: 'budget identity' };
  let response;
  const context = vm.createContext({ URLSearchParams: Query, PUBLIC_API_URL: '/api/', historyJson: async (url, actualBudget, native) => {
    calls.push({ url, budget: actualBudget, native });
    const data = new URL(url, 'https://native.fixture').searchParams.getAll('id').map(id => ({ id }));
    return response ?? { status: 200, body: { total: data.length, limit: 100, page: 1, data } };
  } });
  if (prefixed) vm.runInContext(stripTypeScriptTypes(layout.slice(layout.indexOf('const _toString ='), layout.indexOf('Number.prototype.short ='))), context);
  const encoder = library.split('\n').find(line => line.startsWith('export const encodeParams =')).replace('export ', '');
  vm.runInContext(stripTypeScriptTypes(encoder + '\n' + source.slice(source.indexOf('async function nativeHistoryBatch'), source.indexOf('\n// The caller owns'))), context);
  return { calls, budget, Query, run: vm.runInContext('nativeHistoryBatch', context), respond(value) { response = value; } };
}
const keys = Array.from({ length: 100 }, (_, index) => ({ id: 'chapter-' + index }));
for (const prefixed of [false, true]) test(`current native history keeps all filters and bounded identities with retained prefix=${prefixed}`, async () => {
  const f = fixture(current, prefixed), query = new f.Query('contentRating=safe&contentRating=suggestive&lang=ja&tag=a%26b&title=a+b&id=unwanted&page=99&limit=3&sort=title&order=asc');
  const rows = await f.run(keys, query, f.budget), sent = f.calls[0], parsed = new URL(sent.url, 'https://native.fixture').searchParams;
  assert.equal(rows.length, 100); assert.equal(f.calls.length, 1); assert.equal(sent.url.includes('??'), false);
  assert.equal(sent.budget, f.budget); assert.equal(sent.native, true); assert.deepEqual(parsed.getAll('id'), keys.map(row => row.id));
  assert.deepEqual(parsed.getAll('contentRating'), ['safe', 'suggestive']); assert.equal(parsed.get('lang'), 'ja'); assert.equal(parsed.get('tag'), 'a&b'); assert.equal(parsed.get('title'), 'a b');
  assert.equal(parsed.get('page'), '1'); assert.equal(parsed.get('limit'), '100'); assert.equal(parsed.has('sort'), false); assert.equal(parsed.has('order'), false);
});
test('executed prior history function loses the first filter under the actual retained extension', async () => {
  const f = fixture(prior), query = new f.Query('lang=ja&contentRating=safe'); await f.run(keys.slice(0, 1), query, f.budget);
  const sent = f.calls[0], parsed = new URL(sent.url, 'https://native.fixture').searchParams;
  assert.equal(sent.url.includes('??'), true); assert.equal(parsed.get('lang'), null); assert.equal(parsed.get('?lang'), 'ja');
});
test('empty native history selection still makes no transport request', async () => {
  const f = fixture(); assert.equal((await f.run([], new f.Query('lang=ja'), f.budget)).length, 0); assert.equal(f.calls.length, 0);
});
test('failed and incomplete native history reads still reject rather than skipping records', async () => {
  for (const response of [{ status: 503 }, { status: 200, body: { total: 2, data: [{ id: 'chapter-0' }], limit: 100, page: 1 } }]) {
    const f = fixture(); f.respond(response); await assert.rejects(f.run(keys.slice(0, 1), new f.Query(), f.budget), /failed|incomplete/); assert.equal(f.calls.length, 1);
  }
});
test('history product delta is exactly the encoder import and one URL expression', () => {
  assert.equal(current.replace('import { encodeParams, assertCurrentCatalogueScopeValue,', 'import { assertCurrentCatalogueScopeValue,').replace("+ '/chapter/feed?' + encodeParams(query), budget", "+ '/chapter/feed?' + query, budget"), prior);
});
