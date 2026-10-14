#!/usr/bin/env node
// Real composed admission/decoder/replace/drop functions and the retained Mutex.
// Svelte bindings are live accessor stand-ins; image decode, transport and owner
// services are explicit stand-ins. This is not image/browser/network acceptance.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import vm from 'node:vm';
import { createRequire } from 'node:module';
import { fileURLToPath, pathToFileURL } from 'node:url';
import test from 'node:test';
import { createFrontendOverlays } from '../../platform-integrations/frontend-overlays.mjs';
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const frontend = path.join(root, 'frontend'), relative = 'src/lib/components/upload/UploadChapterPages.svelte';
const file = path.join(frontend, relative), original = fs.readFileSync(file, 'utf8');
const composed = createFrontendOverlays(frontend).transform.handler(original, file).code;
const dependencies = process.env.WEEBDEX_FRONTEND_ROOT;
assert(dependencies, 'Set WEEBDEX_FRONTEND_ROOT to the verified exact-lock dependency tree');
const require = createRequire(path.join(dependencies, 'package.json')), ts = require('typescript'), { compile } = require('svelte/compiler');
const { get, writable } = await import(pathToFileURL(path.join(dependencies, 'node_modules/svelte/src/store/shared/index.js')));
const emit = source => ts.transpileModule(source, { compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.None } }).outputText;
const instance = composed.match(/<script lang="ts">([\s\S]*?)<\/script>/)[1];
function declarations(source, names) {
  const ast = ts.createSourceFile('seam.ts', source, ts.ScriptTarget.Latest, true, ts.ScriptKind.TS);
  const found = ast.statements.filter(row => ts.isVariableStatement(row) && row.declarationList.declarations.some(decl => names.includes(decl.name.getText(ast))));
  assert.equal(found.reduce((n, row) => n + row.declarationList.declarations.length, 0), names.length);
  return found.map(row => row.getText(ast)).join('\n');
}
const functions = declarations(instance, ['assertFileSession','fileSession','targetFile','replaceFile','inspectImageDimensions','decodeUploadImage','processFiles','scanFiles','onInputChange','onDrop','createPage','revokeThumbnail','uploadChunk','progress']);
const moduleScript = composed.match(/<script lang="ts" module>([\s\S]*?)<\/script>/)[1];
const mutexCode = fs.readFileSync(path.join(frontend, 'src/lib/Mutex.ts'), 'utf8').replace(/^import[^\n]+\n/, '').replace('export default Mutex;', 'globalThis.Mutex = Mutex;');
class VisibleError extends Error { constructor(title, body) { super(title); this.title = title; this.body = body; } }
function fixture({ busy = false, frozen = false, external = false, imageError = false, actualBatch = false } = {}) {
  let session = { id: 'OwnedStage', can_mutate: !frozen, version: 1, relationships: { files: [] } }, pages = [], alive = true;
  const calls = { batches: 0, replacements: 0, reads: 0, hashes: 0, decoded: 0, revoked: [], feedback: [], unknown: [] }, held = [];
  let holdDecode = false, batchFailure, readGate, releaseRead, hashGate, releaseHash;
  const context = vm.createContext({ console: { error: error => calls.unknown.push(error) }, crypto: { randomUUID: () => crypto.randomUUID(), subtle: { async digest(...args) { calls.hashes++; if (hashGate) await hashGate; return crypto.subtle.digest(...args); } } }, File, Blob, FormData, Uint8Array, DataView, ArrayBuffer, Symbol, Map, URL: { createObjectURL: () => 'blob:fixture', revokeObjectURL: value => calls.revoked.push(value) },
    setTimeout: () => 0, window: { setTimeout: () => 0, clearTimeout() {} }, get, writable,
    $state: value => value, Constants: { Error: { Error: VisibleError, Unknown: { title: 'Unknown', body: 'Unknown' } } },
    StatusCodes: { OK: 200 }, PUBLIC_API_URL: 'http://127.0.0.1:1/api', PUBLIC_UPLOAD_URL: 'http://127.0.0.1:1/uploads', VisibleError,
    currentUpload: async owner => (await owner.request('/upload', 'GET')).body,
    toast: { error: (...args) => calls.feedback.push(args), success: (...args) => calls.feedback.push(args) },
    Image: class { width = 2; height = 2; set src(value) { if (!value) return; calls.decoded++; const complete = () => imageError ? this.onerror?.() : this.onload?.(); if (holdDecode) held.push(complete); else queueMicrotask(complete); } },
    runUploadBatch: async (_session, chunk) => { calls.batches++; if (batchFailure) throw batchFailure; return chunk.map(row => ({ id: row.id, original_file_name: row.file.name, file_hash: 'a'.repeat(64), file_size: String(row.file.size), thumbnail: row.thumbnail })); },
    resolveUploadPage: async (_session, target) => target, markUnsentUploadPages() {},
    owner: { actor: 'SyntheticOwner', alive: () => alive, assert() { if (!alive) throw Error('owner expired'); }, async request() { calls.reads++; const snapshot = { ...session, relationships: { files: [...session.relationships.files] } }; if (readGate) await readGate; return { status: 200, body: snapshot }; } },
    HTTP: class { constructor(url, form) { this.url = url; this.form = form; } setWithCredentials() {} setRequestHeader() {} addUploadListener() {} async POST() {
      if (this.url.endsWith('/replace')) { calls.replacements++; return { status: 200, body: { ...pages[0], original_file_name: 'replacement.png', version: 2 } }; }
      calls.batches++; return { status: 200, body: this.form.getAll('file').map((file, index) => ({ id: 'RemotePag' + index, file_hash: 'a'.repeat(64), file_name: 'remote.png', original_file_name: file.name, file_size: String(file.size) })) };
    } },
  });
  Object.defineProperties(context, { uploadSession: { get: () => session }, pages: { get: () => pages, set: value => { pages = value; } }, disabled: { get: () => external || context.mutex.isLocked() || session?.can_mutate === false } });
  vm.runInContext(emit(mutexCode), context); context.mutex = new context.Mutex(); if (busy) context.mutex.lock();
  if (actualBatch) {
    const batch = fs.readFileSync(path.join(root, 'platform-integrations/frontend-behavior/upload-batch.ts'), 'utf8').replace(/^import[^\n]*\n/gm, '').replace(/^export const \[getUploadOwner[^\n]*\n/m, '').replace(/\bexport /g, '');
    vm.runInContext(emit(batch), context);
  }
  vm.runInContext(emit(moduleScript + '\n' + functions + '\nglobalThis.controls = {onInputChange,onDrop,replaceFile,processFiles,setTarget: value => {targetFile = value;}};'), context);
  const input = files => { const target = { files, value: 'fixture' }; return { target, run: () => context.controls.onInputChange({ target }) }; };
  return { context, calls, input, get pages() { return pages; }, set pages(value) { pages = value; }, get session() { return session; }, switchSession() { session = { ...session, id: 'NewStage' }; }, expire() { alive = false; }, hold() { holdDecode = true; }, release() { held.splice(0).forEach(done => done()); }, failBatch(error) { batchFailure = error; }, holdRead() { readGate = new Promise(resolve => { releaseRead = resolve; }); }, releaseRead() { releaseRead(); }, holdHash() { hashGate = new Promise(resolve => { releaseHash = resolve; }); }, releaseHash() { releaseHash(); } };
}
function image(name = 'page.png') { const bytes = new Uint8Array(24); bytes.set([137,80,78,71,13,10,26,10]); new DataView(bytes.buffer).setUint32(16,2); new DataView(bytes.buffer).setUint32(20,2); return new File([bytes], name, { type: 'image/png' }); }
const turn = () => new Promise(resolve => setImmediate(resolve));
test('composed upload scripts compile; inherited disabled is a live shared-mutex expression', () => {
  assert.deepEqual(compile(composed, { filename: file, generate: 'client' }).warnings, []);
  const parent = fs.readFileSync(path.join(frontend, 'src/lib/components/upload/UploadChapter.svelte'), 'utf8');
  assert.match(parent, /disabled=\{\$mutex \|\| !uploadSession\}/);
  assert.match(instance, /const disabled = \$derived\(propDisabled \|\| uploadSession\?\.can_mutate === false\)/);
  assert.match(compile(composed, { filename: file, generate: 'client' }).js.code, /\$\$props.disabled/);
  assert.doesNotMatch(functions, /can_mutate === false \|\| disabled/);
  // A dynamic inherited disabled value changes synchronously when real Mutex
  // locks, reproducing the old return before decode without a stale rune stub.
  const value = fixture(); assert.equal(value.context.disabled, false); value.context.mutex.lock(); assert.equal(value.context.disabled, true);
});
for (const gesture of ['input','drop']) test(gesture + ' admits before locking, uploads once and releases its mutex', async () => {
  const value = fixture();
  if (gesture === 'input') { const input = value.input([image()]); await input.run(); assert.equal(input.target.value, ''); }
  else await value.context.controls.onDrop({ preventDefault() {}, dataTransfer: { items: [], files: [image()] } });
  assert.equal(value.calls.decoded, 1); assert.equal(value.calls.batches, 1); assert.equal(value.pages.length, 1); assert.equal(value.context.mutex.isLocked(), false); assert.deepEqual(value.calls.feedback, []);
});
for (const kind of ['busy','frozen','external']) test(kind + ' rejects add/replace/drop before decoding or mutation', async () => {
  const value = fixture({ [kind]: true });
  const old = { id: 'OldPage', file_hash: 'b'.repeat(64) }; value.pages = [old]; value.context.controls.setTarget(old);
  const input = value.input([image()]); await input.run();
  await value.context.controls.onDrop({ preventDefault() {}, dataTransfer: { items: [], files: [image()] } });
  assert.equal(input.target.value, ''); assert.equal(value.calls.decoded, 0); assert.equal(value.calls.batches, 0); assert.equal(value.calls.replacements, 0); assert.equal(value.pages[0], old);
});
test('concurrent gestures cannot decode or upload a second selection', async () => {
  const value = fixture(); value.hold(); const first = value.input([image()]).run(); await turn();
  assert.equal(value.context.mutex.isLocked(), true); await value.input([image('other.png')]).run(); value.release(); await first;
  assert.equal(value.calls.decoded, 1); assert.equal(value.calls.batches, 1); assert.equal(value.context.mutex.isLocked(), false);
});
for (const interruption of ['switch','owner','freeze']) for (const operation of ['add','replace']) test(operation + ' discards selection after ' + interruption + ' during decode', async () => {
  const value = fixture(); const old = { id: 'OldPage', file_hash: 'b'.repeat(64), file_size: '24' }; if (operation === 'replace') { value.pages = [old]; value.session.relationships.files = [old]; value.context.controls.setTarget(old); }
  value.hold(); const pending = value.input([image()]).run(); await turn();
  if (interruption === 'switch') value.switchSession(); else if (interruption === 'owner') value.expire(); else value.session.can_mutate = false;
  value.release(); await pending;
  assert.equal(value.calls.batches, 0); assert.equal(value.calls.replacements, 0); assert.equal(value.pages.length, operation === 'replace' ? 1 : 0); if (operation === 'replace') assert.equal(value.pages[0], old);
  assert.equal(value.calls.feedback.length, interruption === 'freeze' ? 1 : 0); assert.equal(value.context.mutex.isLocked(), false);
});
test('real corrupt image/archive admission produces honest feedback and no candidate', async () => {
  const value = fixture();
  await value.input([new File(['not an image'], 'broken.png', { type: 'image/png' })]).run();
  await value.input([new File(['not an archive'], 'broken.zip', { type: 'application/zip' })]).run();
  assert.match(value.calls.feedback[0][1].description, /Could not decode image/); assert.match(value.calls.feedback[1][1].description, /Failed to extract archive/);
  assert.equal(value.calls.batches, 0); assert.equal(value.pages.length, 0); assert.equal(value.context.mutex.isLocked(), false);
});
test('replacement admission preserves old bytes on corrupt input, then invokes one atomic replacement', async () => {
  const value = fixture(), old = { id: 'OldPage', file_hash: 'b'.repeat(64), file_size: '24', thumbnail: 'blob:old' };
  value.pages = [old]; value.session.relationships.files = [old]; value.context.controls.setTarget(old);
  await value.input([new File(['bad'], 'broken.png', { type: 'image/png' })]).run();
  assert.equal(value.pages[0], old); assert.equal(value.calls.replacements, 0); assert.match(value.calls.feedback[0][1].description, /Existing page is retained/);
  value.context.controls.setTarget(old); await value.input([image('replacement.png')]).run();
  assert.equal(value.calls.replacements, 1); assert.equal(value.calls.batches, 0); assert.equal(value.pages[0].id, old.id); assert.equal(value.session.version, 2);
});
test('genuine decoder and batch failures retain feedback and release controls', async () => {
  const decode = fixture({ imageError: true }); await decode.input([image()]).run(); assert.match(decode.calls.feedback[0][1].description, /Could not decode image/); assert.equal(decode.context.mutex.isLocked(), false);
  const batch = fixture(), failure = new VisibleError('Upload was not confirmed','Outcome unknown'); batch.failBatch(failure); await batch.input([image()]).run();
  assert.equal(batch.calls.batches, 1); assert.equal(batch.calls.feedback[0][0], failure.title); assert.equal(batch.context.mutex.isLocked(), false);
});

for (const boundary of ['read', 'hash']) for (const interruption of ['switch', 'freeze', 'owner']) for (const gesture of ['input','drop']) test('actual batch ' + gesture + ' blocks POST after ' + interruption + ' during ' + boundary, async () => {
  const value = fixture({ actualBatch: true });
  if (boundary === 'read') value.holdRead(); else value.holdHash();
  const pending = gesture === 'input' ? value.input([image()]).run() : value.context.controls.onDrop({ preventDefault() {}, dataTransfer: { items: [], files: [image()] } });
  await turn(); assert.equal(value.calls.reads, 1); if (boundary === 'hash') assert.equal(value.calls.hashes, 1); assert.equal(value.calls.batches, 0);
  if (interruption === 'switch') value.switchSession(); else if (interruption === 'freeze') value.session.can_mutate = false; else value.expire();
  if (boundary === 'read') value.releaseRead(); else value.releaseHash();
  await pending;
  assert.equal(value.calls.batches, 0, 'The actual non-idempotent batch POST must never start');
  assert.equal(value.context.mutex.isLocked(), false); assert.equal(value.calls.feedback.length, interruption === 'freeze' ? 1 : 0);
});
test('actual batch keeps its genuine final read/hash/POST sequence with captured staging', async () => {
  const value = fixture({ actualBatch: true }); await value.input([image()]).run();
  assert.equal(value.calls.reads, 1); assert.equal(value.calls.batches, 1, JSON.stringify({feedback:value.calls.feedback,unknown:value.calls.unknown.map(String)})); assert.equal(value.pages[0].id, 'RemotePag0'); assert.equal(value.pages[0].file, undefined); assert.equal(value.context.mutex.isLocked(), false);
});

for (const boundary of ['read', 'hash']) for (const interruption of ['switch','freeze','owner']) test('atomic replacement blocks POST after ' + interruption + ' during ' + boundary, async () => {
  const value = fixture(), old = { id: 'OldPage', file_hash: 'b'.repeat(64), file_size: '24' }; value.pages = [old]; value.session.relationships.files = [old]; value.context.controls.setTarget(old);
  if (boundary === 'read') value.holdRead(); else value.holdHash();
  const pending = value.input([image('replacement.png')]).run(); await turn();
  assert.equal(value.calls.reads,1); if (boundary === 'hash') assert.equal(value.calls.hashes,1);
  if (interruption === 'switch') value.switchSession(); else if (interruption === 'freeze') value.session.can_mutate = false; else value.expire();
  if (boundary === 'read') value.releaseRead(); else value.releaseHash(); await pending;
  assert.equal(value.calls.replacements,0); assert.equal(value.pages[0],old); assert.equal(value.context.mutex.isLocked(),false); assert.equal(value.calls.feedback.length,interruption === 'freeze' ? 1 : 0);
});
test('folder scan cannot admit files into newer staging after its asynchronous entry callback', async () => {
  const value = fixture(); let finish;
  const entry = { isFile:true, file(resolve) { finish = resolve; } };
  const pending = value.context.controls.onDrop({ preventDefault() {}, dataTransfer: { items: [{ webkitGetAsEntry: () => entry }] } });
  await turn(); value.switchSession(); finish(image()); await pending;
  assert.equal(value.calls.decoded,0); assert.equal(value.calls.batches,0); assert.equal(value.pages.length,0); assert.equal(value.calls.feedback.length,0); assert.equal(value.context.mutex.isLocked(),false);
});

test('replacement uses its nonoptional captured staging only after exact live-session guards', () => {
  const replacement = declarations(instance,['replaceFile']);
  assert.match(replacement,/file: File, session: UploadSession/);
  assert.doesNotMatch(replacement,/uploadSession\.(?:id|version)/);
  assert.equal((replacement.match(/session\.id/g) ?? []).length,3);
  assert.match(replacement,/session\.version = version/);
  assert.match(replacement,/await owner.request<UploadSession>\("\/upload", "GET"\); assertFileSession\(session\)/);
  assert.match(replacement,/const result = await http.POST\(\); assertFileSession\(session\)/);
  assert.match(replacement,/catch\(\(\) => undefined\); assertFileSession\(session\)/);
  assert.doesNotMatch(replacement,/\bas any\b|@ts-ignore|@ts-expect-error|uploadSession!/);
});
