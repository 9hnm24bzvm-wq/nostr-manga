// Real WHATWG streams and exact source-bound transport; no sockets/browser.
// Stream adapters intentionally separate local byte/ownership proof from native
// Chromium terminal outcomes, which require the dedicated reviewed comparison.
import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import vm from 'node:vm';
import {createHash} from 'node:crypto';
import {stripTypeScriptTypes} from 'node:module';
import {withBoundedBodyPipe,restoreBoundedBodyPipe,TRANSPORT_SOURCE_SHA256} from '../../scripts/ci/native-upload-pipe-candidate.mjs';
import {browserModules} from '../../scripts/ci/run-native-upload-diagnostic.mjs';
const source=fs.readFileSync(new URL('../../platform-integrations/public-api/nostr-upload-request.ts',import.meta.url),'utf8');
const digest=value=>createHash('sha256').update(value).digest('hex');
const encoder=new TextEncoder(),flush=async()=>{for(let i=0;i<100;i++)await Promise.resolve();};
const deferred=()=>{let resolve;return {promise:new Promise(r=>{resolve=r;}),resolve};};

async function fixture({candidate=true,chunks=['{"x":','1}'],limit=7,declared=null,status=404,type='application/json',hold=false,streamError=false,close=true,delayedCancel=false,mutation}={}){
 let clock=0,alive=true,fetched=false,reads=0,cancels=0,aborts=0,writes=0,closes=0,pipeSettled=false,result,error,settled=false;
 const timers=new Map(),allocations=[],signals=[],readerCalls=[],entered=deferred(),cancelled=deferred();
 const external=new AbortController();let sourceController;
 const bytes=chunks.map(value=>typeof value==='string'?encoder.encode(value):value);
 const body=new ReadableStream({start(controller){sourceController=controller;},pull(controller){reads++;entered.resolve();if(streamError){controller.error(Error('finite stream error'));return;}
  if(hold)return; if(bytes.length)controller.enqueue(bytes.shift());else if(close)controller.close();
 },cancel(){cancels++;if(delayedCancel)return cancelled.promise;}}, {highWaterMark:0});
 const NativeArray=Uint8Array;
 const ArrayProbe=new Proxy(NativeArray,{construct(target,args){if(fetched)allocations.push(typeof args[0]==='number'?args[0]:args[0]?.byteLength??0);return Reflect.construct(target,args);}});
 class SinkProbe extends WritableStream{constructor(sink,...args){super({...sink,write(chunk){writes++;return sink.write(chunk);},close(){closes++;return sink.close?.();}},...args);}}
 const pipe=body.pipeTo;body.pipeTo=function(destination,options){signals.push(options?.signal);const work=Reflect.apply(pipe,this,[destination,options]);void work.then(()=>{pipeSettled=true;},()=>{pipeSettled=true;});return work;};
 const getReader=body.getReader;body.getReader=function(...args){readerCalls.push(1);return Reflect.apply(getReader,this,args);};
 const context=vm.createContext({URL,TextEncoder,TextDecoder,Uint8Array:ArrayProbe,AbortController,WritableStream:SinkProbe,performance,crypto,setTimeout,clearTimeout});
 let input=candidate?withBoundedBodyPipe(source):source;if(mutation)input=mutation(input);
 const module=new vm.SourceTextModule(stripTypeScriptTypes(input),{context});
 const nostr=new vm.SyntheticModule(['signExact','origin'],function(){this.setExport('signExact',()=>{throw Error('Uninjected signer');});this.setExport('origin',value=>new URL(value).origin);},{context});
 const codec=new vm.SyntheticModule(['hash','b64'],function(){this.setExport('hash',()=>{throw Error('No mutation hash expected');});this.setExport('b64',()=> 'finite');},{context});
 await module.link(name=>name.endsWith('/nostr.js')?nostr:codec);await module.evaluate();
 let fetchSignal;const identity={owner:'b'.repeat(64),generation:1,signer:{},check(){if(!alive)throw Error('obsolete authority');}};
 const request=module.namespace.createNostrUploadRequest({scope:{origin:'http://127.0.0.1:12345',serviceKey:'a'.repeat(64),catalogNamespace:'fixture'},identity,assertCurrent(){if(!alive)throw Error('obsolete owner');}},
  {signExact:async()=>({id:'c'.repeat(64)}),hash:()=>'',b64:()=>'',nonce:()=> 'finite_pipe_0123456789',now:()=>0,clock:()=>clock,
   setTimer(fn){const token={};timers.set(token,fn);return token;},clearTimer(token){timers.delete(token);},
   fetch:async(_url,options)=>{fetchSignal=options.signal;options.signal.addEventListener('abort',()=>aborts++);fetched=true;return {status,headers:new Headers({'content-type':type,...(declared===null?{}:{'content-length':declared})}),body};}});
 const pending=request({url:'http://127.0.0.1:12345/api/v1/upload',method:'GET',signal:external.signal,responseLimit:limit===null?undefined:limit,timeoutMs:1000,responseType:type.startsWith('image/')?'image':'json'}).then(value=>{result=value;settled=true;},cause=>{error=cause;settled=true;});
 return {pending,body,allocations,signals,readerCalls,timers,entered:entered.promise,identity,namespace:module.namespace,
  releaseCancel:cancelled.resolve,enqueue:bytes=>sourceController.enqueue(bytes),close:()=>sourceController.close(),abort:()=>external.abort(),invalidate:()=>{alive=false;},advance(){clock=1000;for(const fn of [...timers.values()])fn();},
  async finish(){await pending;await flush();assert.equal(body.locked,false,'Underlying stream lock must settle');assert.equal(timers.size,0);},
  get state(){return {reads,cancels,aborts,writes,closes,pipeSettled,result,error,settled,fetchSignal};}};
}
const refuses=f=>{assert.ok(f.state.error);assert.equal(f.state.error.name,'PublicUploadRequestError');assert.equal(f.state.error.status,404);assert.equal(f.state.error.bodyAvailable,false);assert.equal(f.state.aborts,1);};

test('candidate is an exactly reversible edit of the frozen production transport',()=>{
 assert.equal(digest(source),TRANSPORT_SOURCE_SHA256);const candidate=withBoundedBodyPipe(source);
 assert.equal(restoreBoundedBodyPipe(candidate),source);assert.match(candidate,/pipeTo\(sink, \{ signal: controller.signal \}\)/);assert.doesNotMatch(candidate,/getReader\(|reader\.read\(|releaseLock\(|chunks\.push|arrayBuffer\(/);
 assert.ok(candidate.indexOf('chunk.byteLength > limit - size')<candidate.indexOf('new Uint8Array(limit)'));
 for(const drift of [source+'\n',source.replace('limit - size','limit + size'),source.replace('if (!completed) controller.abort();','controller.abort();')])assert.throws(()=>withBoundedBodyPipe(drift));
 assert.throws(()=>restoreBoundedBodyPipe(candidate+'\n'));
 const served=browserModules().get('/modules/transport-pipe.js');assert.equal(served,stripTypeScriptTypes(candidate.replace('const uploadRequestRefusalRecording = false;','const uploadRequestRefusalRecording = true;'),{mode:'transform'}).replace(/from\s+(['"])[^'"]+\1/g,'from "/modules/stubs.js"'));
});

test('both source readers accept exact-cap chunked JSON and preserve actual status',async()=>{
 for(const candidate of [false,true]){const f=await fixture({candidate});await f.finish();assert.equal(f.state.result.body.x,1);assert.equal(f.state.result.status,404);assert.equal(f.state.aborts,0);assert.equal(f.state.cancels,0);
  assert.equal(f.signals.length,candidate?1:0);if(candidate){assert.equal(f.signals[0],f.state.fetchSignal);assert.equal(f.state.pipeSettled,true);assert.equal(f.state.closes,1);assert.equal(f.readerCalls.length,0);}else assert.equal(f.readerCalls.length,1);
 }
});

test('missing understated malformed and huge declared lengths never authorize overflow',async()=>{
 for(const candidate of [false,true])for(const declared of [null,'1','garbage','999','99999999999999999999999999999999']){const f=await fixture({candidate,chunks:['123','45'],limit:4,declared});await f.finish();refuses(f);assert.equal(f.namespace.uploadRequestRefusalStage(f.state.error),declared?.startsWith('999')?'declared-size':'body-budget');assert.equal(f.state.cancels,1);assert.ok(Math.max(0,...f.allocations)<=4);}
});

test('single oversized or wrong-type chunks are refused before backing allocation',async()=>{
 for(const chunk of [new Uint8Array(8),'bad-type']){const chunks=chunk==='bad-type'?[{unexpected:true}]:[chunk];const f=await fixture({chunks});await f.finish();refuses(f);assert.deepEqual(f.allocations,[]);assert.equal(f.state.cancels,1);}
});

test('fixed backing allocation removes old per-fragment and empty-fragment metadata growth',async()=>{
 const chunks=[...Array.from({length:2048},()=>''),...Array.from({length:128},()=> ' '),'{}'];
 const candidate=await fixture({chunks,limit:130});await candidate.finish();assert.ok(candidate.state.result);assert.deepEqual(candidate.allocations,[130,130]);assert.equal(candidate.state.aborts,0);
 const old=await fixture({candidate:false,chunks,limit:130});await old.finish();assert.ok(old.state.result);assert.equal(old.allocations.length,chunks.length+1);assert.ok(old.allocations.length>candidate.allocations.length);
});

test('zero cap admits empty204 and rejects one actual byte without allocation',async()=>{
 const good=await fixture({chunks:[],limit:0,status:204});await good.finish();assert.equal(good.state.result.body,undefined);assert.deepEqual(good.allocations,[0]);assert.equal(good.state.aborts,0);
 const bad=await fixture({chunks:['x'],limit:0});await bad.finish();refuses(bad);assert.deepEqual(bad.allocations,[]);
});

test('external cancellation and absolute deadline cancel a pending actual pipe and release its lock',async()=>{
 for(const mode of ['abort','deadline']){const f=await fixture({hold:true});await f.entered;assert.equal(f.body.locked,true);assert.equal(f.state.settled,false);if(mode==='abort')f.abort();else f.advance();await f.finish();refuses(f);assert.equal(f.state.cancels,1);assert.equal(f.state.pipeSettled,true);assert.equal(f.state.writes,0);assert.equal(f.state.closes,0);assert.deepEqual(f.allocations,[]);}
});

test('owner and signer invalidations prevent a subsequent sink write from copying',async()=>{
 for(const mode of ['owner','generation','signer']){const f=await fixture({hold:true});await f.entered;
  if(mode==='owner')f.invalidate();else if(mode==='generation')f.identity.generation++;else f.identity.signer={};
  f.enqueue(encoder.encode('{"x":1}'));await f.finish();refuses(f);assert.equal(f.namespace.uploadRequestRefusalStage(f.state.error),'checkpoint');assert.deepEqual(f.allocations,[]);assert.equal(f.state.cancels,1);
 }
});

test('owner invalidation at EOF cannot promote a completed body to success',async()=>{
 const f=await fixture({hold:true});await f.entered;f.enqueue(encoder.encode('{"x":1}'));await flush();assert.equal(f.state.writes,1);f.invalidate();f.close();await f.finish();refuses(f);assert.equal(f.namespace.uploadRequestRefusalStage(f.state.error),'checkpoint');assert.equal(f.state.closes,1);assert.equal(f.state.pipeSettled,true);
});

test('read failure and malformed bytes retain known status without retries',async()=>{
 for(const options of [{streamError:true},{chunks:['{bad']},{chunks:[new Uint8Array([255])]}]){const f=await fixture(options);await f.finish();refuses(f);assert.equal(f.namespace.uploadRequestRefusalStage(f.state.error),options.streamError?'body-read':'decode');assert.equal(f.signals.length,1);}
});

test('overflow and missing-signal counterfactuals are caught by byte and cancellation controls',async()=>{
 const overflow=await fixture({chunks:['{"x":1}'],limit:4,mutation:s=>s.replace('chunk.byteLength > limit - size','false').replace('new Uint8Array(limit)','new Uint8Array(limit + 3)')});await overflow.finish();assert.ok(overflow.state.result);assert.ok(overflow.allocations.some(size=>size>4));
 const uncancelled=await fixture({hold:true,mutation:s=>s.replace('pipeTo(sink, { signal: controller.signal })','pipeTo(sink)')});await uncancelled.entered;uncancelled.abort();await uncancelled.pending;await flush();assert.equal(uncancelled.state.settled,true);assert.equal(uncancelled.state.cancels,0);assert.equal(uncancelled.body.locked,true);assert.equal(uncancelled.state.pipeSettled,false);
 // Explicitly release even the intentionally broken finite control.
 uncancelled.close();await flush();assert.equal(uncancelled.body.locked,false);assert.equal(uncancelled.state.pipeSettled,true);assert.equal(uncancelled.timers.size,0);
});

test('candidate preserves exact binary image bytes and both hard default caps',async()=>{
 const png=new Uint8Array([137,80,78,71,13,10,26,10]);const image=await fixture({chunks:[png.subarray(0,3),png.subarray(3)],limit:8,status:200,type:'image/png'});await image.finish();assert.deepEqual(image.state.result.body,png);assert.deepEqual(image.allocations,[8,8]);assert.equal(image.state.aborts,0);
 for(const [type,cap] of [['application/json',1024*1024],['image/png',32*1024*1024]]){const f=await fixture({chunks:[new Uint8Array(cap+1)],limit:null,status:200,type});await f.finish();assert.equal(f.state.error.status,200);assert.equal(f.namespace.uploadRequestRefusalStage(f.state.error),'body-budget');assert.equal(f.state.aborts,1);assert.equal(f.state.cancels,1);assert.deepEqual(f.allocations,[]);}
});


test('guarded abort cannot imply settlement while underlying cancellation is delayed',async()=>{
 const unhandled=[];const record=reason=>unhandled.push(reason);process.on('unhandledRejection',record);
 try{const f=await fixture({hold:true,delayedCancel:true});await f.entered;f.abort();await f.pending;await flush();
  refuses(f);assert.equal(f.state.settled,true);assert.equal(f.state.cancels,1);assert.equal(f.state.pipeSettled,false);assert.equal(f.body.locked,true);
  f.releaseCancel();await f.finish();await new Promise(resolve=>setImmediate(resolve));
  assert.equal(f.state.cancels,1);assert.equal(f.state.pipeSettled,true);assert.equal(f.body.locked,false);assert.deepEqual(unhandled,[]);
 }finally{process.removeListener('unhandledRejection',record);}
});
