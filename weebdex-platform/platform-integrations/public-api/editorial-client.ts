// Explicit signer-bound local catalogue commands. No cookie/native fallback.
import { capabilities, selectedSigner } from '../local-library/publication';
import { proveService, origin } from '../local-library/protocol/nostr.js';
import { createNostrUploadRequest, PublicUploadRequestError } from './nostr-upload-request';
import type { TitleDependencies, TitleAccount } from './title-client';
import type { Resource, Response } from './types';
import { record, resource, publicUuid, PublicApiContractError } from './validation';
import { PublicApiClient } from './client';
export type EditorialKind='author'|'group';
export type EditorialAction='create'|'update'|'delete'|'claim';
const encoder=new TextEncoder();
function fail(message:string,status=502):never{throw new PublicApiContractError(message,status);}
function positive(value:unknown):number{if(!Number.isSafeInteger(value)||Number(value)<1||Number(value)>2147483647)fail('A current positive version is required',422);return Number(value);}
function strings(value:unknown):string[]{if(!Array.isArray(value)||value.some(v=>typeof v!=='string'))fail('Invalid current account grants');return [...value]as string[];}
const defaults:TitleDependencies={capabilities,selectedSigner,proveService,publicOrigin:()=>window.location.origin,createRequest:createNostrUploadRequest,fences:{get:k=>sessionStorage.getItem(k),set:(k,v)=>sessionStorage.setItem(k,v),delete:k=>sessionStorage.removeItem(k)}};
function classification(kind:EditorialKind){return kind==='author'?'author':'scanlation_group';}
function entity(kind:EditorialKind,response:Response<unknown>,expected?:string):Resource{
 const body=record(response.body);if(body.result!=='ok'||body.response!=='entity')fail('Invalid editorial entity receipt');
 const row=resource(body.data,classification(kind));if(expected&&row.id!==expected)fail('Editorial receipt changed its exact canonical target');
 positive(row.attributes.version);if(typeof row.attributes.name!=='string'||!row.attributes.name.trim())fail('Editorial receipt lacks an actual name');return row;
}
export interface EditorialController{account:TitleAccount;current():Resource|undefined;pending():boolean;assert():void;dispose():void;read():Promise<Response<Resource>>;mutate(action:EditorialAction,input?:unknown):Promise<Response<unknown>>;}
export async function createEditorialController(kind:EditorialKind,target:string|undefined,assertCurrent:()=>void,injected:Partial<TitleDependencies>={}):Promise<EditorialController>{
 if(!['author','group'].includes(kind))fail('Unsupported editorial resource',422);
 const deps={...defaults,...injected},identity=deps.selectedSigner();identity.check();assertCurrent();
 function scope(policy:Awaited<ReturnType<typeof capabilities>>){if(!policy.available||!policy.schema_ready||!policy.origin||!policy.service_key||!/^[0-9a-f]{64}$/.test(policy.service_key)||!policy.catalog_namespace)fail('Configured signing service unavailable',503);return{origin:origin(deps.publicOrigin()),proofOrigin:origin(policy.origin),serviceKey:policy.service_key,catalogNamespace:publicUuid(policy.catalog_namespace)};}
 const captured=scope(await deps.capabilities());identity.check();assertCurrent();
 const proof=await deps.proveService(captured.proofOrigin,captured.serviceKey);identity.check();assertCurrent();if(proof.origin!==captured.proofOrigin||proof.serviceKey!==captured.serviceKey)fail('Editorial service proof changed');
 let disposed=false,busy=false,id=target===undefined?undefined:publicUuid(target),current:Resource|undefined;
 const check=()=>{if(disposed)fail('Editorial view closed',409);identity.check();assertCurrent();};
 const refresh=async()=>{check();if(JSON.stringify(scope(await deps.capabilities()))!==JSON.stringify(captured))fail('Editorial service/catalogue changed',409);check();};
 const request=deps.createRequest({scope:captured,identity,assertCurrent:check,beforeSend:async()=>refresh()});
 function facts(response:Response<unknown>):TitleAccount{if(response.status!==200)fail(`Bound account refused (HTTP ${response.status})`,response.status);const body=record(response.body),user=record(body.user),nostr=record(body.nostr);if(body.result!=='ok'||nostr.pubkey!==identity.owner||nostr.serviceKey!==captured.serviceKey||publicUuid(nostr.catalogNamespace)!==captured.catalogNamespace||nostr.authentication!=='NIP-98'||nostr.requestScoped!==true||nostr.nativeSessionAuthority!==false||nostr.catalogueScope!=='instance-local')fail('Editorial account binding changed');positive(user.version);if(typeof user.username!=='string'||!user.username)fail('Invalid bound account name');return{id:publicUuid(user.id),name:user.username,permissions:strings(user.permissions),roles:strings(user.roles)};}
 const account=facts(await request({url:captured.origin+'/api/v1/nostr/account-context',method:'GET'}));check();
 const mutationRequest=deps.createRequest({scope:captured,identity,assertCurrent:check,beforeSend:async()=>{await refresh();const actual=facts(await request({url:captured.origin+'/api/v1/nostr/account-context',method:'GET'}));check();if(actual.id!==account.id)throw new PublicUploadRequestError('Signer is now bound to another native account; reopen the editor.','unsent',409);}});
 const key=()=>`weebdex.signed-editorial-pending.v1:${captured.origin}:${captured.serviceKey}:${captured.catalogNamespace}:${identity.owner}:${kind}:${id??'new'}`;
 const pending=()=>deps.fences.get(key())!==null;
 const read=async():Promise<Response<Resource>>=>{check();if(busy||!id)fail('An exact idle editorial target is required',409);busy=true;const requested=id;try{
  const query=kind==='group'?'?includes%5B%5D=leader&includes%5B%5D=member':'';
  const response=await request({url:`${captured.origin}/api/v1/${kind}/${id}${query}`,method:'GET'});check();if(response.status!==200)return response as Response<Resource>;
  const row=entity(kind,response),canonical=publicUuid(row.id);
  if(canonical!==requested){const api=new PublicApiClient(async({path,method='GET',body,signal})=>request({url:captured.origin+'/api/v1'+path,method,signal,...(body===undefined?{}:{body:encoder.encode(JSON.stringify(body)),contentType:'application/json'})}));const mapping=await api.resolveLegacy([{kind,id:requested}],captured.catalogNamespace);check();if(mapping.status!==200||!mapping.body?.data.some(v=>v.kind===kind&&v.legacy_id===requested&&v.id===canonical))fail('Editorial read changed target without exact typed alias proof');}
  id=canonical;current=row;return{...response,body:row};
 }finally{busy=false;}};
 const mutate=async(action:EditorialAction,input?:unknown):Promise<Response<unknown>>=>{
  check();if(busy||pending())fail('An editorial command is running or unconfirmed. It will not be replayed.',409);
  if(!['create','update','delete','claim'].includes(action)||action==='claim'&&kind!=='group')fail('Unsupported editorial command',422);
  if(action==='create'?(!!id||!!current):(!id||!current))fail('Read the exact current target before changing it',409);
  let payload:Record<string,unknown>|undefined;
  if(action==='delete'){if(input!==undefined)fail('Deletion accepts no body',422);}
  else {payload=record(input,'editorial payload');const allowed=action==='claim'?['version','details']:kind==='author'?['name','biography','twitter','pixiv','melonBook','fanBox','booth','nicoVideo','skeb','fantia','tumblr','youtube','weibo','naver','website','version']:['name','website','ircServer','ircChannel','discord','contactEmail','description','twitter','mangaUpdates','inactive','publishDelay','leader','members','focusedLanguages','locked','version'];if(Object.keys(payload).some(k=>!allowed.includes(k)))fail('Unsupported editorial field',422);if(action==='create'){if(Object.hasOwn(payload,'version')||typeof payload.name!=='string'||!payload.name.trim())fail('Create needs an actual name and no assigned version',422);}else if(positive(payload.version)!==positive(current!.attributes.version))fail('Use the exact current editorial version',409);if(action==='claim'&&(typeof payload.details!=='string'||!payload.details.trim()||payload.details.length>8000))fail('Provide bounded ownership evidence for moderator review',422);if(encoder.encode(JSON.stringify(payload)).byteLength>(action==='claim'?8192:32768))fail(action==='claim'?'Group claim exceeds the signed 8 KiB budget':'Editorial request exceeds the signed 32 KiB budget',422);}
  if(payload){payload={...payload,expectedAccountId:account.id};if(encoder.encode(JSON.stringify(payload)).byteLength>(action==='claim'?8192:32768))fail('Signed editorial account intent exceeds its request budget',422);}
  const expected=id,previous=current,route=action==='create'?`/${kind}`:`/${kind}/${id}${action==='claim'?'/claim':''}${action==='delete'?'?expectedAccountId='+account.id:''}`,fenceKey=key();busy=true;let installed=false;
  try{const marker=JSON.stringify({action,target:id??null,version:previous?.attributes.version??null});deps.fences.set(fenceKey,marker);installed=true;if(deps.fences.get(fenceKey)!==marker)fail('Editorial recovery fence was not saved',409);
   const response=await mutationRequest({url:captured.origin+'/api/v1'+route,method:action==='delete'?'DELETE':action==='update'?'PUT':'POST',...(payload===undefined?{}:{body:encoder.encode(JSON.stringify(payload)),contentType:'application/json'})});check();
   if(response.status>=400&&response.status<500){deps.fences.delete(fenceKey);installed=false;return response;}
   if(action==='delete'){if(response.status!==200||record(response.body).result!=='ok')fail('Editorial deletion outcome is unconfirmed');current=undefined;}
   else if(action==='claim'){const receipt=record(response.body);if(response.status!==202||receipt.result!=='ok'||receipt.status!=='pending-moderation'||receipt.groupId!==expected||receipt.targetVersion!==previous!.attributes.version||receipt.scope!=='instance-local'||receipt.originalSigningAuthorityChanged!==false)fail('Group claim receipt changed its exact local review target');publicUuid(receipt.requestId);}
   else{if(response.status!==200)fail('Editorial mutation outcome is unconfirmed');const row=entity(kind,response,expected);if(row.attributes.version!==(action==='create'?1:positive(previous!.attributes.version)+1))fail('Editorial mutation has an unexpected optimistic version');id=row.id;current=row;}
   deps.fences.delete(fenceKey);installed=false;return response;
  }catch(error){if(installed&&error instanceof PublicUploadRequestError&&error.outcome==='unsent')deps.fences.delete(fenceKey);throw error;}finally{busy=false;}
 };
 return{account,current:()=>current,pending,assert:check,dispose:()=>{disposed=true;},read,mutate};
}
