import {restoreModeratorTransportSource} from './reviewed-moderator-transport-source-successors.mjs';
import {restoreReportTransportSource,readReportTransportAssociation} from './reviewed-report-transport-source-successors.mjs';
// Finite producer and build-hook controls. Explicit fake signing/fetch and fake
// Rollup graph inputs. No Vite build, browser, network, native service or install.
import test from 'node:test';
import {restoreNativeTransportSource} from './reviewed-native-source-successors.mjs';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import vm from 'node:vm';
import { stripTypeScriptTypes } from 'node:module';
import { createHash } from 'node:crypto';
import { fileURLToPath } from 'node:url';
import { createUploadRefusalQaPlugin, readUploadRefusalQaGate, stripUploadRefusalQaSource, uploadRefusalReaderFile, uploadRefusalGraphFile } from '../../scripts/public-upload-refusal-qa.mjs';
assert.ok(vm.SourceTextModule, 'Use --experimental-vm-modules');
const root = fileURLToPath(new URL('../../', import.meta.url));
const transportPath = path.join(root, 'platform-integrations/public-api/nostr-upload-request.ts');
const clientPath = path.join(root, 'platform-integrations/public-api/upload-client.ts');
const source = fs.readFileSync(transportPath, 'utf8');
const digest = bytes => createHash('sha256').update(bytes).digest('hex');
const clone = value => JSON.parse(JSON.stringify(value));
const text = new TextEncoder();
function deferred() { let resolve; const promise = new Promise(yes => { resolve = yes; }); return { promise, resolve }; }
const oldFixtureSource = fs.readFileSync(new URL('./test_public_upload_request_recovery.mjs', import.meta.url), 'utf8');
let fixtureCode = oldFixtureSource.slice(oldFixtureSource.indexOf('async function fixture('), oldFixtureSource.indexOf('const errorIs ='));
assert.equal(digest(fixtureCode), '4be1bf0631ce5984b0b174b99ca4d0734088b4136d8ca08f38d9a60137f9d6c9');
const oldProof = "return { ...clone(event), id: 'synthetic-id', sig: 'synthetic-signature' };";
assert.equal(fixtureCode.split(oldProof).length, 2);
fixtureCode = fixtureCode.replace(oldProof, "return options.proof ? options.proof(event) : { ...clone(event), id: 'a'.repeat(64), sig: 'synthetic-signature' };");
fixtureCode = fixtureCode.replace('return { api, transport,', 'return { context, module, api, transport,');
const fixture = new Function('vm', 'stripTypeScriptTypes', 'source', 'digest', 'clone', 'text', 'deferred', fixtureCode + ';return fixture;')(vm, stripTypeScriptTypes, source, digest, clone, text, deferred);
function plugin(enabled = true, ssr = false) {
  const value = createUploadRefusalQaPlugin(root, { enabled, publicUploadEnabled: true }), emitted = [];
  value.configResolved({ build: { ssr } }); value.buildStart.call({ emitFile(row) { emitted.push(row); } });
  return { value, emitted, code: value.transform(source, transportPath).code };
}
const enabledSource = plugin().code;
async function caught(f, input) { try { await f.request(input); assert.fail('Expected finite transport refusal'); } catch (error) { assert.equal(error.name, 'PublicUploadRequestError'); return error; } }
const failing = options => fixture({ transportSource: enabledSource, fetch: async () => { throw Error('Controlled fetch refusal'); }, ...options });
const snapshot = f => clone(f.api.readUploadRequestRefusals());

test('default module getter is disabled and copied; explicit QA composition alone enables recording', async () => {
  const f = await fixture({ fetch: async () => { throw Error('finite'); } }); await caught(f);
  assert.deepEqual(snapshot(f), { schema: 1, enabled: false, record_valid: true, truncated: false, records: [] });
  const qa = await failing(); await caught(qa);
  assert.deepEqual(snapshot(qa), { schema: 1, enabled: true, record_valid: true, truncated: false, records: [{ event_id: 'a'.repeat(64), stage: 'fetch', transport_status: null }] });
});
test('same serialized bounded proof is parsed without rereading signer result getters', async () => {
  let reads = 0;
  const f = await failing({ proof(event) { return Object.defineProperty({ ...event, sig: 'synthetic' }, 'id', { enumerable: true, get() { reads++; return 'b'.repeat(64); } }); } });
  await caught(f); assert.equal(reads, 1); assert.equal(snapshot(f).records[0].event_id, 'b'.repeat(64));
});
test('a reader facade and application import share one actual VM transport-module instance', async () => {
  const f = await failing(), bridgeSource = plugin().value.load('\0virtual:public-upload-refusal-reader');
  const reader = new vm.SourceTextModule(bridgeSource, { context: f.context });
  await reader.link(id => { assert.equal(id, transportPath); return f.module; }); await reader.evaluate();
  assert.equal(reader.namespace.readUploadRequestRefusals, f.api.readUploadRequestRefusals);
  await caught(f); assert.equal(reader.namespace.readUploadRequestRefusals().records.length, 1);
});
test('known200 body refusal is recorded synchronously before cleanup abort', async () => {
  let f, atAbort;
  f = await failing({ fetch: async (_url, init) => {
    init.signal.addEventListener('abort', () => { atAbort = snapshot(f); }, { once: true });
    return { status: 200, headers: new Headers({ 'content-type': 'application/json' }), body: { getReader: () => ({ async read() { throw Error('finite body'); }, async cancel() {}, releaseLock() {} }) } };
  } });
  const error = await caught(f); assert.equal(error.status, 200); assert.equal(error.outcome, 'response');
  assert.deepEqual(atAbort.records, [{ event_id: 'a'.repeat(64), stage: 'body-read', transport_status: 200 }]); assert.equal(f.requests[0].abortCount, 1);
});
test('getter returns copies and cannot erase or mutate private history', async () => {
  const f = await failing(); await caught(f); const first = f.api.readUploadRequestRefusals(); first.records[0].stage = 'other'; first.records.length = 0; first.record_valid = false;
  assert.equal(snapshot(f).records[0].stage, 'fetch'); assert.equal(snapshot(f).record_valid, true);
});
for (const id of [undefined, '', 'A'.repeat(64), 'a'.repeat(63), 12, [], {}]) test('dispatched noncanonical proof ID makes recording sticky unverified: ' + JSON.stringify(id), async () => {
  let attempt = 0; const f = await failing({ proof: event => ({ ...event, id: ++attempt === 1 ? id : 'c'.repeat(64) }) });
  await caught(f); assert.equal(snapshot(f).record_valid, false); assert.equal(snapshot(f).records.length, 0);
  await caught(f); assert.equal(snapshot(f).record_valid, false); assert.equal(snapshot(f).records.length, 1);
});
test('duplicate proof IDs are refused rather than silently deduplicated inside one ring', async () => {
  const f = await failing(); await caught(f); await caught(f); assert.equal(snapshot(f).record_valid, false); assert.equal(snapshot(f).records.length, 1);
});
test('sixteen immutable rows are bounded and overflow remains sticky', async () => {
  let id = 0; const f = await failing({ proof: event => ({ ...event, id: (++id).toString(16).padStart(64, '0') }) });
  for (let index = 0; index < 18; index++) await caught(f);
  const value = snapshot(f); assert.equal(value.records.length, 16); assert.equal(value.truncated, true); assert.equal(value.record_valid, true);
});
test('successful transport and unsent failures never enter the dispatched-refusal ring', async () => {
  const success = await fixture({ transportSource: enabledSource }); await success.request(); assert.equal(snapshot(success).records.length, 0);
  const unsent = await failing({ onSign() { throw Error('finite signing refusal'); } }); await caught(unsent); assert.equal(snapshot(unsent).records.length, 0); assert.equal(snapshot(unsent).record_valid, true);
  const oversized = await failing({ proof: event => ({ ...event, id: 'a'.repeat(64), padding: 'x'.repeat(20000) }) }); await caught(oversized); assert.equal(oversized.requests.length, 0); assert.equal(snapshot(oversized).record_valid, true);
});
test('recording failure preserves the genuine transport error/status/outcome, brand and abort cleanup', async () => {
  const changed = enabledSource.replace('  if (!uploadRequestRefusalRecording) return;', "  throw Error('Controlled diagnostic-only fault');");
  assert.notEqual(changed, enabledSource); const f = await failing({ transportSource: changed }); const error = await caught(f);
  assert.equal(error.outcome, 'uncertain'); assert.equal(error.status, undefined); assert.equal(f.api.uploadRequestRefusalStage(error), 'fetch');
  assert.equal(snapshot(f).record_valid, false); assert.equal(f.requests[0].abortCount, 1); assert.equal(f.timers.size, 0);
});

function chunk(fileName, modules, imports = [], extra = {}) { return { type: 'chunk', fileName, modules: Object.fromEntries(modules.map(id => [id, {}])), imports, dynamicImports: [], exports: [], isEntry: false, facadeModuleId: null, code: '/* finite Rollup output stand-in */', ...extra }; }
function graph() {
  const target = '_app/immutable/chunks/transport-finite.js', app = '_app/immutable/chunks/upload-finite.js', entry = '_app/immutable/entry/app-finite.js';
  return {
    [target]: chunk(target, [transportPath]), [app]: chunk(app, [clientPath], [target]),
    [entry]: chunk(entry, ['finite-app-entry'], [], { isEntry: true, dynamicImports: [app] }),
    [uploadRefusalReaderFile]: chunk(uploadRefusalReaderFile, ['\0virtual:public-upload-refusal-reader'], [target], { isEntry: true, exports: ['readUploadRequestRefusals'], facadeModuleId: '\0virtual:public-upload-refusal-reader' }),
  };
}
function emit(f, bundle = graph()) { const output = []; f.value.generateBundle.handler.call({ emitFile(row) { output.push(row); } }, {}, bundle); return JSON.parse(output[0].source); }
for (const value of [undefined, '0', '1']) test('explicit finite QA gate ' + value, () => assert.equal(readUploadRefusalQaGate(value), value === '1'));
for (const value of ['', 'true', 1, true, null]) test('unknown QA gate refuses ' + value, () => assert.throws(() => readUploadRefusalQaGate(value)));
test('QA build requires explicit public-upload gate', () => assert.throws(() => createUploadRefusalQaPlugin(root, { enabled: true, publicUploadEnabled: false })));
test('default-OFF/SSR keeps current native budgets, exact historical QA stripping and no reader', () => {
  const comparison = restoreReportTransportSource(readReportTransportAssociation().path, restoreModeratorTransportSource(readReportTransportAssociation().path, source, 'raw'), 'raw');
  const historical = restoreNativeTransportSource(comparison, 'raw');
  assert.equal(digest(stripUploadRefusalQaSource(historical)), '6b3e14bfdbe5b2dd518297a493534e8ce353ea2fa21aa0e5904bfbfa05f54e23');
  assert.equal(restoreNativeTransportSource(stripUploadRefusalQaSource(comparison), 'qa_stripped'), stripUploadRefusalQaSource(historical));
  for (const [enabled, ssr] of [[false, false], [true, true]]) { const f = plugin(enabled, ssr); assert.equal(f.emitted.length, 0); assert.equal(digest(f.code), digest(stripUploadRefusalQaSource(source))); f.value.generateBundle.handler.call({}, {}, {}); }
});
test('actual source composition emits only a sole-getter facade and records the shared fake graph provenance', () => {
  const f = plugin(), manifest = emit(f); assert.equal(f.emitted.length, 1); assert.equal(f.emitted[0].fileName, uploadRefusalReaderFile); assert.equal(f.emitted[0].preserveSignature, 'strict');
  assert.equal(f.value.load(f.emitted[0].id), `export { readUploadRequestRefusals } from ${JSON.stringify(transportPath)};`);
  assert.equal(manifest.transport_source_sha256, digest(source)); assert.equal(manifest.client_source_sha256, digest(fs.readFileSync(clientPath)));
  assert.equal(manifest.client_entry_chunks.length, 1); assert.equal(manifest.chunks.filter(row => row.contains_transport).length, 1);
  assert.equal(JSON.stringify(manifest).includes(root), false); assert.equal(manifest.reader_reaches_transport, true); assert.equal(manifest.entry_reaches_client, true);
});
test('strict Rollup re-export facades may contain zero rendered modules', () => {
  const b = graph(); b[uploadRefusalReaderFile].modules = {};
  const manifest = emit(plugin(), b); assert.equal(manifest.reader_module_count, 1);
  assert.equal(manifest.chunks.filter(row => row.contains_reader).length, 1);
});
for (const [name, change] of [
  ['same-chunk transport fragment alias', b => { b[Object.keys(b)[0]].modules[transportPath + '#copy'] = {}; }],
  ['duplicate canonical transport query alias', b => { b[Object.keys(b)[1]].modules[transportPath + '?alias'] = {}; }],
  ['missing transport', b => { delete b[Object.keys(b)[0]].modules[transportPath]; }],
  ['missing client', b => { delete b[Object.keys(b)[1]].modules[clientPath]; }],
  ['reader export widening', b => { b[uploadRefusalReaderFile].exports.push('write'); }],
  ['reader not statically connected', b => { b[uploadRefusalReaderFile].dynamicImports = b[uploadRefusalReaderFile].imports; b[uploadRefusalReaderFile].imports = []; }],
  ['client not connected', b => { b[Object.keys(b)[1]].imports = []; }],
  ['entry cannot reach client', b => { b[Object.keys(b)[2]].dynamicImports = []; }],
  ['missing app entry', b => { b[Object.keys(b)[2]].isEntry = false; }],
  ['unsafe emitted chunk path', b => { b[Object.keys(b)[0]].fileName = '../outside.js'; }],
  ['reader facade mismatch', b => { b[uploadRefusalReaderFile].facadeModuleId = 'other'; }],
]) test('closed actual-graph validator refuses ' + name, () => { const b = graph(); change(b); assert.throws(() => emit(plugin(), b)); });
test('OFF build rejects QA reader/graph artifacts and QA marker code', () => {
  for (const fileName of [uploadRefusalReaderFile, uploadRefusalGraphFile]) assert.throws(() => plugin(false).value.generateBundle.handler({}, { test: { type: 'asset', fileName } }));
  const b = graph(); delete b[uploadRefusalReaderFile]; b[Object.keys(b)[0]].code = 'readUploadRequestRefusals'; assert.throws(() => plugin(false).value.generateBundle.handler({}, b));
});
test('query aliases, prior transformation and changed transport source refuse before build output', () => {
  const f = plugin(); assert.throws(() => f.value.transform(source, transportPath)); assert.throws(() => plugin().value.transform(source, transportPath + '#copy')); assert.throws(() => plugin().value.transform(source, transportPath + '?other')); assert.throws(() => plugin().value.transform(source + ' ', transportPath));
});

test('QA graph hashes are captured in an explicit post hook after normal code rewrites', () => {
  const f = plugin(), b = graph(); assert.equal(f.value.generateBundle.order, 'post');
  const app = Object.keys(b)[1]; b[app].code += '\n/* controlled preceding Vite preload rewrite */';
  const manifest = emit(f, b); assert.equal(manifest.chunks.find(row => row.file_name === app).sha256, digest(b[app].code));
});

for (const trigger of ['deadline', 'external-signal']) test('an earlier abort can precede catch recording, then the fixed ring captures it: ' + trigger, async () => {
  const entered = deferred(), body = deferred(), external = new AbortController(); let f, atAbort;
  f = await failing({ fetch: async (_url, init) => {
    init.signal.addEventListener('abort', () => { atAbort = snapshot(f); }, { once: true });
    return { status: 200, headers: new Headers({ 'content-type': 'application/json' }), body: { getReader: () => ({
      read() { entered.resolve(); return body.promise; }, async cancel() { body.resolve({ done: true }); }, releaseLock() {},
    }) } };
  } });
  const pending = f.request({ timeoutMs: 10, signal: external.signal }); pending.catch(() => {}); await entered.promise;
  if (trigger === 'deadline') f.advance(10); else external.abort();
  assert.deepEqual(atAbort.records, [], 'The initiating abort occurs before the rejection reaches catch');
  await assert.rejects(pending, error => error.name === 'PublicUploadRequestError' && error.status === 200 && error.outcome === 'response');
  assert.deepEqual(snapshot(f).records, [{ event_id: 'a'.repeat(64), stage: 'body-read', transport_status: 200 }]);
  assert.equal(f.requests[0].abortCount, 1); assert.equal(f.timers.size, 0);
});
