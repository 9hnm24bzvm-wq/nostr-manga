// Source-authored goldens for the unchanged retained reader's real comparator
// and real overlaid edition selector. No database/server/browser is started.
// This file is not evidence that backend SQL agrees until both suites execute.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import vm from 'node:vm';
import { stripTypeScriptTypes } from 'node:module';
import test from 'node:test';
import { fileURLToPath, pathToFileURL } from 'node:url';

// A source-only sparse lane can read the unchanged frozen owner tree without
// copying dependencies or overlay inputs. Default remains this checkout.
const root = path.resolve(process.env.READER_GOLDEN_SOURCE_ROOT ?? fileURLToPath(new URL('../../', import.meta.url)));
const { createFrontendOverlays } = await import(pathToFileURL(path.join(root, 'platform-integrations/frontend-overlays.mjs')).href);
const context = vm.createContext({});
const modules = new Map();
async function load(file) {
  if (!modules.has(file)) {
    const module = new vm.SourceTextModule(stripTypeScriptTypes(fs.readFileSync(file, 'utf8'), { mode: 'transform' }), { context, identifier: file });
    modules.set(file, module);
    await module.link((specifier, importer) => load(path.resolve(path.dirname(importer.identifier), specifier + '.ts')));
  }
  return modules.get(file);
}
const comparator = await load(path.join(root, 'platform-integrations/public-api/reader-order.ts'));
await comparator.evaluate();
const compare = comparator.namespace.compareReaderChapters;
const labels = values => values.map(chapter => ({ volume: '1', chapter })).sort(compare).map(row => row.chapter);
const file = path.join(root, 'frontend/src/lib/components/reader/context.svelte.ts');
const transformed = createFrontendOverlays(path.join(root, 'frontend')).transform.handler(fs.readFileSync(file, 'utf8'), file).code;
const start = transformed.indexOf('\tfindChapter(idx: number)'), end = transformed.indexOf('\n\tasync changeChapter(', start);
assert(start > 0 && end > start, 'Actual overlaid findChapter method must be found');
const findChapter = new Function('return ' + stripTypeScriptTypes('function ' + transformed.slice(start, end).trim(), { mode: 'transform' }))();
const choose = (entries, currentGroups = [], groups = []) => findChapter.call({
  chapter: { id: 'current', relationships: { groups: currentGroups } },
  aggregated: { chapters: [{ volume: '1', chapter: '1', entries }], languages: ['en'], groups }
}, 0)?.id;
const entry = groups => ({ language: 0, published_at: '2026-10-22T00:00:00Z', groups });

test('actual public comparator retains complete decimals, exact ties and null-last specials', () => {
  assert.deepEqual(labels(['2a', '1', null, 'special', '02', '2', '2.0', '']), ['2.0', '2', '02', '1', 'special', '2a', '', null]);
  assert.deepEqual(labels(['9007199254740992', '9007199254740993', '2.0000000000000000000000000001', '2.0000000000000000000000000002']),
    ['9007199254740993', '9007199254740992', '2.0000000000000000000000000002', '2.0000000000000000000000000001']);
});
test('actual non-multiline JavaScript anchors do not classify final line terminators as decimal', () => {
  for (const suffix of ['\n', '\r', '\u2028', '\u2029']) {
    assert.deepEqual(labels(['2' + suffix, '1']), ['1', '2' + suffix]);
  }
});
test('actual public raw labels use UTF-16 surrogate order, not UTF-8 scalar order', () => {
  assert.deepEqual(labels(['\u{10000}', '\ue000', '😀', '\uffff']), ['\uffff', '\ue000', '😀', '\u{10000}']);
});
test('actual Object.entries promotion matches native serde map serialization golden', () => {
  // Exact key sequence of a serde_json default BTreeMap serialization. The
  // backend HTTP fixture checks the raw aggregate wire separately.
  const entries = JSON.parse('{"02":{"language":0,"groups":[]},"10":{"language":0,"groups":[]},"2":{"language":0,"groups":[]},"4294967294":{"language":0,"groups":[]},"4294967295":{"language":0,"groups":[]}}');
  assert.deepEqual(Object.keys(entries), ['2', '10', '4294967294', '02', '4294967295']);
  assert.equal(choose(entries), '2');
});
test('actual public UUID edition records keep /chapter insertion order rather than alias sorting', () => {
  const late = 'ffffffff-ffff-8fff-8fff-ffffffffffff', early = '00000000-0000-8000-8000-000000000001';
  // Transport already sorted by publication timestamp, then native ID.
  assert.equal(choose(Object.fromEntries([[late, entry([])], [early, entry([])]])), late);
});
test('actual retained stable scoring resolves a full-set winner beyond ten thousand editions', () => {
  const groups = [{ id: 'preferred', name: 'Preferred' }, { id: 'other', name: 'Other' }];
  const entries = Object.fromEntries(Array.from({ length: 10001 }, (_, index) => ['edition' + String(index).padStart(5, '0'), entry([index === 10000 ? 0 : 1])]));
  assert.equal(choose(entries, [groups[0]], groups), 'edition10000');
});
test('actual overlay uses IDs before same-name groups and only uses name for missing legacy IDs', () => {
  const groups = [{ id: 'other', name: 'Same name' }, { id: 'preferred', name: 'Same name' }];
  const entries = { a: entry([0]), b: entry([1]) };
  assert.equal(choose(entries, [groups[1]], groups), 'b');
  assert.equal(choose(entries, [{ name: 'Same name' }], groups), 'a');
});
