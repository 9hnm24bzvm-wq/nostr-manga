// Scoped signed title metadata commands. No cookie/native command fallback.
import { capabilities, selectedSigner } from '../local-library/publication';
import { proveService, origin as canonicalOrigin } from '../local-library/protocol/nostr.js';
import { createNostrUploadRequest, PublicUploadRequestError, type UploadRequestContext, type UploadRequestIdentity, type PublicUploadRequest, type PublicUploadResponse } from './nostr-upload-request';
import { normalizeManga } from './normalize';
import { PublicApiClient } from './client';
import { PublicApiContractError, publicUuid, record, resource } from './validation';
import type { MangaView, Resource, Response } from './types';
type Policy = Awaited<ReturnType<typeof capabilities>>;
type Scope = { origin:string; proofOrigin:string; serviceKey:string; catalogNamespace:string };
export type TitleAction = 'create'|'update'|'submit'|'publish'|'reject';
export interface TitleAccount { id:string; name:string; permissions:string[]; roles:string[]; }
export interface TitleFenceStore { get(key:string):string|null; set(key:string,value:string):void; delete(key:string):void; }
export interface TitleDependencies {
 capabilities():Promise<Policy>;
 selectedSigner():UploadRequestIdentity;
 proveService(origin:string,key:string):Promise<{origin:string;serviceKey:string}>;
 publicOrigin():string;
 createRequest(context:UploadRequestContext):(request:PublicUploadRequest)=>Promise<PublicUploadResponse>;
 fences:TitleFenceStore;
}
const defaults:TitleDependencies={capabilities,selectedSigner,proveService,publicOrigin:()=>window.location.origin,createRequest:createNostrUploadRequest,
 fences:{get:key=>window.sessionStorage.getItem(key),set:(key,value)=>window.sessionStorage.setItem(key,value),delete:key=>window.sessionStorage.removeItem(key)}};
const encoder=new TextEncoder(),includes=new URLSearchParams([['includes[]','cover_art'],['includes[]','author'],['includes[]','artist']]);
function fail(message:string,status=502):never{throw new PublicApiContractError(message,status);}
const strings=(value:unknown):string[]=>{if(!Array.isArray(value)||value.some(row=>typeof row!=='string'))fail('Invalid bound title account grants');return [...value] as string[];};
const positive=(value:unknown):number=>{if(!Number.isSafeInteger(value)||Number(value)<1||Number(value)>2147483647)fail('A current positive title version is required',422);return Number(value);};
function scope(policy:Policy,origin:string):Scope {
 if(!policy.available||!policy.schema_ready||!policy.origin||!policy.service_key||!/^[0-9a-f]{64}$/.test(policy.service_key)||!policy.catalog_namespace)fail('The configured signed catalogue service is unavailable',503);
 return {origin:canonicalOrigin(origin),proofOrigin:canonicalOrigin(policy.origin),serviceKey:policy.service_key,catalogNamespace:publicUuid(policy.catalog_namespace)};
}
function sameScope(a:Scope,b:Scope){return a.origin===b.origin&&a.proofOrigin===b.proofOrigin&&a.serviceKey===b.serviceKey&&a.catalogNamespace===b.catalogNamespace;}
function plainText(value:unknown,label:string,maximum:number,empty=false):string {
 if(typeof value!=='string'||(!empty&&!value.trim())||value.length>maximum||/[\u0000\u000b\u000c]/.test(value))fail(`Invalid ${label}`,422);return value as string;
}
function uuids(value:unknown):string[]{if(!Array.isArray(value)||value.length>100)fail('Title relationship UUID list exceeds budget',422);const result=(value as unknown[]).map(publicUuid);if(new Set(result).size!==result.length)fail('Duplicate title relationship UUID',422);return result;}
// Retained snake_case inputs are converted once, before signing exact bytes.
// Unsupported cover/relation inputs reject before any metadata mutation.
export function titleMetadata(input:unknown,language?:string):Record<string,unknown> {
 const row=record(input,'title metadata'),out:Record<string,unknown>={};
 const allowed=['title','description','year','language','demographic','status','content_rating','last_volume','last_chapter','alt_titles','tags','authors','artists','links','version','chapterNumbersResetOnNewVolume'];
 if(Object.keys(row).some(key=>!allowed.includes(key)))fail('Unsupported title field; covers, relations, official links and primary cover require separate reviewed commands',422);
 const lang=plainText(row.language??language,'original language',12);
 if(!/^[a-z]{2,3}(?:-[A-Za-z0-9]{2,8}){0,2}$/.test(lang))fail('Invalid original language',422);
 if(Object.hasOwn(row,'language'))out.originalLanguage=lang;
 for(const[key,maximum]of [['title',500],['description',50000]]as const)if(Object.hasOwn(row,key)){const value=plainText(row[key],key,maximum,key==='description');if(key==='title'&&encoder.encode(value).byteLength>1024)fail('Title exceeds the original protocol text budget',422);out[key]={[lang]:value};}
 if(Object.hasOwn(row,'year')){if(row.year!==null&&row.year!==0&&(!Number.isSafeInteger(row.year)||Number(row.year)<1||Number(row.year)>9999))fail('Invalid publication year',422);out.year=row.year===0?null:row.year;}
 if(Object.hasOwn(row,'demographic')){if(row.demographic!==null&&!['none','shounen','shoujo','josei','seinen'].includes(String(row.demographic)))fail('Invalid publication demographic',422);out.publicationDemographic=row.demographic==='none'?null:row.demographic;}
 for(const[key,wire,values]of [['status','status',['ongoing','completed','hiatus','cancelled']],['content_rating','contentRating',['safe','suggestive','erotica','pornographic']]]as const)if(Object.hasOwn(row,key)){if(!values.some(value=>value===row[key]))fail('Invalid title classification',422);out[wire]=row[key];}
 for(const[key,wire]of [['last_volume','lastVolume'],['last_chapter','lastChapter']]as const)if(Object.hasOwn(row,key))out[wire]=row[key]===null||row[key]===''?null:plainText(row[key],key,32);
 for(const key of ['authors','artists','tags'])if(Object.hasOwn(row,key))out[key]=uuids(row[key]);
 if(Object.hasOwn(row,'alt_titles')){const titles=record(row.alt_titles,'alternate titles');out.altTitles=Object.entries(titles).flatMap(([lang,values])=>{if(!/^[a-z]{2,3}(?:-[A-Za-z0-9]{2,8}){0,2}$/.test(lang)||!Array.isArray(values)||values.length>100)fail('Invalid alternate title language or list',422);return (values as unknown[]).map(value=>({[lang]:plainText(value,'alternate title',500)}));});if((out.altTitles as unknown[]).length>100)fail('Alternate titles exceed budget',422);}
 if(Object.hasOwn(row,'links')){const links=record(row.links,'title links');out.links=Object.fromEntries(Object.entries(links).map(([key,value])=>[plainText(key,'link key',32),plainText(value,'link value',2048)]));}
 if(Object.hasOwn(row,'version'))out.version=positive(row.version);
 if(Object.hasOwn(row,'chapterNumbersResetOnNewVolume')){if(typeof row.chapterNumbersResetOnNewVolume!=='boolean')fail('Invalid chapter reset flag',422);out.chapterNumbersResetOnNewVolume=row.chapterNumbersResetOnNewVolume;}
 if(encoder.encode(JSON.stringify(out)).byteLength>32768)fail('Signed title metadata exceeds the 32 KiB request budget',422);
 return out;
}
function titleEntity(response:Response<unknown>,expected?:string):Resource {
 const body=record(response.body,'signed title entity');if(body.result!=='ok'||body.response!=='entity')fail('Invalid signed title response envelope');
 const row=resource(body.data,'manga'),attrs=row.attributes;
 if(expected&&publicUuid(row.id)!==expected)fail('Title receipt changed the exact canonical target');
 positive(attrs.version);plainText(attrs.originalLanguage,'actual original language',12);
 if(!['draft','submitted','rejected','published'].includes(String(attrs.state)))fail('Invalid actual title state');
 const titles=record(attrs.title,'actual localized title');if(!Object.values(titles).some(value=>typeof value==='string'&&value.trim()))fail('Title response lacks actual localized title');
 return row;
}
export interface SignedTitleController {
 readonly account:TitleAccount;
 current():MangaView|undefined;
 pending():boolean;
 assert():void;
 dispose():void;
 read():Promise<Response<MangaView>>;
 mutate(action:TitleAction,input?:unknown):Promise<Response<MangaView>>;
}
export async function createSignedTitleController(assertCurrent:()=>void,target?:string,injected:Partial<TitleDependencies>={}):Promise<SignedTitleController> {
 const deps={...defaults,...injected},identity=deps.selectedSigner();identity.check();assertCurrent();
 const captured=scope(await deps.capabilities(),deps.publicOrigin());identity.check();assertCurrent();
 const proof=await deps.proveService(captured.proofOrigin,captured.serviceKey);identity.check();assertCurrent();
 if(proof.origin!==captured.proofOrigin||proof.serviceKey!==captured.serviceKey)fail('Signed title service proof changed');
 let disposed=false,busy=false,reading=false,id=target===undefined?undefined:publicUuid(target),current:MangaView|undefined;
 const check=()=>{if(disposed)fail('Title editor is closed',409);identity.check();assertCurrent();};
 const refresh=async()=>{check();if(!sameScope(captured,scope(await deps.capabilities(),deps.publicOrigin())))fail('Title service/catalogue changed. Review the target again.',409);check();};
 const request=deps.createRequest({scope:captured,identity,assertCurrent:check,beforeSend:async signal=>{if(signal.aborted)fail('Title request cancelled',409);await refresh();}});
 await refresh();
 const bound=await request({url:captured.origin+'/api/v1/nostr/account-context',method:'GET'});check();
 if(bound.status!==200)fail(`Signed account authorization refused (HTTP ${bound.status})`,bound.status);
 function accountFacts(bound:Response<unknown>):TitleAccount {
 if(bound.status!==200)fail(`Signed account authorization refused (HTTP ${bound.status})`,bound.status);
 const accountBody=record(bound.body,'signed title account'),user=record(accountBody.user,'bound native account'),nostr=record(accountBody.nostr,'bound signer');
 if(accountBody.result!=='ok'||nostr.pubkey!==identity.owner||nostr.serviceKey!==captured.serviceKey||publicUuid(nostr.catalogNamespace)!==captured.catalogNamespace||nostr.authentication!=='NIP-98'||nostr.requestScoped!==true||nostr.nativeSessionAuthority!==false||nostr.catalogueScope!=='instance-local')fail('Signed title account does not match the exact signer/service/catalogue');
 positive(user.version);
 return {id:publicUuid(user.id),name:plainText(user.username,'bound username',500),permissions:strings(user.permissions),roles:strings(user.roles)};
 }
 const account=accountFacts(bound);
 const mutationRequest=deps.createRequest({scope:captured,identity,assertCurrent:check,beforeSend:async signal=>{
  if(signal.aborted)fail('Title request cancelled',409);await refresh();
  const fresh=accountFacts(await request({url:captured.origin+'/api/v1/nostr/account-context',method:'GET'}));check();
  if(fresh.id!==account.id)throw new PublicUploadRequestError('The selected signer is now bound to a different native account. Reopen the editor.','unsent',409);
 }});
 const key=()=>`weebdex.signed-title-pending.v1:${captured.origin}:${captured.serviceKey}:${captured.catalogNamespace}:${identity.owner}:${id??'new'}`;
 const pending=()=>deps.fences.get(key())!==null;
 const read=async():Promise<Response<MangaView>>=>{
  check();if(!id)fail('A created title UUID is required for reload',422);
  if(busy||reading)fail('Another title request is in progress',409);
  const requested=id;reading=true;
  try {
  let response=await request({url:`${captured.origin}/api/v1/manga/${id}?${includes.toString().replace(/^\?/, '')}`,method:'GET'});check();
  if(response.status===404){response=await request({url:`${captured.origin}/api/v1/manga/draft/${id}?${includes.toString().replace(/^\?/, '')}`,method:'GET'});check();}
  if(response.status!==200)return response as unknown as Response<MangaView>;
  // A route may be a genuine alias; only a successful read establishes the
  // canonical write target. Mutation receipts never silently switch targets.
  const row=titleEntity(response),canonical=publicUuid(row.id);
  if(canonical!==requested){
   const api=new PublicApiClient(async({path,method='GET',body,signal})=>request({url:captured.origin+'/api/v1'+path,method,signal,...(body===undefined?{}:{body:encoder.encode(JSON.stringify(body)),contentType:'application/json'})}));
   const proof=await api.resolveLegacy([{kind:'manga',id:requested}],captured.catalogNamespace);check();
   if(proof.status!==200||!proof.body?.data.some(row=>row.kind==='manga'&&row.legacy_id===requested&&publicUuid(row.id)===canonical))fail('Title read changed target without exact canonical alias proof');
  }
  id=canonical;current=normalizeManga(row);
  return {...response,body:current};
  }finally{reading=false;}
 };
 const mutate=async(action:TitleAction,input?:unknown):Promise<Response<MangaView>>=>{
  check();if(busy||reading||pending())fail('A title command is in progress or its outcome is unconfirmed. No mutation was replayed.',409);
  if(!['create','update','submit','publish','reject'].includes(action))fail('Unsupported title command',422);
  if(action==='create'&&(id||current))fail('This editor already has a title target',409);
  if(action!=='create'&&(!id||!current))fail('Read the current canonical title before changing it',409);
  const previous=current,version=previous&&positive(previous.version);
  let payload:Record<string,unknown>;
  if(action==='create'||action==='update'){
   payload=titleMetadata(input,previous?.language);
   if(action==='create'){if(!['title','originalLanguage','status','contentRating'].every(field=>Object.hasOwn(payload,field))||Object.hasOwn(payload,'version'))fail('Create requires complete basic metadata and no assigned version',422);}
   else {
    if(payload.version!==version)fail('Metadata edit must use the exact current title version',409);
    // The public API replaces each localized map atomically. A one-language
    // retained edit must preserve all other actual server translations.
    for(const field of ['title','description'])if(Object.hasOwn(payload,field))payload[field]={...record(previous!.public_api.resource.attributes[field]??{},'current localized metadata'),...record(payload[field])};
    if(encoder.encode(JSON.stringify(payload)).byteLength>32768)fail('Localized title edit exceeds the 32 KiB request budget',422);
   }
  }else{if(input!==undefined)fail('Draft transitions accept only the captured current version',422);payload={version};}
  payload={...payload,expectedAccountId:account.id};
  if(encoder.encode(JSON.stringify(payload)).byteLength>32768)fail('Signed title account intent exceeds the 32 KiB request budget',422);
  const route=action==='create'?'/manga':action==='update'?`/manga/${id}`:`/manga/draft/${id}/${action==='submit'?'commit':action}`;
  busy=true;const fenceKey=key();let installed=false;
  try {
   // Persist before the first async/signing boundary. Unknown creates must not
   // silently become duplicate drafts after a reload or a new route owner.
   const marker=JSON.stringify({action,target:id??null,version:version??null});
   deps.fences.set(fenceKey,marker);installed=true;
   if(deps.fences.get(fenceKey)!==marker)fail('Title recovery fence was not persisted',409);
   const response=await mutationRequest({url:captured.origin+'/api/v1'+route,method:action==='update'?'PUT':'POST',body:encoder.encode(JSON.stringify(payload)),contentType:'application/json'});check();
   if(response.status>=400&&response.status<500){deps.fences.delete(fenceKey);installed=false;return response as unknown as Response<MangaView>;}
   const expectedStatus=action==='submit'?201:200;if(response.status!==expectedStatus)fail(`Title command outcome is unconfirmed (HTTP ${response.status})`);
   const row=titleEntity(response,id),next=normalizeManga(row),expectedState=action==='create'?'draft':action==='submit'?'submitted':action==='publish'?'published':action==='reject'?'rejected':previous!.state;
   if(next.version!==(action==='create'?1:version!+1)||next.state!==expectedState)fail('Title receipt has an unexpected version or state');
   deps.fences.delete(fenceKey);installed=false;id=next.id;current=next;
   return {...response,body:next};
  }catch(error){if(installed&&error instanceof PublicUploadRequestError&&error.outcome==='unsent')deps.fences.delete(fenceKey);throw error;}
  finally{busy=false;}
 };
 return {account,current:()=>current,pending,assert:check,dispose:()=>{disposed=true;},read,mutate};
}
export async function publicTitleRead(id:string):Promise<Response<MangaView>> {
 const path=window.location.pathname,controller=await createSignedTitleController(()=>{if(window.location.pathname!==path)fail('Title route changed',409);},id);
 try{return await controller.read();}finally{controller.dispose();}
}
