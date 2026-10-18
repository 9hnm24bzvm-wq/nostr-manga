// The retained MAL lookup explicitly returns native IDs. Only the typed,
// namespace-bound public resolver can turn those results into UUID references.
import {PublicApiClient} from './client';
import {publicUuid,record,PublicApiContractError} from './validation';
export interface PublicImportRows{data:{myanimelist:string;id:string;title:string}[];missing:string[];ambiguous:string[];}
export async function publicImportMappings(input:unknown,requested:readonly string[],namespace:string,assertCurrent=()=>{},api=new PublicApiClient()):Promise<PublicImportRows>{
 assertCurrent();publicUuid(namespace);
 if(!requested.length||requested.length>400||requested.some(id=>!/^\d{1,20}$/.test(id))||new Set(requested).size!==requested.length)throw new PublicApiContractError('Invalid requested import scope',422);
 const body=record(input,'import mapping'),wanted=new Set(requested),seen=new Set<string>(),data:{myanimelist:string;id:string;title:string}[]=[],missing:string[]=[],ambiguous:string[]=[];
 const checked=(value:unknown)=>{if(typeof value!=='string'||!wanted.has(value)||seen.has(value))throw new PublicApiContractError('Import mapping escaped its exact requested MAL scope');seen.add(value);return value;};
 if(!Array.isArray(body.data)||!Array.isArray(body.missing)||!Array.isArray(body.ambiguous))throw new PublicApiContractError('Invalid import mapping disposition');
 for(const inputRow of body.data){const row=record(inputRow,'import mapping');if(typeof row.id!=='string'||!/^[A-Za-z0-9]{10}$/.test(row.id)||typeof row.title!=='string'||row.title.length>4096)throw new PublicApiContractError('Invalid native import resource');data.push({myanimelist:checked(row.myanimelist),id:row.id,title:row.title});}
 missing.push(...body.missing.map(checked));ambiguous.push(...body.ambiguous.map(checked));if(seen.size!==wanted.size)throw new PublicApiContractError('Import mapping omitted requested MAL IDs');
 const native=[...new Set(data.map(row=>row.id))],resolved=new Map<string,string>();
 for(let start=0;start<native.length;start+=256){assertCurrent();const response=await api.resolveLegacy(native.slice(start,start+256).map(id=>({kind:'manga',id})),namespace);assertCurrent();if(response.status<200||response.status>=300||!response.body)throw new PublicApiContractError('Public import identity lookup failed ('+response.status+')',response.status);for(const row of response.body.data)resolved.set(row.legacy_id,publicUuid(row.id));}
 const visible=data.filter(row=>{if(resolved.has(row.id))return true;missing.push(row.myanimelist);return false;}).map(row=>({...row,id:resolved.get(row.id)!}));
 assertCurrent();return{data:visible,missing,ambiguous};
}
