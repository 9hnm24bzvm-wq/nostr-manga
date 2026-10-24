// Source-bound finite lifecycle/ordering controls. No browser, network, native
// service, real signing, decoder or hosted-cause claim is made by these tests.
import assert from 'node:assert/strict';
import test from 'node:test';
import fs from 'node:fs';
import vm from 'node:vm';
import { createHash } from 'node:crypto';
import { stripTypeScriptTypes } from 'node:module';
import { createUploadRefusalQaPlugin } from '../../scripts/public-upload-refusal-qa.mjs';
import { harness } from './public-upload-observer-query-fixture.mjs';
import { fileURLToPath } from 'node:url';
import { restoreReviewedLanguageSource } from './reviewed-local-page-source-successors.mjs';
const root=fileURLToPath(new URL('../../',import.meta.url)), read=name=>fs.readFileSync(new URL(name,import.meta.url),'utf8');
const digest=value=>createHash('sha256').update(value).digest('hex'), clone=value=>JSON.parse(JSON.stringify(value));
const text=new TextEncoder();
function deferred(){let resolve;const promise=new Promise(yes=>{resolve=yes;});return {promise,resolve};}
const transportPath=root+'platform-integrations/public-api/nostr-upload-request.ts';
const transport=fs.readFileSync(transportPath,'utf8');
const prior=read('./test_public_upload_request_recovery.mjs');
let fixtureCode=prior.slice(prior.indexOf('async function fixture('),prior.indexOf('const errorIs ='));
assert.equal(digest(fixtureCode),'4be1bf0631ce5984b0b174b99ca4d0734088b4136d8ca08f38d9a60137f9d6c9');
fixtureCode=fixtureCode.replace("id: 'synthetic-id'", "id: 'a'.repeat(64)");
const makeFixture=source=>new Function('vm','stripTypeScriptTypes','source','digest','clone','text','deferred',fixtureCode+';return fixture;')(vm,stripTypeScriptTypes,source,digest,clone,text,deferred);
const plugin=createUploadRefusalQaPlugin(root,{enabled:true,publicUploadEnabled:true});plugin.configResolved({build:{ssr:false}});plugin.buildStart.call({emitFile(){}});
const fixture=makeFixture(plugin.transform(transport,transportPath).code);
const consumerCode=prior.slice(prior.indexOf('function previewConsumer('),prior.indexOf("test('valid 200 preview consumption"));
const {previewConsumer,heldPreviewBody}=new Function('fs','URL','base','assert','digest','restoreReviewedLanguageSource','stripTypeScriptTypes','Blob','Headers','Response','deferred',consumerCode.replaceAll('import.meta.url','base')+';return {previewConsumer,heldPreviewBody};')(fs,URL,new URL('./test_public_upload_request_recovery.mjs',import.meta.url).href,assert,digest,restoreReviewedLanguageSource,stripTypeScriptTypes,Blob,Headers,Response,deferred);
async function leaseApi(){const context=vm.createContext({URL,Blob,Error,Object,Map,Set,Symbol,Number,Promise});const module=new vm.SourceTextModule(stripTypeScriptTypes(read('../../platform-integrations/public-api/upload-preview.ts')),{context});await module.link(()=>{throw Error('Unexpected dependency');});await module.evaluate();return module.namespace;}
const bytes=new Uint8Array([137,80,78,71,13,10,26,10]);
const previewPath='/api/v1/nostr/uploads/11111111-1111-8111-8111-111111111111/files/22222222-2222-8222-8222-222222222222/preview';
const snapshot=f=>clone(f.api.readUploadRequestRefusals());
for(const lifecycle of ['dispose','drag-clear','same-key-metadata']) test('actual lease '+lifecycle+' does not cancel an in-flight signed preview',{timeout:3000},async()=>{
 const stream=heldPreviewBody(bytes),f=await fixture({fetch:async()=>stream.response}),consumer=previewConsumer(f),api=await leaseApi();let allocations=0,changes=0;
 const lease=api.createUploadPreviewLease({load:()=>consumer.load(),isCurrent:()=>true,onChange(){changes++;},url:{createObjectURL(){allocations++;return 'blob:finite';},revokeObjectURL(){}}});
 const context={sessionId:'session',fileId:'file',fileHash:digest(bytes),fileSize:bytes.length,mimeType:'image/png',owner:{}};
 const pending=lease.update(context);await stream.entered;
 if(lifecycle==='dispose')lease.dispose();else if(lifecycle==='drag-clear')await lease.update(null);else {consumer.owned.latest={version:2,relationships:{files:[{...consumer.input}]}};assert.equal(lease.update({...context}),pending);}
 const before=changes;stream.finish();await pending;
 assert.equal(f.requests.length,1);assert.equal(f.requests[0].abortCount,0);assert.equal(stream.cancelled,0);assert.equal(stream.released,1);assert.equal(f.timers.size,0);assert.deepEqual(snapshot(f).records,[]);
 assert.equal(allocations,lifecycle==='same-key-metadata'?1:0);if(lifecycle!=='same-key-metadata')assert.equal(changes,before);lease.dispose();
});
for(const order of ['native-before-catch','catch-before-native'])test('HTTP200 native abort stays fatal under '+order+' ordering',{timeout:3000},async()=>{
 const h=harness(),request=h.emit({pathname:previewPath});h.page.emit('response',{request:()=>request,status:()=>200});
 const entered=deferred(),terminal=deferred();let f,atAbort;
 f=await fixture({fetch:async(_url,init)=>{init.signal.addEventListener('abort',()=>{atAbort=snapshot(f);if(order==='catch-before-native')h.page.emit('requestfailed',request);},{once:true});return {status:200,headers:new Headers({'content-type':'image/png'}),body:{getReader:()=>({read:async()=>{entered.resolve();return terminal.promise;},cancel:async()=>{},releaseLock(){}})}};}});
 const pending=previewConsumer(f).load();pending.catch(()=>{});await entered.promise;
 if(order==='native-before-catch'){h.page.emit('requestfailed',request);assert.deepEqual(snapshot(f).records,[]);assert.throws(h.observer.assertHealthy);}
 terminal.resolve(Promise.reject(Error('finite stream failure')));await assert.rejects(pending,e=>e.name==='PublicUploadRequestError'&&e.status===200);
 assert.deepEqual(atAbort.records,[{event_id:'a'.repeat(64),stage:'body-read',transport_status:200}]);assert.equal(f.requests[0].abortCount,1);assert.equal(f.timers.size,0);
 assert.deepEqual(h.observer.snapshot().failures,['signed_native_failed']);assert.equal(h.observer.rows[0].status,200);assert.throws(h.observer.settled);h.observer.close();
});
test('exact current completeWrites labels observer-health abort as mutation-count without count comparison',{timeout:3000},async()=>{
 const source=read('../../scripts/public-upload-recovery-browser.mjs');const start=source.indexOf('  async function completeWrites('),end=source.indexOf('\n',start);assert.ok(start>0&&end>start);
 const untilStart=source.indexOf('  async function until('),untilEnd=source.indexOf('\n',untilStart);assert.ok(untilStart>0);
 const h=harness();for(let i=1;i<=3;i++){const r=h.emit({method:'POST',pathname:'/api/v1/upload/begin',index:i,body:Buffer.from('{}')});h.page.emit('response',{request:()=>r,status:()=>200});h.page.emit('requestfinished',r);}
 const p=h.emit({pathname:previewPath,index:4});h.page.emit('response',{request:()=>p,status:()=>200});h.page.emit('requestfailed',p);
 let stage,routeChecks=0;
 const fn=new Function('observer','assert','mark','healthy','page','assertUploadRoute',source.slice(untilStart,untilEnd)+source.slice(start,end)+';return completeWrites;')(h.observer,assert,x=>{stage=x;},h.observer.assertHealthy,{waitForTimeout:async()=>{}},()=>{routeChecks++;});
 await assert.rejects(fn(2,1),/signed_native_failed/);assert.equal(h.observer.mutationCount(),3);assert.equal(stage,'mutation-count');assert.equal(routeChecks,0);h.observer.close();
});
test('success-abort counterfactual is detected while unchanged old transport keeps complete reads un-aborted',{timeout:3000},async()=>{
 const mutation=transport.replace('if (!completed) controller.abort();','controller.abort();');assert.notEqual(mutation,transport);
 for(const [source,expected]of [[transport,0],[mutation,1]]){const f=await makeFixture(source)({fetch:async()=>new Response(bytes,{headers:{'content-type':'image/png','content-length':String(bytes.length),'cache-control':'private, no-store',vary:'Authorization','x-content-type-options':'nosniff'}})});await previewConsumer(f).load();assert.equal(f.requests[0].abortCount,expected);assert.equal(f.timers.size,0);}
});
test('exact historical transport distinguishes successful completion from owner invalidation', {timeout:3000,skip:!process.env.WEEBDEX_UPLOAD_REFUSAL_BASELINE_SOURCE},async()=>{
 const baseline=fs.readFileSync(process.env.WEEBDEX_UPLOAD_REFUSAL_BASELINE_SOURCE,'utf8');assert.equal(digest(baseline),'9418845c867fd7fd0706756197900221e3cd73cd87a48133d5ed3ff6ef0d3056');
 for(const invalidate of [false,true]){const stream=heldPreviewBody(bytes),f=await makeFixture(baseline)({fetch:async()=>stream.response}),consumer=previewConsumer(f);const pending=consumer.load();pending.catch(()=>{});await stream.entered;if(invalidate)f.close();stream.finish();if(invalidate)await assert.rejects(pending,e=>e.name==='PublicUploadRequestError'&&e.status===200);else assert.equal((await pending).size,bytes.length);assert.equal(f.requests[0].abortCount,invalidate?1:0);assert.equal(stream.cancelled,invalidate?1:0);assert.equal(f.timers.size,0);}
});
