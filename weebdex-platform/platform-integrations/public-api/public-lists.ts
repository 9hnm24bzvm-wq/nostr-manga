// Public signed-graph lists are distinct from anonymous device lists and native
// private lists. Preserve signed membership order and source-head provenance.
import {PublicApiClient} from './client';
import {normalizeManga} from './normalize';
import {publicUuid,record,resource,PublicApiContractError} from './validation';
import type{MangaView}from'./types';
export interface PublicListView{id:string;name:string;ownerId:string;sourceOwner:string;head:string;root:string;members:{id:string;eventId:string;position:number}[];}
function fail(message:string,status=502):never{throw new PublicApiContractError(message,status);}
function hash(value:unknown){if(typeof value!=='string'||!/^[0-9a-f]{64}$/.test(value))fail('Invalid public-list signed source');return value;}
export function normalizePublicList(input:unknown,envelope?:unknown):PublicListView{
 const row=resource(input,'custom_list'),attrs=row.attributes,nostr=record(record(row).nostr,'public-list source');
 if(typeof attrs.name!=='string'||attrs.name.length>4096||attrs.visibility!=='public'||nostr.scope!=='published_library_locally_projected_manga'||typeof nostr.listId!=='string'||!/^[0-9a-f]{32}$/.test(nostr.listId))fail('Invalid public signed-list presentation');
 const owner=hash(nostr.pubkey),head=hash(nostr.head),root=hash(nostr.root),owners=row.relationships.filter(row=>row.type==='user');
 if(owners.length!==1||row.relationships.some(row=>!['user','manga'].includes(row.type)))fail('Public list returned ambiguous ownership');
 if(envelope!==undefined){const meta=record(envelope,'public-list snapshot');if(meta.scope!==nostr.scope||meta.pubkey!==owner||meta.head!==head||meta.root!==root)fail('Public list escaped its signed snapshot');}
 let previous=-1;const seen=new Set<string>(),members=row.relationships.filter(row=>row.type==='manga').map(relation=>{const id=publicUuid(relation.id),source=record(record(relation).nostr,'public-list member');if(seen.has(id)||!Number.isSafeInteger(source.position)||Number(source.position)<=previous)fail('Invalid or repeated public-list member');seen.add(id);previous=Number(source.position);return{id,eventId:hash(source.eventId),position:Number(source.position)};});
 if(members.length>5000)fail('Public list exceeds its bounded membership view',422);
 return{id:publicUuid(row.id),name:attrs.name,ownerId:publicUuid(owners[0].id),sourceOwner:owner,head,root,members};
}
// A different returned UUID is an alias only with the backend's exact typed
// persisted mapping proof, repeated on both the entity and its source envelope.
function bindListTarget(requested:string,list:PublicListView,entity:unknown,envelope:unknown){
 const a=record(record(entity).nostr).identity,b=record(envelope).identity;
 if(a===undefined&&b===undefined){if(list.id!==requested)fail('This list URL is not proven to identify the returned list. Open it from its creator’s published lists.',409);return;}
 const expected:Record<string,string>={kind:'list',requestedId:requested,canonicalId:list.id,source:'persisted-public-resource-ids'};
 for(const input of[a,b]){const proof=record(input,'typed list identity proof');if(Object.keys(proof).length!==4||Object.entries(expected).some(([key,value])=>proof[key]!==value))fail('The list identity proof does not match this requested list.',409);}
}
export function createPublicListReader(api=new PublicApiClient()){
 async function get(input:string,assertCurrent=()=>{}){assertCurrent();const requested=publicUuid(input),response=await api.entity('/list/'+requested,'custom_list');assertCurrent();if(response.status!==200||!response.body)fail(response.status===404?'This published list is no longer available.':'The published list could not be loaded ('+response.status+').',response.status);const list=normalizePublicList(response.body.data,record(response.body).nostr);bindListTarget(requested,list,response.body.data,record(response.body).nostr);return list;}
 async function members(list:PublicListView,assertCurrent=()=>{}):Promise<MangaView[]>{
  assertCurrent();const rows=new Map<string,MangaView>();
  for(let start=0;start<list.members.length;start+=100){const wanted=new Set(list.members.slice(start,start+100).map(row=>row.id)),query=new URLSearchParams({limit:'100',offset:'0'});for(const id of wanted)query.append('ids[]',id);for(const value of['safe','suggestive','erotica','pornographic'])query.append('contentRating[]',value);for(const include of['cover_art','author','artist'])query.append('includes[]',include);const response=await api.collection('/manga','manga',query);assertCurrent();if(response.status!==200||!response.body)fail('Public list titles could not be loaded',response.status);const body=response.body;if(body.offset!==0||body.limit!==100||body.total!==wanted.size||body.data.length!==wanted.size)fail('Public list visibility changed. Reload the list.',409);for(const input of body.data){const row=normalizeManga(input);if(!wanted.has(row.id)||rows.has(row.id))fail('Public list returned an unrequested or repeated title');rows.set(row.id,row);}}
  const final=await api.entity('/list/'+list.id,'custom_list',new URLSearchParams({head:list.head}));assertCurrent();if(final.status!==200||!final.body)fail('The published list changed while loading. Reload it.',final.status===200?409:final.status);const current=normalizePublicList(final.body.data,record(final.body).nostr);if(JSON.stringify(current)!==JSON.stringify(list))fail('The published list membership changed while loading',409);
  return list.members.map(member=>rows.get(member.id)!);
 }
 async function owner(input:string,offset=0,head?:string,assertCurrent=()=>{}){assertCurrent();const ownerId=publicUuid(input);if(!Number.isSafeInteger(offset)||offset<0||offset>10000)fail('Invalid public-list page',422);const query=new URLSearchParams({limit:'20',offset:String(offset)});if(head)query.set('head',hash(head));const response=await api.collection('/user/'+ownerId+'/list','custom_list',query);assertCurrent();if(response.status!==200||!response.body)fail('Published lists could not be loaded ('+response.status+').',response.status);const body=response.body,meta=record(record(body).nostr,'public-list snapshot');if(body.offset!==offset||body.limit!==20||body.data.length>20||body.data.length>Math.max(0,body.total-offset)||(!body.data.length&&offset<body.total))fail('Invalid public-list page');if(meta.scope!=='published_library_locally_projected_manga')fail('Invalid public-list directory scope');hash(meta.pubkey);const currentHead=meta.head===null?null:hash(meta.head);if(currentHead===null){if(meta.root!==null||body.total!==0)fail('Invalid empty public-list snapshot');}else hash(meta.root);const lists=body.data.map(row=>normalizePublicList(row,meta)),seen=new Set<string>();for(const row of lists){if(row.ownerId!==ownerId||seen.has(row.id))fail('Public-list directory returned a different publisher or duplicate list');seen.add(row.id);}if(head&&currentHead!==head)fail('Publisher list snapshot changed',409);return{data:lists,total:body.total,offset,limit:20,head:currentHead};}
 return{get,members,owner};
}
export const publicLists=createPublicListReader();
