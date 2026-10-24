// Finite synthetic response/stream fixtures. No HTTP, DB or real signing.
import test from 'node:test';
import assert from 'node:assert/strict';
import {id,threadId,plain,targetBody,contextFixture,contextModule,readerModule} from './native-discussion-context-fixture.mjs';
const deferred=()=>{let release,reject;return{promise:new Promise((resolve,no)=>{release=resolve;reject=no;}),release,reject};};
const turns=async(count=40)=>{for(let i=0;i<count;i++)await Promise.resolve();};
for(const kind of ['manga','chapter','group'])test('persisted '+kind+' context selects only its exact typed UUID and native thread',async()=>{
 const f=contextFixture({kind}),value=await f.read();
 assert.deepEqual(plain(value),{kind,targetId:id,threadId});assert.equal(f.calls.length,1);
 assert.equal(f.calls[0].url,'https://catalog.example/api/v1/nostr/native-forum/threads/'+threadId+'/context');
 assert.equal(f.calls[0].request.method,'GET');assert.equal(f.calls[0].request.credentials,'omit');assert.equal(f.calls[0].request.redirect,'error');assert.equal(f.calls[0].request.cache,'no-store');assert.equal(f.calls[0].request.signal.aborted,false);assert.equal(f.timers.size,0);
});
test('404 means no persisted signed target, without alias creation or retry',async()=>{const f=contextFixture({status:404});assert.equal(await f.read(),null);assert.equal(f.calls.length,1);assert.equal(f.timers.size,0);});
for(const status of [401,403,409,429,500,503])test('target lookup '+status+' stays unavailable, never retries with cookies',async()=>{const f=contextFixture({status});await assert.rejects(()=>f.read(),error=>error.status===status);assert.equal(f.calls.length,1);assert.equal(f.calls[0].request.credentials,'omit');assert.equal(f.timers.size,0);});
for(const change of [body=>body.nostr.source='guessed-thread-target',body=>body.data.threadId='another-thread',body=>body.data.kind='user',body=>body.data.kind='publisher',body=>body.data.id='native-short-id',body=>body.result='error',body=>delete body.data,body=>delete body.nostr])test('untrusted target is rejected: '+String(change),async()=>{const body=targetBody();change(body);const f=contextFixture({body});await assert.rejects(()=>f.read());assert.equal(f.calls.length,1);assert.equal(f.timers.size,0);});
test('explicit typed target validation rejects changed thread, kind and fabricated UUID',()=>{assert.deepEqual(plain(contextModule.nativeDiscussionTarget({kind:'manga',targetId:id,threadId},threadId)),{kind:'manga',targetId:id,threadId});for(const value of [{kind:'manga',targetId:id,threadId:'different'},{kind:'user',targetId:id,threadId},{kind:'manga',targetId:'native-short-id',threadId}])assert.throws(()=>contextModule.nativeDiscussionTarget(value,threadId));});
for(const value of ['', '../escape', 'contains space', 'x'.repeat(129)])test('invalid thread is rejected before dispatch '+JSON.stringify(value),async()=>{const f=contextFixture({threadId:value});await assert.rejects(()=>f.read());assert.equal(f.calls.length,0);});
test('already aborted context lookup does not dispatch',async()=>{const f=contextFixture(),abort=new AbortController();abort.abort();await assert.rejects(()=>f.read(abort.signal),error=>error.status===499);assert.equal(f.calls.length,0);});
for(const change of ['signal','target','origin'])test(change+' change discards a late target without retry',async()=>{const gate=deferred(),f=contextFixture({gate}),abort=new AbortController(),pending=f.read(abort.signal);await turns();assert.equal(f.calls.length,1);if(change==='signal')abort.abort();else if(change==='target')f.invalidate();else f.changeOrigin();gate.release();await assert.rejects(()=>pending);assert.equal(f.calls.length,1);assert.equal(f.timers.size,0);});
for(const variant of ['declared','stream','utf8','json','media','missing'])test('bounded context rejects '+variant+' payload',async()=>{
 let cancelled=false,response;
 if(variant==='declared')response=new Response('{}',{headers:{'content-type':'application/json','content-length':String(16*1024+1)}});
 if(variant==='stream')response=new Response(new ReadableStream({start(controller){controller.enqueue(new Uint8Array(16*1024+1));},cancel(){cancelled=true;}}),{headers:{'content-type':'application/json'}});
 if(variant==='utf8')response=new Response(new Uint8Array([255]),{headers:{'content-type':'application/json'}});
 if(variant==='json')response=new Response('{broken',{headers:{'content-type':'application/json'}});
 if(variant==='media')response=new Response('{}',{headers:{'content-type':'text/html; application/json'}});
 if(variant==='missing')response=new Response(null,{headers:{'content-type':'application/json'}});
 const f=contextFixture({response});await assert.rejects(()=>f.read());assert.equal(f.calls.length,1);assert.equal(f.timers.size,0);if(variant==='stream')assert.ok(cancelled);
});
for(const stage of ['fetch','stream'])test('deadline bounds a stalled '+stage+' and observes late cancellation',async()=>{
 let cancelled=false;const gate=stage==='fetch'?{promise:new Promise(()=>{})}:undefined;
 const response=stage==='stream'?new Response(new ReadableStream({pull(){return new Promise(()=>{});},cancel(){cancelled=true;}}),{headers:{'content-type':'application/json'}}):undefined;
 const f=contextFixture({gate,response}),pending=f.read();await turns();f.expire();await turns();f.expire(250);await assert.rejects(()=>pending,error=>error.status===504);assert.equal(f.timers.size,0);assert.equal(f.calls.length,1);assert.equal(f.calls[0].request.signal.aborted,true);if(stage==='stream')assert.ok(cancelled);
});
test('stream cancellation settles before lookup releases ownership',async()=>{const gate=deferred();let started=false,finished=false;const response=new Response(new ReadableStream({pull(){return new Promise(()=>{});},cancel(){started=true;return gate.promise;}}),{headers:{'content-type':'application/json'}}),f=contextFixture({response}),abort=new AbortController(),pending=f.read(abort.signal).finally(()=>finished=true);await turns();abort.abort();await turns();assert.ok(started);assert.equal(finished,false);gate.release();await assert.rejects(()=>pending,error=>error.status===499&&!error.message.includes('unconfirmed'));assert.equal(f.timers.size,0);});
test('successful EOF does not abort or cancel the healthy response',async()=>{const response=new Response(JSON.stringify(targetBody()),{headers:{'content-type':'application/json'}}),f=contextFixture({response}),abort=new AbortController();let cancelled=0;const original=response.body.cancel.bind(response.body);response.body.cancel=(...args)=>{cancelled++;return original(...args);};assert.equal((await f.read(abort.signal)).threadId,threadId);abort.abort();assert.equal(f.calls[0].request.signal.aborted,false);assert.equal(response.body.locked,false);assert.equal(cancelled,0);assert.equal(f.timers.size,0);});

const limits=readerModule.NATIVE_DISCUSSION_CONTEXT_LIMITS;
const unsettled=error=>error instanceof readerModule.NativeDiscussionContextUnsettled&&error.unsettled===true&&/New lookups are stopped/.test(error.message);
const encoded=()=>new TextEncoder().encode(JSON.stringify(targetBody()));
function controlledResponse({status=200,readGate,cancelGate,parts=[encoded()],releaseFailure=false}={}){
 const calls={read:0,cancel:0,release:0,getReader:0};let index=0;
 const reader={read(){calls.read++;return readGate?.promise??Promise.resolve(index<parts.length?{done:false,value:parts[index++]}:{done:true});},cancel(){calls.cancel++;return cancelGate?.promise??Promise.resolve();},releaseLock(){calls.release++;if(releaseFailure)throw Error('Still locked');}};
 const response={status,headers:new Headers({'content-type':'application/json'}),body:{getReader(){calls.getReader++;return reader;}}};
 return{calls,response};
}
async function blocked(f){const count=f.calls.length;for(let i=0;i<4;i++)await assert.rejects(()=>f.read(undefined,'another-native-thread'),unsettled);assert.equal(f.calls.length,count,'No new request may dispatch while old work owns its slot');}
for(const stage of ['fetch','read','cancel'])test('one owned slot blocks concurrent lookups during '+stage,async()=>{
 const fetchGate=stage==='fetch'?deferred():undefined,readGate=stage==='read'?deferred():undefined,cancelGate=stage==='cancel'?deferred():undefined;
 const r=controlledResponse({readGate,cancelGate,status:stage==='cancel'?404:200}),f=contextFixture({gate:fetchGate,response:r.response}),abort=new AbortController(),pending=f.read(abort.signal);
 await turns();await blocked(f);abort.abort();await turns();fetchGate?.release();readGate?.release({done:true});cancelGate?.release();await assert.rejects(()=>pending);assert.equal(f.timers.size,0);
});
for(const outcome of ['fulfillment','rejection'])test('late fetch '+outcome+' after grace keeps ownership through eventual body disposal',async()=>{
 const fetchGate=deferred(),cancelGate=deferred(),r=controlledResponse({cancelGate}),f=contextFixture({gate:fetchGate,response:count=>count===1?r.response:undefined}),abort=new AbortController(),pending=f.read(abort.signal);
 await turns();abort.abort();await turns();f.expire(250);await assert.rejects(()=>pending,unsettled);await blocked(f);assert.equal(r.calls.getReader,0);assert.equal(r.calls.cancel,0);
 if(outcome==='fulfillment'){
  fetchGate.release();await turns();assert.equal(r.calls.cancel,1);assert.equal(r.calls.release,0);await blocked(f);
  cancelGate.release();await turns();assert.equal(r.calls.release,1);assert.equal((await f.read()).threadId,threadId);assert.equal(f.calls.length,2);
 }else{
  fetchGate.reject(Error('Late rejected fetch'));await turns();assert.equal(r.calls.cancel,0);
  // The same rejected synthetic dependency may fail again, but the old owner
  // no longer blocks dispatch once its actual promise has settled.
  await assert.rejects(()=>f.read(),/Late rejected fetch/);assert.equal(f.calls.length,2);
 }
 assert.equal(f.timers.size,0);
});
test('late fetch during grace cannot omit a newly started pending body cancellation',async()=>{
 const fetchGate=deferred(),cancelGate=deferred(),r=controlledResponse({cancelGate}),f=contextFixture({gate:fetchGate,response:count=>count===1?r.response:undefined}),abort=new AbortController();let finished=false;
 const pending=f.read(abort.signal).finally(()=>finished=true);await turns();abort.abort();await turns();fetchGate.release();await turns();
 assert.equal(r.calls.cancel,1);assert.equal(finished,false);f.expire(250);await assert.rejects(()=>pending,unsettled);await blocked(f);assert.equal(r.calls.release,0);
 cancelGate.release();await turns();assert.equal(r.calls.release,1);assert.equal((await f.read()).threadId,threadId);
});
for(const first of ['read','cancel'])test('pending read and cancel both remain owned after grace when '+first+' settles first',async()=>{
 const readGate=deferred(),cancelGate=deferred(),r=controlledResponse({readGate,cancelGate}),f=contextFixture({response:count=>count===1?r.response:undefined}),abort=new AbortController(),pending=f.read(abort.signal);
 await turns();abort.abort();await turns();f.expire(250);await assert.rejects(()=>pending,unsettled);assert.equal(r.calls.cancel,1);await blocked(f);
 if(first==='read')readGate.release({done:true});else cancelGate.release();await turns();assert.equal(r.calls.release,0);await blocked(f);
 if(first==='read')cancelGate.release();else readGate.release({done:true});await turns();assert.equal(r.calls.release,1);assert.equal((await f.read()).threadId,threadId);assert.equal(f.calls.length,2);
});
for(const failure of ['read','cancel'])test('late '+failure+' rejection is observed before releasing the stopped slot',async()=>{
 const readGate=deferred(),cancelGate=deferred(),r=controlledResponse({readGate,cancelGate}),f=contextFixture({response:count=>count===1?r.response:undefined}),abort=new AbortController(),pending=f.read(abort.signal);
 await turns();abort.abort();await turns();f.expire(250);await assert.rejects(()=>pending,unsettled);
 if(failure==='read'){readGate.reject(Error('Late read failure'));cancelGate.release();}else{readGate.release({done:false,value:encoded()});cancelGate.reject(Error('Late cancellation failure'));}
 await turns();assert.equal(r.calls.release,1);assert.equal((await f.read()).threadId,threadId);assert.equal(f.calls.length,2);
});
for(const afterGrace of [false,true])test('404 keeps body cancellation owned '+(afterGrace?'after grace':'through successful cleanup'),async()=>{
 const cancelGate=deferred(),r=controlledResponse({status:404,cancelGate}),f=contextFixture({response:count=>count===1?r.response:undefined});let finished=false;
 const pending=f.read().finally(()=>finished=true);await turns();assert.equal(r.calls.cancel,1);assert.equal(r.calls.read,0);assert.equal(finished,false);await blocked(f);
 if(afterGrace){f.expire(250);await assert.rejects(()=>pending,unsettled);await blocked(f);}
 cancelGate.release();await turns();if(!afterGrace)assert.equal(await pending,null);
 assert.equal(r.calls.release,1);assert.equal(f.calls[0].request.signal.aborted,false);assert.equal((await f.read()).threadId,threadId);
});
for(const variant of ['json','provenance','utf8'])test('clean EOF with rejected '+variant+' never aborts or cancels transport',async()=>{
 const body=targetBody();body.nostr.source='untrusted';const bytes=variant==='utf8'?new Uint8Array([255]):new TextEncoder().encode(variant==='json'?'{bad':JSON.stringify(body));
 const r=controlledResponse({parts:[bytes]}),f=contextFixture({response:count=>count===1?r.response:undefined}),abort=new AbortController();
 await assert.rejects(()=>f.read(abort.signal));abort.abort();assert.equal(r.calls.cancel,0);assert.equal(r.calls.release,1);assert.equal(f.calls[0].request.signal.aborted,false);assert.equal((await f.read()).threadId,threadId);
});
for(const variant of ['fragments','emptyFragments'])test('fixed buffer independently bounds '+variant+' before additional reads',async()=>{
 const count=limits[variant]+1,part=variant==='fragments'?new Uint8Array([32]):new Uint8Array(),r=controlledResponse({parts:Array.from({length:count},()=>part)}),f=contextFixture({response:counter=>counter===1?r.response:undefined});
 await assert.rejects(()=>f.read(),variant==='fragments'?/fragment budget/:/empty-fragment budget/);assert.equal(r.calls.read,variant==='fragments'?limits.fragments:limits.emptyFragments+1);assert.equal(r.calls.cancel,1);assert.equal(r.calls.release,1);assert.equal((await f.read()).threadId,threadId);
});
test('bounded tiny fragments and the allowed empty-fragment count preserve exact payload and clean EOF',async()=>{
 const parts=[...Array.from({length:limits.emptyFragments},()=>new Uint8Array()),...Array.from(encoded(),byte=>new Uint8Array([byte]))],r=controlledResponse({parts}),f=contextFixture({response:r.response});
 assert.equal((await f.read()).threadId,threadId);assert.equal(r.calls.cancel,0);assert.equal(r.calls.release,1);assert.equal(f.calls[0].request.signal.aborted,false);
});
test('failed reader release fails closed instead of allowing detached replacement requests',async()=>{
 const r=controlledResponse({releaseFailure:true}),f=contextFixture({response:r.response});await assert.rejects(()=>f.read(),unsettled);await blocked(f);assert.equal(r.calls.cancel,0);assert.equal(r.calls.release,1);
});
