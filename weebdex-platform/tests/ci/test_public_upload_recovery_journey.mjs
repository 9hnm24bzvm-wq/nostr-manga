// Explicit finite fakes/source seams. No actual signature, HTTP, DB or browser.
import assert from 'node:assert/strict';
import test from 'node:test';
import { EventEmitter } from 'node:events';
import fs from 'node:fs';
import path from 'node:path';
import { inflateSync } from 'node:zlib';
import { uploadDigest, validateUploadHttpEvent, observePublicUpload, createUploadIndependentReader } from '../../scripts/public-upload-recovery-observer.mjs';
import { tinyUploadPng, validateDurableUpload, assertUploadRecoveryRoute, assertUploadCompletion, uploadCompletionTitles, assertReplacementReadback, assertReplacementMutation } from '../../scripts/public-upload-recovery-browser.mjs';
const origin = 'http://127.0.0.1:50123', owner = 'a'.repeat(64), service = 'b'.repeat(64), now = 1700000000000;
const scope = { origin, owner, service, verify: () => true, now };
function proof(method = 'POST', body = Buffer.from('{}'), path = '/api/v1/upload/begin') {
  const event = { kind: 27235, created_at: now / 1000, content: '', pubkey: owner,
    tags: [['u', origin + path], ['method', method], ['service', service], ['nonce', 'explicit_fake_nonce_1234'], ...(method === 'GET' ? [] : [['payload', uploadDigest(body)]])], sig: 'c'.repeat(128) };
  event.id = uploadDigest(JSON.stringify([0, owner, event.created_at, event.kind, event.tags, event.content])); return event;
}
const encode = event => 'Nostr ' + Buffer.from(JSON.stringify(event)).toString('base64');
const input = (event = proof(), body = Buffer.from('{}')) => ({ ...scope, authorization: encode(event), method: 'POST', url: origin + '/api/v1/upload/begin', body });
test('fake event seam preserves actual body hash without claiming genuine Schnorr', () => { const row = validateUploadHttpEvent(input()); assert.equal(row.observed_body_sha256, uploadDigest('{}')); assert.equal(row.body_observation, 'actual-playwright-bytes'); });
test('unavailable multipart observation stays unavailable and never becomes expected bytes', () => { const row = validateUploadHttpEvent(input(proof(), null)); assert.equal(row.observed_body_sha256, null); assert.equal(row.observed_body_bytes, null); assert.equal(row.body_observation, 'unavailable'); });
for (const [name, change] of [
  ['wrong body', x => { x.body = Buffer.from('{ }'); }],
  ['wrong signature', x => { x.verify = () => false; }],
  ['wrong owner', x => { x.owner = 'd'.repeat(64); }],
  ['wrong service', x => { x.service = 'd'.repeat(64); }],
  ['wrong method', x => { x.method = 'DELETE'; }],
  ['foreign origin', x => { x.url = 'http://127.0.0.1:50124/api/v1/upload/begin'; }],
  ['query', x => { x.url += '?secret=hidden'; }],
  ['empty fragment', x => { x.url += '#'; }],
  ['stale time', x => { x.now += 121000; }],
]) test(`HTTP event refuses ${name}`, () => { const value = input(); change(value); assert.throws(() => validateUploadHttpEvent(value)); });
test('duplicate authorization tags cannot borrow a valid fake verification result', () => { const event = proof(); event.tags.push(['service', service]); event.id = uploadDigest(JSON.stringify([0, owner, event.created_at, event.kind, event.tags, event.content])); assert.throws(() => validateUploadHttpEvent(input(event))); });
test('observer owns exact native failure and never changes ERR_ABORTED into success', () => {
  const page = new EventEmitter(), current = proof('GET', Buffer.alloc(0), '/api/v1/upload'); current.created_at = Math.floor(Date.now() / 1000); current.id = uploadDigest(JSON.stringify([0, owner, current.created_at, current.kind, current.tags, current.content]));
  const observer = observePublicUpload(page, { ...scope, now: Date.now() }), request = { url: () => origin + '/api/v1/upload', method: () => 'GET', headers: () => ({ authorization: encode(current) }), postDataBuffer: () => null, failure: () => ({ errorText: 'net::ERR_ABORTED' }) };
  page.emit('request', request); page.emit('response', { request: () => request, status: () => 200 }); page.emit('requestfailed', request);
  assert.equal(observer.rows[0].observed_body_bytes, null); assert.equal(observer.rows[0].body_observation, 'unavailable');
  assert.throws(observer.assertHealthy); assert.equal(observer.rows[0].native, 'failed'); assert.equal(observer.rows[0].native_error, 'net::ERR_ABORTED'); observer.close(); assert.equal(page.listenerCount('request'), 0);
});
test('real PNG encoder creates bounded distinct originals with exact dimensions and complete scanlines', () => {
  const seen = new Set(); for (let i = 0; i < 3; i++) { const image = tinyUploadPng(`page-${i}.png`, 24+i, 32+i, [30+i,80,130]); assert.ok(image.buffer.length <= 512); assert.equal(image.buffer.readUInt32BE(16), 24+i); assert.equal(image.buffer.readUInt32BE(20), 32+i); let at = 8; const chunks = []; while (at < image.buffer.length) { const size = image.buffer.readUInt32BE(at); if (image.buffer.toString('ascii',at+4,at+8) === 'IDAT') chunks.push(image.buffer.subarray(at+8,at+8+size)); at += size+12; } assert.equal(at, image.buffer.length); assert.equal(inflateSync(Buffer.concat(chunks)).length, (32+i) * (1+(24+i)*3)); seen.add(image.sha256); } assert.equal(seen.size, 3);
});
const id = '11111111-1111-8111-8111-111111111111';
function stored() { const image = tinyUploadPng('page.png',24,32,[1,2,3]), account = { pubkey: owner, user: {id}, group: {id}, manga: {id} }; return { account, image, value: { fixture_only:true, scenario:'public-upload-v1', ...account, session:{id,state:'open',version:2,draft:{language:'en'},page_order_source:'effective_stored_plus_current_files',stored_page_order:[id],page_order:[id],files:[{id,original_file_name:image.name,file_hash:image.sha256,file_size:image.buffer.length,storage_sha256:image.sha256,storage_bytes:image.buffer.length,fresh_original_read:true}] } } }; }
test('durable seam checks genuinely stored order separately from normalized order', () => { const { account,image,value } = stored(); validateDurableUpload(value,{account,sessionId:id,images:[image],order:[id],draft:{language:'en'}}); value.session.stored_page_order=[]; assert.throws(() => validateDurableUpload(value,{account,sessionId:id,images:[image],order:[id]})); });
for (const [name,change] of [
  ['original storage hash', v => {v.session.files[0].storage_sha256='d'.repeat(64);}],
  ['fresh original read', v => {v.session.files[0].fresh_original_read=false;}],
  ['foreign actor', v => {v.pubkey='d'.repeat(64);}],
  ['file membership', v => {v.session.files=[];}],
]) test(`durable state refuses changed ${name}`, () => {const {account,image,value}=stored();change(value);assert.throws(()=>validateDurableUpload(value,{account,sessionId:id,images:[image],order:[id]}));});
test('independent reader retains actual refusal and uses one fresh exact credential-free GET', async () => { const seen=[]; const reader=createUploadIndependentReader({origin,owner,service,sign:async template=>({...template,id:'e'.repeat(64),pubkey:owner,sig:'f'.repeat(128)}),fetch:async(url,init)=>{seen.push({url:String(url),init});return new Response('{"result":"error"}',{status:404,headers:{'content-type':'application/json'}});}}); const value=await reader.get('/api/v1/upload');assert.equal(value.status,404);assert.equal(seen.length,1);assert.equal(seen[0].init.credentials,'omit');assert.equal(seen[0].init.redirect,'error');const event=JSON.parse(Buffer.from(seen[0].init.headers.authorization.slice(6),'base64'));assert.equal(event.tags.find(t=>t[0]==='u')[1],origin+'/api/v1/upload');assert.equal(event.tags.find(t=>t[0]==='payload'),undefined);reader.close();assert.equal(reader.snapshot().pending,0);assert.equal(reader.snapshot().closed,true); });
test('independent reader refuses over-cap response without retry and releases owner', async () => {let calls=0;const reader=createUploadIndependentReader({origin,owner,service,sign:async()=>({}),fetch:async()=>{calls++;return new Response('x'.repeat(65537),{headers:{'content-type':'application/json'}});}});await assert.rejects(reader.get('/api/v1/upload'));assert.equal(calls,1);assert.equal(reader.snapshot().pending,0);reader.close();});
test('journey never fabricates application transport success and declares only the signed replacement path', () => { const source = fs.readFileSync(new URL('../../scripts/public-upload-recovery-browser.mjs',import.meta.url),'utf8'); assert.ok(!source.includes('.fulfill(') && !source.includes('dispatchEvent(') && !source.includes('window.nostr')); assert.match(source,/supported_replacement: true/); assert.doesNotMatch(source,/Page replacement unavailable|replacement-refused/); assert.match(source,/Mandatory fresh document before signed-preview proof/); assert.match(source,/signed-preview-after-hard-reload-and-key-reimport/); assert.match(source,/source_basis: sourceBasis/); assert.match(source,/value\.chapter\?\.event_id, value\.publication\.event_id/); assert.match(source,/stored_page_order, order/); });
function cleanupSeam(overrides = {}) {
  const source = fs.readFileSync(new URL('../../scripts/browser-acceptance.mjs',import.meta.url),'utf8');
  const start = source.indexOf('finally {\n\tconst cleanupFailure'); assert.ok(start > 0);
  const body = source.slice(start + 'finally {'.length, source.indexOf('\n}\nprocess.exitCode', start));
  const AsyncFunction = Object.getPrototypeOf(async function(){}).constructor, calls = [];
  const report = { cleanup: { failures: [] }, steps: [{status:'passed'}], page_errors: [] };
  const values = { report, runnerStage: stage => calls.push(stage), safeError: error => error.message,
    context: {close:async()=>calls.push('context-close')}, browser:{close:async()=>calls.push('browser-close')},closeDenyProxy:async()=>calls.push('proxy-close'),save:()=>{},phase:'finite',root:'/r',output:'/r/o',process:{stdout:{write:()=>{}},exitCode:0},path,...overrides };
  return { calls, report: values.report, process: values.process, run: () => new AsyncFunction(...Object.keys(values),body)(...Object.values(values)) };
}
test('source-extracted cleanup still closes every owner after stage persistence and context errors', async () => {
  const report={cleanup:{failures:[]},steps:[{status:'passed'}],page_errors:[],harness_error:'original journey failure'};
  const seam=cleanupSeam({report,runnerStage:()=>{throw Error('save');},safeError:()=>{throw Error('serializer');},context:{close:async()=>{throw Error('context');}}});
  await seam.run();assert.ok(seam.calls.includes('browser-close'));assert.ok(seam.calls.includes('proxy-close'));assert.equal(report.harness_error,'original journey failure');assert.equal(report.passed,false);assert.equal(report.cleanup.browser_closed,true);assert.equal(report.cleanup.proxy_closed,true);assert.equal(report.cleanup.failures.length,6);
});
test('final evidence write failure preserves nonzero verdict after actual close attempts', async () => {const seam=cleanupSeam({save:()=>{throw Error('disk');}});await seam.run();assert.ok(seam.calls.includes('context-close')&&seam.calls.includes('browser-close')&&seam.calls.includes('proxy-close'));assert.equal(seam.report.passed,false);assert.equal(seam.process.exitCode,1);});
test('complete-stage persistence failure is included before final verdict and save', async () => { let saved; const seam=cleanupSeam({runnerStage:stage=>{if(stage==='complete')throw Error('complete write');},save:()=>{saved=seam.report.passed;}}); await seam.run();assert.equal(seam.report.passed,false);assert.equal(saved,false);assert.equal(seam.report.cleanup.failures[0].stage,'evidence-complete'); });
for (const status of [403,503,202]) test(`source-extracted success stage rejects native-finished HTTP ${status}`, async () => {
  const source=fs.readFileSync(new URL('../../scripts/public-upload-recovery-browser.mjs',import.meta.url),'utf8'), token='async function completeWrites(start, count) {';
  const start=source.indexOf(token);assert.ok(start>0);const body=source.slice(start+token.length,source.indexOf('\n  async function durable',start)).trim();assert.ok(body.endsWith('}'));
  const AsyncFunction=Object.getPrototypeOf(async function(){}).constructor, observer={rows:[{method:'POST',native:'finished',status}],mutationCount:()=>1,settled:()=>true};
  const until=async(read,valid)=>{const value=await read();assert.equal(valid(value),true);return value;};
  await assert.rejects(new AsyncFunction('observer','until','assertUploadRoute','assert','start','count',body.slice(0,-1))(observer,until,()=>{},assert,0,1));
});
test('final route guard requires the exact full current URL', () => { const route='/title/exact/upload';assertUploadRecoveryRoute(origin+route,origin,route);for(const suffix of ['?x=1','?','#changed','#','/other'])assert.throws(()=>assertUploadRecoveryRoute(origin+route+suffix,origin,route));assert.throws(()=>assertUploadRecoveryRoute('http://127.0.0.1:50124'+route,origin,route)); });
test('completion titles are closed to the actual three source branches', () => {assert.deepEqual([...uploadCompletionTitles],['Upload request accepted','Chapter added to approval queue','Chapter uploaded successfully']);for(const title of uploadCompletionTitles)assert.equal(assertUploadCompletion(title,{receiptStatus:'available',publicationStatus:'materialized',receipt:{attributes:{isUnavailable:false}}}),title);assert.throws(()=>assertUploadCompletion('Publication outcome unresolved',{receiptStatus:'available'}));});
for(const state of [{receiptStatus:'unavailable'},{receiptStatus:'available',publicationStatus:'accepted_pending_projection'},{receiptStatus:'available',receipt:{attributes:{isUnavailable:true}}}])test('actual unavailable presentation requires the accepted-only visible branch '+JSON.stringify(state),()=>{assert.equal(assertUploadCompletion('Upload request accepted',state),'Upload request accepted');assert.throws(()=>assertUploadCompletion('Chapter uploaded successfully',state));assert.throws(()=>assertUploadCompletion('Chapter added to approval queue',state));});
test('missing committed receipt status cannot borrow a completion title',()=>{assert.throws(()=>assertUploadCompletion('Upload request accepted',{}));});
for(const changed of [false,true])test(`source-extracted restored metadata observes actual field values, changed=${changed}`,async()=>{
  const source=fs.readFileSync(new URL('../../scripts/public-upload-recovery-browser.mjs',import.meta.url),'utf8'),token='async function restoredMetadata() {';const at=source.indexOf(token);assert.ok(at>0);const body=source.slice(at+token.length,source.indexOf('\n  async function visibleCompletion',at)).trim();assert.ok(body.endsWith('}'));
  const p={getByPlaceholder:name=>({inputValue:async()=>name==='Chapter Name'?'Synthetic public upload recovery':changed?'changed':'9001'}),locator:()=>({innerText:async()=>' English '})};
  const until=async(read,valid)=>{const value=await read();assert.equal(valid(value),true);return value;};const AsyncFunction=Object.getPrototypeOf(async function(){}).constructor;const pending=new AsyncFunction('until','panel','assertUploadRoute',body.slice(0,-1))(until,async()=>p,()=>{});if(changed)await assert.rejects(pending);else await pending;
});

function replacementSnapshot() {
  const image=tinyUploadPng('replacement.png',30,38,[91,58,173]);
  const target={id,originalFileName:'old.png',fileHash:'1'.repeat(64),fileSize:93,mimeType:'image/png',dimensions:{width:24,height:32},previewUrl:`/api/v1/nostr/uploads/${id}/files/${id}/preview`};
  const neighbour={...target,id:'22222222-2222-8222-8222-222222222222',originalFileName:'neighbour.png',fileHash:'2'.repeat(64)};
  const before={id,state:'open',canMutate:true,canCancel:true,version:5,updatedAt:'2026-10-17T01:00:00Z',draft:{language:'en',chapter:'9001'},pageOrder:[neighbour.id,id],files:[neighbour,target],mangaId:id,groupIds:[id]};
  const after={...structuredClone(before),version:6,updatedAt:'2026-10-17T01:00:01Z'};
  after.files[1]={...after.files[1],originalFileName:image.name,fileHash:image.sha256,fileSize:image.buffer.length,mimeType:image.mimeType,dimensions:{width:image.width,height:image.height}};
  return {before,after,image};
}
test('positive replacement readback preserves exact stable ID, order, metadata, neighbour and actual version',()=>{const {before,after,image}=replacementSnapshot();const file=assertReplacementReadback(before,after,{fileId:id,image});assert.equal(file.id,id);assert.equal(file.fileHash,image.sha256);image.buffer.fill(0);});
for(const [name,change]of[
  ['target identity',v=>{v.after.files[1].id='33333333-3333-8333-8333-333333333333';}],
  ['old digest',v=>{v.after.files[1].fileHash=v.before.files[1].fileHash;}],
  ['wrong actual version',v=>{v.after.version=7;}],
  ['neighbour digest',v=>{v.after.files[0].fileHash='3'.repeat(64);}],
  ['page order',v=>{v.after.pageOrder.reverse();}],
  ['draft',v=>{v.after.draft.chapter='9002';}],
  ['authority',v=>{v.after.canMutate=false;}],
  ['wrong dimensions',v=>{v.after.files[1].dimensions.width=31;}],
  ['wrong timestamp',v=>{v.after.updatedAt='invalid';}],
])test(`replacement readback rejects changed ${name}`,()=>{const value=replacementSnapshot();try{change(value);assert.throws(()=>assertReplacementReadback(value.before,value.after,{fileId:id,image:value.image}));}finally{value.image.buffer.fill(0);}});
test('new exact public replacement path admits a signed raw body without broad file-route widening',()=>{const body=Buffer.from('explicit synthetic multipart bytes'),path=`/api/v1/nostr/uploads/${id}/files/${id}/replace`,event=proof('POST',body,path);const row=validateUploadHttpEvent({...scope,authorization:encode(event),method:'POST',url:origin+path,body});assert.equal(row.path,path);assert.equal(row.observed_body_sha256,uploadDigest(body));for(const suffix of ['/extra','ment','?x=1','#'])assert.throws(()=>validateUploadHttpEvent({...scope,authorization:encode(event),method:'POST',url:origin+path+suffix,body}));});

function replacementMutation(){return {method:'POST',path:`/api/v1/nostr/uploads/${id}/files/${id}/replace`,status:200,native:'finished',event_id:'e'.repeat(64)};}
test('replacement step requires its exact observed POST path/status/native and retains the real event ID',()=>{const row=replacementMutation();assert.equal(assertReplacementMutation([row],{sessionId:id,fileId:id}),row);});
for(const [name,change]of[
  ['method',rows=>{rows[0].method='PATCH';}],
  ['path',rows=>{rows[0].path=rows[0].path.replace('/replace','/preview');}],
  ['HTTP refusal',rows=>{rows[0].status=503;}],
  ['native error',rows=>{rows[0].native='failed';}],
  ['missing native',rows=>{rows[0].native='pending';}],
  ['extra write',rows=>{rows.push({...rows[0]});}],
])test(`replacement mutation refuses changed ${name}`,()=>{const rows=[replacementMutation()];change(rows);assert.throws(()=>assertReplacementMutation(rows,{sessionId:id,fileId:id}));});
