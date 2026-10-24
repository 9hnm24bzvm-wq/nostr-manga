// Hydration for a device-owned UUID library. Saved membership and reading state
// stay local; this adapter only reads the exact visible resources requested.
import {publicCatalogueRead, type ReadOptions} from './site-client';
import type {Response} from './types';
import {publicUuid, PublicApiContractError, record} from './validation';
import {utcOrderKey, decimalKey} from './exact-order';
type Row={id:string;[key:string]:unknown};
type Read=(operation:string,options:ReadOptions)=>Promise<Response<unknown>>;
function fail(detail:string,status=502):never{throw new PublicApiContractError(detail,status);}
const maxRows=10000,maxParents=1000;
function sortFor(path:string,input:URLSearchParams){
 const isManga=path==='/manga',requested=input.get('sort')??(isManga?'createdAt':'publishedAt');
 const sort=({readAt:'publishedAt',lastUploadedChapterAt:'latestUploadedChapter',follows:'followedCount'} as Record<string,string>)[requested]??requested;
 const supported=isManga?['title','year','createdAt','updatedAt','latestUploadedChapter','followedCount','rating']:['publishedAt','publishAt','createdAt','updatedAt','chapter','volume'];
 if(!supported.includes(sort))fail('This saved-library sort has no verified public catalogue value: '+requested,422);
 if(input.getAll('sort').length>1||input.getAll('order').length>1)fail('Duplicate saved-library ordering',422);
 if([...input.keys()].some(key=>key.startsWith('order[')))fail('Saved-library ordering requires one explicit view sort',422);
 if(input.has('order')&&!['asc','desc'].includes(input.get('order')!))fail('Invalid saved-library sort direction',422);
 return sort;
}
function mangaSortable(row:Row,sort:string):Row{
 const attributes=record(record(record(row.public_api,'saved Manga provenance').resource).attributes),nostr=record(attributes.nostr,'saved Manga comparison keys');
 if(nostr.sortKeySemantics!=='postgresql-exact-v1')fail('Saved Manga comparison keys are unavailable');
 const keys=record(nostr.sortKeys,'saved Manga comparison keys');
 if(publicUuid(keys.tieBreak)!==row.id)fail('Saved Manga comparison keys identify a different title');
 let value=keys[sort];const decimal=['year','followedCount','rating'].includes(sort);
 if(value===null){if(!['year','latestUploadedChapter'].includes(sort))fail('Invalid null Manga comparison key');}
 else if(decimal)value=decimalKey(value);
 else if(sort==='title'){if(typeof value!=='string'||!value.length||value.length>8000||value.length%2||!/^[0-9a-f]+$/.test(value))fail('Invalid exact title byte key');}
 else if(typeof value!=='string'||!/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d{6}Z$/.test(value)||!Number.isFinite(Date.parse(value)))fail('Invalid exact Manga timestamp key');
 return {...row,_sort_value:value,_sort_text:'',_sort_decimal:decimal};
}
function sortable(row:Row,sort:string):Row{
 let value:unknown,text='';
 if(sort==='title'){if(typeof row.title!=='string')fail('Saved title has no actual title');value=(row.title as string).toLowerCase();}
 else if(sort==='year')value=row.year??null;
 else if(sort==='chapter'||sort==='volume'){const source=row[sort];value=typeof source==='string'&&/^\d+(?:\.\d+)?$/.test(source)?decimalKey(source):null;text=typeof source==='string'?source.toLowerCase():'';}
 else {const field=sort==='createdAt'?'created_at':sort==='updatedAt'?'updated_at':'published_at';value=utcOrderKey(row[field]);}
 if(typeof value==='number'&&!Number.isFinite(value))fail('Invalid actual saved-library sort value');
 return {...row,_sort_value:value,_sort_text:text,_sort_decimal:sort==='chapter'||sort==='volume'};
}
export async function publicLibraryCandidates<T extends {id:string}>(path:string,key:string,ids:string[],input:URLSearchParams,read:Read=publicCatalogueRead):Promise<T[]>{
 if(!ids.length)return [];
 if(!((path==='/manga'&&key==='id')||(path==='/chapter/feed'&&['id','manga'].includes(key))))fail('Unsupported saved-library catalogue scope',422);
 const unique=[...new Set(ids.map(publicUuid))],parents=key==='manga';
 if(unique.length>(parents?maxParents:maxRows))fail('Saved-library view exceeds its bounded catalogue budget',422);
 const sort=sortFor(path,input),base=new URLSearchParams(input);
 for(const field of ['page','limit','offset',key,key+'[]',...(key==='id'?['ids[]']:[])])base.delete(field);
 base.set('sort',sort==='publishedAt'?'publishAt':sort);base.set('limit','100');
 const results=new Map<string,Row>(),scopes=parents?unique.map(id=>[id]):Array.from({length:Math.ceil(unique.length/100)},(_,index)=>unique.slice(index*100,index*100+100));
 for(const scope of scopes){
  const allowed=new Set(scope),query=new URLSearchParams(base);for(const id of scope)query.append(key,id);
  const operation=path==='/manga'?'manga.search':'chapter.search';let expected:number|undefined,offset=0;
  for(;;){
   query.set('offset',String(offset));const response=await read(operation,{query:new URLSearchParams(query)});
   if(response.status<200||response.status>=300||!response.body)fail('Saved-library catalogue lookup failed ('+response.status+')',response.status);
   const body=response.body as {total?:unknown;limit?:unknown;offset?:unknown;data?:unknown};
   if(!Number.isSafeInteger(body.total)||Number(body.total)<0||!Array.isArray(body.data)||body.limit!==100||body.offset!==offset)fail('Invalid saved-library catalogue page');
   const total=Number(body.total);if(total>maxRows||(!parents&&total>scope.length))fail('Saved-library catalogue exceeded its exact requested scope',422);
   if(expected!==undefined&&expected!==total)fail('Saved-library catalogue changed during pagination; reload the view',409);expected=total;
   if(body.data.length>100||body.data.length>total-offset||(!body.data.length&&offset<total))fail('Saved-library catalogue returned an incomplete page');
   for(const inputRow of body.data){
    if(!inputRow||typeof inputRow!=='object'||Array.isArray(inputRow))fail('Invalid saved-library resource');
    const row=inputRow as Row,id=publicUuid(row.id);
    if(results.has(id))fail('Saved-library catalogue repeated a resource');
    if(parents){const relationships=row.relationships as {manga?:{id?:unknown}}|undefined;if(!relationships?.manga||!allowed.has(publicUuid(relationships.manga.id)))fail('Saved-library chapter returned the wrong parent');}
    else if(!allowed.has(id))fail('Saved-library catalogue returned an unrequested resource');
    results.set(id,path==='/manga'?mangaSortable(row,sort):sortable(row,sort));if(results.size>maxRows)fail('Saved-library view exceeds its bounded catalogue budget',422);
   }
   offset+=body.data.length;if(offset>=total)break;
  }
 }
 return [...results.values()] as T[];
}
