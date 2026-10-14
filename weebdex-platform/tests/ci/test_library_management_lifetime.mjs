#!/usr/bin/env node
// Execute the real publication, snapshot queue, delivery, signing validation,
// device store and immutable IndexedDB outbox. Fake IndexedDB supplies the IDB
// implementation; Web Locks, workers and service proof/network are finite test
// adapters. No browser, extension, server, network request, or native DB starts.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import vm from 'node:vm';
import { createRequire } from 'node:module';
import { fileURLToPath, pathToFileURL } from 'node:url';
import test from 'node:test';

assert(vm.SourceTextModule, 'Run with node --experimental-vm-modules');
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const sourceRoot = path.join(root, 'platform-integrations/local-library');
const dependencies = process.env.WEEBDEX_FRONTEND_ROOT;
assert(dependencies, 'Use the existing exact-lock frontend dependency root');
const require = createRequire(path.join(dependencies, 'package.json'));
const ts = require('typescript');
assert.equal(require('typescript/package.json').version, '5.9.3');
const domModules = process.env.WEEBDEX_BROWSER_DOM_NODE_MODULES;
assert(domModules, 'Use the existing fake-indexeddb dependency root');
assert.equal(require(path.join(domModules, 'fake-indexeddb/package.json')).version, '6.2.4');
const { IDBFactory, IDBKeyRange } = require(path.join(domModules, 'fake-indexeddb'));
const noble = path.join(root, '.build/local-library-noble-2.0.1/node_modules/@noble');
assert.equal(JSON.parse(fs.readFileSync(path.join(noble, 'curves/package.json'))).version, '2.0.1');
const curves = await import(pathToFileURL(path.join(noble, 'curves/secp256k1.js')));
const hashes = await import(pathToFileURL(path.join(noble, 'hashes/sha2.js')));
const deferred = () => { let resolve; const promise = new Promise(yes => { resolve = yes; }); return { promise, resolve }; };
const clean = value => JSON.parse(JSON.stringify(value));
function writable(initial) {
  let value = initial; const subscribers = new Set();
  return { subscribe(listener) { subscribers.add(listener); listener(value); return () => subscribers.delete(listener); }, set(next) { value = next; for (const listener of subscribers) listener(value); }, update(operation) { this.set(operation(value)); } };
}
const get = store => { let value; store.subscribe(next => { value = next; })(); return value; };

async function fixture({ gateAt, changeDuringSign } = {}) {
  const gate = deferred(), entered = deferred();
  const calls = { kinds: [], posts: 0, worker: 0, finalRead: 0, publicKey: 0 };
  let alive = true, protocol, device, graph, codec, signerMode = gateAt;
  const assertCurrent = () => { if (!alive) throw Error('Management closed'); };
  const ownerSecret = new Uint8Array(32).fill(7), owner = Buffer.from(curves.schnorr.getPublicKey(ownerSecret)).toString('hex');
  const recipient = { origin: 'http://127.0.0.1:2', serviceKey: 'a'.repeat(64), selected: true };
  const scope = { origin: 'http://127.0.0.1:1', serviceKey: recipient.serviceKey, catalogNamespace: '0'.repeat(32) };
  const policy = { available: true, schema_ready: true, origin: recipient.origin, service_key: recipient.serviceKey, catalog_namespace: scope.catalogNamespace, connect_origins: [recipient.origin] };
  const signed = template => {
    const event = { ...clean(template), pubkey: owner };
    const digest = hashes.sha256(new TextEncoder().encode(JSON.stringify([0, owner, event.created_at, event.kind, event.tags, event.content])));
    return { ...event, id: Buffer.from(digest).toString('hex'), sig: Buffer.from(curves.schnorr.sign(digest, ownerSecret, new Uint8Array(32))).toString('hex') };
  };
  const signer = {
    async getPublicKey() { calls.publicKey++; if (signerMode === 'connect') { entered.resolve(); await gate.promise; } return owner; },
    async signEvent(template) {
      calls.kinds.push(template.kind);
      if ((signerMode === 'snapshot' && template.kind === 11999) || (['proof', 'recovery-proof'].includes(signerMode) && template.kind === 27235)) { entered.resolve(); await gate.promise; }
      return signed(template);
    },
  };
  const context = vm.createContext({
    console, URL, URLSearchParams, TextEncoder, TextDecoder, Uint8Array, ArrayBuffer, structuredClone, Blob, File, Date, JSON, Buffer,
    crypto, performance, AbortSignal, btoa, atob, setTimeout, clearTimeout, queueMicrotask,
    indexedDB: new IDBFactory(), IDBKeyRange,
    window: { location: { origin: scope.origin }, nostr: signer },
    navigator: { locks: { request: async (_name, options, callback) => (callback ?? options)({ mode: 'exclusive' }) } },
    Worker: class {
      terminate() {}
      postMessage() { calls.worker++; void (async () => { if (gateAt === 'worker') { entered.resolve(); await gate.promise; } this.onmessage({ data: { mappingProof: 'same' } }); })(); }
    },
    fetch: async () => { calls.posts++; throw Error('Synthetic transport boundary reached'); },
  });
  const modules = new Map();
  function synthetic(name, values) {
    const module = new vm.SyntheticModule(Object.keys(values), function () { for (const [key, value] of Object.entries(values)) this.setExport(key, value); }, { context, identifier: name });
    modules.set(name, module); return module;
  }
  synthetic('svelte/store', { writable, get });
  synthetic('$env/static/public', { PUBLIC_API_URL: '/api' });
  synthetic('@noble/curves/secp256k1.js', curves);
  synthetic('@noble/hashes/sha2.js', hashes);
  synthetic('library-adapter', {
    stores: async () => ({ protocol, device }), currentDeviceSnapshot: () => device.snapshot(), apiRead: async () => policy,
    refreshLibrary: async () => {}, verifyCatalogueScope: async () => {}, message: cause => String(cause),
  });
  synthetic('catalog-adapter', { scopedCatalogCandidates: async () => [] });
  function moduleFor(file) {
    if (modules.has(file)) return modules.get(file);
    let source = fs.readFileSync(file, 'utf8');
    if (file.endsWith('.ts')) source = ts.transpileModule(source, { compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ES2022 } }).outputText;
    const module = new vm.SourceTextModule(source, { context, identifier: file, initializeImportMeta(meta) { meta.url = pathToFileURL(file).href; } });
    modules.set(file, module); return module;
  }
  let nostrAdapter;
  function resolve(specifier, importer) {
    if (modules.has(specifier)) return modules.get(specifier);
    const target = path.resolve(path.dirname(importer.identifier), specifier);
    if (target === path.join(sourceRoot, 'library')) return modules.get('library-adapter');
    if (target === path.join(sourceRoot, 'catalog')) return modules.get('catalog-adapter');
    if (nostrAdapter && target === path.join(sourceRoot, 'protocol/nostr.js')) return nostrAdapter;
    return moduleFor(fs.existsSync(target) ? target : target + '.ts');
  }
  async function load(file) { const module = moduleFor(path.join(sourceRoot, file)); if (module.status === 'unlinked') await module.link(resolve); if (module.status !== 'evaluated') await module.evaluate(); return module.namespace; }
  const nostr = await load('protocol/nostr.js');
  nostrAdapter = synthetic('verified-service-adapter', { ...nostr, proveService: async origin => ({ origin, serviceKey: recipient.serviceKey }) });
  const { DeviceStore } = await load('device-store.ts');
  const { LocalLibrary } = await load('protocol/store.js');
  device = await DeviceStore.open(scope, () => {}, () => {});
  protocol = await LocalLibrary.open('lifetime-' + crypto.randomUUID());
  await protocol.setRelays([recipient], 0);
  codec = await load('protocol/codec.js'); graph = codec.buildGraph({});
  const publication = await load('publication.ts');
  if (gateAt !== 'connect') await publication.connectSigner(assertCurrent);
  const identity = get(publication.signerIdentity);
  const review = { mappingProof: 'same', deviceRevision: 0, protocolRevision: 0, relayRevision: 1, owner, generation: identity.generation, recipients: [recipient], graph, excluded: [], sourceOrigin: recipient.origin, serviceKey: recipient.serviceKey };
  // Hold the real cancellation read used by signedRequest's final beforeSend,
  // after the exact event is durable and the HTTP authentication proof is signed.
  if (gateAt === 'final-read') {
    const original = protocol.job.bind(protocol);
    protocol.job = async id => {
      const result = await original(id);
      if (calls.kinds.includes(27235) && calls.finalRead === 0) { calls.finalRead++; entered.resolve(); await gate.promise; }
      return result;
    };
  }
  return {
    calls, review, owner, graph, nostr, protocol, device, publication, assertCurrent, entered: entered.promise,
    close() { alive = false; }, release: gate.resolve,
    async change() {
      if (changeDuringSign === 'device') await device.writeBatch([{ family: 'series', key: 'TitleOne01', id: 'TitleOne01', status: 'reading', notification: false, followedAt: new Date().toISOString(), updated_at: 1 }]);
      if (changeDuringSign === 'recipients') await protocol.setRelays([{ ...recipient, selected: false }], 1);
    },
    async dispose() { device.db.close(); device.channel?.close(); protocol.close(); },
  };
}

for (const reason of ['Close', 'Back', 'navigation']) test('dismissed management prevents initial snapshot signing after deferred storage/worker: ' + reason, { timeout: 5000 }, async () => {
  const f = await fixture({ gateAt: 'worker' });
  try {
    const sending = f.publication.publishReviewed(f.review, true, f.assertCurrent);
    await Promise.race([f.entered, sending.then(() => { throw Error('Operation ended before the selected deferred boundary'); })]); f.close(); f.release(); await assert.rejects(sending, /Management closed/);
    assert.deepEqual(f.calls.kinds, []); assert.equal(f.calls.posts, 0); assert.deepEqual(clean(await f.protocol.jobs()), []);
  } finally { await f.dispose(); }
});

test('dismissal during external snapshot signature retains exact CAS-checked immutable job before stopping delivery', { timeout: 5000 }, async () => {
  const f = await fixture({ gateAt: 'snapshot' });
  try {
    const sending = f.publication.publishReviewed(f.review, true, f.assertCurrent);
    await Promise.race([f.entered, sending.then(() => { throw Error('Operation ended before the selected deferred boundary'); })]); f.close(); f.release(); await assert.rejects(sending, /Management closed/);
    const jobs = await f.protocol.jobs(); assert.equal(jobs.length, 1);
    const job = jobs[0]; assert(f.nostr.verifyEvent(job.event)); assert.equal(job.event.kind, 11999);
    assert.deepEqual(JSON.parse(job.event.content), { manifest: f.graph.root.hash, version: 1 });
    assert.deepEqual(clean(job.recipients.map(({ origin, serviceKey, selected }) => ({ origin, serviceKey, selected }))), clean(f.review.recipients));
    assert.equal(job.recipients[0].state, 'pending'); assert.equal(job.deviceRevision, 0);
    const batch = await f.protocol.jobBatch(job.id, 0); assert.equal(batch.objects[0].hash, f.graph.objects[0].hash);
    assert.deepEqual([...batch.objects[0].data], [...f.graph.objects[0].data]);
    assert.deepEqual(f.calls.kinds, [11999]); assert.equal(f.calls.posts, 0);
  } finally { await f.dispose(); }
});

for (const gateAt of ['proof', 'final-read']) test('dismissal blocks POST after exact job is durable at ' + gateAt, { timeout: 5000 }, async () => {
  const f = await fixture({ gateAt });
  try {
    const sending = f.publication.publishReviewed(f.review, true, f.assertCurrent);
    await Promise.race([f.entered, sending.then(() => { throw Error('Operation ended before the selected deferred boundary'); })]); f.close(); f.release(); await sending;
    const jobs = await f.protocol.jobs(); assert.equal(jobs.length, 1); assert(f.nostr.verifyEvent(jobs[0].event));
    assert.equal(jobs[0].recipients[0].state, 'retryable'); assert.equal(f.calls.posts, 0);
    assert.deepEqual(f.calls.kinds, [11999, 27235]);
    if (gateAt === 'final-read') assert.equal(f.calls.finalRead, 1);
  } finally { await f.dispose(); }
});

for (const changeDuringSign of ['device', 'recipients']) test('dismissal never bypasses original durable revision CAS: ' + changeDuringSign, { timeout: 5000 }, async () => {
  const f = await fixture({ gateAt: 'snapshot', changeDuringSign });
  try {
    const sending = f.publication.publishReviewed(f.review, true, f.assertCurrent);
    await Promise.race([f.entered, sending.then(() => { throw Error('Operation ended before the selected deferred boundary'); })]); await f.change(); f.close(); f.release(); await assert.rejects(sending, /changed.*signing|changed while signing/i);
    assert.deepEqual(clean(await f.protocol.jobs()), []); assert.equal(f.calls.posts, 0);
  } finally { await f.dispose(); }
});

test('late external public-key selection cannot reconnect a dismissed settings view', { timeout: 5000 }, async () => {
  const f = await fixture({ gateAt: 'connect' });
  try {
    const selecting = f.publication.connectSigner(f.assertCurrent);
    await Promise.race([f.entered, selecting.then(() => { throw Error('Selection ended before the deferred key response'); })]); f.close(); f.release(); await assert.rejects(selecting, /Management closed/);
    assert.equal(get(f.publication.signerIdentity).owner, null); assert.equal(f.calls.posts, 0);
  } finally { await f.dispose(); }
});

test('active management still signs and reaches the original transport boundary', { timeout: 5000 }, async () => {
  const f = await fixture();
  try {
    const id = await f.publication.publishReviewed(f.review, true, f.assertCurrent);
    assert.equal((await f.protocol.jobs())[0].id, id); assert.equal(f.calls.posts, 1);
    assert.deepEqual(f.calls.kinds, [11999, 27235]);
  } finally { await f.dispose(); }
});


test('closing recovery preview abandons a late HTTP signing prompt without starting a request', { timeout: 5000 }, async () => {
  const f = await fixture({ gateAt: 'recovery-proof' });
  try {
    const previewing = f.publication.previewRecovery(f.review.recipients[0], f.assertCurrent);
    await Promise.race([f.entered, previewing.then(() => { throw Error('Recovery ended before its signing prompt'); })]);
    f.close(); f.release(); await assert.rejects(previewing, /Management closed/);
    assert.deepEqual(f.calls.kinds, [27235]); assert.equal(f.calls.posts, 0); assert.deepEqual(clean(await f.protocol.jobs()), []);
  } finally { await f.dispose(); }
});

test('unchanged IndexedDB schemas reopen all saved device families and exact public jobs after management relocation', { timeout: 5000 }, async () => {
  const f = await fixture(); let reopenedDevice, reopenedProtocol;
  try {
    await f.publication.publishReviewed(f.review, true, f.assertCurrent);
    const list = 'd'.repeat(32), date = '2026-10-14T00:00:00.000Z';
    await f.device.writeBatch([
      { family: 'series', key: 'TitleOne01', id: 'TitleOne01', status: 'reading', notification: true, followedAt: date, updated_at: 1 },
      { family: 'chapters', key: 'Chapter001', id: 'Chapter001', mangaId: 'TitleOne01', completed: false, page: 12, readAt: date, updated_at: 1 },
      { family: 'lists', key: list, id: list, name: 'Saved list', updated_at: 1 },
      { family: 'members', key: list + ':TitleOne01', list, series: 'TitleOne01', position: 2, updated_at: 1 },
    ]);
    await f.protocol.write('series', { id: 'e'.repeat(64), status: 'completed', updated_at: 1 });
    const before = await f.device.snapshot(), jobs = await f.protocol.jobs(), counts = await f.protocol.counts();
    const Device = f.device.constructor, Protocol = f.protocol.constructor, protocolName = f.protocol.db.name;
    f.device.db.close(); f.device.channel?.close(); f.protocol.close();
    reopenedDevice = await Device.open(before.scope, () => {}, () => {});
    reopenedProtocol = await Protocol.open(protocolName);
    const after = await reopenedDevice.snapshot();
    assert.equal(reopenedDevice.db.version, 1); assert.equal(reopenedProtocol.db.version, 5);
    assert.deepEqual(clean(after.records), clean(before.records)); assert.deepEqual(clean(after.scope), clean(before.scope)); assert.equal(after.revision, before.revision);
    assert.deepEqual(clean(await reopenedProtocol.jobs()), clean(jobs)); assert.deepEqual(clean(await reopenedProtocol.counts()), clean(counts));
    assert(f.nostr.verifyEvent((await reopenedProtocol.jobs())[0].event));
  } finally { reopenedDevice?.db.close(); reopenedDevice?.channel?.close(); reopenedProtocol?.close(); await f.dispose(); }
});
