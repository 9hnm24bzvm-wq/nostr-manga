// Actual pinned Svelte component/runes/lifecycle in JSDOM. The owner manager is
// a deliberate stand-in: not browser layout, signing, HTTP, SQL or source proof.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { createRequire } from 'node:module';
import { fileURLToPath, pathToFileURL } from 'node:url';
import test, { after } from 'node:test';
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const dependencies = process.env.WEEBDEX_FRONTEND_ROOT, domModules = process.env.WEEBDEX_BROWSER_DOM_NODE_MODULES;
assert(dependencies && domModules, 'Use the existing pinned Svelte and JSDOM roots');
const require = createRequire(path.join(dependencies, 'package.json'));
assert.equal(require('svelte/package.json').version, '5.55.1'); assert.equal(require('typescript/package.json').version, '5.9.3'); assert.equal(require(path.join(domModules, 'jsdom/package.json')).version, '27.0.1');
const { compile } = require('svelte/compiler'), ts = require('typescript'), { JSDOM } = require(path.join(domModules, 'jsdom'));
const { window } = new JSDOM('<main></main>', { pretendToBeVisual: true, url: 'http://127.0.0.1:1' });
for (const key of ['window', 'document', 'Node', 'Element', 'HTMLElement', 'HTMLButtonElement', 'HTMLInputElement', 'Text', 'Comment', 'Event', 'CustomEvent', 'MutationObserver', 'requestAnimationFrame', 'cancelAnimationFrame', 'getComputedStyle']) globalThis[key] = window[key];
const client = pathToFileURL(path.join(dependencies, 'node_modules/svelte/src/index-client.js')).href;
const { mount, unmount, flushSync } = await import(client);
fs.mkdirSync(path.join(root, '.build'), { recursive: true });
const generation = fs.mkdtempSync(path.join(root, '.build/scheduled-upload-dom-'));
// Register before symlink/compilation; remove only this test-created generation.
test.after(() => {
  try { window.close(); }
  finally { fs.rmSync(generation, { recursive: true, force: true }); assert.equal(fs.existsSync(generation), false); }
});

fs.symlinkSync(path.join(dependencies, 'node_modules'), path.join(generation, 'node_modules'));
const sourceFile = path.join(root, 'platform-integrations/public-api/UploadSchedulesPanel.svelte');
let source = fs.readFileSync(sourceFile, 'utf8');
source = source.replace(/<script lang="ts">([\s\S]*?)<\/script>/, (_match, script) => {
  const ast = ts.createSourceFile('UploadSchedulesPanel.ts', script, ts.ScriptTarget.Latest, true, ts.ScriptKind.TS);
  for (const item of [...ast.statements].reverse()) if (ts.isImportDeclaration(item)) {
    const specifier = item.moduleSpecifier.text; assert(specifier === 'svelte' || item.importClause.isTypeOnly);
    const replacement = specifier === 'svelte' ? item.getText(ast).replace(/(['"])svelte\1/, JSON.stringify(client)) : '';
    script = script.slice(0, item.getStart(ast)) + replacement + script.slice(item.end);
  }
  return '<script lang="ts">' + script + '</script>';
});
function generated(name, source) { const compiled = compile(source, { filename: name + '.svelte', generate: 'client', runes: true }); assert.deepEqual(compiled.warnings, []); const file = path.join(generation, name + '.mjs'); fs.writeFileSync(file, compiled.js.code); return pathToFileURL(file).href; }
const panel = generated('UploadSchedulesPanel', source);
const wrapper = generated('ScheduleHarness', `<script>import {untrack} from ${JSON.stringify(client)};import Panel from ${JSON.stringify(panel)};let {initialOwner}=$props();let owner=$state.raw(untrack(()=>initialOwner)),revision=$state(0);export function replaceOwner(next){owner=next;}export function changed(){revision++;}</script><Panel {owner} {revision}/>`);
const Component = (await import(wrapper)).default;

const SESSION = '11111111-1111-8111-8111-111111111111', CHAPTER = '22222222-2222-8222-8222-222222222222';
const limits = { maximumOutstanding: 32, maximumRetainedBytes: 500 * 1048576, maximumHorizonSeconds: 30 * 86400, retryWindowSeconds: 86400, timezone: 'UTC' };
const deferred = () => { let resolve; const promise = new Promise(yes => resolve = yes); return { promise, resolve }; };
function ownerFixture(options = {}) {
  const state = { calls: [], pending: undefined, disposed: false, entries: [{ sessionId: SESSION, receipt: { chapterId: CHAPTER, state: 'scheduled', publishAt: '2026-12-01T12:30:45', retryWindowSeconds: 2, sourceAccepted: false, readerAvailable: false, canCancel: true, canRetry: false } }], ...options.state };
  const result = () => ({ recent: structuredClone(state.recent ?? []), entries: structuredClone(state.entries).map(row => ({ ...row, uncertain: state.pending })), limits });
  const owner = { alive: () => !state.disposed, schedules: {
    pending: id => state.pendingById?.[id] ?? state.pending,
    async list() { state.calls.push('list'); if (options.list) return options.list(state, result); return result(); },
    async get(id) { assert(state.entries.some(row => row.sessionId === id) || state.recent?.some(row => row.sessionId === id)); state.calls.push('get'); if (options.get) return options.get(state, result); return result().entries[0]; },
    async cancel(id) { assert.equal(id, SESSION); state.calls.push('cancel'); if (options.cancel) return options.cancel(state, result); state.entries[0].receipt.state = 'cancelled'; state.entries[0].receipt.canCancel = false; return result().entries[0]; },
    async retry(id) { assert.equal(id, SESSION); state.calls.push('retry'); if (options.retry) return options.retry(state, result); return result().entries[0]; },
    async readableChapter(id) { assert.equal(id, SESSION); state.calls.push('read'); return options.read ? options.read(state) : { id: CHAPTER }; },
  } };
  return { state, owner };
}
async function settle() { for (let i = 0; i < 3; i++) { await new Promise(resolve => setTimeout(resolve, 0)); flushSync(); } }
async function render(options = {}) { const f = ownerFixture(options), target = document.createElement('div'); document.querySelector('main').append(target); const component = mount(Component, { target, props: { initialOwner: f.owner } }); await settle(); return { ...f, target, component, async close() { await unmount(component); target.remove(); } }; }
function button(f, text) { const rows = [...f.target.querySelectorAll('button')].filter(row => row.textContent.trim() === text); assert.equal(rows.length, 1, text); return rows[0]; }
async function click(f, text) { button(f, text).click(); await settle(); }
test('mounted pending schedule shows UTC, limits and captured deadline but no chapter link', async () => {
  const f = await render(); try { assert.match(f.target.textContent, /2026-12-01T12:30:45/); assert.match(f.target.textContent, /500 MiB/); assert.match(f.target.textContent, /Captured retry window: 2 seconds/); assert.match(f.target.textContent, /not published chapters/); assert.equal(f.target.querySelector('a'), null); assert.deepEqual(f.state.calls, ['list']); } finally { await f.close(); }
});
test('mounted cancellation locks duplicate clicks and keeps an unknown response fenced after unchanged status', async () => {
  const gate = deferred(), f = await render({ cancel: async state => { await gate.promise; state.pending = 'cancel'; throw Error('Cancellation response unconfirmed'); } });
  try { const cancel = button(f, 'Cancel scheduled upload'); cancel.click(); cancel.click(); await settle(); assert.equal(f.state.calls.filter(value => value === 'cancel').length, 1); assert.equal(cancel.disabled, true); gate.resolve(); await settle(); assert.match(f.target.textContent, /response is unconfirmed/); assert.equal(button(f, 'Cancel scheduled upload').disabled, true); await click(f, 'Check saved status'); assert.equal(button(f, 'Cancel scheduled upload').disabled, true); assert.equal(f.state.calls.filter(value => value === 'cancel').length, 1); } finally { await f.close(); }
});
test('mounted complete/source-accepted but unavailable receipt never exposes reader action', async () => {
  const f = await render(); try { Object.assign(f.state.entries[0].receipt, { state: 'complete', canCancel: false, sourceAccepted: true }); await click(f, 'Refresh private schedules'); assert.match(f.target.textContent, /Original source acceptance verified/); assert.equal(f.target.querySelector('a'), null); assert.equal([...f.target.querySelectorAll('button')].some(row => row.textContent.includes('Verify readable')), false); } finally { await f.close(); }
});
test('mounted reader link appears only after explicit resolved reader verification', async () => {
  const gate = deferred(), f = await render({ read: () => gate.promise }); try { Object.assign(f.state.entries[0].receipt, { state: 'complete', canCancel: false, readerAvailable: true, sourceAccepted: true }); await click(f, 'Refresh private schedules'); await click(f, 'Verify readable chapter'); assert.equal(f.target.querySelector('a'), null); gate.resolve({ id: CHAPTER }); await settle(); assert.equal(f.target.querySelector('a').getAttribute('href'), '/chapter/' + CHAPTER); } finally { await f.close(); }
});
test('mounted owner replacement discards delayed private list and reader results', async () => {
  const gate = deferred(), f = await render({ list: async (_state, result) => { await gate.promise; return result(); } });
  try { const next = ownerFixture({ state: { entries: [] } }); f.component.replaceOwner(next.owner); await settle(); gate.resolve(); await settle(); assert.doesNotMatch(f.target.textContent, new RegExp(SESSION)); assert.match(f.target.textContent, /No pending/); assert.deepEqual(next.state.calls, ['list']); } finally { await f.close(); }
});
test('mounted disposal cannot adopt a late readable chapter', async () => {
  const gate = deferred(), f = await render({ read: () => gate.promise }); Object.assign(f.state.entries[0].receipt, { state: 'complete', canCancel: false, readerAvailable: true, sourceAccepted: true }); await click(f, 'Refresh private schedules'); await click(f, 'Verify readable chapter'); await f.close(); gate.resolve({ id: CHAPTER }); await settle(); assert.equal(document.querySelector('a'), null);
});
test('mounted recent unavailable status is lazy, stays explicit and never hides active schedules', async () => {
  const recentId = '33333333-3333-8333-8333-333333333333';
  const f = await render({ state: { pendingById: { [recentId]: 'cancel' }, recent: [{ sessionId: recentId, chapterId: CHAPTER, uncertain: 'cancel' }] }, get: () => { throw Object.assign(Error('Private schedule status unavailable (HTTP 404)'), { status: 404 }); } });
  try { assert.deepEqual(f.state.calls, ['list']); await click(f, 'Check saved status for ' + recentId); assert.match(f.target.textContent, /HTTP 404/); assert.match(f.target.textContent, /Unconfirmed cancel/); assert.match(f.target.textContent, /Schedule: scheduled/); assert.equal(f.target.querySelector('a'), null); assert.deepEqual(f.state.calls, ['list', 'get']); } finally { await f.close(); }
});
test('mounted rebind conflict removes stale private rows and links without sending a mutation', async () => {
  const f = await render({ get: () => { throw Object.assign(Error('Captured account changed (HTTP 409)'), { status: 409 }); } });
  try { await click(f, 'Check saved status'); assert.doesNotMatch(f.target.textContent, new RegExp(SESSION)); assert.match(f.target.textContent, /Captured account changed/); assert.deepEqual(f.state.calls, ['list', 'get']); assert.equal(f.target.querySelector('a'), null); } finally { await f.close(); }
});
