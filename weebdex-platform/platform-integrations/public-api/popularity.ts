// Instance-local observed reads, never follows/ratings or a synthetic ranking.
import type {PublicApiClient} from './client';
import type {Collection,Response} from './types';
import {publicQuery} from './query';
import {publicUuid,PublicApiContractError,record} from './validation';
export type PopularityKind='manga'|'chapter';
const windows={'24h':86400,'7d':604800,'30d':2592000} as const;
type Window=keyof typeof windows;
export interface PopularityQuery{query:URLSearchParams;time:Window;limit:number;emptySelection:boolean;}
function fail(message:string,status=502):never{throw new PublicApiContractError(message,status);}
export function popularityQuery(kind:PopularityKind,input:URLSearchParams,includes:readonly string[]=[]):PopularityQuery {
 if(!['manga','chapter'].includes(kind))fail('Unsupported popularity resource',422);
 const effective=new URLSearchParams(input);let time:Window='30d';
 for(const key of['rank','time']){const values=effective.getAll(key);if(values.length>1)fail('Duplicate popularity '+key,422);if(key==='rank'&&values.length&&values[0]!=='read')fail('Only observed read popularity is supported.',422);if(key==='time'&&values.length){if(!Object.hasOwn(windows,values[0]))fail('Unsupported read-popularity window.',422);time=values[0] as Window;}effective.delete(key);}
 if([...effective.keys()].some(key=>key==='sort'||key.startsWith('sort[')||key==='order'||key.startsWith('order[')))fail('Read popularity has a fixed observed-view order.',422);
 const selected=[...effective.getAll('contentRating'),...effective.getAll('contentRating[]')],emptySelection=selected.includes('__none__');
 if(emptySelection){if(selected.length!==1)fail('An empty rating selection cannot include other ratings.',422);effective.delete('contentRating');effective.delete('contentRating[]');}
 if(!effective.has('limit'))effective.set('limit','10');
 const query=publicQuery(kind,effective,includes),limit=Number(query.get('limit'));
 if(limit>10||Number(query.get('offset')??0)!==0)fail('Popularity exposes only the first ten observed resources.',422);
 query.set('rank','read');query.set('time',time);return{query,time,limit,emptySelection};
}
function views(input:unknown):string{if(typeof input!=='string'||! /^[1-9][0-9]{0,18}$/.test(input)||input.length===19&&input>'9223372036854775807')fail('Invalid exact observed view count');return input;}
function compare(a:string,b:string){return a.length-b.length||(a<b?-1:a>b?1:0);}
export function retainedPopularityViews(input:unknown):string{const resource=record(record(record(input).public_api).resource),score=record(record(record(resource.attributes).nostr).popularity);return views(score.views);}
export async function popularityCollection(api:PublicApiClient,kind:PopularityKind,prepared:PopularityQuery,signal?:AbortSignal):Promise<Response<Collection>> {
 if(prepared.emptySelection)fail('An explicitly empty filter is resolved by the view, not a server observation.',422);
 const response=await api.collection('/'+kind+'/top',kind,prepared.query,signal);
 if(response.status<200||response.status>=300||!response.body)return response;
 const body=response.body,meta=record(record(body).nostr,'popularity provenance');
 if(meta.scope!=='instance-catalogue'||meta.source!=='persisted-native-view-events'||meta.metric!=='deduplicated-views'||meta.rank!=='read'||meta.time!==prepared.time||meta.windowSeconds!==windows[prepared.time]||meta.order!=='window-views-desc-native-id-asc'||typeof meta.observedAt!=='string'||!/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d{6}Z$/.test(meta.observedAt)||!Number.isFinite(Date.parse(meta.observedAt)))fail('Invalid observed popularity window or source');
 if(body.limit!==prepared.limit||body.offset!==0||body.data.length!==Math.min(body.total,prepared.limit))fail('Invalid bounded popularity collection');
 const seen=new Set<string>();let previous:string|undefined;
 for(const row of body.data){const id=publicUuid(row.id),score=record(record(row.attributes.nostr,'resource provenance').popularity,'observed popularity'),count=views(score.views);if(row.id!==id||seen.has(id)||score.rank!=='read'||score.time!==prepared.time||previous!==undefined&&compare(previous,count)<0)fail('Invalid observed popularity resource, score or order');seen.add(id);previous=count;}
 return response;
}
