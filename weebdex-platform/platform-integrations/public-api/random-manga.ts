// Retained random-title constraints include language and available chapters,
// which /manga/random does not accept. Use a bounded uniform collection sample
// for that case, keeping every constraint in both requests.
import type {PublicApiClient} from './client';
import type {Entity,Response} from './types';
import {publicQuery} from './query';
import {PublicApiContractError} from './validation';
const randomKeys=new Set(['includedTags[]','excludedTags[]','includedTagsMode','excludedTagsMode','contentRating[]','includes[]']);
export async function randomManga(api:PublicApiClient,input:URLSearchParams,includes:readonly string[],signal?:AbortSignal,random:()=>number=Math.random):Promise<Response<Entity>>{
 for(const key of input.keys())if(['limit','offset','page','sort','order'].includes(key)||key.startsWith('order['))throw new PublicApiContractError('Random title does not accept pagination or ranking controls',422);
 const query=publicQuery('manga',input,includes);query.delete('limit');
 if([...query.keys()].every(key=>randomKeys.has(key)))return api.entity('/manga/random','manga',publicQuery('random',input,includes),signal);
 const countQuery=new URLSearchParams(query);countQuery.set('limit','0');countQuery.set('offset','0');const count=await api.collection('/manga','manga',countQuery,signal);if(count.status<200||count.status>=300||!count.body)return count as unknown as Response<Entity>;
 const total=count.body.total;if(total===0)return {status:404,body:undefined};if(total>10001)throw new PublicApiContractError('Random title scope exceeds the bounded 10,001-title sampling window',422);
 const sample=random();if(!Number.isFinite(sample)||sample<0||sample>=1)throw new PublicApiContractError('Invalid random sample');const selectedQuery=new URLSearchParams(query);selectedQuery.set('limit','1');selectedQuery.set('offset',String(Math.floor(sample*total)));const selected=await api.collection('/manga','manga',selectedQuery,signal);if(selected.status<200||selected.status>=300||!selected.body)return selected as unknown as Response<Entity>;
 if(selected.body.total!==total||selected.body.data.length!==1)throw new PublicApiContractError('The random title scope changed; try again',409);
 return {status:200,body:{result:'ok',response:'entity',data:selected.body.data[0]}};
}
