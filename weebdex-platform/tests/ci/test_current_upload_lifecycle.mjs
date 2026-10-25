// Full currentUpload adapter + actual transport + strict observer, with finite
// signer, fetch-stream, and native-terminal adapters. Not browser/network proof.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import vm from 'node:vm';
import test from 'node:test';
import {stripTypeScriptTypes} from 'node:module';
import {fileURLToPath} from 'node:url';
import {createHash} from 'node:crypto';
import {EventEmitter} from 'node:events';
import {observePublicUpload} from '../../scripts/public-upload-recovery-observer.mjs';
import {createUploadRefusalQaPlugin} from '../../scripts/public-upload-refusal-qa.mjs';
import { restoreReviewedLanguageSource } from './reviewed-local-page-source-successors.mjs';
const root=fileURLToPath(new URL('../../',import.meta.url)),digest=b=>createHash('sha256').update(b).digest('hex');
const read=n=>fs.readFileSync(new URL(n,import.meta.url),'utf8');
const AsyncFunction=Object.getPrototypeOf(async function(){}).constructor;
const adapterSource=read('./test_public_upload_adapter_recovery.mjs');
const prefix=adapterSource.slice(adapterSource.indexOf('const context ='),adapterSource.indexOf('async function ready('));
assert.equal(digest(restoreReviewedLanguageSource(prefix,'adapterFixturePrefix')),'ab243b74e652d5766e41f3e10af337ac16bbbba9df04c4af5246dfa6a05f48a0');
const helpers=await new AsyncFunction('assert','fs','path','vm','stripTypeScriptTypes','createHash','root',prefix+';return {fixture,PUBLIC,SERVICE,ACTOR,USER,MANGA};')(assert,fs,path,vm,stripTypeScriptTypes,createHash,root);
const prior=read('./test_public_upload_request_recovery.mjs');
let fixtureCode=prior.slice(prior.indexOf('async function fixture('),prior.indexOf('const errorIs ='));
assert.equal(digest(fixtureCode),'4be1bf0631ce5984b0b174b99ca4d0734088b4136d8ca08f38d9a60137f9d6c9');
fixtureCode=fixtureCode.replace("url.protocol !== 'https:'", "!['https:','http:'].includes(url.protocol)").replace('return { api, transport, request, requests, templates, timers, timerHistory, scope, identity,','return { api, primitives, transport, request, requests, templates, timers, timerHistory, scope, identity,');
const deferred=()=>{let resolve;return {promise:new Promise(r=>resolve=r),resolve};};
const transportPath=root+'platform-integrations/public-api/nostr-upload-request.ts',transportSource=fs.readFileSync(transportPath,'utf8');
const plugin=createUploadRefusalQaPlugin(root,{enabled:true,publicUploadEnabled:true});plugin.configResolved({build:{ssr:false}});plugin.buildStart.call({emitFile(){}});
const transformed=plugin.transform(transportSource,transportPath).code;
const fixture= new Function('vm','stripTypeScriptTypes','source','digest','clone','text','deferred',fixtureCode+';return fixture;')(vm,stripTypeScriptTypes,transformed,digest,x=>JSON.parse(JSON.stringify(x)),new TextEncoder(),deferred);
async function composed({body='{"result":"error"}',hold=false,nativeAbort=false,transportMutation=false,baseline,earlyNativeFinish=false}={}){
 const page=new EventEmitter(),observer=observePublicUpload(page,{origin:helpers.PUBLIC,owner:helpers.ACTOR,service:helpers.SERVICE,verify:()=>true});observer.bindAccount(helpers.USER);
 const entered=deferred(),release=deferred(),requests=[];let nonce=0,f;
 if(transportMutation)assert.equal(transformed.split('if (!completed) controller.abort();').length-1,1);
 const t=await fixture({transportSource:baseline??(transportMutation?transformed.replace('if (!completed) controller.abort();','controller.abort();'):undefined)});
 const primitives={...t.primitives,nonce:()=>`finite_current_upload_${++nonce}`,now:()=>Math.floor(Date.now()/1000),signExact:async(_signer,owner,event)=>{const signed={...event,pubkey:owner,sig:'c'.repeat(128)};signed.id=digest(JSON.stringify([0,signed.pubkey,signed.created_at,signed.kind,signed.tags,signed.content]));return signed;},fetch:async(url,init)=>{
  const isCurrent=new URL(url).pathname==='/api/v1/upload';const state={url,init,aborts:0,terminal:'pending',reads:0,cancelled:0,released:0};requests.push(state);
  const request={url:()=>url,method:()=>init.method,headers:()=>Object.fromEntries(new Headers(init.headers)),postDataBuffer:()=>null,failure:()=>({errorText:'net::ERR_ABORTED'})};page.emit('request',request);
  const status=isCurrent?404:200;page.emit('response',{request:()=>request,status:()=>status});
  const terminal=kind=>{if(state.terminal!=='pending')return;state.terminal=kind;page.emit(kind==='finished'?'requestfinished':'requestfailed',request);};
  init.signal.addEventListener('abort',()=>{state.aborts++;terminal('failed');},{once:true});
  if(isCurrent&&earlyNativeFinish)terminal('finished');
  const bytes=new TextEncoder().encode(isCurrent?body:JSON.stringify(f.account));
  return {status,headers:new Headers({'content-type':'application/json'}),body:{getReader:()=>({async read(){if(isCurrent&&hold&&state.reads===0){entered.resolve();await release.promise;}if(isCurrent&&nativeAbort){terminal('failed');throw Error('finite native body interruption');}if(state.reads++===0)return {value:bytes,done:false};terminal('finished');return {done:true};},async cancel(){state.cancelled++;terminal('failed');},releaseLock(){state.released++;}})}};
 }};
 f=helpers.fixture({dependencies:{createRequest:config=>t.api.createNostrUploadRequest(config,primitives)}});const {owner}=await f.owner();
 return {f,owner,t,requests,observer,entered:entered.promise,release:release.resolve,close(){observer.close();}};
}
for(const change of ['none','dispose-before-body','destroy-before-body','identity-before-body','dispose-after-complete'])test('actual currentUpload404 lifecycle '+change,{timeout:5000},async()=>{
 const c=await composed({hold:change.endsWith('before-body')});try{const pending=c.f.api.currentUpload(c.owner);pending.catch(()=>{});
 if(change.endsWith('before-body')){await c.entered;if(change==='dispose-before-body')c.owner.dispose();else if(change==='destroy-before-body')c.f.destroys.forEach(fn=>fn());else c.f.state.generation++;assert.equal(c.requests.at(-1).aborts,0);c.release();await assert.rejects(pending);assert.equal(c.requests.at(-1).aborts,1);assert.throws(c.observer.assertHealthy);assert.equal(c.t.api.readUploadRequestRefusals().records.at(-1).stage,'checkpoint');}
 else{assert.equal(await pending,undefined);if(change==='dispose-after-complete')c.owner.dispose();assert.equal(c.requests.at(-1).aborts,0);c.observer.assertHealthy();assert.equal(c.requests.at(-1).terminal,'finished');}
 assert.equal(c.requests.filter(r=>r.init.method!=='GET').length,0);assert.equal(c.t.timers.size,0);assert.equal(c.requests.at(-1).released,1);
 }finally{c.close();}
});
test('native404 body abort is not normal missing-current-upload',{timeout:5000},async()=>{const c=await composed({nativeAbort:true});try{await assert.rejects(c.f.api.currentUpload(c.owner),e=>e.status===404);assert.throws(c.observer.assertHealthy);assert.equal(c.t.api.readUploadRequestRefusals().records.at(-1).stage,'body-read');assert.equal(c.requests.at(-1).terminal,'failed');assert.equal(c.t.timers.size,0);}finally{c.close();}});
test('malformed404 body does not become empty upload success',{timeout:5000},async()=>{const c=await composed({body:'not json'});try{await assert.rejects(c.f.api.currentUpload(c.owner),e=>e.status===404);assert.equal(c.t.api.readUploadRequestRefusals().records.at(-1).stage,'decode');assert.equal(c.requests.at(-1).aborts,1);assert.equal(c.requests.at(-1).terminal,'finished');c.observer.assertHealthy();assert.equal(c.t.timers.size,0);}finally{c.close();}});
test('unconditional abort counterfactual detected after native completion',{timeout:5000},async()=>{const c=await composed({transportMutation:true});try{assert.equal(await c.f.api.currentUpload(c.owner),undefined);assert.equal(c.requests.at(-1).aborts,1);assert.equal(c.requests.at(-1).terminal,'finished');c.observer.assertHealthy();assert.equal(c.t.timers.size,0);}finally{c.close();}});

test('exact historical transport also accepts fully consumed fresh404 without cancellation',{timeout:5000,skip:!process.env.WEEBDEX_UPLOAD_REFUSAL_BASELINE_SOURCE},async()=>{const baseline=fs.readFileSync(process.env.WEEBDEX_UPLOAD_REFUSAL_BASELINE_SOURCE,'utf8');assert.equal(digest(baseline),'9418845c867fd7fd0706756197900221e3cd73cd87a48133d5ed3ff6ef0d3056');const c=await composed({baseline});try{assert.equal(await c.f.api.currentUpload(c.owner),undefined);assert.equal(c.requests.at(-1).aborts,0);assert.equal(c.requests.at(-1).terminal,'finished');c.observer.assertHealthy();assert.equal(c.t.timers.size,0);}finally{c.close();}});

test('native completion before application body consumption separates owner failure from observer abort',{timeout:5000},async()=>{const c=await composed({hold:true,earlyNativeFinish:true});try{const pending=c.f.api.currentUpload(c.owner);pending.catch(()=>{});await c.entered;c.owner.dispose();assert.equal(c.requests.at(-1).aborts,0);c.release();await assert.rejects(pending);assert.equal(c.requests.at(-1).aborts,1);assert.equal(c.requests.at(-1).terminal,'finished');assert.equal(c.t.api.readUploadRequestRefusals().records.at(-1).stage,'checkpoint');c.observer.assertHealthy();assert.equal(c.t.timers.size,0);}finally{c.close();}});
