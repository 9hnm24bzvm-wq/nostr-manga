// Source-only reconstruction seams. No Svelte runtime, signer, HTTP, socket or DB.
import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { createHash } from 'node:crypto';
import { fileURLToPath } from 'node:url';
import { applyPublicUploadOverlay, createPublicUploadOverlays, publicUploadOverlayManifest } from '../../platform-integrations/public-api/upload-overlays.mjs';
import { readPublicApiGate } from '../../platform-integrations/public-api/overlays.mjs';
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const frontend = path.join(root, 'frontend');
const hash = value => createHash('sha256').update(value).digest('hex');
const spec = JSON.parse(fs.readFileSync(path.join(root, 'platform-integrations/public-api/upload-anchors.json'), 'utf8'));
const nativeText = fs.readFileSync(path.join(root, 'platform-integrations/frontend-overlays.mjs'), 'utf8');
const native = JSON.parse(nativeText.slice(nativeText.indexOf('const patches = [') + 'const patches = '.length, nativeText.indexOf('\n];\nconst hash') + 2));
function compose(relative) {
  let code = fs.readFileSync(path.join(frontend, relative), 'utf8');
  for (const patch of native.filter(patch => patch.path === relative)) {
    assert.equal(hash(fs.readFileSync(path.join(frontend, relative))), patch.sha256);
    assert.equal(code.split(patch.before).length - 1, patch.count, patch.label);
    code = code.split(patch.before).join(patch.after);
  }
  return code;
}
const codes = new Map();
for (const file of spec.files) codes.set(file.path, applyPublicUploadOverlay(compose(file.path), file.path));
const main = codes.get('src/lib/components/upload/UploadChapter.svelte');
const pages = codes.get('src/lib/components/upload/UploadChapterPages.svelte');
const entry = codes.get('src/lib/components/upload/UploadChapterPagesEntry.svelte');
const route = codes.get('src/routes/(app)/title/[id]/upload/+page.svelte');
const edit = codes.get('src/routes/(app)/chapter/[id]/edit/+page.svelte');

test('public upload uses the existing explicit selected-instance default-off flag', () => {
  assert.equal(readPublicApiGate('0'), false); assert.equal(readPublicApiGate('1'), true);
  assert.throws(() => readPublicApiGate('true')); assert.throws(() => createPublicUploadOverlays(frontend, { enabled: '1' }));
  const before = process.env.PUBLIC_MANGADEX_CATALOGUE; delete process.env.PUBLIC_MANGADEX_CATALOGUE;
  try { assert.equal(readPublicApiGate(), false); assert.equal(createPublicUploadOverlays('/does-not-exist').transform.handler('ignored','/does-not-exist/src/lib/components/upload/UploadChapter.svelte'), null); }
  finally { if (before === undefined) delete process.env.PUBLIC_MANGADEX_CATALOGUE; else process.env.PUBLIC_MANGADEX_CATALOGUE = before; }
});
for (const file of spec.files) test(`exact original/composed/output bytes: ${file.path}`, () => {
  assert.equal(hash(fs.readFileSync(path.join(frontend, file.path))), file.source_sha256);
  const input = compose(file.path); assert.equal(hash(input), file.composed_sha256);
  const result = createPublicUploadOverlays(frontend, { enabled: true }).transform.handler(input, path.join(frontend, file.path));
  assert.equal(hash(result.code), file.output_sha256); assert.equal(result.map, null);
  assert.throws(() => applyPublicUploadOverlay(input + ' ', file.path), /composed source changed/);
});
test('query virtual modules and unrelated paths are never transformed', () => {
  const plugin = createPublicUploadOverlays(frontend, { enabled: true });
  assert.equal(plugin.transform.handler('bad', path.join(frontend,spec.files[0].path)+'?svelte&type=style'), null);
  assert.equal(plugin.transform.handler('bad',path.join(frontend,'src/unrelated.svelte')),null);
  assert.equal(applyPublicUploadOverlay('unrelated','src/unrelated.svelte'),null);
});
test('Vite composes signed upload only after original source and catalogue overlays', () => {
  const config = fs.readFileSync(path.join(root,'scripts/vite.platform.config.mjs'),'utf8');
  assert.match(config, /createFrontendOverlays\(frontend\).*createPublicApiOverlays\(frontend\), createPublicUploadOverlays\(frontend\)/);
});
test('public route uses current selected signer and actual bound account instead of native account lookup', () => {
  assert.match(route,/loadUploadAccount\(assertLookup\)/); assert.match(route,/signerGeneration !== \$signerIdentity.generation/);
  assert.doesNotMatch(route,/\$Session|API\.User\.Get|X-WeebDex|setWithCredentials/);
  assert.match(route,/Connect a signing identity/); assert.match(route,/Admission and upload permissions are checked separately/);
  for (const file of ['src/routes/(app)/title/[id]/upload/+layout.svelte','src/routes/(app)/chapter/[id]/edit/+layout.svelte']) assert.doesNotMatch(codes.get(file), /ProtectedVerified|\$Session|Permission\./);
});
test('normal begin and file deletion retain actual public200 statuses', () => {
  assert.doesNotMatch(main,/StatusCodes.CREATED/); assert.doesNotMatch(pages,/StatusCodes.NO_CONTENT/); assert.doesNotMatch(entry,/StatusCodes.NO_CONTENT/);
  assert.match(main,/snapshot\(res.body.draft \?\? \{\}, res.body.relationships.files \?\? \[\]\)/);
  assert.match(main,/every\(e => isRemoteUploadFile\(e\)\)/); assert.match(main,/files.some\(file => !isRemoteUploadFile\(file\)\)/);
});
test('single real owner is preserved while selection guards cross asynchronous file work', () => {
  assert.match(pages,/runUploadBatch\(session, chunk, owner, onProgress, \(\) => assertFileSession\(session\)\)/);
  for (const code of [main,pages,entry]) assert.doesNotMatch(code,/\$platform\/(?:upload-batch|upload-resume|target-owner)|new HTTP|setWithCredentials|PUBLIC_API_URL/);
});
test('unsupported edits and replacement refuse before native transport', () => {
  assert.match(edit,/Public signed chapter editing is unavailable/); assert.doesNotMatch(edit,/UploadChapterEdit|owner.request|API\./);
  const replacement = pages.slice(pages.indexOf('const replaceFile ='),pages.indexOf('const inspectImageDimensions'));
  assert.match(replacement,/Page replacement unavailable/); assert.doesNotMatch(replacement,/request\(|HTTP|FormData|fetch\(/);
});
test('approval decision precedes commit and uses the actual response boolean', () => {
  assert.ok(main.indexOf('const approvalRequired = await isPending()') < main.indexOf('const res = await owner.request<Chapter>(`/upload/${sessionId}/commit`'));
  assert.match(main,/termsAccepted: isTemsAccepted/); assert.match(main,/return res.body.requiresApproval/); assert.doesNotMatch(main,/else if \(await isPending\(\)\)/);
});
test('signed preview is exact file/context scoped, independently byte checked by adapter, and cleaned on unmount', () => {
  assert.match(entry,/uploadPreview\(owner, context.sessionId, page\)/); assert.match(entry,/page.file_hash === context.fileHash/);
  assert.match(entry,/Number\(page.file_size\) === context.fileSize/); assert.match(entry,/onDestroy\(\(\) => \{ previewContext = null; previewLease.dispose\(\); \}\)/);
  assert.match(entry,/page.thumbnail\?\.startsWith\("blob:"\)/); assert.match(entry,/ownership: "transferred"/); assert.match(entry,/\{#key page.thumbnail\}/);
  assert.match(entry,/role="status"/); assert.match(entry,/revokeUploadPreviewUrl\(page.thumbnail\)/);
  assert.match(pages,/revokeUploadPreviewUrl\(page.thumbnail\)/);
});
function fenceHarness(state) {
  const start = main.indexOf('const showSignedCommitFence = () => {') + 'const showSignedCommitFence = () => {'.length;
  const end = main.indexOf('\r\n\t};',start);
  const model = { uploadSession:{id:'exact',can_mutate:true,can_cancel:true,can_retry_commit:true},chapter:{id:'stale'},isBeginAnotherUploadSession:true,signedReceiptPending:false,message:undefined,isShowMessage:false };
  const run = new Function('owner','model',`let {uploadSession,chapter,isBeginAnotherUploadSession,signedReceiptPending,message,isShowMessage}=model;const apply=()=>{${main.slice(start,end)}}; const result=apply();Object.assign(model,{chapter,isBeginAnotherUploadSession,signedReceiptPending,message,isShowMessage});return result;`);
  return { model, result:run({commitState:()=>state},model) };
}
for (const state of ['uncertain','accepted-unavailable']) test(`exact current owner ${state} clears stale presentation and fences all local mutation affordances`, () => {
  const {model,result}=fenceHarness(state);assert.equal(result,true);assert.equal(model.uploadSession.id,'exact');
  for(const key of ['can_mutate','can_cancel','can_retry_commit'])assert.equal(model.uploadSession[key],false);
  assert.equal(model.chapter,undefined);assert.equal(model.isBeginAnotherUploadSession,false);assert.equal(model.signedReceiptPending,true);assert.equal(model.isShowMessage,true);
  assert.match(model.message.description,/read-only/);
});
test('no unresolved owner state does not fake an accepted state',()=>{const {model,result}=fenceHarness(undefined);assert.equal(result,false);assert.equal(model.chapter.id,'stale');assert.equal(model.signedReceiptPending,false);});
test('receipt recovery is explicit exact-session read with no implicit replay',()=>{
  const source=main.slice(main.indexOf('const checkSignedUploadReceipt ='), main.indexOf('const reconcileUploadFailure ='));
  assert.match(source,/owner.pendingCommitSession\(\)/);assert.match(source,/reconcileUploadReceipt\(owner, sessionId\)/);assert.doesNotMatch(source,/commit["'`]|"POST"|createUploadSession\(/);
  assert.match(main,/onclick=\{checkSignedUploadReceipt\}/);assert.match(main,/if \(showSignedCommitFence\(\)\) return;/);
});
test('manifest closes the exact integration files and original source remains untouched',()=>{
  assert.equal(publicUploadOverlayManifest.length,8);assert.equal(new Set(spec.files.map(row=>row.path)).size,8);
  for(const file of spec.files)assert.equal(hash(fs.readFileSync(path.join(frontend,file.path))),file.source_sha256);
});

test('read-only session flags disable metadata and terms while dispatch is in flight',()=>{
  assert.match(main,/disabled=\{\$mutex \|\| !uploadSession \|\| uploadSession.can_mutate === false\}/);
  assert.match(main,/<Checkbox id="terms" bind:checked=\{isTemsAccepted\} disabled=\{\$mutex \|\| uploadSession.can_mutate === false\}/);
});

test('locked-group affordances use actual bound roles and never cookie identity',()=>{
  const groups=codes.get('src/lib/components/upload/UploadChapterGroups.svelte');
  assert.doesNotMatch(groups,/Session|\$Session/);
  assert.equal((groups.match(/some\(role => user.roles\?\.includes\(role\)\)/g)??[]).length,2);
  assert.match(groups,/API.Group.Search/);
});
