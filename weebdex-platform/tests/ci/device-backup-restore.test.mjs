// Actual library -> DeviceStore -> v2 restore/validation/codec, with a finite
// atomic transaction model and store/capability adapters. No browser, native
// IndexedDB persistence, signing, account request, build or install is exercised.
import fs from 'node:fs';
import path from 'node:path';
import vm from 'node:vm';
import { createHash } from 'node:crypto';
import { stripTypeScriptTypes } from 'node:module';
import { fileURLToPath } from 'node:url';
import test from 'node:test';
import assert from 'node:assert/strict';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const base = path.join(root, 'platform-integrations');
const baselineRoot = process.env.WEEBDEX_DEVICE_RESTORE_BASELINE_ROOT;
assert.ok(baselineRoot && path.isAbsolute(baselineRoot), 'Provide the exact20f2 platform root for the source-bound counterfactual');
const owner = '00000000-0000-8000-8000-000000000000';
const foreignOwner = '44444444-4444-8444-8444-444444444444';
const manga = '11111111-1111-8111-8111-111111111111';
const chapter = '22222222-2222-8222-8222-222222222222';
const other = '33333333-3333-8333-8333-333333333333';
const list = 'a'.repeat(32);
const scope = { origin: 'https://library.example', serviceKey: null, catalogNamespace: '55555555-5555-8555-8555-555555555555' };
const clone = structuredClone;
const series = (id = manga, updated_at = 100, status = 'reading') => ({ family: 'series', key: id, id, status, notification: true, followedAt: '2026-10-01T00:00:00Z', updated_at });
const progress = () => ({ family: 'chapters', key: chapter, id: chapter, mangaId: manga, completed: false, page: 5, readAt: '2026-10-01T00:01:00Z', updated_at: 101 });
const listRow = { family: 'lists', key: list, id: list, name: 'Saved list', updated_at: 99 };
const member = { family: 'members', key: list + ':' + manga, list, series: manga, position: 0, updated_at: 102 };
const stable = value => { const { exported_at, ...body } = clone(value); return body; };
function writable(value) { const listeners = new Set(); return { subscribe(fn) { listeners.add(fn); fn(value); return () => listeners.delete(fn); }, set(next) { value = next; for (const fn of [...listeners]) fn(value); }, update(fn) { this.set(fn(value)); } }; }
function get(store) { let value; store.subscribe(next => { value = next; })(); return value; }
function derived(stores, fn) { return { subscribe(callback) { const values = [], ready = new Set(); const offs = stores.map((store, index) => store.subscribe(value => { values[index] = value; ready.add(index); if (ready.size === stores.length) callback(fn(values)); })); return () => offs.forEach(off => off()); } }; }
function store(name, maps, onWrite = () => {}) {
  const rows = maps.get(name), key = value => JSON.stringify(value);
  const rowKey = value => name === 'records' || name === 'idMigrationTombstones' ? [value.family, value.key] : name === 'idMigrationArchives' ? value.digest : name === 'idMigrationIssues' ? value.sourceKey : [value.kind, value.legacyId];
  return { get: id => clone(rows.get(key(id))), getAll: () => [...rows.values()].map(value => clone(value)), count: () => rows.size,
    put(value, id) { onWrite(); rows.set(key(id ?? rowKey(value)), clone(value)); }, add(value, id) { onWrite(); const k = key(id ?? rowKey(value)); if (rows.has(k)) throw Error('duplicate'); rows.set(k, clone(value)); }, delete(id) { onWrite(); return rows.delete(key(id)); }, clear() { onWrite(); rows.clear(); } };
}
async function fixture({ records = [], baseline = false, publicFormat = true, legacy = false } = {}) {
  const selectedBase = baseline ? path.join(baselineRoot, 'platform-integrations') : base;
  const calls = { reads: [], locks: 0, changes: [], accounts: 0, downloads: [] };
  let locked, beforeCommit, writeHook, fileReads = 0;
  const context = vm.createContext({ console, URL, URLSearchParams, TextEncoder, TextDecoder, Uint8Array, ArrayBuffer, AbortSignal, Blob, structuredClone, Date, crypto, Proxy,
    navigator: { locks: { async request(_name, operation) { calls.locks++; if (locked) await locked; return operation(); } } },
    window: { location: { href: scope.origin + '/', origin: scope.origin }, addEventListener() {} },
    async fetch(url, options) { calls.reads.push({ url, method: options?.method ?? 'GET' }); assert.equal(url, '/api/protocol/capabilities'); return { ok: true, policy: { catalog_namespace: scope.catalogNamespace, service_key: null } }; } });
  const cache = new Map();
  const Session = Object.assign(writable(undefined), { async isValid() { calls.accounts++; throw Error('No account should be requested'); } });
  class HTTPError extends Error { constructor(status, message, title) { super(message); this.status = status; this.error = message; this.title = title; } }
  const protocol = { db: { stores: new Map([['meta', new Map()]]) }, async getMeta() { return 'device'; } };
  const deps = {
    'svelte/store': { writable, get, derived }, '$lib/stores/Session': { default: Session }, '$lib/HTTP': { HTTPError },
    '$env/static/public': { PUBLIC_API_URL: '/api', PUBLIC_MANGADEX_CATALOGUE: publicFormat ? '1' : '0' },
    '$platform/mutation-authority': { captureNativeAuthority: () => ({ actor: null, epoch: 0 }), sameNativeReadAuthority: () => true },
    '@noble/hashes/sha2.js': { sha256: bytes => new Uint8Array(createHash('sha256').update(bytes).digest()) },
    './protocol/store.js': { LocalLibrary: { open: async () => protocol } },
    './protocol/nostr.js': { bytesResponse: async response => new TextEncoder().encode(JSON.stringify(response.policy)) },
  };
  const indexed = { request: query => Promise.resolve(query), async transaction(db, names, mode, operation) {
    const included = typeof names === 'string' ? [names] : names;
    const maps = new Map([...db.stores].map(([name, rows]) => [name, new Map([...rows].map(([key, value]) => [key, clone(value)]))]));
    let aborted = false;
    const tx = { objectStoreNames: { contains: name => included.includes(name) }, objectStore: name => { assert.ok(included.includes(name)); return store(name, maps, () => writeHook?.(name)); }, abort() { aborted = true; } };
    const result = await operation(tx);
    if (mode === 'readwrite') { if (beforeCommit) await beforeCommit(); if (aborted) throw Error('Transaction aborted'); db.stores = maps; }
    return result;
  } };
  function synthetic(name, values) { if (!cache.has(name)) cache.set(name, new vm.SyntheticModule(Object.keys(values), function() { for (const [key, value] of Object.entries(values)) this.setExport(key, value); }, { context, identifier: name })); return cache.get(name); }
  function resolve(specifier, importer) {
    if (specifier.endsWith('/protocol/indexeddb.js') || specifier === './protocol/indexeddb.js') return synthetic('atomic-transaction-model', indexed);
    if (deps[specifier]) return synthetic(specifier, deps[specifier]);
    assert.ok(specifier.startsWith('.'), 'Unexpected import ' + specifier);
    const target = path.resolve(path.dirname(importer.identifier), specifier);
    return load(target + (path.extname(target) ? '' : '.ts'));
  }
  function load(file) { if (!cache.has(file)) { const source = fs.readFileSync(file, 'utf8'); const module = new vm.SourceTextModule(file.endsWith('.ts') ? stripTypeScriptTypes(source, { mode: 'transform' }) : source, { context, identifier: file, importModuleDynamically: async (specifier, importer) => { const child = resolve(specifier, importer); if (child.status === 'unlinked') await child.link(resolve); if (child.status !== 'evaluated') await child.evaluate(); return child; } }); cache.set(file, module); } return cache.get(file); }
  async function namespace(relative) { const module = load(path.join(selectedBase, relative)); if (module.status === 'unlinked') await module.link(resolve); if (module.status !== 'evaluated') await module.evaluate(); return module.namespace; }
  const state = await namespace('public-api/device-id-state.ts');
  const { DeviceStore } = await namespace('local-library/device-store.ts');
  const db = { name: 'finite-device', stores: new Map(state.MIGRATION_STORES.map(name => [name, new Map()])), objectStoreNames: { contains: name => state.MIGRATION_STORES.includes(name) }, close() {} };
  for (const [key, value] of Object.entries({ instanceScope: scope, publicIdOwner: { version: 2, ownerId: owner, scope }, deviceOwnerId: owner, revision: 7, count: records.length })) store('meta', db.stores).put(value, key);
  if (legacy) store('meta', db.stores).delete('publicIdOwner');
  for (const row of records) store('records', db.stores).put(row);
  const device = new DeviceStore(db, clone(scope), () => {}, () => {}, { idFormat: publicFormat ? 'public_uuid-v2' : 'native-v1' });
  device.channel = { postMessage: value => calls.changes.push(value), close() {} };
  DeviceStore.open = async () => device;
  const library = await namespace('local-library/library.ts');
  await library.openLibrary(); await library.verifyCatalogueScope();
  const core = await namespace('public-api/device-id-migration.ts');
  return { device, library, db, calls, Session, state, core, namespace, context,
    snapshot: () => device.snapshot(), meta: (key, value) => store('meta', db.stores).put(value, key),
    file(input, { size, text } = {}) { const bytes = JSON.stringify(input); return { name: 'synthetic-library-backup.json', size: size ?? Buffer.byteLength(bytes), async text() { fileReads++; return text ? text() : bytes; } }; },
    reads: () => fileReads, holdLock(promise) { locked = promise; }, beforeCommit(fn) { beforeCommit = fn; }, onWrite(fn) { writeHook = fn; },
    async panel() {
      const source = fs.readFileSync(path.join(selectedBase, 'local-library/LocalLibraryPanel.svelte'), 'utf8');
      const script = stripTypeScriptTypes(source.match(/<script[^>]*>([\s\S]*?)<\/script>/)[1].replace(/^\s*import[^;]+;\s*$/gm, ''), { mode: 'transform' });
      let cleanup;
      const sandbox = vm.createContext({ ...Object.fromEntries(Object.keys(library).map(name => [name, library[name]])), sameScope: (a, b) => JSON.stringify(a) === JSON.stringify(b), publicAccountLibraryUnavailable: true, $localLibraryState: get(library.localLibraryState), $state: Object.assign(value => value, { raw: value => value }), $derived: value => value, onMount: fn => { cleanup = fn(); }, disposePanel: () => cleanup(), Blob, navigator: {}, confirm: () => true });
      vm.runInContext(script + '\n;globalThis.panel={preview,prepareRestore,confirmRestore,closeReview,run,dispose:()=>disposePanel(),consent:v=>foreignOwnerConsent=v,get imported(){return imported},get restore(){return deviceRestore},get notice(){return notice},get error(){return error},get watchers(){return restoreWatchers.size}};', sandbox);
      await Promise.resolve(); return sandbox.panel;
    },
  };
}
async function preview(f, input) { return f.library.previewImport(f.file(input)); }
async function exported(f) { return JSON.parse(await (await f.library.exportBackup()).text()); }
const unchanged = async (f, before) => assert.deepEqual(stable(await f.snapshot()), stable(before));

if (baselineRoot) test('exact20f2 counterfactual: default full-v2 export previews but both offered legacy actions refuse', async () => {
  assert.equal(createHash('sha256').update(fs.readFileSync(path.join(baselineRoot, 'platform-integrations/local-library/device-store.ts'))).digest('hex'), '4b9f9ab7eb4d01dd1c903cfae86168fb2c81cf391d1731a9bbaf2217e2a1d210');
  for (const [name, expected] of Object.entries({ 'library.ts': '2c093421cbeeb368b681f09cd1ae7cb3319316979e4c3304ab3de8fabbd23c76', 'LocalLibraryPanel.svelte': '61216a341f11ad5c2ea854e014a9c704c82eb39ff121ceb9e51062cd96945877' })) assert.equal(createHash('sha256').update(fs.readFileSync(path.join(baselineRoot, 'platform-integrations/local-library', name))).digest('hex'), expected);
  const f = await fixture({ records: [series(), progress(), listRow, member], baseline: true });
  const backup = await exported(f), review = await preview(f, backup), before = await f.snapshot();
  assert.equal(backup.version, 2); assert.equal(review.snapshot.version, 2);
  for (const mode of ['merge', 'replace']) await assert.rejects(() => f.library.applyImport(review, mode), /dedicated reviewed restore flow/);
  await unchanged(f, before); assert.equal(f.library.prepareDeviceBackupImport, undefined);
});

test('default v2 export -> frozen preview -> prepare -> explicit confirm roundtrip retains complete state locally', async () => {
  const f = await fixture({ records: [series(), progress(), listRow, member] }), backup = await exported(f), before = await f.snapshot();
  const review = await preview(f, backup); assert.equal(Object.isFrozen(review), true); assert.equal(Object.isFrozen(review.snapshot.records), true); assert.equal(review.foreignOwner, false);
  const prepared = await f.library.prepareDeviceBackupImport(review, 'merge', false, () => {});
  assert.equal(prepared.mode, 'merge'); assert.equal(prepared.summary.changed, false); assert.equal(Object.isFrozen(prepared.summary), true);
  await unchanged(f, before); await f.library.applyDeviceBackupImport(prepared, () => {}); await unchanged(f, before);
  assert.equal(f.calls.accounts, 0); assert.ok(f.calls.reads.every(row => row.method === 'GET' && row.url === '/api/protocol/capabilities'));
});
test('foreign-device restoration requires explicit acknowledgement and retains target owner plus source archive provenance', async () => {
  const f = await fixture({ records: [series()] }), input = { ...await exported(f), ownerId: foreignOwner, records: [series(manga, 200, 'completed')] };
  const review = await preview(f, input), before = await f.snapshot(); assert.equal(review.foreignOwner, true);
  await assert.rejects(() => f.library.prepareDeviceBackupImport(review, 'merge', false, () => {}), /foreign-device owner/); await unchanged(f, before);
  const prepared = await f.library.prepareDeviceBackupImport(review, 'merge', true, () => {}); await unchanged(f, before);
  await f.library.applyDeviceBackupImport(prepared, () => {});
  const result = await f.snapshot(); assert.equal(result.ownerId, owner); assert.equal(result.records[0].status, 'completed'); assert.ok(result.archives.some(row => row.ownerId === owner && row.record.status === 'reading'));
  const sourceArchive = f.state.archiveRecord(series(other, 80), foreignOwner, scope);
  const second = await preview(f, { ...input, archives: [sourceArchive] });
  await f.library.applyDeviceBackupImport(await f.library.prepareDeviceBackupImport(second, 'merge', true, () => {}), () => {});
  assert.ok((await f.snapshot()).archives.some(row => row.ownerId === foreignOwner && row.digest === sourceArchive.digest));
});
test('replacement archives current rows and retains history, aliases and deletion fences rather than clearing sidecars', async () => {
  const f = await fixture({ records: [series(), progress(), listRow, member] }), input = { ...await exported(f), records: [series(other)] };
  const review = await preview(f, input), prepared = await f.library.prepareDeviceBackupImport(review, 'replace', false, () => {});
  assert.equal(prepared.summary.records, 1); assert.equal(prepared.backup.records.length, 4);
  await f.library.applyDeviceBackupImport(prepared, () => {});
  const result = await f.snapshot(); assert.deepEqual(result.records.map(row => row.id), [other]); assert.equal(result.archives.length, 4); assert.equal(result.ownerId, owner); assert.equal(result.revision, 8);
});
test('old full backup merge cannot resurrect deleted saved titles or memberships', async () => {
  const f = await fixture({ records: [series(), listRow, member] }), backup = await exported(f); await f.device.remove('series', manga);
  const before = await f.snapshot(), review = await preview(f, backup), prepared = await f.library.prepareDeviceBackupImport(review, 'merge', false, () => {});
  await f.library.applyDeviceBackupImport(prepared, () => {}); const result = await f.snapshot();
  assert.equal(result.records.some(row => row.family === 'series' || row.family === 'members'), false); assert.deepEqual(result.tombstones, before.tombstones); assert.ok(result.archives.length >= 2);
});
test('newer and equal-clock local values win merge while incoming conflict is archived', async () => {
  for (const time of [99, 100]) { const f = await fixture({ records: [series()] }), backup = { ...await exported(f), records: [series(manga, time, 'completed')] };
    const prepared = await f.library.prepareDeviceBackupImport(await preview(f, backup), 'merge', false, () => {}); await f.library.applyDeviceBackupImport(prepared, () => {});
    const result = await f.snapshot(); assert.equal(result.records[0].status, 'reading'); assert.ok(result.archives.some(row => row.record.status === 'completed'));
  }
});
test('fabricated, mutable copy and replayed prepared reviews cannot authorize a restore', async () => {
  const f = await fixture(), review = await preview(f, await exported(f));
  await assert.rejects(() => f.library.prepareDeviceBackupImport({ ...review }, 'merge', true, () => {}), /Preview/);
  assert.throws(() => { review.snapshot.ownerId = foreignOwner; });
  const prepared = await f.library.prepareDeviceBackupImport(review, 'merge', false, () => {});
  await assert.rejects(() => f.library.applyDeviceBackupImport({ ...prepared }, () => {}), /fresh device-backup/);
  await f.library.applyDeviceBackupImport(prepared, () => {});
  await assert.rejects(() => f.library.applyDeviceBackupImport(prepared, () => {}), /fresh device-backup/);
});
test('revision or durable owner changes after file preview require re-preview before preparation', async () => {
  for (const change of [f => f.meta('revision', 8), f => { f.meta('deviceOwnerId', foreignOwner); f.meta('publicIdOwner', { version: 2, ownerId: foreignOwner, scope }); }]) {
    const f = await fixture({ records: [series()] }), review = await preview(f, await exported(f)); change(f); const before = await f.snapshot();
    await assert.rejects(() => f.library.prepareDeviceBackupImport(review, 'merge', false, () => {}), /library changed/); await unchanged(f, before);
  }
});
test('same-revision exact-state changes after preparation refuse rather than overwrite current rows', async () => {
  const f = await fixture({ records: [series()] }), prepared = await f.library.prepareDeviceBackupImport(await preview(f, await exported(f)), 'replace', false, () => {});
  store('records', f.db.stores).put(series(other)); f.meta('count', 2); const before = await f.snapshot();
  await assert.rejects(() => f.library.applyDeviceBackupImport(prepared, () => {}), /changed in another tab/); await unchanged(f, before);
});
test('durable scope/fence changes and facade scope changes cannot retarget a prepared restore', async () => {
  for (const change of [f => f.meta('scopeMigrationReason', 'Known catalogue mismatch'), f => { f.device.scope = { ...scope, catalogNamespace: other }; }]) {
    const f = await fixture({ records: [series()] }), prepared = await f.library.prepareDeviceBackupImport(await preview(f, await exported(f)), 'replace', false, () => {}); change(f); const before = await f.snapshot();
    await assert.rejects(() => f.library.applyDeviceBackupImport(prepared, () => {}), /mismatch|scope changed/); await unchanged(f, before);
  }
});
test('foreign scope, immutable alias rebind, chapter-parent change and credential fields fail closed', async () => {
  const f = await fixture({ records: [progress()] }), input = await exported(f), before = await f.snapshot();
  await assert.rejects(async () => f.library.prepareDeviceBackupImport(await preview(f, { ...input, scope: { ...scope, catalogNamespace: other } }), 'merge', false, () => {}), /different verified catalogue/);
  await assert.rejects(async () => f.library.prepareDeviceBackupImport(await preview(f, { ...input, records: [{ ...progress(), mangaId: other }] }), 'merge', false, () => {}), /chapter parent/);
  await assert.rejects(() => preview(f, { ...input, privateKey: 'synthetic-disallowed' }), /Unexpected field/);
  store('publicAliases', f.db.stores).put({ kind: 'manga', legacyId: 'Abcdef1234', id: manga });
  const aliasBackup = { ...input, aliases: [{ kind: 'manga', legacyId: 'Abcdef1234', id: other }] };
  await assert.rejects(async () => f.library.prepareDeviceBackupImport(await preview(f, aliasBackup), 'merge', false, () => {}), /immutable alias/);
  assert.deepEqual((await f.snapshot()).records, before.records);
});
test('file budget is checked before reading and malformed JSON/sidecar hashes are refused', async () => {
  const f = await fixture(), input = await exported(f);
  await assert.rejects(() => f.library.previewImport(f.file(input, { size: 256 * 1024 * 1024 + 1 })), /256 MiB/); assert.equal(f.reads(), 0);
  await assert.rejects(() => f.library.previewImport(f.file(input, { text: () => '{' })), error => error.name === 'SyntaxError');
  const archive = f.state.archiveRecord(series(), owner, scope); archive.record.status = 'completed';
  await assert.rejects(() => preview(f, { ...input, archives: [archive] }), /digest mismatch/);
});
test('closing the database or invalidating the view while waiting for the device lock prevents all writes', async () => {
  for (const close of [true, false]) { const f = await fixture({ records: [series()] }); let active = true, release; const current = () => { if (!active) throw Error('Closed review'); };
    const backup = { ...await exported(f), records: [series(other)] }, prepared = await f.library.prepareDeviceBackupImport(await preview(f, backup), 'replace', false, current), before = await f.snapshot();
    f.holdLock(new Promise(resolve => { release = resolve; })); const saving = f.library.applyDeviceBackupImport(prepared, current);
    await Promise.resolve(); if (close) f.db.onversionchange(); else active = false; release();
    await assert.rejects(() => saving, /database changed|Closed review/);
    assert.deepEqual(store('records', f.db.stores).getAll(), clone(before.records)); assert.equal(store('meta', f.db.stores).get('revision'), before.revision);
  }
});
test('view invalidation after restore writes are queued rolls back all rows and sidecars', async () => {
  const f = await fixture({ records: [series()] }); let queued = false;
  const current = () => { if (queued) throw Error('Closed during queued restore'); };
  const prepared = await f.library.prepareDeviceBackupImport(await preview(f, { ...await exported(f), records: [series(other)] }), 'replace', false, current), before = await f.snapshot(); f.onWrite(() => { queued = true; });
  await assert.rejects(() => f.library.applyDeviceBackupImport(prepared, current), /Closed during queued restore/); assert.equal(queued, true); await unchanged(f, before);
});
test('actual panel controls require prepare then confirm and reset foreign acknowledgement on every new preview', async () => {
  const f = await fixture({ records: [series()] }), panel = await f.panel(), file = f.file({ ...await exported(f), ownerId: foreignOwner, records: [series(other)] });
  await panel.preview(() => f.library.previewImport(file)); const before = await f.snapshot();
  await assert.rejects(() => panel.prepareRestore('replace'), /foreign-device owner/); panel.consent(true);
  await panel.prepareRestore('replace'); assert.equal(panel.restore.mode, 'replace'); await unchanged(f, before);
  await panel.confirmRestore(); assert.equal((await f.snapshot()).records[0].id, other); assert.equal(panel.imported, undefined); assert.match(panel.notice, /restored/);
  await panel.preview(() => f.library.previewImport(file)); await assert.rejects(() => panel.prepareRestore('merge'), /foreign-device owner/);
});
test('actual panel dismiss and destroy guards prevent prepared confirmations after the task lifetime', async () => {
  for (const destroy of [false, true]) { const f = await fixture({ records: [series()] }), panel = await f.panel();
    await panel.preview(async () => preview(f, { ...await exported(f), records: [series(other)] })); await panel.prepareRestore('replace');
    const before = await f.snapshot(); if (destroy) panel.dispose(); else panel.closeReview(false);
    await assert.rejects(() => panel.confirmRestore()); await unchanged(f, before);
  }
});
test('actual panel markup exposes dedicated v2 controls and keeps legacy merge/replace behind its own branch', () => {
  const source = fs.readFileSync(path.join(base, 'local-library/LocalLibraryPanel.svelte'), 'utf8');
  assert.match(source, /\{#if scopeMatches && imported\.snapshot\.version === 2\}/);
  assert.match(source, /bind:checked=\{foreignOwnerConsent\}/); assert.match(source, /!!imported\.foreignOwner && !foreignOwnerConsent/);
  assert.match(source, /Review backup merge/); assert.match(source, /Review backup replacement/); assert.match(source, /Confirm reviewed/);
  assert.match(source, /\{:else if scopeMatches\}[\s\S]*await applyImport\(selected, 'merge'\)/);
});

test('retained native-v1 backup preview and import still use their original row-import flow', async () => {
  const f = await fixture({ records: [listRow], publicFormat: false, legacy: true }), backup = await exported(f);
  assert.equal(backup.version, 1); backup.records[0].name = 'Legacy restored list'; backup.records[0].updated_at++;
  const review = await preview(f, backup); assert.equal(review.foreignOwner, undefined);
  await f.library.applyImport(review, 'merge'); assert.equal((await f.snapshot()).records[0].name, 'Legacy restored list');
});

test('actual panel invalidation aborts an active restore transaction even after its last synchronous guard', async () => {
  for (const destroy of [false, true]) {
    const f = await fixture({ records: [series()] }), panel = await f.panel();
    await panel.preview(async () => preview(f, { ...await exported(f), records: [series(other)] }));
    await panel.prepareRestore('replace'); const before = await f.snapshot(); let sawWatcher = false;
    f.beforeCommit(() => { sawWatcher = panel.watchers === 1; if (destroy) panel.dispose(); else panel.closeReview(false); });
    await assert.rejects(() => panel.confirmRestore(), /Transaction aborted/);
    assert.equal(sawWatcher, true); assert.equal(panel.watchers, 0); await unchanged(f, before);
  }
});
