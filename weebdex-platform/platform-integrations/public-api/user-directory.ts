// The directory requires an explicitly bound instance account. Public profile
// reads are separate; this path never substitutes cookies or protocol publishers.
import {boundAccountAccess} from './bound-account-request';
import {normalizePublicUser} from './user-profile';
import {publicUuid,record,resource,PublicApiContractError} from './validation';
function fail(message:string,status=422):never{throw new PublicApiContractError(message,status);}
export function userDirectoryQuery(input:URLSearchParams){
 const query=new URLSearchParams(),seen=new Set<string>(),ids=new Set<string>();
 let limit=100,offset=0,page:string|undefined,sort:string|undefined,order:string|undefined;
 for(const [raw,value] of input){
  const key=raw==='name'?'username':raw==='id'?'ids[]':raw;
  if(key==='ids[]'){let id:string;try{id=publicUuid(value);}catch{fail('User filters require public User UUIDs.');}if(ids.has(id)||ids.size>=100)fail('Duplicate or excessive User UUID filters.');ids.add(id);query.append(key,id);continue;}
  if(!['username','limit','offset','page','sort','order','order[username]'].includes(key)||seen.has(key))fail('Unsupported or duplicate user search option: '+raw+'. The filter was not dropped.');
  seen.add(key);
  if(key==='username'){if(new TextEncoder().encode(value).byteLength>4096||value.includes('\u0000'))fail('Invalid username search.');query.set(key,value);}
  else if(key==='sort'){if(!['name','username'].includes(value))fail('User search supports username ordering only.');sort=value;}
  else if(key==='order'||key==='order[username]'){if(!['asc','desc'].includes(value))fail('Invalid username order.');if(order!==undefined)fail('Ambiguous username order.');order=value;}
  else{if(!/^\d+$/.test(value)||!Number.isSafeInteger(Number(value)))fail('Invalid user search '+key+'.');if(key==='page'){if(Number(value)<1)fail('Invalid user search page.');page=value;}else if(key==='limit'){limit=Number(value);if(limit>100)fail('User search limit exceeds 100.');}else{offset=Number(value);if(offset>10000)fail('User search offset exceeds 10000.');}}
 }
 if(page!==undefined){if(seen.has('offset')||(limit===0&&Number(page)!==1))fail('Ambiguous user search page/offset.');offset=(Number(page)-1)*limit;if(!Number.isSafeInteger(offset)||offset>10000)fail('User search page exceeds the directory budget.');}
 if(sort!==undefined||order!==undefined){if(sort!==undefined&&seen.has('order[username]'))fail('Ambiguous username ordering.');query.set('order[username]',order??'asc');}
 query.set('limit',String(limit));query.set('offset',String(offset));
 if(new TextEncoder().encode(query.toString()).byteLength>32768)fail('User search exceeds the query budget.');
 return{query,limit,offset,page:limit?Math.floor(offset/limit)+1:1,ids};
}
export function createUserDirectorySearch(access=boundAccountAccess){return async(input=new URLSearchParams(),signal?:AbortSignal)=>{
 const plan=userDirectoryQuery(input),check=()=>{if(signal?.aborted)fail('The user search was cancelled.',499);};check();
 const account=await access(check);account.check();const response=await account.request('/user?'+plan.query.toString().replace(/^\?/,''));account.check();
 if(response.status!==200)fail('The authenticated user directory could not be loaded ('+response.status+').',response.status);
 const body=record(response.body),meta=record(body.nostr);
 if(body.result!=='ok'||body.response!=='collection'||meta.scope!=='instance-account'||meta.source!=='persisted-native-account-library'||meta.requires!=='request-bound-nostr-proof-and-explicit-native-account-binding'||publicUuid(meta.accountId)!==account.accountId)fail('The directory account changed. Search again with the current account.',409);
 if(body.limit!==plan.limit||body.offset!==plan.offset||!Number.isSafeInteger(body.total)||Number(body.total)<0||!Array.isArray(body.data)||body.data.length!==Math.min(plan.limit,Math.max(0,Number(body.total)-plan.offset)))fail('Invalid user directory page.',502);
 const seen=new Set<string>(),data=body.data.map(input=>{
  const row=resource(input,'user'),id=publicUuid(row.id),attrs=row.attributes;
  if(id!==row.id||seen.has(id)||plan.ids.size&&!plan.ids.has(id))fail('The user directory returned an unproven or repeated User target.',502);seen.add(id);
  if(Object.keys(attrs).some(key=>!['username','roles','version'].includes(key))||row.relationships.length||!Number.isSafeInteger(attrs.version)||Number(attrs.version)<1)fail('Invalid public directory projection.',502);
  // Copy the public allowlist before retaining resource provenance. Unexpected
  // private attributes or relationships must never reach a search consumer.
  return normalizePublicUser({id,type:'user',attributes:{username:attrs.username,roles:attrs.roles,version:attrs.version},relationships:[]});
 });
 return{status:200,body:{data,total:Number(body.total),limit:plan.limit,offset:plan.offset,page:plan.page},headers:response.headers};
};}
export const searchUserDirectory=createUserDirectorySearch();
