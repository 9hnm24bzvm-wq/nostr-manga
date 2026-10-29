// Versioned directional catalogue edges under the current browser account.
import {createNativeContributionScope,type NativeContributionDependencies} from './native-contribution-scope';

import {PublicUploadRequestError} from './nostr-upload-request';
import type {TitleAccount} from './title-client';
import type {Resource,Response} from './types';
import type {EditorChoice} from './editor-fields';
import {record,resource,publicUuid,localized,PublicApiContractError} from './validation';
import {PublicApiClient} from './client';
export const relationKinds=['main_story','side_story','prequel','sequel','adapted_from','spin_off','based_on','doujinshi','monochrome','colored','preserialization','serialization','alternate_story','alternate_version','same_franchise','shared_universe']as const;
export const relationLimit=4096;
export const relationPageSize=32;
export interface RelationPage{readonly offset:number;readonly total:number;readonly persistedTotal:number;readonly nextOffset:number|null;readonly snapshot:string;readonly ownerVersion:number;}
export type RelationAction='create'|'update'|'delete';
export interface RelationController{account:TitleAccount;pending():boolean;dispose():void;version():number|undefined;page():Readonly<RelationPage>|undefined;search(query:string,signal:AbortSignal):Promise<EditorChoice[]>;read(version:number,state:string,offset?:number):Promise<Response<Resource[]>>;mutate(action:RelationAction,input:unknown):Promise<Response<unknown>>;}
const encoder=new TextEncoder();
function fail(message:string,status=502):never{throw new PublicApiContractError(message,status);}
function version(value:unknown):number{if(!Number.isSafeInteger(value)||Number(value)<1||Number(value)>2147483647)fail('A current owner-title version is required',422);return Number(value);}
function grants(value:unknown):string[]{if(!Array.isArray(value)||value.some(v=>typeof v!=='string'))fail('Invalid current account grants');return[...value]as string[];}
function targetOf(row:Resource):string{const matches=row.relationships.filter(r=>r.type==='manga');if(matches.length!==1)fail('Relation lacks one exact target title');return publicUuid(matches[0].id);}
function edge(input:unknown,owner:string,expectedVersion:number):Resource{const row=resource(input,'manga_relation'),a=row.attributes,n=record(a.nostr);if(version(a.version)!==expectedVersion||!relationKinds.some(v=>v===a.relation)||n.scope!=='instance-local'||n.versionScope!=='owner_manga'||a.ownerMangaId!==undefined&&a.ownerMangaId!==owner)fail('Relation receipt changed owner version or scope');targetOf(row);return row;}
export async function createSessionRelationController(nativeAccount:string,target:string,expectedAccountId:string,assertCurrent:()=>void,injected:NativeContributionDependencies={}):Promise<RelationController>{
 const owner=publicUuid(target),expectedAccount=publicUuid(expectedAccountId),access=await createNativeContributionScope(nativeAccount,assertCurrent,injected),deps={fences:access.fences},captured={origin:access.origin,catalogNamespace:access.catalogNamespace},account=access.account;
 if(account.id!==expectedAccount){access.dispose();fail('Relation account changed',409);}
 let disposed=false,busy=false,rows:Resource[]|undefined,currentVersion:number|undefined,pageState:Readonly<RelationPage>|undefined,readState:string|undefined;
 const visited=new Map<number,string[]>(),edgeOffsets=new Map<string,number>(),targetOffsets=new Map<string,number>();
 const check=()=>{access.check();if(disposed)fail('Relation view closed',409);assertCurrent();};
 const request=access.request,publicRequest=access.publicRequest,mutationRequest=async(input:Parameters<typeof request>[0])=>{check();const fresh=await access.context();check();if(fresh.account.id!==account.id||fresh.catalogNamespace!==captured.catalogNamespace)throw new PublicUploadRequestError('Relation account changed','unsent',409);return request(input);};
 const key=access.key('relations-pending')+':'+captured.catalogNamespace+':'+owner;
 const pending=()=>deps.fences.get(key)!==null;
 async function read(expectedVersion:number,state:string,offset?:number):Promise<Response<Resource[]>>{check();if(busy)fail('A relation command is running',409);const v=version(expectedVersion);if(!['draft','submitted','rejected','published'].includes(state))fail('Invalid actual title state',422);
  const previous=pageState,wanted=offset??0,continuation=offset!==undefined;
  if(!Number.isSafeInteger(wanted)||wanted<0||wanted>relationLimit)fail('Invalid relation page offset',422);
  if(continuation&&(!previous||previous.ownerVersion!==v||readState!==state||wanted>previous.total))fail('Reload the current relation snapshot before changing pages',409);
  if(continuation&&!visited.has(wanted)&&wanted!==previous!.nextOffset)fail('Use the exact next relation page or a previously verified page',409);
  if(!continuation){visited.clear();edgeOffsets.clear();targetOffsets.clear();}
  busy=true;rows=undefined;currentVersion=undefined;pageState=undefined;readState=undefined;
  try{
   const query=new URLSearchParams({limit:String(relationPageSize),offset:String(wanted),ownerVersion:String(v),expectedAccountId:account.id});
   if(continuation)query.set('snapshot',previous!.snapshot);
   // The retained native layout prefixes URLSearchParams.toString() with '?'.
   const encoded=query.toString().replace(/^\?/, '');
   const response=await request({url:`${captured.origin}/api/v1/account/manga/${owner}/relations?${encoded}`,method:'GET'});check();if(response.status!==200)return response as Response<Resource[]>;
   const body=record(response.body),n=record(body.nostr),proof=record(n.identity);const total=body.total,persisted=body.persistedTotal;
   if(body.result!=='ok'||body.response!=='collection'||!Array.isArray(body.data)||body.data.length>relationPageSize||body.limit!==relationPageSize||body.offset!==wanted||!Number.isSafeInteger(total)||Number(total)<0||Number(total)>relationLimit||!Number.isSafeInteger(persisted)||Number(persisted)<Number(total)||Number(persisted)>relationLimit)fail('Relation page is incomplete or invalid');
   if(body.ownerMangaId!==owner||body.ownerVersion!==v||body.version!==v||typeof body.snapshot!=='string'||!/^[0-9a-f]{64}$/.test(body.snapshot)||n.scope!=='instance-account'||n.versionScope!=='owner_manga'||n.pagination!=='bounded-relations-v1'||n.snapshot!==body.snapshot||proof.kind!=='manga'||proof.requestedId!==owner||proof.canonicalId!==owner||proof.source!=='persisted-public-resource-ids')fail('Relation owner/snapshot identity changed; reload the actual title',409);
   const end=wanted+body.data.length,next=end<Number(total)?end:null;
   if(end>Number(total)||body.nextOffset!==next||body.data.length===0&&wanted<Number(total))fail('Relation continuation skipped or omitted resources');
   if(continuation&&(body.snapshot!==previous!.snapshot||total!==previous!.total||persisted!==previous!.persistedTotal))fail('Relation collection changed between pages; refresh from the first page',409);
   const result=body.data.map(value=>edge(value,owner,v)),ids=result.map(r=>r.id),targets=result.map(targetOf),proofs=result.map((row,i)=>JSON.stringify([row.id,targets[i],row.attributes.relation]));if(new Set(ids).size!==result.length||new Set(targets).size!==result.length)fail('Duplicate relation UUID or target');
   const old=visited.get(wanted);if(old&&JSON.stringify(old)!==JSON.stringify(proofs)||ids.some(id=>edgeOffsets.has(id)&&edgeOffsets.get(id)!==wanted)||targets.some(id=>targetOffsets.has(id)&&targetOffsets.get(id)!==wanted))fail('Relation pages repeated or changed a directional edge or target');
   visited.set(wanted,proofs);for(const id of ids)edgeOffsets.set(id,wanted);for(const id of targets)targetOffsets.set(id,wanted);
   rows=JSON.parse(JSON.stringify(result)) as Resource[];currentVersion=v;readState=state;pageState=Object.freeze({offset:wanted,total:Number(total),persistedTotal:Number(persisted),nextOffset:next,snapshot:body.snapshot,ownerVersion:v});return{...response,body:result};
  }finally{busy=false;}
 }
 async function search(query:string,signal:AbortSignal):Promise<EditorChoice[]> {
  check();if(signal.aborted)fail('Title search cancelled',409);
  const name=query.trim();if(!name||name.length>200)fail('Enter a title name to search (up to 200 characters).',422);
  const api=new PublicApiClient(({path,signal})=>publicRequest({url:captured.origin+'/api/v1'+path,method:'GET',signal}));
  const result=await api.collection('/manga','manga',new URLSearchParams({title:name,limit:'25',offset:'0'}),signal);
  check();if(signal.aborted)fail('Title search cancelled',409);
  if(result.status!==200||!result.body)fail(`Title search is unavailable (HTTP ${result.status})`,result.status);
  if(result.body.limit!==25||result.body.offset!==0)fail('Title search returned an unexpected page');
  return result.body.data.filter(row=>row.id!==owner&&row.attributes.state==='published').map(row=>({id:publicUuid(row.id),name:localized(row.attributes.title)||'Untitled title'}));
 }
 async function canonicalTarget(input:string):Promise<string>{const requested=publicUuid(input),response=await publicRequest({url:`${captured.origin}/api/v1/manga/${requested}`,method:'GET'});check();if(response.status!==200)fail('Related title is not currently public',response.status);const body=record(response.body);if(body.result!=='ok'||body.response!=='entity')fail('Invalid related-title read');const row=resource(body.data,'manga');if(row.attributes.state!=='published')fail('Related title is not published');const canonical=publicUuid(row.id);if(canonical!==requested){const api=new PublicApiClient(async({path,method='GET',body,signal})=>publicRequest({url:captured.origin+'/api/v1'+path,method,signal,...(body===undefined?{}:{body:encoder.encode(JSON.stringify(body)),contentType:'application/json'})}));const mapping=await api.resolveLegacy([{kind:'manga',id:requested}],captured.catalogNamespace);check();if(mapping.status!==200||!mapping.body?.data.some(r=>r.kind==='manga'&&r.legacy_id===requested&&r.id===canonical))fail('Related title alias lacks exact typed proof');}return canonical;}
 async function mutate(action:RelationAction,input:unknown):Promise<Response<unknown>>{check();if(busy||pending()||!rows||!currentVersion)fail('Reload current relations before changing them; uncertain commands are never replayed',409);if(!['create','update','delete'].includes(action))fail('Unsupported relation command',422);
  if(action==='create'&&pageState!.persistedTotal>=relationLimit)fail('This title has reached its 4096-relation limit; remove an existing relation first',409);
  const value={...record(input)},allowed=action==='create'?['targetManga','relation','version']:action==='update'?['relationId','relation','version']:['relationId','version'];if(Object.keys(value).some(k=>!allowed.includes(k)))fail('Unsupported relation command field',422);if(version(value.version)!==currentVersion)fail('Use the exact current owner-title version',409);
  const previous=action==='create'?undefined:rows.find(r=>r.id===publicUuid(value.relationId));if(action!=='create'&&!previous)fail('Relation is not in this exact owner snapshot',409);if(action!=='delete'&&!relationKinds.some(k=>k===value.relation))fail('Invalid relation kind',422);
  const capturedVersion=currentVersion;busy=true;let installed=false;try{
   const target=action==='create'?await canonicalTarget(publicUuid(value.targetManga)):targetOf(previous!);check();if(target===owner)fail('A title cannot relate to itself',422);
   const payload=action==='delete'?{version:capturedVersion,expectedAccountId:account.id}:{targetManga:target,relation:value.relation,version:capturedVersion,expectedAccountId:account.id};const bytes=encoder.encode(JSON.stringify(payload));if(bytes.byteLength>4096)fail('Relation changes are too large',422);
   const marker=JSON.stringify({action,owner,relationId:previous?.id??null,target,version:capturedVersion});deps.fences.set(key,marker);installed=true;if(deps.fences.get(key)!==marker)fail('Relation recovery fence was not persisted',409);
   const response=await mutationRequest({url:`${captured.origin}/api/v1/account/manga/${owner}/relation${action==='delete'?'/'+previous!.id:''}`,method:action==='create'?'POST':action==='update'?'PUT':'DELETE',body:bytes,contentType:'application/json'});check();
   if(response.status>=400&&response.status<500){deps.fences.delete(key);installed=false;return response;}
   const nextVersion=action==='update'&&previous!.attributes.relation===value.relation?capturedVersion:capturedVersion+1;
   if(action==='delete'){const receipt=record(response.body),n=record(receipt.nostr);if(response.status!==200||receipt.result!=='ok'||receipt.mangaId!==owner||receipt.relationId!==previous!.id||receipt.version!==nextVersion||n.scope!=='instance-account'||n.versionScope!=='owner_manga')fail('Relation deletion outcome is unconfirmed');}
   else{const body=record(response.body);if(response.status!==200||body.result!=='ok'||body.response!=='entity')fail('Relation mutation outcome is unconfirmed');const result=edge(body.data,owner,nextVersion);if(result.attributes.ownerMangaId!==owner||targetOf(result)!==target||result.attributes.relation!==value.relation||previous&&result.id!==previous.id)fail('Relation receipt changed the exact directional edge');}
   deps.fences.delete(key);installed=false;rows=undefined;currentVersion=undefined;pageState=undefined;readState=undefined;return response;
  }catch(error){if(installed&&error instanceof PublicUploadRequestError&&error.outcome==='unsent')deps.fences.delete(key);throw error;}finally{busy=false;}
 }
 return{account,pending,dispose:()=>{disposed=true;access.dispose();},version:()=>currentVersion,page:()=>pageState,search,read,mutate};
}
