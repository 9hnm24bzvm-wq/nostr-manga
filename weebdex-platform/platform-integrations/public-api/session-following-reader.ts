// Private native-account collections only. Resource revisions are presentation
// data, never membership CAS; management must load a fresh per-target status.
import {createNativeAccountScope,nativeAccountError,type NativeAccountDependencies} from './native-account-scope';
import type{AccountFollowKind} from './account-follow-client';
import {normalizeManga,normalizeNamed} from './normalize';
import {normalizePublicUser} from './user-profile';
import type{MangaView} from './types';
import {publicUuid,record,resource,PublicApiContractError} from './validation';
export interface AccountFollowingRow{id:string;name:string;manga?:MangaView;}
function fail(detail:string,status=502):never{throw new PublicApiContractError(detail,status);}
export async function loadSessionFollowing(kind:AccountFollowKind,nativeAccount:string,assertCurrent=()=>{},injected:Partial<NativeAccountDependencies>={}){
 if(!['manga','group','user'].includes(kind))fail('Unsupported following view',422);const scope=await createNativeAccountScope(nativeAccount,assertCurrent,injected,{requireVerified:false}),account={accountId:scope.account.id,username:scope.account.name,check:scope.check,request:scope.send};account.check();
 return Object.freeze({kind,dispose:scope.dispose,accountId:account.accountId,username:account.username,async page(offset=0){
  account.check();if(!Number.isSafeInteger(offset)||offset<0||offset>10000)fail('Invalid following page',422);
  const query=new URLSearchParams({limit:'20',offset:String(offset)});if(kind==='manga')for(const include of['cover_art','author','artist'])query.append('includes[]',include);
  const response=await account.request('/user/follows/'+kind+'?'+query.toString().replace(/^\?/,''));account.check();if(response.status!==200)throw nativeAccountError(response.status,response.body);
  const body=record(response.body),meta=record(body.nostr);if(body.result!=='ok'||body.response!=='collection'||meta.scope!=='instance-account'||meta.authentication!=='native-session'||meta.source!=='persisted-native-account-library'||meta.requires!=='native-session-account'||publicUuid(meta.accountId)!==account.accountId)fail('The following account changed. Refresh this view.',409);
  if(body.limit!==20||body.offset!==offset||!Number.isSafeInteger(body.total)||Number(body.total)<0||!Array.isArray(body.data)||body.data.length!==Math.min(20,Math.max(0,Number(body.total)-offset)))fail('Invalid account-following page');
  const seen=new Set<string>(),type=kind==='group'?'scanlation_group':kind,data:AccountFollowingRow[]=body.data.map(input=>{const row=resource(input,type),id=publicUuid(row.id);if(!Number.isSafeInteger(row.attributes.version)||Number(row.attributes.version)<1)fail('Invalid followed-resource revision');if(row.id!==id||seen.has(id))fail('The following view returned a repeated or noncanonical resource');seen.add(id);if(kind==='manga'){const manga=normalizeManga(row);return{id,name:manga.title,manga};}const normalized=kind==='user'?normalizePublicUser(row):normalizeNamed(row);return{id,name:normalized.name};});
  return{data,total:Number(body.total),offset,limit:20};
 }});
}
