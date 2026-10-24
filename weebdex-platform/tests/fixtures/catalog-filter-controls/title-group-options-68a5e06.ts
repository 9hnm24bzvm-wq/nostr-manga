// Application-owned optional title-wide group choices. Never derive choices
// from the first chapter page, current language, or native identity strings.
import { publicUuid, PublicApiContractError, record, resource } from './validation';
import type { Resource } from './types';
export interface TitleGroupPage { result:'ok'; semantics:'visible-title-groups-v1'; data:Resource[]; total:number; limit:number; next:string|null; }
export interface TitleGroupOptions { data:{id:string;name:string}[]; total:number; limit:number; next:string|null; }
export function titleGroupQuery(input:URLSearchParams):URLSearchParams {
 const query=new URLSearchParams(),seen=new Set<string>();let count=0;
 for(const[key,value]of input){
  if(!['limit','after','ids[]'].includes(key)||key!=='ids[]'&&seen.has(key))throw new PublicApiContractError('Invalid title-group query',422);
  if(key==='limit'){if(!/^\d+$/.test(value)||Number(value)<1||Number(value)>100)throw new PublicApiContractError('Invalid title-group page size',422);query.set(key,String(Number(value)));}
  else {const id=publicUuid(value);if(key==='ids[]'&&++count>100)throw new PublicApiContractError('Too many selected groups',422);query.append(key,id);}
  seen.add(key);
 }
 if(query.has('after')&&query.has('ids[]'))throw new PublicApiContractError('Ambiguous title-group cursor',422);
 if(!query.has('limit'))query.set('limit','100');return query;
}
export function titleGroupPage(input:unknown,query:URLSearchParams):TitleGroupPage {
 const body=record(input,'title-group page'),limit=Number(query.get('limit'));
 if(body.result!=='ok'||body.semantics!=='visible-title-groups-v1'||!Array.isArray(body.data)||body.limit!==limit||!Number.isSafeInteger(body.total)||Number(body.total)<body.data.length||body.data.length>limit||body.next!==null&&typeof body.next!=='string')throw new PublicApiContractError('Invalid title-group page');
 let previous=query.get('after')??'';const requested=new Set(query.getAll('ids[]'));
 for(const value of body.data){const row=resource(value,'scanlation_group'),id=publicUuid(row.id);if(id<=previous||requested.size&&!requested.has(id)||typeof row.attributes.name!=='string'||!row.attributes.name||new TextEncoder().encode(row.attributes.name).length>4096)throw new PublicApiContractError('Invalid, reordered, or unrelated title-group choice');previous=id;}
 if(body.next!==null&&(publicUuid(body.next)!==previous||body.data.length!==limit))throw new PublicApiContractError('Invalid title-group continuation');
 if(!query.has('after')&&body.next===null&&body.data.length!==body.total)throw new PublicApiContractError('Incomplete first title-group page');
 if(body.next!==null&&Number(body.total)<=body.data.length)throw new PublicApiContractError('Spurious title-group continuation');
 if(!body.data.length&&body.next!==null)throw new PublicApiContractError('Empty title-group continuation');
 return body as unknown as TitleGroupPage;
}
