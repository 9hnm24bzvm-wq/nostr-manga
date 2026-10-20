// Synthetic authority primitives, actual adapter/transport, actual browser fetch.
// No user key, account, database, or production service is accessed.
import {createPublicUploadAdapter} from '/modules/adapter.js';
import {createNostrUploadRequest,readUploadRequestRefusals} from '/modules/transport.js';
const SERVICE='a'.repeat(64),ACTOR='b'.repeat(64),USER='33333333-3333-8333-8333-333333333333',MANGA='11111111-1111-8111-8111-111111111111',NAMESPACE='66666666-6666-8666-8666-666666666666';
let generation=1,owner,application='pending';const destroys=[];const fail=()=>{throw Error('Synthetic unsupported primitive');};
const hex=b=>[...new Uint8Array(b)].map(x=>x.toString(16).padStart(2,'0')).join('');
const sign=async(_signer,pubkey,event)=>{const id=hex(await crypto.subtle.digest('SHA-256',new TextEncoder().encode(JSON.stringify([0,pubkey,event.created_at,event.kind,event.tags,event.content]))));return {...event,pubkey,id,sig:'c'.repeat(128)};};
const storage={get:()=>null,set:fail,delete:fail};
// Transparent observers retain native receiver, return identity and original promise.
export function installLifecycleObservation(realm=globalThis){
 const signals=new WeakSet(),bodies=new WeakSet(),readers=new WeakSet();let truncated=false;
 const names=['controller-abort','signal-abort','reader-cancel','reader-release','body-eof','fetch-resolved','current-return','current-rejected','native-terminal-observed'];
 const counts=Object.fromEntries(names.map(name=>[name,0])),order=[];
 const mark=name=>{if(!Object.hasOwn(counts,name))throw Error('Invalid observation');if(counts[name]<128)counts[name]++;else truncated=true;if(order.length<32)order.push(name);else truncated=true;};
 const wrap=(prototype,name,observer)=>{const original=prototype[name];prototype[name]=function(...args){return observer.call(this,original,args);};};
 wrap(realm.AbortController.prototype,'abort',function(original,args){if(signals.has(this.signal))mark('controller-abort');return Reflect.apply(original,this,args);});
 wrap(realm.ReadableStream.prototype,'getReader',function(original,args){const reader=Reflect.apply(original,this,args);if(bodies.has(this))readers.add(reader);return reader;});
 wrap(realm.ReadableStreamDefaultReader.prototype,'read',function(original,args){const result=Reflect.apply(original,this,args);if(readers.has(this))void result.then(part=>{if(part.done)mark('body-eof');},()=>{});return result;});
 for(const [method,event]of [['cancel','reader-cancel'],['releaseLock','reader-release']])wrap(realm.ReadableStreamDefaultReader.prototype,method,function(original,args){if(readers.has(this))mark(event);return Reflect.apply(original,this,args);});
 return {mark,fetch(original,url,options){signals.add(options.signal);options.signal.addEventListener('abort',()=>mark('signal-abort'),{once:true});const result=Reflect.apply(original,realm,[url,options]);void result.then(response=>{mark('fetch-resolved');if(response.body)bodies.add(response.body);},()=>{});return result;},snapshot(){return {state:'available',truncated,counts:{...counts},order:[...order]};}};
}
const observation=installLifecycleObservation();
const api=createPublicUploadAdapter({capabilities:async()=>({available:true,schema_ready:true,origin:location.origin,service_key:SERVICE,catalog_namespace:NAMESPACE,connect_origins:[],external_signer_required:true,account_admission_required:true}),publicOrigin:()=>location.origin,proveService:async()=>({origin:location.origin,serviceKey:SERVICE}),selectedSigner:()=>{const captured=generation;return {owner:ACTOR,generation:captured,signer:{signEvent:fail},check:()=>{if(captured!==generation)throw Error('Synthetic identity changed');}};},createRequest:config=>createNostrUploadRequest(config,{signExact:sign,hash:fail,b64:bytes=>btoa(String.fromCharCode(...bytes)),fetch:(url,options)=>{if(new URL(url).origin!==location.origin||options.method!=='GET')throw Error('Synthetic read boundary');return new URL(url).pathname==='/api/v1/upload'?observation.fetch(fetch,url,options):fetch(url,options);},nonce:()=>crypto.randomUUID()}),catalogueRead:fail,multipart:fail,digest:fail,decodeImage:fail,destroy:callback=>destroys.push(callback),newAttempt:fail,fences:storage,replacementFences:storage,replacementLock:fail,replacementMultipart:fail});
const ledger=()=>{const value=readUploadRequestRefusals();return {state:value.enabled&&value.record_valid&&!value.truncated?'available':'unavailable',count:value.records.length,stages:value.records.map(r=>r.stage)};};
// Test-owned strong references remain live through native terminal and context close.
const retained=[];
export async function consumeBareResponse(response,mode,retain){
 retain.response=response;let text;
 if(mode==='bare-reader-release'||mode==='bare-reader-retained'){
  const reader=response.body.getReader();retain.reader=reader;const chunks=[];let size=0;
  for(;;){const part=await reader.read();if(part.done)break;size+=part.value.byteLength;if(size>4096)throw Error('Synthetic body budget');chunks.push(part.value);}
  const bytes=new Uint8Array(size);let offset=0;for(const chunk of chunks){bytes.set(chunk,offset);offset+=chunk.byteLength;}
  text=new TextDecoder().decode(bytes);if(mode==='bare-reader-release')reader.releaseLock();
 }else if(mode==='bare-text')text=await response.text();
 else if(mode==='bare-array-buffer')text=new TextDecoder().decode(await response.arrayBuffer());
 else throw Error('Unknown synthetic consumer');
 if(response.status!==404||response.headers.get('content-type')!=='application/json'||new TextEncoder().encode(text).byteLength!==18||text!=='{"result":"error"}')throw Error('Unexpected synthetic response');
 return undefined;
}
async function bareCurrent(mode){const url=location.origin+'/api/v1/upload?expectedAccountId='+USER;const event=await sign(null,ACTOR,{kind:27235,created_at:Math.floor(Date.now()/1000),content:'',tags:[['u',url],['method','GET'],['service',SERVICE],['nonce',crypto.randomUUID()]]});const controller=new AbortController(),keep={controller};retained.push(keep);const response=await observation.fetch(fetch,url,{method:'GET',headers:{authorization:'Nostr '+btoa(JSON.stringify(event)),accept:'application/json'},credentials:'omit',redirect:'error',cache:'no-store',signal:controller.signal});return consumeBareResponse(response,mode,keep);}
window.diagnostic={ready:false,ledger,observation:()=>observation.snapshot(),nativeObserved:()=>observation.mark('native-terminal-observed'),get application(){return application;},start(mode='adapter-completed-404'){application='pending';void (mode.startsWith('adapter-')?api.currentUpload(owner):bareCurrent(mode)).then(value=>{observation.mark('current-return');application=value===undefined?'missing':'rejected';},()=>{observation.mark('current-rejected');application='rejected';});}};
const account=await api.loadUploadAccount();owner=api.publicUploadOwner(()=>MANGA,account);window.diagnostic.ready=true;
