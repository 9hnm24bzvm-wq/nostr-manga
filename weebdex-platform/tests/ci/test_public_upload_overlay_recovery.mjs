// Source-only reconstruction seams. No Svelte runtime, signer, HTTP, socket or DB.
import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { createHash } from 'node:crypto';
import { fileURLToPath } from 'node:url';
import { stripTypeScriptTypes } from 'node:module';
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
  assert.match(route,/let user = \$state\.raw<User>\(\);/); assert.match(route,/loadUploadAccount\(assertLookup\)/); assert.match(route,/signerGeneration !== \$signerIdentity.generation/);
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
test('chapter editing uses the signed review-first editor while page replacement uses only its public helper', () => {
  assert.match(edit,/PublicChapterEditor/); assert.doesNotMatch(edit,/UploadChapterEdit|API\./);
  const replacement = pages.slice(pages.indexOf('const replaceFile ='),pages.indexOf('const inspectImageDimensions'));
  assert.match(replacement,/replaceUploadPage\(session, target, file, owner, current\)/); assert.doesNotMatch(replacement,/request\(|HTTP|FormData|fetch\(/);
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

test('explicit accepted empty-body200 or202 is handled without inventing a chapter or refusal toast',()=>{
  const line=main.split('\n').find(line=>line.includes('res.accepted === true && !res.body'));
  assert.ok(line); const condition=line.slice(line.indexOf('(')+1,line.lastIndexOf(')'));
  const accepts=new Function('res','StatusCodes',`return (${condition});`);
  for(const status of [200,202])assert.equal(accepts({status,accepted:true,body:undefined},{OK:200,ACCEPTED:202}),true);
  for(const response of [{status:200,body:undefined},{status:202,accepted:true,body:{id:'raw'}},{status:403,accepted:true}])assert.equal(accepts(response,{OK:200,ACCEPTED:202}),false);
  assert.match(main,/if \(owner.commitState\(\)\) return;/);
  assert.match(main,/Upload request accepted/);assert.doesNotMatch(main,/The saved publication is accepted, but/);
});

test('mount receipt-read failure exposes the persisted exact-session recovery control',()=>{
  const show=main.slice(main.indexOf('const showResumeError ='),main.indexOf('const restoreUpload ='));
  assert.match(show,/if \(showSignedCommitFence\(\)\) return;/);
  assert.ok(show.indexOf('showSignedCommitFence()') < show.indexOf('toast.error'));
});


// Execute only these extracted local UI functions with controlled seam inputs.
// Actual signer/HTTP/DB/Svelte rendering remains separate qualification.
function replacementUi(options = {}) {
  const old = { id:'stable-old',file_hash:'a'.repeat(64),file_size:'12',original_file_name:'old.png',thumbnail:'blob:old-owned' };
  const neighbour = { id:'unchanged-neighbour',file_hash:'c'.repeat(64),file_size:'12',thumbnail:'blob:neighbour' };
  const session = { id:'exact-session',version:2,can_mutate:true,can_cancel:true,can_retry_commit:true };
  const calls = { helper:0,created:[],revoked:[],acknowledged:[],checks:0,toasts:[] };
  const model = { pages:[old,neighbour],targetFile:old,uploadSession:session,...options.model };
  class Visible extends Error { constructor(title,body) { super(title);this.title=title;this.body=body; } }
  const start=pages.indexOf('const keepReplacementFence ='),end=pages.indexOf('const inspectImageDimensions =',start);
  assert.ok(start>=0&&end>start);
  const extracted=stripTypeScriptTypes(pages.slice(start,end),{mode:'transform'});
  const wrapper=new Function('env',`with(env){${extracted};return {replaceFile,checkSignedReplacement,adoptReplacement};}`);
  const context={...model,owner:{assert(){calls.checks++;options.check?.(model);},alive:()=>true,pendingReplacement:()=>options.pending?.()},Constants:{Error:{Error:Visible,Unknown:new Visible('Unknown','Unconfirmed')}},
    URL:{createObjectURL:file=>{calls.created.push(file);return 'blob:new-owned';}},revokeThumbnail:page=>calls.revoked.push(page.thumbnail),
    acknowledge:(current,saved)=>{calls.acknowledged.push(saved.version);Object.assign(current,{version:saved.version,can_mutate:saved.can_mutate,can_cancel:saved.can_cancel,can_retry_commit:saved.can_retry_commit});},
    assertFileSession:current=>{if(current!==context.uploadSession||current.can_mutate===false)throw new Visible('Upload changed','Old selection');},
    imageMimeTypes:['image/png'],maxFileSize:25*1024*1024,maxImageWidth:10000,maxImageHeight:10000,maxSessionFiles:500,maxSessionSize:500*1024*1024,
    decodeUploadImage:async file=>options.decode?options.decode(file):({width:2,height:3}),
    replaceUploadPage:async(...args)=>{calls.helper++;return options.replace?options.replace(...args):{session:{...session,version:3},file:{...old,file_hash:'b'.repeat(64),original_file_name:'new.png'}};},
    reconcileUploadReplacement:async(...args)=>{calls.helper++;return options.reconcile(...args);},mutex:{wait:callback=>callback()},
    createPage(){throw Error('Not a local-placeholder seam');},uploadChunk(){throw Error('Unexpected upload replay');},toast:{success:(...args)=>calls.toasts.push(args),error:(...args)=>calls.toasts.push(args)},...options.context};
  const api=wrapper(context); return {api,context,calls,old,neighbour,session,file:{name:'new.png',type:'image/png',size:12}};
}
function deferred(){let resolve,reject;const promise=new Promise((yes,no)=>{resolve=yes;reject=no;});return {promise,resolve,reject};}
test('remote replacement keeps its original row and owned preview until the exact helper confirmation',async()=>{
  const held=deferred(),fixture=replacementUi({replace:()=>held.promise});const pending=fixture.api.replaceFile(fixture.file,fixture.session);
  await Promise.resolve();assert.equal(fixture.calls.helper,1);assert.equal(fixture.context.pages[0],fixture.old);assert.deepEqual(fixture.calls.created,[]);assert.deepEqual(fixture.calls.revoked,[]);
  held.resolve({session:{...fixture.session,version:3},file:{...fixture.old,file_hash:'b'.repeat(64),file_size:'12'}});await pending;
  assert.equal(fixture.context.pages[0],fixture.old);assert.equal(fixture.context.pages[0].id,fixture.old.id);assert.equal(fixture.context.pages[0].file_hash,'b'.repeat(64));assert.equal(fixture.context.pages[0].thumbnail,'blob:new-owned');assert.equal(fixture.context.pages[1],fixture.neighbour);
  assert.deepEqual(fixture.calls.revoked,['blob:old-owned']);assert.deepEqual(fixture.calls.acknowledged,[3]);
});
test('definitive replacement rejection retains the original row and preview without creating a local URL',async()=>{
  const error=Error('Explicit403'),fixture=replacementUi({replace:async()=>{throw error;}});await assert.rejects(fixture.api.replaceFile(fixture.file,fixture.session),cause=>cause===error);
  assert.equal(fixture.context.pages[0],fixture.old);assert.deepEqual(fixture.calls.created,[]);assert.deepEqual(fixture.calls.revoked,[]);assert.equal(fixture.session.can_mutate,true);
});
test('unknown replacement outcome preserves displayed bytes while disabling every session mutation affordance',async()=>{
  let pending;const fixture=replacementUi({pending:()=>pending,replace:async()=>{pending={sessionId:'exact-session',fileId:'stable-old'};throw Error('Unknown transport outcome');}});
  await assert.rejects(fixture.api.replaceFile(fixture.file,fixture.session));assert.equal(fixture.context.pages[0],fixture.old);assert.equal(fixture.old.thumbnail,'blob:old-owned');
  for(const key of ['can_mutate','can_cancel','can_retry_commit'])assert.equal(fixture.session[key],false);assert.deepEqual(fixture.calls.created,[]);assert.deepEqual(fixture.calls.revoked,[]);
});
test('late replacement result cannot adopt into a different card selection',async()=>{
  const held=deferred(),fixture=replacementUi({replace:async(_s,_t,_f,_o,check)=>{await held.promise;check();return {session:{...fixture.session,version:3},file:{...fixture.old,file_hash:'b'.repeat(64)}};}});
  const pending=fixture.api.replaceFile(fixture.file,fixture.session);await Promise.resolve();fixture.context.targetFile=fixture.neighbour;held.resolve();await assert.rejects(pending,/Upload changed/);
  assert.equal(fixture.context.pages[0],fixture.old);assert.deepEqual(fixture.calls.created,[]);assert.deepEqual(fixture.calls.revoked,[]);
});
test('a reordered or removed card invalidates the original replacement callback before adoption',async()=>{
  for(const changed of ['reordered','removed']){const held=deferred(),fixture=replacementUi({replace:async(_s,_t,_f,_o,check)=>{await held.promise;check();throw Error('Unreachable dispatch');}});const pending=fixture.api.replaceFile(fixture.file,fixture.session);await Promise.resolve();fixture.context.pages=changed==='reordered'?[fixture.neighbour,fixture.old]:[fixture.neighbour];held.resolve();await assert.rejects(pending,/Upload changed/);assert.deepEqual(fixture.calls.created,[]);assert.deepEqual(fixture.calls.revoked,[]);}
});
test('invalid or oversized replacement and decode failure perform no helper request',async()=>{
  for(const file of [{name:'bad.txt',type:'text/plain',size:12},{name:'empty.png',type:'image/png',size:0},{name:'huge.png',type:'image/png',size:25*1024*1024+1}]){const fixture=replacementUi();await assert.rejects(fixture.api.replaceFile(file,fixture.session));assert.equal(fixture.calls.helper,0);assert.equal(fixture.context.pages[0],fixture.old);}
  const fixture=replacementUi({decode:async()=>{throw Error('Decode failed');}});await assert.rejects(fixture.api.replaceFile(fixture.file,fixture.session),/Decode failed/);assert.equal(fixture.calls.helper,0);assert.deepEqual(fixture.calls.revoked,[]);
});
test('explicit replacement receipt recovery is read-only and restores actual saved authority only after confirmation',async()=>{
  const fixture=replacementUi({pending:()=>({sessionId:'exact-session',fileId:'stable-old'}),reconcile:async()=>({session:{id:'exact-session',version:3,can_mutate:true,can_cancel:true,can_retry_commit:true},file:{id:'stable-old',file_hash:'b'.repeat(64),file_size:'12'}})});
  fixture.session.can_mutate=false;fixture.session.can_cancel=false;fixture.session.can_retry_commit=false;await fixture.api.checkSignedReplacement();
  assert.equal(fixture.calls.helper,1);assert.equal(fixture.context.pages[0].id,'stable-old');assert.equal(fixture.context.pages[0].file_hash,'b'.repeat(64));assert.equal(fixture.context.pages[1],fixture.neighbour);assert.equal(fixture.session.can_mutate,true);assert.deepEqual(fixture.calls.created,[]);assert.deepEqual(fixture.calls.revoked,['blob:old-owned']);
  const source=pages.slice(pages.indexOf('const checkSignedReplacement ='),pages.indexOf('const replaceFile ='));assert.doesNotMatch(source,/replaceUploadPage|uploadChunk|"POST"|fetch\(/);
});
test('a same-hash confirmed replacement retains the existing owned preview without a second URL',()=>{
  const fixture=replacementUi();fixture.api.adoptReplacement(fixture.session,{...fixture.session,version:3},{...fixture.old,original_file_name:'renamed.png'},fixture.old,fixture.file);
  assert.equal(fixture.context.pages[0],fixture.old);assert.equal(fixture.context.pages[0].thumbnail,'blob:old-owned');assert.deepEqual(fixture.calls.created,[]);assert.deepEqual(fixture.calls.revoked,[]);assert.match(pages,/Object.assign\(target, replacement, \{ thumbnail \}\)/);
});
test('replacement uncertainty recovery control is explicit and cannot be used as a fresh mutation',()=>{
  assert.match(pages,/uploadSession\?\.can_mutate === false && owner.pendingReplacement\(\)\?\.sessionId === uploadSession.id/);
  assert.match(pages,/onclick=\{checkSignedReplacement\}>Check replacement outcome/);assert.doesNotMatch(pages,/Page replacement unavailable|new HTTP|FormData|setWithCredentials/);
});

// A confirmed receipt can clear the persisted fence after a reload has already
// mounted this same remote file/hash without any local or signed-preview URL.
test('confirmed same-hash receipt without a thumbnail remounts only its own card and preserves authority/neighbours',()=>{
  const fixture=replacementUi();fixture.old.thumbnail=undefined;fixture.session.can_mutate=false;
  const saved={...fixture.session,version:3,can_mutate:true,can_cancel:true,can_retry_commit:true};
  fixture.api.adoptReplacement(fixture.session,saved,{...fixture.old,original_file_name:'confirmed.png'},fixture.old);
  assert.notEqual(fixture.context.pages[0],fixture.old);assert.equal(fixture.context.pages[0].id,fixture.old.id);
  assert.equal(fixture.context.pages[0].file_hash,fixture.old.file_hash);assert.equal(fixture.context.pages[0].thumbnail,undefined);
  assert.equal(fixture.context.pages[1],fixture.neighbour);assert.equal(fixture.session.can_mutate,true);
  assert.deepEqual(fixture.calls.acknowledged,[3]);assert.deepEqual(fixture.calls.created,[]);assert.deepEqual(fixture.calls.revoked,[]);
});
test('a failed exact-session adoption cannot remount a no-thumbnail card or restore mutation authority',()=>{
  for(const changed of ['session','file','removed']){const fixture=replacementUi();fixture.old.thumbnail=undefined;fixture.session.can_mutate=false;
    const saved={...fixture.session,version:3,can_mutate:true};const file={...fixture.old};
    if(changed==='session')saved.id='different-session';else if(changed==='file')file.id='different-file';else fixture.context.pages=[fixture.neighbour];
    assert.throws(()=>fixture.api.adoptReplacement(fixture.session,saved,file,fixture.old),/Upload changed/);
    assert.equal(fixture.session.can_mutate,false);assert.deepEqual(fixture.calls.acknowledged,[]);assert.deepEqual(fixture.calls.created,[]);assert.deepEqual(fixture.calls.revoked,[]);
    assert.equal(fixture.context.pages.includes(fixture.old),changed!=='removed');
  }
});
test('changed confirmed bytes update the existing keyed card for its hash-keyed preview lease',()=>{
  const fixture=replacementUi();fixture.old.thumbnail=undefined;
  fixture.api.adoptReplacement(fixture.session,{...fixture.session,version:3},{...fixture.old,file_hash:'b'.repeat(64)},fixture.old);
  assert.equal(fixture.context.pages[0],fixture.old);assert.equal(fixture.old.file_hash,'b'.repeat(64));
  assert.equal(fixture.old.thumbnail,undefined);assert.equal(fixture.context.pages[1],fixture.neighbour);assert.deepEqual(fixture.calls.created,[]);
});
test('signed chapter editor keeps local annotations, exact owner and distinct moderator confirmation', () => {
  const source = fs.readFileSync(path.join(root, 'platform-integrations/public-api/PublicChapterEditor.svelte'), 'utf8');
  assert.match(source, /loadUploadAccount\(current\)/);
  assert.match(source, /publicUploadOwner\(\(\) => initial.id, account, 'chapter'\)/);
  assert.match(source, /approvesHide = \$derived\([\s\S]*?\['moderator', 'staff', 'sysop'\][\s\S]*?includes\('chapter.remove'\)/);
  assert.match(source, /Approve hiding this chapter on this instance immediately/);
  assert.match(source, /chapter remains visible until approval/);
  assert.match(source, /removalChoice = approvesHide \? 'approve-hide' : 'request-review'/);
  assert.match(source, /removalChoice === 'approve-hide' \? 'Approve and hide on this instance' : 'Submit removal request'/);
  assert.match(source, /command\('remove', removalChoice\)/);
  assert.match(source, /action === 'remove' \? \{ removalMode \} : undefined/);
  assert.match(source, /Signed source metadata and published image pages are immutable/);
  assert.doesNotMatch(source, /API\.|UploadChapterEdit|replaceUploadPage|setInterval/);
  assert.match(source, /pending = true/);
  assert.match(source, /No mutation was replayed/);
});
