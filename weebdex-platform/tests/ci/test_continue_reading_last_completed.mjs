#!/usr/bin/env node
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';
import { createFrontendOverlays, frontendOverlayManifest } from '../../platform-integrations/frontend-overlays.mjs';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const frontend = path.join(root, 'frontend');
const target = path.join(frontend, 'src/routes/(app)/title/MangaActions.svelte');
const original = fs.readFileSync(target, 'utf8');
const transformed = createFrontendOverlays(frontend).transform.handler(original, target).code;
// Exercise the actual source-bound production selection block, rather than a
// separately implemented algorithm. API aggregate order is newest to oldest.
const start = transformed.indexOf('\t\t\t\t\tif (history) {');
const end = transformed.indexOf('\t\t\t\t\tif (targetAggregatedChapter) {', start);
assert(start > 0 && end > start, 'Production Continue Reading selection block missing');
const select = new Function('aggregated', 'history', 'console',
  'let targetAggregatedChapter;\n' + transformed.slice(start, end) + '\nreturn targetAggregatedChapter;');
const silent = { info() {} };
const chapter = (volume, number, ...ids) => ({ volume, chapter: number, entries: Object.fromEntries(ids.map(id => [id, {}])) });
const newest = chapter('2', '1', 'v2c1');
const middle = chapter('1', '10', 'v1c10');
const oldest = chapter('1', '2', 'v1c2en', 'v1c2ja');
const aggregated = { chapters: [newest, middle, oldest] };
const choose = history => select(aggregated, history, silent);

test('all-read device and account history continue at latest completed chapter', () => {
  for (const mode of ['device', 'account']) {
    const history = { v2c1: null, v1c10: null, v1c2en: null };
    assert.equal(choose(history), newest, mode + ' all-read must not restart at oldest chapter');
  }
});

test('no history still starts at oldest chapter', () => {
  assert.equal(choose(undefined), oldest);
  assert.equal(choose({}), oldest);
});

test('partially read history still chooses oldest unread chapter', () => {
  assert.equal(choose({ v1c2en: null }), middle);
  assert.equal(choose({ v1c2en: null, v1c10: null }), newest);
  assert.equal(choose({ v2c1: null }), oldest);
});

test('alternate edition completion and native volume/chapter ordering are preserved', () => {
  assert.equal(choose({ v1c2ja: null }), middle);
  assert.equal(choose({ v1c2ja: null, v1c10: null, v2c1: null }), newest);
  assert.deepEqual(aggregated.chapters, [newest, middle, oldest]);
  assert.equal(select({ chapters: [oldest] }, { v1c2en: null }, silent), oldest);
});

test('only the all-read fallback changes and existing authority fences remain', () => {
  const manifest = frontendOverlayManifest.filter(row => row.label === 'all-read Continue Reading stays at newest completed aggregate chapter');
  assert.equal(manifest.length, 1);
  assert.equal(manifest[0].count, 1);
  assert.equal(manifest[0].path, 'src/routes/(app)/title/MangaActions.svelte');
  assert.match(transformed, /targetAggregatedChapter = filtered\[filtered\.length - 1\]/);
  assert.match(transformed, /else targetAggregatedChapter = aggregated\.chapters\[aggregated\.chapters\.length - 1\]/);
  assert.match(transformed, /readDisposed/);
  assert.match(transformed, /generation !== readGeneration/);
  assert.match(transformed, /context !== readContext\(\)/);
  assert.equal(fs.readFileSync(target, 'utf8'), original, 'Preserved frontend source must remain unchanged');
});

// Execute the complete composed click handler with finite API/navigation boundaries.
// This covers empty/repeated reads and actual selected-language query precedence.
const handlerStart = transformed.indexOf('\tconst startRead = () =>');
const handlerEnd = transformed.indexOf('</script>', handlerStart);
const contextDeclaration = transformed.match(/\tconst readContext = \(\) => [^\r\n]+;/)[0];
const createHandler = new Function('environment', 'with (environment) {' + contextDeclaration + transformed.slice(handlerStart, handlerEnd) + '; return startRead; }');
function clickFixture({ chapters = aggregated.chapters, history, url = 'https://fixture.example.invalid/title/title?tab=chapters', gated = false } = {}) {
  const calls = { aggregate: [], navigation: [], errors: [] };
  let release; const gate = new Promise(resolve => { release = resolve; });
  class UserError extends Error {}
  const environment = {
    mutex: { exec: operation => operation() }, isPublished: true, isEmpty: false, isStartReading: false,
    aggregated: { chapters: [chapter('99','99','stale')] }, targetAggregatedChapter: chapter('99','99','stale'), isChapterSelectionDialogOpen: true,
    readGeneration: 0, readDisposed: false, manga: { id: 'title' }, $readHistory: history === undefined ? undefined : { title: history },
    page: { url: new URL(url) }, window: { location: new URL(url) },
    $prefs: {}, $libraryIdentity: { mode: 'device', accountId: null }, $localLibraryState: { scopeStatus: 'verified' },
    prefs: { applyChapterPreferences(query) { query.append('tlang','fr'); query.append('groupx','hidden-group'); } },
    API: { Chapter: { async Aggregate(id, query) { calls.aggregate.push({ id, query: new URLSearchParams(query) }); if (gated) await gate; return { status: 200, body: { chapters } }; } } },
    StatusCodes: { OK: 200, TOO_MANY_REQUESTS: 429 }, Constants: { Error: { Error: UserError, TooManyRequests: new UserError('rate limited'), Unknown: new UserError('unknown') } },
    console: { info() {}, error() {} }, toast: { error: (...args) => calls.errors.push(args) },
    goto: async target => { calls.navigation.push(target); },
  };
  return { environment, calls, run: createHandler(environment), release };
}
test('an empty current aggregate clears earlier targets and never opens a stale chapter', async () => {
  const f = clickFixture({ chapters: [], history: { deleted: null } }); await f.run();
  assert.equal(f.environment.targetAggregatedChapter, undefined); assert.equal(f.environment.isChapterSelectionDialogOpen, false);
  assert.equal(f.environment.isStartReading, false); assert.deepEqual(f.calls.navigation, []); assert.deepEqual(f.calls.errors, []);
});
test('single completed visible chapter is the only Continue target', async () => {
  const only = chapter(null, null, 'only'), f = clickFixture({ chapters: [only], history: { only: null, deleted: null } }); await f.run();
  assert.equal(f.environment.targetAggregatedChapter, only); assert.deepEqual(f.calls.navigation, ['/chapter/only/1']);
});
test('deleted last-read IDs never become routes or count as a current visible completion', async () => {
  const f = clickFixture({ history: { deleted: null } }); await f.run();
  assert.equal(f.environment.targetAggregatedChapter, oldest); assert.deepEqual(f.calls.navigation, []); assert.equal(f.environment.isChapterSelectionDialogOpen, true);
  const complete = clickFixture({ history: { deleted: null, v2c1: null, v1c10: null, v1c2en: null } }); await complete.run();
  assert.equal(complete.environment.targetAggregatedChapter, newest); assert.deepEqual(complete.calls.navigation, ['/chapter/v2c1/1']);
});
test('visible aggregate order is preserved across volume and decimal chapter labels', async () => {
  const rows = [chapter('2','1','newest'), chapter('1','10','ten'), chapter('1','2.5','decimal'), chapter('1','2','oldest')];
  const f = clickFixture({ chapters: rows, history: { newest: null, ten: null, decimal: null, oldest: null } }); await f.run();
  assert.equal(f.environment.targetAggregatedChapter, rows[0]); assert.deepEqual(f.calls.navigation, ['/chapter/newest/1']); assert.deepEqual(rows.map(row=>row.chapter), ['1','10','2.5','2']);
});
test('an explicit chapter language/group selection overrides device language preferences exactly', async () => {
  const f = clickFixture({ chapters: [chapter('1','1','english')], url: 'https://fixture.example.invalid/title/title?tlang=en&group=actual-group', history: { french: null } }); await f.run();
  assert.deepEqual(f.calls.aggregate[0].query.getAll('tlang'), ['en']); assert.deepEqual(f.calls.aggregate[0].query.getAll('group'), ['actual-group']);
  assert.deepEqual(f.calls.aggregate[0].query.getAll('groupx'), ['hidden-group']); assert.deepEqual(f.calls.navigation, ['/chapter/english/1']);
});
test('a late aggregate cannot navigate after a language or title selection changes', async () => {
  for (const kind of ['language','title','library','destroy']) {
    const f = clickFixture({ chapters: [chapter('1','1','late')], gated: true }), pending = f.run();
    if (kind === 'language') f.environment.page.url.searchParams.set('tlang','ja');
    else if (kind === 'title') f.environment.manga.id = 'new-title';
    else if (kind === 'library') f.environment.$libraryIdentity.mode = 'account'; else f.environment.readDisposed = true;
    f.release(); await pending; assert.equal(f.environment.aggregated, undefined); assert.equal(f.environment.targetAggregatedChapter, undefined); assert.deepEqual(f.calls.navigation, []);
  }
});
