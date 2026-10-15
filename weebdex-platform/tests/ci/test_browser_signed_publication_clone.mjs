#!/usr/bin/env node
// Actual pinned Svelte deep proxies and original IndexedDB outbox code. The
// fake-IDB adapter performs real structuredClone insertion checks; no browser,
// service or signing is started and this does not establish publication acceptance.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import vm from 'node:vm';
import test from 'node:test';
import { createHash } from 'node:crypto';
import { createRequire, stripTypeScriptTypes } from 'node:module';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { PublicationOutbox } from '../../platform-integrations/local-library/protocol/outbox.js';
const read = relative => fs.readFileSync(new URL(relative, import.meta.url), 'utf8');
const hash = bytes => createHash('sha256').update(bytes).digest('hex');
const frontend = process.env.WEEBDEX_FRONTEND_ROOT;
assert(frontend, 'Use the existing exact-lock frontend dependency stage');
const svelte = path.join(frontend, 'node_modules/svelte');
assert.equal(JSON.parse(fs.readFileSync(path.join(svelte, 'package.json'))).version, '5.55.1');
for (const [file, digest] of Object.entries({
  'src/internal/client/proxy.js': '24e27613bb0a0fa263c900ea1f9d4fdae84042530cc75ead82342a959410ed9a',
  'src/internal/shared/clone.js': '2577a0e3ff3815d12aa9a451d42bd66d5c068bed886c817afaa150645a06bf9e',
})) assert.equal(hash(fs.readFileSync(path.join(svelte, file))), digest);
const { proxy } = await import(pathToFileURL(path.join(svelte, 'src/internal/client/proxy.js')));
const { snapshot } = await import(pathToFileURL(path.join(svelte, 'src/internal/shared/clone.js')));
const modules = process.env.WEEBDEX_BROWSER_DOM_NODE_MODULES ?? fileURLToPath(new URL('../../../manga-protocol-lab/browser-client/node_modules', import.meta.url));
const adapter = path.join(modules, 'fake-indexeddb');
assert.equal(JSON.parse(fs.readFileSync(path.join(adapter, 'package.json'))).version, '6.2.4');
assert.equal(hash(fs.readFileSync(path.join(adapter, 'build/cjs/FDBObjectStore.js'))), 'b356c6d741681946a59cf72917e6fdf958e8eddcf26404cb2d84b33d5ffe9576');
assert.match(fs.readFileSync(path.join(adapter, 'build/cjs/lib/cloneValueForInsertion.js'), 'utf8'), /structuredClone\(value\)/);
const { indexedDB } = createRequire(import.meta.url)(adapter);
const event = JSON.parse(read('./fixtures/signed-envelope/node-event.json'));
const makeReview = () => proxy({
  deviceRevision: 4, protocolRevision: 0, relayRevision: 0, mappingProof: 'private fingerprint',
  owner: event.pubkey, generation: 2, sourceOrigin: 'http://127.0.0.1:4001', serviceKey: 'a'.repeat(64),
  recipients: [{ origin: 'http://127.0.0.1:4001', serviceKey: 'a'.repeat(64), selected: true }, { origin: 'http://127.0.0.1:4002', serviceKey: 'b'.repeat(64), selected: true }],
  graph: { root: { hash: 'c'.repeat(64) }, objects: [{ type: 'root', hash: 'c'.repeat(64), data: new Uint8Array([1, 2, 3]) }] }, excluded: [],
});
async function withOutbox(operation) {
  const db = await new Promise((resolve, reject) => {
    const opening = indexedDB.open('synthetic-clone-' + crypto.randomUUID(), 1);
    opening.onupgradeneeded = () => { const db = opening.result; db.createObjectStore('jobs', { keyPath: 'id' }); db.createObjectStore('jobObjects', { keyPath: ['job', 'index'] }); db.createObjectStore('meta'); };
    opening.onsuccess = () => resolve(opening.result); opening.onerror = () => reject(opening.error);
  });
  try { await operation(new PublicationOutbox(db)); } finally { db.close(); indexedDB.deleteDatabase(db.name); }
}
const job = review => ({ id: event.id, event, revision: review.protocolRevision, relayRevision: review.relayRevision,
  cancelled: false, objects: review.graph.objects, recipients: review.recipients.map(row => ({ ...row, cursor: 0, state: 'pending', message: 'Not sent' })) });
const panel = read('../../platform-integrations/local-library/LibraryManagementTools.svelte');
const send = panel.match(/run\('publish', (async \(\) => \{[^\n]+\})\)/)?.[1];
assert(send, 'Execute the exact production publication handoff');
const handoff = (publication, publishReviewed) => vm.runInNewContext('(' + stripTypeScriptTypes(send) + ')()', { publication, publishReviewed, $state: { snapshot }, publicConsent: true, notice: '', assertActive() {} });

test('actual pinned Svelte compiler lowers the owned handoff to snapshot before clearing state or awaiting', () => {
  const { compile } = createRequire(import.meta.url)(path.join(svelte, 'compiler/index.js'));
  const { js } = compile(panel, { filename: 'LocalLibraryPanel.svelte', generate: 'client', dev: false, runes: true });
  assert.match(js.code, /const reviewed = \$\.snapshot\(\$\.get\(publication\)\);\s*\$\.set\(publicConsent, false\);\s*\$\.set\(publication, undefined\);\s*await publishReviewed\(reviewed, true, assertActive\)/);
});

test('old reactive graph handoff really fails original outbox insertion and rolls back the job', async () => {
  const review = makeReview();
  assert.throws(() => structuredClone(review.graph.objects[0]), error => error.name === 'DataCloneError');
  await withOutbox(async outbox => {
    await assert.rejects(outbox.create(job(review)), error => error.name === 'DataCloneError');
    assert.deepEqual(await outbox.list(), [], 'The failed transaction must not leave a partial immutable job');
  });
});

test('exact panel captures one plain immutable review before await and preserves recipients and graph bytes', async () => {
  const review = makeReview(), before = snapshot(review);
  let captured, release, entered;
  const waiting = new Promise(resolve => { entered = resolve; });
  const paused = new Promise(resolve => { release = resolve; });
  await withOutbox(async outbox => {
    const sending = handoff(review, async (selected, consent) => { assert.equal(consent, true); captured = selected; entered(); await paused; return outbox.create(job(selected)); });
    await waiting;
    assert.deepEqual(captured, before); assert.deepEqual(structuredClone(captured), before);
    review.recipients[0].origin = 'http://127.0.0.1:4999'; review.mappingProof = 'changed after selection'; review.graph.objects[0].hash = 'd'.repeat(64); review.graph.objects[0].data[0] = 99;
    assert.deepEqual(captured, before, 'A later reactive review cannot replace the captured publication intent');
    release(); await sending;
    const saved = (await outbox.list())[0];
    assert.equal(JSON.stringify(saved.event), JSON.stringify(event));
    assert.deepEqual(saved.recipients.map(({ origin, serviceKey }) => ({ origin, serviceKey })), before.recipients.map(({ origin, serviceKey }) => ({ origin, serviceKey })));
    const objects = await new Promise((resolve, reject) => { const tx = outbox.db.transaction('jobObjects', 'readonly'), get = tx.objectStore('jobObjects').getAll(); get.onsuccess = () => resolve(get.result); get.onerror = () => reject(get.error); });
    assert.deepEqual(objects[0].object, before.graph.objects[0]);
  });
});

test('snapshot does not bypass the actual original revision and recipient-intent CAS', async () => {
  await withOutbox(async outbox => {
    for (const changed of [{ revision: 1 }, { relayRevision: 1 }]) await assert.rejects(outbox.create({ ...job(snapshot(makeReview())), ...changed }), /changed.*review again/i);
    assert.deepEqual(await outbox.list(), []);
  });
  const source = read('../../platform-integrations/local-library/publication.ts');
  for (const guard of ['review.owner !== identity.owner', 'review.generation !== identity.generation', 'current.revision !== review.deviceRevision', 'canonical.revision !== review.protocolRevision', 'preferences.revision !== review.relayRevision', 'fingerprint.mappingProof !== review.mappingProof', 'latest.revision !== review.deviceRevision']) assert(source.includes(guard), guard);
  assert.match(panel, /const reviewed = \$state\.snapshot\(publication!\); publicConsent = false; publication = undefined; await publishReviewed\(reviewed, true, assertActive\)/);
});

// Reconstructed relay-checkbox regression: actual pinned deep proxies and the
// original outbox's real structured-clone transaction, without a network claim.
const selection = panel.match(/run\('select-relay', (async \(\) => \{[^\n]+\})\)/)?.[1];
assert(selection, 'Execute the exact destination checkbox handoff');
const select = (relays, relay, relayRevision, selectRelays, resetReviews=()=>{}) => vm.runInNewContext('(' + stripTypeScriptTypes(selection) + ')()', {relays,relay,relayRevision,selectRelays,resetReviews,event:{target:{checked:true}},$state:{snapshot}});
const destinations = () => [{origin:'http://127.0.0.1:4001',serviceKey:'a'.repeat(64),selected:true},{origin:'http://127.0.0.1:4002',serviceKey:'b'.repeat(64),selected:false}];

test('old two-row shallow map fails actual outbox preference insertion and rolls back',async()=>{
 await withOutbox(async outbox=>{const plain=destinations();await outbox.setRelays(plain,0);const relays=proxy(structuredClone(plain));const changed=relays.map(row=>row.origin===relays[1].origin?{...row,selected:true}:row);assert.throws(()=>structuredClone(changed),error=>error.name==='DataCloneError');await assert.rejects(outbox.setRelays(changed,1),error=>error.name==='DataCloneError');assert.deepEqual(await outbox.preferences(),{relays:plain,revision:1});});
});

test('compiled checkbox snapshots every row and exact handoff retains both identities across await',async()=>{
 const {compile}=createRequire(import.meta.url)(path.join(svelte,'compiler/index.js'));const {js}=compile(panel,{filename:'LibraryManagementTools.svelte',generate:'client',dev:false,runes:true});assert.match(js.code,/selectRelays\(\$\.snapshot\(\$\.get\(relays\)\)\.map/);
 await withOutbox(async outbox=>{const plain=destinations();await outbox.setRelays(plain,0);const relays=proxy(structuredClone(plain));let captured,release,entered,resets=0;const paused=new Promise(resolve=>release=resolve),started=new Promise(resolve=>entered=resolve);const pending=select(relays,relays[1],1,async(rows,revision)=>{captured=rows;assert.equal(revision,1);structuredClone(rows);entered();await paused;return outbox.setRelays(rows,revision);},()=>resets++);await started;const expected=plain.map(row=>({...row,selected:true}));assert.deepEqual(captured,expected);relays[0].origin='http://127.0.0.1:4999';relays[1].serviceKey='c'.repeat(64);assert.deepEqual(captured,expected);release();await pending;assert.equal(resets,1);assert.deepEqual(await outbox.preferences(),{relays:expected,revision:2});});
});

test('snapshot selection retains the original relay-revision CAS and does not change an approved job',async()=>{
 await withOutbox(async outbox=>{const plain=destinations();await outbox.setRelays(plain,0);await outbox.setRelays(plain,1);let resets=0;await assert.rejects(select(proxy(structuredClone(plain)),plain[1],1,(rows,revision)=>outbox.setRelays(rows,revision),()=>resets++),/selection changed.*review/i);assert.equal(resets,0);assert.deepEqual(await outbox.preferences(),{relays:plain,revision:2});assert.deepEqual(await outbox.list(),[]);});
});
