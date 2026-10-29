// Actual native clients and byte transport. Synthetic HTTP/Session boundary only;
// no external network, real account, credentials, signing or publication.
import fs from'node:fs';import path from'node:path';import vm from'node:vm';import{stripTypeScriptTypes}from'node:module';import{fileURLToPath}from'node:url';import{createHash}from'node:crypto';import assert from'node:assert/strict';
export const root=path.resolve(path.dirname(fileURLToPath(import.meta.url)),'../..'),dir=path.join(root,'platform-integrations/public-api');
export let signerActivations=0;const forbidden=()=>{signerActivations++;throw Error('Signer activation forbidden');};
const context=vm.createContext({URL,URLSearchParams,TextEncoder,TextDecoder,Uint8Array,ArrayBuffer,Blob,AbortController,AbortSignal,Error,Date,Number,Response,crypto,structuredClone,fetch(){throw Error('External fetch forbidden');}}),cache=new Map();
class RequestFailure extends Error{constructor(message,outcome,status){super(message);this.outcome=outcome;this.status=status;this.bodyAvailable=false;}}
function stub(name,exports){if(!cache.has(name))cache.set(name,new vm.SyntheticModule(Object.keys(exports),function(){for(const[k,v]of Object.entries(exports))this.setExport(k,v);},{context,identifier:name}));return cache.get(name);}
export async function load(name){const file=path.join(dir,name+'.ts');if(cache.has(file))return cache.get(file);const m=new vm.SourceTextModule(stripTypeScriptTypes(fs.readFileSync(file,'utf8'),{mode:'transform'}),{context,identifier:file});cache.set(file,m);await m.link(async(specifier,importer)=>{
 if(specifier==='svelte')return stub(specifier,{onDestroy:()=>{},createContext:()=>{let v;return[()=>v,x=>v=x];}});
 if(specifier==='svelte/store')return stub(specifier,{get:()=>({identity:{id:'Reader1234'}})});
 if(specifier==='$lib/stores')return stub(specifier,{Session:{}});
 if(specifier==='$lib/constants/Error')return stub(specifier,{Error:class extends Error{constructor(title,description){super(title+': '+description);this.title=title;this.description=description;}}});
 if(specifier.includes('local-library/publication'))return stub(specifier,{capabilities:forbidden,selectedSigner:forbidden});
 if(specifier.includes('protocol/nostr.js'))return stub(specifier,{origin:x=>new URL(x).origin,proveService:forbidden});
 if(specifier.includes('protocol/codec.js'))return stub(specifier,{hash:bytes=>createHash('sha256').update(bytes).digest('hex')});
 if(specifier==='./nostr-upload-request')return stub(specifier,{createNostrUploadRequest:forbidden,PublicUploadRequestError:RequestFailure});
 if(specifier==='./site-client')return stub(specifier,{publicCatalogueRead:()=>{throw Error('Explicit fixture catalogue needed');}});
 return load(path.relative(dir,path.resolve(path.dirname(importer.identifier),specifier)).replace(/\.ts$/,''));
 });return m;}
export async function api(name){const m=await load(name);await m.evaluate();return m.namespace;}
export const title=await api('session-title-client'),drafts=await api('session-draft-client'),covers=await api('session-cover-client'),relations=await api('session-relation-client'),uploads=await api('native-upload-client'),transport=await api('native-contribution-scope');
export const uuid=x=>`${x.repeat(8)}-${x.repeat(4)}-8${x.repeat(3)}-8${x.repeat(3)}-${x.repeat(12)}`,MANGA=uuid('1'),SESSION=uuid('2'),USER=uuid('3'),FILE=uuid('4'),CHAPTER=uuid('5'),NAMESPACE=uuid('6'),SECOND_FILE=uuid('7'),SECOND_SESSION=uuid('8'),NATIVE='Reader1234',ORIGIN='https://site.example';
export const PNG=Uint8Array.from([137,80,78,71,13,10,26,10,1,2,3,4]),digest=bytes=>createHash('sha256').update(bytes).digest('hex'),clone=x=>structuredClone(x);
export const image=(name='page.png')=>({name,type:'image/png',size:PNG.length,arrayBuffer:async()=>PNG.slice().buffer}),localPage=(name='page.png')=>({id:'local-page',file:image(name),original_file_name:name,file_hash:'',file_name:'',file_ext:'.png',file_size:String(PNG.length),thumbnail:'blob:synthetic-local-preview'});
export const marker={scope:'instance-account',authentication:'native-session',accountId:USER};
export const titleRow=(state='draft',version=1)=>({id:MANGA,type:'manga',attributes:{title:{ja:'A title'},description:{ja:'A description'},altTitles:[],originalLanguage:'ja',status:'ongoing',contentRating:'safe',year:null,publicationDemographic:null,state,version,tags:[]},relationships:[{id:USER,type:'creator'}]});
export const entity=(row,status=200)=>({status,body:{result:'ok',response:'entity',data:row,...(status<400?{nostr:marker}:{})}});
export const response=(body,status=200,mime='application/json')=>({status,body,headers:{'content-type':mime,'cache-control':'private, no-store',vary:'Cookie, Authorization, X-Weebdex-Expected-Account','x-content-type-options':'nosniff'}}),refusal=(status,detail='Permission refused')=>response({result:'error',errors:[{status,title:'Refused',detail}]},status);
export function chapterResource(version=2,pending=false){return{id:CHAPTER,type:'chapter',attributes:{version,title:null,volume:null,chapter:'1',translatedLanguage:'en',pages:1,isUnavailable:pending,externalUrl:null,nostr:{publicationScope:'instance-local',publicationStatus:pending?'accepted_pending_projection':'materialized'}},relationships:[{id:MANGA,type:'manga'}]};}
export function chapterView(receipt=chapterResource()){return{id:receipt.id,version:receipt.attributes.version,title:null,chapter:'1',language:'en',public_api:{resource:receipt},relationships:{manga:{id:MANGA}},is_unavailable:receipt.attributes.isUnavailable,external_url:null,data:[{name:'original.png',public_url:'/data/exact/original.png',dimensions:[2,3]}]};}
export function session(id=SESSION){return{id,createdAt:'2026-10-28T01:02:03Z',updatedAt:'2026-10-28T02:03:04Z',mangaId:MANGA,chapterId:null,groupIds:[],version:1,draft:{},pageOrder:[],files:[],canMutate:true,canCancel:true,canRetryCommit:true,isCommitted:false,isProcessed:false,isDeleted:false,isExpired:false,expiresAt:'2026-10-29T00:00:00Z',state:'open',publicationStatus:'unprepared',targetStatus:'available',receiptStatus:'none',receipt:null};}
export function staged(id=FILE,name='page.png',sessionId=SESSION){return{id,originalFileName:name,fileHash:digest(PNG),fileSize:PNG.length,mimeType:'image/png',dimensions:{width:2,height:3},previewUrl:`/api/v1/account/uploads/${sessionId}/files/${id}/preview`};}
export function fixture(options={}){
 const calls=[],storage=options.storage??new Map(),state={active:true,generation:0,native:NATIVE,title:titleRow(),sessions:new Map(),current:undefined,beginCount:0,attachCount:0,commits:0,...options.state};
 const permissions=['manga.create','chapter.create','self.chapter.update','manga.relation.edit','manga.cover.edit'];
 const account={result:'ok',user:{id:USER,username:'Reader',version:7,roles:[],permissions},nostr:{...marker,catalogNamespace:NAMESPACE,nativeSessionAuthority:true,catalogueScope:'instance-local'},upload:{canBegin:true,canManage:true,conditional:true,publication:{configured:false,requiresOperatorOrAuthorizedGroup:true,scope:'instance-local'},limits:{maxFiles:500,maxFileBytes:25*1024*1024}}};
 options.account?.(account);
 const check=()=>{if(!state.active)throw Error('View/account changed');};
 const fences={get:k=>storage.get(k)??null,set:(k,v)=>storage.set(k,v),delete:k=>storage.delete(k)};
 const defaultRequest=async request=>{
  const {route,method,body}=request;
  if(route==='/me')return response({result:'ok',response:'entity',data:{id:USER,type:'user',attributes:{username:'Reader',isVerified:options.verified!==false,permissions,roles:[],version:7}},nostr:{...marker,identity:'native-session-account',nativeSessionAuthority:true}});
  if(route==='/contribution-context')return response(clone(account));
  if(route==='/manga'&&method==='POST'){state.title=titleRow();state.title.attributes.title=body.title;state.title.attributes.originalLanguage=body.originalLanguage;if(body.description)state.title.attributes.description=body.description;return entity(clone(state.title));}
  if(route==='/manga/'+MANGA&&method==='PUT'){assert.equal(body.expectedAccountId,USER);assert.equal(body.version,state.title.attributes.version);state.title.attributes.version++;for(const k of ['title','description'])if(body[k])state.title.attributes[k]=body[k];return entity(clone(state.title));}
  if(route==='/manga/'+MANGA&&method==='GET')return state.title.attributes.state==='published'?entity(clone(state.title)):refusal(404);
  if(route==='/manga/draft/'+MANGA)return entity(clone(state.title));
  if(route==='/manga/draft')return response({result:'ok',response:'collection',data:[clone(state.title)],limit:36,offset:0,total:1});
  if(route==='/manga/draft/'+MANGA+'/commit'){state.title.attributes.version++;state.title.attributes.state='submitted';return entity(clone(state.title),201);}
  if(route==='/manga/'+MANGA+'/relations'){const q=request.url.searchParams,v=Number(q.get('ownerVersion')),snapshot='a'.repeat(64);assert.equal(q.get('limit'),'32');assert.equal(q.get('offset'),'0');assert.equal(q.get('expectedAccountId'),USER);return response({result:'ok',response:'collection',data:[],limit:32,offset:0,total:0,persistedTotal:0,nextOffset:null,ownerMangaId:MANGA,ownerVersion:v,version:v,snapshot,nostr:{versionScope:'owner_manga',pagination:'bounded-relations-v1',snapshot,identity:{kind:'manga',requestedId:MANGA,canonicalId:MANGA,source:'persisted-public-resource-ids'}}});}
  if(route==='/manga/'+MANGA+'/covers'){const q=request.url.searchParams;if([...q.keys()].some(key=>!['limit','offset','expectedAccountId'].includes(key))||q.size!==3||q.get('limit')!=='100'||q.get('expectedAccountId')!==USER||!/^\d+$/.test(q.get('offset')??''))return refusal(400,'Invalid cover query');return response({result:'ok',response:'collection',data:[],total:0,limit:100,offset:Number(q.get('offset'))});}
  if(route==='/upload'&&method==='GET'){const current=state.sessions.get(state.current);return current?response({id:current.id,type:'upload_session',attributes:{version:current.version,isCommitted:current.isCommitted,isProcessed:current.isProcessed,isDeleted:current.isDeleted}}):refusal(404);}
  if(route==='/upload/begin'){const id=state.beginCount++?SECOND_SESSION:SESSION,current=session(id);state.sessions.set(id,current);state.current=id;return response({id,type:'upload_session',attributes:{version:1,isCommitted:false,isProcessed:false,isDeleted:false}});}
  if(route==='/upload/check-approval-required')return response({result:'ok',requiresApproval:false});
  const exact=/^\/uploads\/([^/]+)$/.exec(route);if(exact){const current=state.sessions.get(exact[1]);if(!current)return refusal(404);if(method==='PATCH'){if(body.version!==current.version)return refusal(409);current.draft=clone(body.draft);current.pageOrder=clone(body.pageOrder);current.version++;}return response({result:'ok',session:clone(current)});}
  if(route.endsWith('/preview'))return response(PNG.slice(),200,'image/png');
  const upload=/^\/upload\/([^/]+)(?:\/([^/]+))?$/.exec(route);if(upload){const current=state.sessions.get(upload[1]);if(!current)return refusal(404);
   if(method==='POST'&&!upload[2]){const id=state.attachCount++?SECOND_FILE:FILE,name=/filename="([^"]+)"/.exec(new TextDecoder().decode(request.bytes))?.[1],file=staged(id,name,current.id);current.files.push(file);current.pageOrder.push(id);current.version++;return response({result:'ok',errors:[],data:[{id,type:'upload_session_file',attributes:{originalFileName:name,fileHash:file.fileHash,fileSize:file.fileSize,mimeType:file.mimeType,source:'local',version:1}}]});}
   if(method==='POST'&&upload[2]==='commit'){state.commits++;const receipt=chapterResource();Object.assign(current,{isCommitted:true,chapterId:CHAPTER,state:'committed',canMutate:false,canCancel:false,canRetryCommit:false,publicationStatus:'materialized',receiptStatus:'available',receipt});return response(receipt);}
   if(method==='DELETE'){if(!upload[2]){Object.assign(current,{state:'deleted',isDeleted:true,canMutate:false,canCancel:false,canRetryCommit:false});state.current=undefined;}else{const removed=upload[2]==='batch'?body:[upload[2]];current.files=current.files.filter(x=>!removed.includes(x.id));current.pageOrder=current.pageOrder.filter(x=>!removed.includes(x));current.version++;}return response({result:'ok'});}
  }
  throw Error('Unexpected synthetic route '+method+' '+route);
 };
 const deps={origin:()=>ORIGIN,fences,fetch:async(url,init)=>{
  const u=new URL(url),native=u.pathname.startsWith('/api/v1/account/');assert.equal(init.headers?.Authorization,undefined);assert.equal(init.headers?.authorization,undefined);assert.equal(init.credentials,native?'same-origin':'omit');if(native)assert.equal(init.headers['X-Weebdex-Expected-Account'],NATIVE);
  const route=u.pathname.slice(native?'/api/v1/account'.length:'/api/v1'.length);if(/^\/(?:upload|uploads)(?:\/|$)/.test(route))assert.equal(u.searchParams.get('expectedAccountId'),USER);
  const request={url:u,route,method:init.method,bytes:init.body,body:init.body&&init.headers?.['Content-Type']==='application/json'?JSON.parse(new TextDecoder().decode(init.body)):undefined,init};calls.push(request);
  const result=await(options.request?options.request(request,defaultRequest,state):defaultRequest(request));
  let body=result.body;if(native&&result.status<300&&result.status!==204&&!(body instanceof Uint8Array)&&options.autoMarker!==false)body={...body,nostr:{...(body?.nostr??{}),...marker}};
  return new Response(result.status===204?null:body instanceof Uint8Array?body:JSON.stringify(body),{status:result.status,headers:result.headers??{'content-type':'application/json'}});
 }};
 const adapter=uploads.createNativeUploadAdapter({nativeAccount:()=>state.native,nativeDependencies:deps,publicOrigin:()=>ORIGIN,catalogueRead:async()=>response(chapterView()),destroy:()=>{},digest:async b=>digest(b),decodeImage:async()=>({width:2,height:3}),newAttempt:()=>String(++state.generation),fences,replacementFences:fences,chapterFences:fences,scheduleFences:fences,replacementLock:async(_key,action)=>action(),scheduleLock:async(_key,action)=>action(),...options.dependencies});
 return{calls,storage,state,account,check,deps,adapter,async title(target){return title.createSessionTitleController(NATIVE,check,target,deps);},async drafts(){return drafts.createSessionDraftReader(NATIVE,check,deps);},async owner(){const user=await adapter.loadUploadAccount(check);return{user,owner:adapter.publicUploadOwner(()=>MANGA,user,'manga','caller')};},async begin(owner){return(await owner.request('/upload/begin','POST',{manga:MANGA,groups:[]})).body;}};
}
export async function ready(options={}){const f=fixture(options),{owner,user}=await f.owner(),current=await f.begin(owner);return{...f,owner,user,current};}
export async function attach(f){const page=localPage();await f.adapter.runUploadBatch(f.current,[page],f.owner);return page;}
export async function stagedCommit(f){const page=await attach(f),saved=await f.adapter.saveResume(f.current,{draft:{chapter:'1',language:'en'},page_order:[page.id]},f.owner);return{page,saved,payload:{draft:saved.draft,page_order:saved.page_order,termsAccepted:true}};}
export function deferred(){let release;const promise=new Promise(r=>release=r);return{promise,release};}

// Execute the actual retained layout override, not a test-only imitation.
export function installRetainedQueryOverride(){
 const source=fs.readFileSync(path.join(root,'frontend/src/routes/+layout.ts'),'utf8'),start=source.indexOf('const _toString = URLSearchParams.prototype.toString;'),end=source.indexOf('Number.prototype.short',start);assert.ok(start>=0&&end>start);
 const previous=URLSearchParams.prototype.toString;vm.runInNewContext(stripTypeScriptTypes(source.slice(start,end),{mode:'transform'}),{URLSearchParams});assert.equal(new URLSearchParams({test:'yes'}).toString(),'?test=yes');return()=>{URLSearchParams.prototype.toString=previous;};
}
