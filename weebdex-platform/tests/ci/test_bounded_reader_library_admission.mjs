// Actual current library and native authority modules. Svelte store behavior,
// storage completion, and module-loading waits are finite controlled seams.
import fs from 'node:fs';
import path from 'node:path';
import vm from 'node:vm';
import { stripTypeScriptTypes } from 'node:module';
import { fileURLToPath } from 'node:url';
import test from 'node:test';
import assert from 'node:assert/strict';
import { deferred, turns } from '../fixtures/bounded-reader/controller-fixture.mjs';
const root = fileURLToPath(new URL('../../', import.meta.url)), fallback = process.env.READER_FROZEN_SOURCE_ROOT ?? root;
function writable(value) { const listeners = new Set(); return { subscribe(fn) { listeners.add(fn); fn(value); return () => listeners.delete(fn); }, set(next) { value = next; for (const fn of [...listeners]) fn(value); }, update(fn) { this.set(fn(value)); } }; }
const get = store => { let value; const stop = store.subscribe(next => { value = next; }); stop(); return value; };
function derived(stores, derive) { const list = Array.isArray(stores) ? stores : [stores], result = writable(undefined); const update = () => result.set(derive(Array.isArray(stores) ? list.map(get) : get(stores))); for (const store of list) store.subscribe(update); return result; }
async function fixture({ initialMode = 'device', heldOpen = false } = {}) {
  const open = deferred(), dynamic = [], writes = [], selection = [], Session = Object.assign(writable({ identity: { id: 'actor-one' }, roles: [], permissions: [] }), { isValid: async () => true });
  const device = { db: {}, snapshot: async () => ({ version: 1, revision: 1, records: [] }) };
  const indexed = { request: query => Promise.resolve(query), transaction: async (_db, _stores, _mode, work) => { work({ objectStore: () => ({ put(value, key) { writes.push({ value, key }); } }) }); } };
  const no = () => {}; class HTTPError extends Error { constructor(status, message) { super(message); this.status = status; this.error = message; } }
  const stubs = new Map([
    ['svelte/store', { writable, derived, get }], ['$lib/stores/Session', { default: Session }], ['$env/static/public', { PUBLIC_API_URL: '/api' }], ['$lib/HTTP', { HTTPError }],
    ['platform-integrations/public-api/library-staging.ts', { cataloguePreviewLibraryUnavailable: false, CATALOGUE_PREVIEW_LIBRARY_FENCE: '', assertCataloguePreviewLibraryReady: no, publicUuidLibraryEnabled: false, publicAccountLibraryUnavailable: false, assertAccountLibraryReady: no }],
    ['platform-integrations/local-library/device-store.ts', { DeviceStore: { open: async () => { if (heldOpen) await open.promise; return device; } }, PUBLIC_DEVICE_FORMAT_FENCE: '', catalogueId: value => value, normalizeDevice: value => value, validateSnapshot: no, sameScope: () => true }],
    ['platform-integrations/local-library/protocol/indexeddb.js', indexed],
    ['platform-integrations/public-api/validation.ts', { uuidPattern: /^[0-9a-f-]+$/ }],
    ['platform-integrations/local-library/protocol/store.js', { LocalLibrary: { open: async () => ({ db: {}, getMeta: async () => initialMode }) } }]
  ]);
  const context = vm.createContext({ console, URL, AbortSignal, window: { location: { href: 'https://reader.test/' }, addEventListener: no }, fetch: async () => { throw Error('Synthetic scope discovery unavailable'); } });
  const modules = new Map(), links = new Map(); let imports = 0;
  function resolve(specifier, importer) { return specifier === '$platform/mutation-authority' ? 'platform-integrations/frontend-behavior/mutation-authority.ts' : specifier.startsWith('.') ? path.posix.normalize(path.posix.join(path.posix.dirname(importer.identifier), specifier + (path.extname(specifier) ? '' : '.ts'))) : specifier; }
  async function load(relative) {
    if (modules.has(relative)) { await links.get(relative); return modules.get(relative); }
    if (stubs.has(relative)) { const values = stubs.get(relative), mod = new vm.SyntheticModule(Object.keys(values), function () { for (const [key, value] of Object.entries(values)) this.setExport(key, value); }, { context }); modules.set(relative, mod); return mod; }
    const local = path.join(root, relative), file = fs.existsSync(local) ? local : path.join(fallback, relative);
    const mod = new vm.SourceTextModule(stripTypeScriptTypes(fs.readFileSync(file, 'utf8'), { mode: 'transform' }), { context, identifier: relative,
      importModuleDynamically: async (specifier, importer) => { const child = await load(resolve(specifier, importer)); await child.evaluate(); if (specifier === './protocol/indexeddb.js') { imports++; const gate = dynamic.shift(); if (gate) await gate.promise; } return child; } }); modules.set(relative, mod);
    const linked = Promise.resolve().then(() => mod.link((specifier, importer) => load(resolve(specifier, importer)))); links.set(relative, linked); await linked; return mod;
  }
  const native = await load('platform-integrations/frontend-behavior/mutation-authority.ts'); await native.evaluate(); native.namespace.bindNativeMutationAuthority(Session);
  const library = await load('platform-integrations/local-library/library.ts'); await library.evaluate(); library.namespace.readerLibrarySelection.subscribe(value => selection.push({ ...value }));
  return { library: library.namespace, native: native.namespace, Session, writes, selection, open, holdImport() { const gate = deferred(); dynamic.push(gate); return gate; }, get imports() { return imports; } };
}
test('real useMode publishes pending and invalidates old device admission before opening storage', async () => {
  const f = await fixture({ heldOpen: true }), old = f.library.captureLibraryAdmission(), work = f.library.useMode('account');
  assert.equal(f.library.readerLibrarySource().mode, 'device'); assert.equal(f.library.readerLibrarySource().selectionPending, true); assert.throws(() => f.library.assertLibraryAdmission(old)); assert.equal(f.writes.length, 0);
  f.open.resolve(); await work; assert.equal(f.library.readerLibrarySource().mode, 'account'); assert.equal(f.library.readerLibrarySource().selectionPending, false);
});
test('superseded dynamic-import transition never opens its write transaction or clears newer pending', async () => {
  const f = await fixture({ initialMode: 'account' }); await f.library.openLibrary(); await turns();
  const firstGate = f.holdImport(), first = f.library.useMode('device'), rejected = assert.rejects(first); await turns(); assert.equal(f.imports, 1);
  const secondGate = f.holdImport(), second = f.library.useMode('account'); await turns(); assert.equal(f.imports, 2);
  firstGate.resolve(); await rejected; assert.equal(f.writes.length, 0); assert.equal(f.library.readerLibrarySource().selectionPending, true);
  secondGate.resolve(); await second; assert.deepEqual(f.writes.map(row => row.value), ['account']); assert.equal(f.library.readerLibrarySource().selectionPending, false);
});
test('real account-device-account selections invalidate original admission even after mode returns', async () => {
  const f = await fixture({ initialMode: 'account' }); await f.library.openLibrary(); await turns(); const old = f.library.captureLibraryAdmission();
  await f.library.useMode('device'); await f.library.useMode('account'); assert.equal(f.library.readerLibrarySource().mode, 'account'); assert.throws(() => f.library.assertLibraryAdmission(old)); assert.deepEqual(f.writes.map(row => row.value), ['device', 'account']);
});
test('actual native authority ignores Session data refresh but rejects authority epoch changes', async () => {
  const f = await fixture(); await f.library.openLibrary(); await turns(); const old = f.library.captureLibraryAdmission(), epoch = old.authority.epoch;
  f.Session.update(value => ({ ...value, follows: { 'manga-one': ['reading', 1] } })); assert.equal(f.native.captureNativeAuthority().epoch, epoch); assert.doesNotThrow(() => f.library.assertLibraryAdmission(old));
  f.Session.update(value => ({ ...value, permissions: ['new-permission'] })); assert.throws(() => f.library.assertLibraryAdmission(old));
});
