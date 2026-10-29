// Historical signed UI regression only; current native coverage is test_native_contribution_dom.mjs.
import {historicalContributionSource} from './historical-contribution-ui-fixture.mjs';
// Actual pinned Svelte/JSdom + upload controller + normalization + affordance
// helper. Signer, service proof and HTTP responses are explicit finite stand-ins.
// No browser, network, Rust server, DB, signature or source-publication claim.
import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import fs from 'node:fs';import path from 'node:path';import vm from 'node:vm';
import {createRequire,stripTypeScriptTypes} from 'node:module';
import {fileURLToPath,pathToFileURL} from 'node:url';import test from 'node:test';
const root=path.resolve(path.dirname(fileURLToPath(import.meta.url)),'../..');
const dependencies=process.env.WEEBDEX_FRONTEND_ROOT,domModules=process.env.WEEBDEX_BROWSER_DOM_NODE_MODULES;
assert(dependencies&&domModules,'Supply pinned frontend and JSdom roots');
const require=createRequire(path.join(dependencies,'package.json'));
assert.equal(require('svelte/package.json').version,'5.55.1');assert.equal(require('typescript/package.json').version,'5.9.3');assert.equal(require(path.join(domModules,'jsdom/package.json')).version,'27.0.1');
const {compile}=require('svelte/compiler'),ts=require('typescript'),{JSDOM}=require(path.join(domModules,'jsdom'));
const {window}=new JSDOM('<main></main>',{pretendToBeVisual:true,url:'https://catalog.example'});
for(const key of ['window','document','Node','Element','HTMLElement','HTMLButtonElement','HTMLInputElement','HTMLTextAreaElement','HTMLSelectElement','HTMLMediaElement','HTMLFormElement','Text','Comment','Event','CustomEvent','MutationObserver','requestAnimationFrame','cancelAnimationFrame','getComputedStyle'])globalThis[key]=window[key];
const client=pathToFileURL(path.join(dependencies,'node_modules/svelte/src/index-client.js')).href,{mount,unmount,flushSync,onDestroy:registerDestroy}=await import(client);
const directory=path.join(root,'platform-integrations/public-api'),context=vm.createContext({Error,TextEncoder,TextDecoder,Uint8Array,ArrayBuffer,Blob,AbortController,URL,structuredClone}),modules=new Map();
export class RequestFailure extends Error {constructor(message,outcome,status){super(message);this.outcome=outcome;this.status=status;this.bodyAvailable=false;}}
const forbidden=()=>{throw Error('Unexpected production primitive in finite chapter seam');};
function stub(name,exports){if(!modules.has(name))modules.set(name,new vm.SyntheticModule(Object.keys(exports),function(){for(const[key,value]of Object.entries(exports))this.setExport(key,value);},{context,identifier:name}));return modules.get(name);}
async function load(file){if(!modules.has(file)){const module=new vm.SourceTextModule(stripTypeScriptTypes(fs.readFileSync(file,'utf8'),{mode:'transform',sourceUrl:file}),{context,identifier:file});modules.set(file,module);await module.link(specifier=>{if(specifier==='svelte')return stub(specifier,{onDestroy:forbidden,createContext:()=>[forbidden,forbidden]});if(specifier==='$lib/constants/Error')return load(path.join(root,'frontend/src/lib/constants/Error.ts'));if(specifier.includes('local-library/publication'))return stub(specifier,{capabilities:forbidden,selectedSigner:forbidden});if(specifier.includes('protocol/nostr.js'))return stub(specifier,{proveService:forbidden,origin:value=>new URL(value).origin});if(specifier==='./nostr-upload-request')return stub(specifier,{createNostrUploadRequest:forbidden,PublicUploadRequestError:RequestFailure});if(specifier==='./site-client')return stub(specifier,{publicCatalogueRead:forbidden});return load(path.resolve(path.dirname(file),specifier+'.ts'));});}return modules.get(file);}
const adapterModule=await load(path.join(process.env.WEEBDEX_LOCAL_PAGE_EDIT_CONTROL_SOURCE??process.env.WEEBDEX_CHAPTER_PENDING_CONTROL_SOURCE??directory,'upload-client.ts'));await adapterModule.evaluate();const adapter=adapterModule.namespace;
const normalizeModule=await load(path.join(directory,'normalize.ts'));await normalizeModule.evaluate();const {normalizeChapter}=normalizeModule.namespace;
const languageModule=await load(path.join(directory,'language.ts'));await languageModule.evaluate();const {chapterMetadataPatch}=languageModule.namespace;
const filename=path.join(process.env.WEEBDEX_LOCAL_PAGE_EDIT_CONTROL_SOURCE??process.env.WEEBDEX_CHAPTER_PENDING_CONTROL_SOURCE??process.env.WEEBDEX_SIGNED_EDITOR_CONTROL_SOURCE??directory,'PublicChapterEditor.svelte');
const rewrite=file=>historicalContributionSource(file).replace(/<script lang="ts">([\s\S]*?)<\/script>/,(_all,script)=>{const ast=ts.createSourceFile('Chapter.ts',script,ts.ScriptTarget.Latest,true,ts.ScriptKind.TS);for(const item of [...ast.statements].reverse()){if(!ts.isImportDeclaration(item))continue;const specifier=item.moduleSpecifier.text,clause=item.importClause;let replacement='';if(specifier==='svelte')replacement=item.getText(ast).replace(/(['"])svelte\1/,JSON.stringify(client));else if(clause&&!clause.isTypeOnly){const names=[];if(clause.name)names.push(clause.name.text);if(clause.namedBindings&&ts.isNamedImports(clause.namedBindings))for(const entry of clause.namedBindings.elements)if(!entry.isTypeOnly)names.push(entry.name.text);if(names.length)replacement=`const {${names.join(',')}}=globalThis.__chapterAccessEnvironment;`;}script=script.slice(0,item.getStart(ast))+replacement+script.slice(item.end);}return'<script lang="ts">'+script+'</script>';});
fs.mkdirSync(path.join(root,'.build'),{recursive:true});const generation=fs.mkdtempSync(path.join(root,'.build/chapter-access-dom-'));fs.symlinkSync(path.join(dependencies,'node_modules'),path.join(generation,'node_modules'));
// Keep all generated modules below 224 KiB before any file write; reserve the
// remaining 32 KiB for three files' allocation rounding, directories and symlink.
let generatedBytes=0;function writeGenerated(file,code){const bytes=Buffer.byteLength(code);assert.ok(generatedBytes+bytes<=224*1024,'Mounted fixture generated output exceeds its admitted 256 KiB budget');fs.writeFileSync(file,code);generatedBytes+=bytes;}
const compiled=compile(rewrite(filename),{filename,generate:'client',runes:true});assert.deepEqual(compiled.warnings,[]);const compiledFile=path.join(generation,'Chapter.mjs');writeGenerated(compiledFile,compiled.js.code);const Component=(await import(pathToFileURL(compiledFile))).default;
const pageFilename=path.join(directory,'PublicChapterPagesEditor.svelte'),pageCompiled=compile(rewrite(pageFilename),{filename:pageFilename,generate:'client',runes:true});
assert.deepEqual(pageCompiled.warnings,[]);const pageFile=path.join(generation,'Pages.mjs');writeGenerated(pageFile,pageCompiled.js.code);const PublicChapterPagesEditor=(await import(pathToFileURL(pageFile))).default;
// The retained upload panel has independent actual-overlay transport controls.
// This finite mounted route seam stubs only its visual children, never the
// signed adapter, admission, order-save, commit or page-controller functions.
const panel=compile('<script>let {pages=$bindable(),disabled=false}=$props();</script><div data-native-pages-panel data-disabled={disabled}>{pages.length} staged pages</div>',{generate:'client',runes:true});
const panelFile=path.join(generation,'Panel.mjs');writeGenerated(panelFile,panel.js.code);const UploadChapterPages=(await import(pathToFileURL(panelFile))).default;
const {writable,get}=await import(pathToFileURL(path.join(dependencies,'node_modules/svelte/src/store/index-client.js')).href);
class FixtureMutex {constructor(){this.state=writable(false);this.subscribe=this.state.subscribe;}isLocked=()=>get(this.state);async exec(fn){if(this.isLocked())return;this.state.set(true);try{return await fn();}finally{this.state.set(false);}}}
const controllerModule=await load(path.join(directory,'chapter-pages-controller.ts'));await controllerModule.evaluate();
const uid=n=>`${String(n).repeat(8)}-${String(n).repeat(4)}-8${String(n).repeat(3)}-8${String(n).repeat(3)}-${String(n).repeat(12)}`;
const CHAPTER=uid(1),MANGA=uid(2),ACCOUNT=uid(3),UPLOADER=uid(4),GROUP=uid(5),FOREIGN=uid(6),NAMESPACE=uid(7),ORIGIN='https://catalog.example',SIGNER='a'.repeat(64),SERVICE='b'.repeat(64),SESSION=uid(8),FILE=uid(9);
const clone=value=>structuredClone(value),deferred=()=>{let release;return{promise:new Promise(resolve=>release=resolve),release};};
const response=(body,status=200,headers={})=>({status,body,bodyAvailable:true,headers:{'content-type':'application/json',...headers}}),entity=data=>response({result:'ok',response:'entity',data:clone(data)});
async function settle(){for(let n=0;n<10;n++){await new Promise(r=>setImmediate(r));flushSync();}}
function button(host,label){const row=[...host.querySelectorAll('button')].find(row=>row.textContent.trim()===label);assert(row,label);return row;}
async function render({role='uploader',permissions=['self.chapter.update','self.chapter.remove'],roles=[],system=false,marker=true,username='Current uploader name',accountError=false,sharedStorage,omitChapterMetadataPatch=false,pageSource='local'}={}){
 const state={permissions:[...permissions],roles:[...roles],accountId:ACCOUNT,active:true,namespace:NAMESPACE,serviceKey:SERVICE,signer:SIGNER,version:7,gate:undefined,readGate:undefined,refuse:false,uncertain:false,accessReads:0};
 const requests=[],writes=[],fences=sharedStorage??new Map();
 const row={id:CHAPTER,type:'chapter',attributes:{...(pageSource==='missing'?{}:{localPageSource:pageSource}),version:7,title:'Actual chapter',chapter:'1',volume:null,translatedLanguage:'en',uploader:role==='uploader'?ACCOUNT:UPLOADER,...(system==='missing'?{}:{localUploaderIsSystem:system})},relationships:[{type:'manga',id:MANGA},{type:'user',id:role==='uploader'?ACCOUNT:UPLOADER,attributes:{username,version:2}},{type:'scanlation_group',id:GROUP,attributes:{name:'Actual group',version:2}}]};
 const group={id:GROUP,type:'scanlation_group',attributes:{name:'Actual group',version:2,...(marker?{localMembershipRoles:'leader-officer-member-v1'}:{})},relationships:role==='uploader'||role==='unrelated'?[]:[{type:role==='leader'?'leader':'member',id:role==='foreign'?FOREIGN:ACCOUNT,attributes:{username:'Bound account',version:3,isOfficer:role==='officer'}}]};
 const initial=()=>({...normalizeChapter(clone(row),{id:MANGA}),data:[{public_url:'/data/exact/page.png',dimensions:[2,3]}]});
 const account=()=>({result:'ok',user:{id:state.accountId,username:'Bound account',version:3,roles:state.roles,permissions:state.permissions},nostr:{pubkey:state.signer,serviceKey:state.serviceKey,catalogNamespace:state.namespace,authentication:'NIP-98',requestScoped:true,nativeSessionAuthority:false,catalogueScope:'instance-local'},upload:{canBegin:state.permissions.includes('chapter.create'),canManage:state.permissions.includes('self.chapter.update'),conditional:true,publication:{requiresOperatorOrAuthorizedGroup:true},limits:{}}});
 let attempt=0;
 // Explicit finite attempt source and exact normalized chapter read. The VM has
 // no production crypto/network globals; neither dependency is a success bypass.
 const deps={newAttempt:()=>`chapter-fixture-${++attempt}`,catalogueRead:async(operation,options)=>{assert.equal(operation,'chapter.get');assert.equal(options.id,CHAPTER);assert.deepEqual(Object.keys(options),['id']);return response(initial());},destroy:registerDestroy,capabilities:async()=>({available:true,schema_ready:true,origin:ORIGIN,service_key:state.serviceKey,catalog_namespace:state.namespace}),selectedSigner:()=>({owner:state.signer,check(){if(!state.active)throw Error('Selected account revoked');}}),proveService:async()=>({origin:ORIGIN,serviceKey:state.serviceKey}),publicOrigin:()=>ORIGIN,fences:{get:k=>{if(state.storageError==='get')throw Error('Storage unavailable');return fences.get(k)??null;},set:(k,v)=>{if(state.storageError==='set')throw Error('Storage unavailable');fences.set(k,v);},delete:k=>{if(state.storageError==='delete')throw Error('Storage unavailable');fences.delete(k);}},replacementFences:{get:()=>null,set:forbidden,delete:forbidden},digest:async bytes=>createHash('sha256').update(bytes).digest('hex'),createRequest:context=>async request=>{
  context.assertCurrent();await context.beforeSend?.(new AbortController().signal);context.assertCurrent();const url=new URL(request.url),body=request.body?JSON.parse(new TextDecoder().decode(request.body)):undefined;requests.push({method:request.method,url:request.url,body});
  if(url.pathname.endsWith('/nostr/account-context'))return response(account(),accountError?403:200);
  if(request.method==='GET' && url.pathname==='/api/v1/upload') return state.session&&!state.session.isDeleted?response({id:SESSION,type:'upload_session',attributes:{version:state.session.version,isCommitted:state.session.isCommitted,isProcessed:false,isDeleted:false}}):response({result:'error',errors:[{status:404,title:'Not found',detail:'No upload'}]},404);
  if(url.pathname==='/api/v1/nostr/uploads/'+SESSION){
    if(request.method==='PATCH'){writes.push({method:request.method,url:request.url,body});assert.equal(body.version,state.session.version);state.session.version++;state.session.draft=clone(body.draft);state.session.pageOrder=clone(body.pageOrder);}
    await state.snapshotGate?.promise;if(state.snapshotUnknown)throw new RequestFailure('Accepted session snapshot unavailable','unsent');return response({result:'ok',session:clone(state.session)});
  }
  if(url.pathname==='/api/v1/upload/begin/'+CHAPTER){
    writes.push({method:request.method,url:request.url,body});assert.equal(body.expectedAccountId,ACCOUNT);assert.equal(body.version,row.attributes.version);
    if(state.beginRefused)return response({result:'error',errors:[{status:409,title:'Conflict',detail:'Exact edit refused'}]},409);
    state.session={id:SESSION,mangaId:MANGA,chapterId:CHAPTER,editVersion:body.version,groupIds:[],createdAt:'2026-10-22T00:00:00Z',updatedAt:'2026-10-22T00:00:00Z',version:1,draft:{title:row.attributes.title,chapter:row.attributes.chapter,language:row.attributes.translatedLanguage},pageOrder:body.replace?[]:[FILE],files:body.replace?[]:[{id:FILE,originalFileName:'1.png',fileHash:'a'.repeat(64),fileSize:12,mimeType:'image/png',dimensions:{width:2,height:3},previewUrl:`/api/v1/nostr/uploads/${SESSION}/files/${FILE}/preview`}],canMutate:true,canCancel:true,canRetryCommit:true,isCommitted:false,isProcessed:false,isDeleted:false,isExpired:false,expiresAt:'2026-10-23T00:00:00Z',state:'open',publicationStatus:'unprepared',targetStatus:'available',receiptStatus:'none',receipt:null};
    if(state.beginUnknown)throw new RequestFailure('Page-edit admission delivery is unknown','uncertain');
    const result=response({id:SESSION,type:'upload_session',attributes:{version:1,isCommitted:false,isProcessed:false,isDeleted:false,edit:{chapterId:CHAPTER,version:body.version,replace:body.replace}}});state.beginTransform?.(result);await state.beginGate?.promise;return result;
  }
  if(url.pathname==='/api/v1/upload/'+SESSION+'/commit'){
    writes.push({method:request.method,url:request.url,body});assert.equal(body.termsAccepted,true);row.attributes.version++;
    Object.assign(state.session,{version:state.session.version+1,state:'committed',isCommitted:true,canMutate:false,canCancel:false,canRetryCommit:false,receiptStatus:'available',receipt:clone(row)});
    if(state.commitUnknown)throw new RequestFailure('Page save delivery is unknown','uncertain');
    const result=response(clone(row));state.commitTransform?.(result);return result;
  }
  if(url.pathname==='/api/v1/upload/'+SESSION&&request.method==='DELETE'){
    writes.push({method:request.method,url:request.url,body});Object.assign(state.session,{state:'deleted',isDeleted:true,canMutate:false,canCancel:false,canRetryCommit:false});return response({result:'ok'});
  }
  if(request.method==='GET'){if(url.pathname==='/api/v1/chapter/'+CHAPTER){state.accessReads++;await state.readGate?.promise;return entity(row);}if(url.pathname==='/api/v1/group/'+GROUP)return entity(group);throw Error('Unexpected finite read '+url.pathname);}
  if(state.preDispatchError)throw new RequestFailure('Definitely not sent','unsent');
  writes.push({method:request.method,url:request.url,body});if(state.uncertain)throw state.deliveryError??new RequestFailure('Unknown delivery','uncertain');if(state.refuse)return response({result:'error',errors:[{status:403,title:'Forbidden',detail:'Server rejected current ownership'}]},403);if(state.customResponse)return state.customResponse;
  assert.equal(body?.expectedAccountId??url.searchParams.get('expectedAccountId'),ACCOUNT);
  if(request.method==='DELETE'){const receipt=response({result:'ok',status:'pending-moderation',instanceRemoval:'pending-moderation',requestId:FOREIGN,sourceDeletionStatus:'instance-local-only',sourceDeletionRequestId:null},202,{'x-weebdex-instance-removal':'pending-moderation','x-weebdex-source-deletion-status':'instance-local-only'});state.replyTransform?.(receipt);await state.gate?.promise;return receipt;}
  if(request.method==='PUT'){
   assert.equal(body.version,row.attributes.version);
   for(const key of ['title','volume','chapter','translatedLanguage'])if(body[key]!==undefined)row.attributes[key]=body[key].replace(/^\p{White_Space}+|\p{White_Space}+$/gu,'');
   if(body.groups!==undefined){
    const previous=new Map(row.relationships.filter(value=>value.type==='scanlation_group').map(value=>[value.id,value]));
    row.relationships=[...row.relationships.filter(value=>value.type!=='scanlation_group'),...body.groups.map(id=>{assert(previous.has(id),'Unexpected finite chapter group');return clone(previous.get(id));})];
   }
  }
  if(request.method==='POST'){row.attributes.uploader=ACCOUNT;row.relationships.find(r=>r.type==='user').id=ACCOUNT;}
  row.attributes.version++;const receipt=entity(row);state.replyTransform?.(receipt);await state.gate?.promise;return receipt;
 }};
 const runtime=adapter.createPublicUploadAdapter(deps);
 // The omitted binding is an explicit harness counterfactual, never a product fallback.
 globalThis.__chapterAccessEnvironment={...adapter,...runtime,publicUploadOwner:(...args)=>{const owner=runtime.publicUploadOwner(...args);(state.owners??=[]).push(owner);return owner;},PublicChapterPagesEditor,UploadChapterPages,Mutex:FixtureMutex,setMutex:()=>{},setUploadSession:()=>{},setUploadOwner:()=>{},beforeNavigate:callback=>state.beforeNavigate=callback,createChapterPagesController:(...args)=>controllerModule.namespace.createChapterPagesController(...args,runtime),normalizeChapter,...(omitChapterMetadataPatch?{}:{chapterMetadataPatch}),record:value=>value,resource:value=>value,publicCatalogueRead:async()=>({status:200,body:initial()})};
 const host=document.createElement('section');document.body.append(host);let instance=mount(Component,{target:host,props:{initial:initial()}});await settle();
 return{state,row,group,host,requests,writes,fences,async openOwner(){const user=await runtime.loadUploadAccount();return runtime.publicUploadOwner(()=>CHAPTER,user,'chapter','caller');},async remount(){await unmount(instance);instance=mount(Component,{target:host,props:{initial:initial()}});await settle();},async close(){await unmount(instance);host.remove();}};
}
test.after(()=>{fs.rmSync(generation,{recursive:true,force:true});window.close();delete globalThis.__chapterAccessEnvironment;});

export {flushSync,render,button,settle,deferred,clone,CHAPTER,MANGA,ACCOUNT,UPLOADER,GROUP,FOREIGN,NAMESPACE,ORIGIN,SIGNER,SERVICE,SESSION,FILE};
