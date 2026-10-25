// Finite mounted component test. Existing locked Svelte/JSDOM only; no browser,
// socket, app server, production build, native API or real user data.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { createRequire } from 'node:module';
import { fileURLToPath, pathToFileURL } from 'node:url';
import test, { after } from 'node:test';
import { createLocalLibraryOverlays } from '../../platform-integrations/local-library/overlays.mjs';
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const dependencies = process.env.WEEBDEX_FRONTEND_ROOT, domRoot = process.env.WEEBDEX_BROWSER_DOM_NODE_MODULES;
assert(dependencies && domRoot, 'Use existing exact-lock dependency roots');
const require = createRequire(path.join(dependencies, 'package.json')), { compile } = require('svelte/compiler'), ts = require('typescript');
assert.equal(require('svelte/package.json').version, '5.55.1'); assert.equal(require(path.join(domRoot, 'jsdom/package.json')).version, '27.0.1');
const { JSDOM } = require(path.join(domRoot, 'jsdom'));
const { window } = new JSDOM('<main></main>', { url: 'https://history.fixture/history', pretendToBeVisual: true });
for (const key of ['window', 'document', 'Node', 'Element', 'HTMLElement', 'HTMLInputElement', 'HTMLTextAreaElement', 'HTMLSelectElement', 'HTMLMediaElement', 'HTMLFormElement', 'Text', 'Comment', 'Event', 'CustomEvent', 'MutationObserver', 'requestAnimationFrame', 'cancelAnimationFrame', 'getComputedStyle']) globalThis[key] = window[key];
const client = pathToFileURL(path.join(dependencies, 'node_modules/svelte/src/index-client.js')).href;
const { mount, unmount, flushSync } = await import(client), { writable } = await import(pathToFileURL(path.join(dependencies, 'node_modules/svelte/src/store/index-client.js')));
const temp = fs.mkdtempSync(path.join(os.tmpdir(), 'device-history-dom-')); fs.symlinkSync(path.join(dependencies, 'node_modules'), path.join(temp, 'node_modules'));
after(() => { window.close(); fs.rmSync(temp, { recursive: true, force: true }); delete globalThis.__historyFixture; });
async function compiled(name, source, replace = false) {
  if (replace) source = source.replace(/<script lang="ts">([\s\S]*?)<\/script>/, (_, script) => {
    const ast = ts.createSourceFile(name + '.ts', script, ts.ScriptTarget.Latest, true, ts.ScriptKind.TS);
    for (const item of [...ast.statements].reverse()) if (ts.isImportDeclaration(item)) {
      const clause = item.importClause; let replacement = '';
      if (item.moduleSpecifier.text === 'svelte') replacement = item.getText(ast).replace(/(['"])svelte\1/, JSON.stringify(client));
      else if (clause && !clause.isTypeOnly) {
        const names = []; if (clause.name) names.push(clause.name.text);
        if (clause.namedBindings && ts.isNamespaceImport(clause.namedBindings)) names.push(clause.namedBindings.name.text);
        else if (clause.namedBindings) for (const entry of clause.namedBindings.elements) if (!entry.isTypeOnly) names.push(entry.name.text);
        if (names.length) replacement = `const { ${names.join(', ')} } = globalThis.__historyFixture;`;
      }
      script = script.slice(0, item.getStart(ast)) + replacement + script.slice(item.end);
    }
    return '<script lang="ts">' + script + '</script>';
  });
  const result = compile(source, { filename: name + '.svelte', generate: 'client', runes: true });
  fs.writeFileSync(path.join(temp, name + '.mjs'), result.js.code); return (await import(pathToFileURL(path.join(temp, name + '.mjs')))).default;
}
const Generic = await compiled('Generic', '<script>let {children}=$props();</script><div>{@render children?.()}</div>');
const Button = await compiled('Button', '<script>let {children,variant,...rest}=$props();</script><button {...rest}>{@render children?.()}</button>');
const Entry = await compiled('Entry', '<script>let {manga}=$props();</script><article>{manga.title}{#each manga.relationships._chapters as chapter}<span>{chapter.id}</span>{/each}</article>');
const Feed = await compiled('DeviceHistoryFeed', fs.readFileSync(path.join(root, 'platform-integrations/local-library/DeviceHistoryFeed.svelte'), 'utf8'), true);
const deferred = () => { let resolve, reject; const promise = new Promise((yes, no) => { resolve = yes; reject = no; }); return { promise, resolve, reject }; };
const data = id => ({ id, read_at: '2026-01-01', relationships: { manga: { id: 'parent-' + id, title: 'Title-' + id, relationships: {} } } });
const result = (overrides = {}) => ({ page: 1, limit: 100, data: [], total: null, scanned: 1000, candidates: 10001, ready: false, complete: false, continuation: true, ...overrides });
const settle = async () => { for (let i = 0; i < 5; i++) { await Promise.resolve(); flushSync(); } };
async function fixture(load, { opening } = {}) {
  const prefs = Object.assign(writable({ version: 0 }), { applyMangaPreferences(q) { q.set('tagx', 'tag-fixture'); }, applyChapterPreferences(q) { q.set('groupx', 'group-fixture'); } });
  const views = writable('device:0:verified'), identity = writable({ accountId: null }); let calls = [], opened = [], disposed = 0, cancelled = 0, history; const displayReleases = [];
  globalThis.__historyFixture = { writable, prefs, libraryViewKey: views, libraryIdentity: identity, page: { url: new URL('https://history.fixture/history') }, Grid: Generic, Button, Spinner: Generic, Empty: { Root: Generic, Header: Generic, Title: Generic, Description: Generic }, ChapterFeedMangaEntry: Entry, setReadHistory(store) { history = store; }, message: error => error.message, HistoryCancelled: class extends Error { constructor() { super('cancelled'); } }, HistoryTransportUnsettled: class extends Error {}, HistoryCapacityError: class extends Error {}, async openDeviceHistoryFeed(query, assertOwner) {
    opened.push({ query, assertOwner }); if (opening) await opening.promise;
    return { async load(page) { calls.push(page); return load(page, opened.length); }, releaseDisplayedPage() { displayReleases.push(document.body.textContent); }, dispose() { disposed++; }, cancel() { cancelled++; } };
  } };
  const component = mount(Feed, { target: document.querySelector('main') }); await settle();
  return { prefs, views, identity, calls, opened, displayReleases, history: () => history, disposed: () => disposed, cancelled: () => cancelled, async close() { await unmount(component); await settle(); }, async click(text) { const button = [...document.querySelectorAll('button')].find(button => button.textContent === text); assert(button, `Missing button ${text}: ${document.body.textContent}`); button.click(); await settle(); } };
}
test('unknown empty sparse page shows Continue scanning, not false empty; exact final page becomes rows', async () => {
  let count = 0; const f = await fixture(async () => ++count === 1 ? result() : result({ data: [data('last')], total: 1, complete: true, ready: true, continuation: false, scanned: 10001 }));
  try { assert.match(document.body.textContent, /No matching chapters found yet/); assert.doesNotMatch(document.body.textContent, /No results found/); assert.match(document.body.textContent, /Filtered total is not known/); await f.click('Continue scanning'); assert.match(document.body.textContent, /Title-last/); assert.deepEqual(f.calls, [1, 1]); assert.match(document.body.textContent, /1 matching chapters/); assert.doesNotMatch(document.body.textContent, /Continue loading/); } finally { await f.close(); }
});
test('next-page failure has an explicit retry without retaining previous page under the new label', async () => {
  let count = 0; const f = await fixture(async n => { count++; if (count === 2) throw Error('503 batch not skipped'); return result({ page: n, data: [data(String(n))], ready: true }); });
  try { await f.click('Continue loading'); assert.match(document.body.textContent, /503 batch not skipped/); assert.doesNotMatch(document.body.textContent, /Title-1/); await f.click('Retry history'); assert.match(document.body.textContent, /Title-2/); assert.deepEqual(f.calls, [1, 2, 2]); } finally { await f.close(); }
});
for (const changed of ['preferences', 'scope/revision', 'account namespace']) test(`${changed} removes stale rows, disposes old paging and rejects old pending success`, async () => {
  const pending = deferred(); const f = await fixture(async (page, open) => open === 1 ? pending.promise : result({ data: [data('new')], ready: true }));
  try { if (changed === 'preferences') f.prefs.set({ version: 1 }); else if (changed === 'scope/revision') f.views.set('device:1:verified'); else f.identity.set({ accountId: 'new-account' }); await settle(); assert.equal(f.disposed(), 1); pending.resolve(result({ data: [data('old')], ready: true })); await settle(); assert.match(document.body.textContent, /Title-new/); assert.doesNotMatch(document.body.textContent, /Title-old/); assert.throws(f.opened[0].assertOwner, /cancelled/); } finally { await f.close(); }
});
test('cancel during snapshot opening makes late open unusable and allows retry', async () => {
  const opening = deferred(), f = await fixture(async () => result({ ready: true }), { opening });
  try { await f.click('Cancel loading'); assert.match(document.body.textContent, /cancelled/); opening.resolve(); await settle(); assert.equal(f.disposed(), 1); assert.deepEqual(f.calls, []); await f.click('Retry history'); assert.deepEqual(f.calls, [1]); } finally { await f.close(); }
});
test('unmount cancels owned paging; late success cannot recreate DOM', async () => {
  const pending = deferred(), f = await fixture(() => pending.promise); await f.close(); assert.equal(f.disposed(), 1); pending.resolve(result({ data: [data('late')], ready: true })); await settle(); assert.doesNotMatch(document.body.textContent, /Title-late/);
});
test('empty out-of-range page does not claim the entire matching history is empty', async () => {
  const f = await fixture(async () => result({ page: 1, data: [], total: 100, complete: true, ready: true })); try { assert.match(document.body.textContent, /past the end/); assert.doesNotMatch(document.body.textContent, /No read chapters match/); } finally { await f.close(); }
});
test('exact composed history route confines new component to device mode and retains original account callback', () => {
  const frontend = path.join(root, 'frontend'), file = path.join(frontend, 'src/routes/(app)/history/+page.svelte'), original = fs.readFileSync(file, 'utf8');
  const composed = createLocalLibraryOverlays(frontend).transform.handler(original, file).code;
  assert.match(composed, /\{#if \$localLibraryState.mode === "device"\}[\s\S]*<DeviceHistoryFeed \/>[\s\S]*\{:else\}[\s\S]*<ChapterFeed/); assert.equal(composed.match(/API\.ChapterHistory\.Feed\(q\)/g).length, 1);
  for (const source of ['prefs.applyMangaPreferences(q);', 'prefs.applyChapterPreferences(q);', 'q.set("limit", "100");']) assert.ok(composed.includes(source));
  assert.equal(fs.readFileSync(file, 'utf8'), original);
});
test('history markup change is restricted to the two declared exact hash-bound device continuation exceptions', async () => {
  const { localLibraryApprovedHistoryContinuationExceptions } = await import('../../platform-integrations/local-library/overlays.mjs');
  assert.equal(localLibraryApprovedHistoryContinuationExceptions.length, 2);
  const frontend = path.join(root, 'frontend'), relative = 'src/routes/(app)/history/+page.svelte', file = path.join(frontend, relative), original = fs.readFileSync(file, 'utf8');
  let normalized = createLocalLibraryOverlays(frontend).transform.handler(original, file).code;
  for (const entry of [...localLibraryApprovedHistoryContinuationExceptions].reverse()) { assert.equal(entry.path, relative); assert.equal(entry.count, 1); assert.equal(entry.approved_history_continuation_exception, 'device_history_owned_continuation'); assert.equal(normalized.split(entry.after).length - 1, 1); normalized = normalized.replace(entry.after, entry.before); }
  const markup = text => text.replace(/<script\b[^>]*>[\s\S]*?<\/script>/g, '<script></script>'); assert.equal(markup(normalized), markup(original));
});

test('page-size fallback visibly restarts page 1 and continues with the new page contract', async () => {
  let count = 0;
  const f = await fixture(async n => ++count === 1 ? result({ page: 1, ready: true, data: [data('original')] }) : count === 2 ? result({ page: 1, limit: 50, scanned: 0, notice: 'Page size reduced to 50 to stay within the retained-data limit. Restarted at page 1; no chapter was skipped.' }) : result({ page: n, limit: 50, ready: true, data: [data('restarted')] }));
  try {
    await f.click('Continue loading'); assert.match(document.body.textContent, /Page size reduced to 50/); assert.match(document.body.textContent, /Restarted at page 1/); assert.match(document.body.textContent, /Page 1\./); assert.doesNotMatch(document.body.textContent, /Title-original|Page 2\./);
    assert.equal(f.displayReleases.length, 1); assert.doesNotMatch(f.displayReleases[0], /Title-original/);
    await f.click('Continue scanning'); assert.deepEqual(f.calls, [1, 2, 1]); assert.match(document.body.textContent, /Title-restarted/);
  } finally { await f.close(); }
});
for (const kind of ['HistoryTransportUnsettled', 'HistoryCapacityError']) test(kind + ' is a visible stopped state with explicit reopen, not ineffective retry or auto-continuation', async () => {
  const f = await fixture(async () => { throw new globalThis.__historyFixture[kind]('Loading stopped without skipping any chapter.'); });
  try { assert.match(document.body.textContent, /Loading stopped without skipping/); assert.match(document.body.textContent, /Reopen history/); assert.doesNotMatch(document.body.textContent, /Retry history|Continue scanning|Continue loading|No results found/); assert.deepEqual(f.calls, [1]); await settle(); assert.deepEqual(f.calls, [1]); await f.click('Reopen history'); assert.deepEqual(f.calls, [1, 1]); assert.equal(f.disposed(), 1); } finally { await f.close(); }
});
test('same-page continuation keeps displayed DTO pin while new work is pending', async () => {
  const pending = deferred(); let calls = 0;
  const f = await fixture(async () => ++calls === 1 ? result({ data: [data('partial')], ready: false }) : pending.promise);
  try { await f.click('Continue scanning'); assert.match(document.body.textContent, /Title-partial/); assert.equal(f.displayReleases.length, 0); pending.resolve(result({ data: [data('finished')], ready: true })); await settle(); assert.match(document.body.textContent, /Title-finished/); } finally { await f.close(); }
});
