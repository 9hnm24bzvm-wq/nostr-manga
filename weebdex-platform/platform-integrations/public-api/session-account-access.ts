// Private account state and explicitly public group-name lookup stay separate.
import {createNativeAccountScope,nativeAccountError,type NativeAccountDependencies} from './native-account-scope';
import {PublicApiClient} from './client';
export interface SessionAccountDependencies extends NativeAccountDependencies { groupLookup(query:URLSearchParams):Promise<{status:number;body?:unknown;headers?:Record<string,string>}>; }
export async function openSessionAccountAccess(nativeAccount:string,assertCurrent=()=>{},injected:Partial<SessionAccountDependencies>={}){
 const scope=await createNativeAccountScope(nativeAccount,assertCurrent,injected,{requireVerified:false});
 return{accountId:scope.account.id,username:scope.account.name,permissions:scope.account.permissions,check:scope.check,dispose:scope.dispose,fences:scope.fences,key:scope.key,async request(path:string,method:'GET'|'POST'|'PUT'|'DELETE'='GET',body?:unknown){
  scope.check();
  // Public group labels are not private state and carry no account authority.
  // Only this exact GET lookup is allowed outside the account namespace.
  if(path.startsWith('/group?')){if(method!=='GET'||body!==undefined)throw Error('Group names are read-only');const query=new URLSearchParams(path.slice(7));const response=await (injected.groupLookup??(query=>new PublicApiClient().collection('/group','scanlation_group',query)))(query);scope.check();if(response.status!==200)throw Object.assign(nativeAccountError(response.status,response.body),{confirmedResponse:true});return response;}
  const response=await scope.send(path,method,body);scope.check();if(response.status!==200)throw Object.assign(nativeAccountError(response.status,response.body),{confirmedResponse:true});return response;
 }};
}
export type SessionAccountAccess=(assertCurrent?:()=>void)=>Promise<Awaited<ReturnType<typeof openSessionAccountAccess>>>;
