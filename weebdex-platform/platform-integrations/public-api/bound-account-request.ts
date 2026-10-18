// Shared request-scoped account access, never a native cookie Session. Consumers
// bind their own exact operation, target and optimistic revision in signed JSON.
import {capabilities,selectedSigner} from '../local-library/publication';
import {proveService} from '../local-library/protocol/nostr.js';
import {createNostrUploadRequest,type UploadRequestContext,type PublicUploadRequest,type PublicUploadResponse} from './nostr-upload-request';
import {publicUuid,record,PublicApiContractError} from './validation';
export interface BoundAccountDependencies{capabilities:typeof capabilities;selectedSigner:typeof selectedSigner;proveService:typeof proveService;publicOrigin():string;request(context:UploadRequestContext):(input:PublicUploadRequest)=>Promise<PublicUploadResponse>;}
const defaults:BoundAccountDependencies={capabilities,selectedSigner,proveService,publicOrigin:()=>window.location.origin,request:createNostrUploadRequest};
function fail(detail:string,status=502):never{throw new PublicApiContractError(detail,status);}
export function createBoundAccountAccess(injected:Partial<BoundAccountDependencies>={}){
 const deps={...defaults,...injected};
 return async(assertCurrent=()=>{})=>{
  const identity=deps.selectedSigner(),publicOrigin=new URL(deps.publicOrigin()).origin,check=()=>{assertCurrent();identity.check();if(new URL(deps.publicOrigin()).origin!==publicOrigin)fail('Account request destination changed',409);};check();
  const policy=await deps.capabilities();check();if(!policy.available||typeof policy.origin!=='string'||typeof policy.service_key!=='string'||!/^[0-9a-f]{64}$/.test(policy.service_key)||!policy.catalog_namespace)fail('This instance has no available signed-account service',503);
  const namespace=publicUuid(policy.catalog_namespace),service=policy.service_key,sourceOrigin=policy.origin,proof=await deps.proveService(sourceOrigin,service);check();if(proof.origin!==sourceOrigin||proof.serviceKey!==service)fail('The account service proof changed');
  const transport=deps.request({scope:{origin:publicOrigin,serviceKey:service,catalogNamespace:namespace},identity,assertCurrent:check,beforeSend:async()=>{check();const current=await deps.capabilities();check();if(!current.available||current.origin!==sourceOrigin||current.service_key!==service||current.catalog_namespace!==namespace)fail('The account service or catalogue changed',409);}});
  async function request(path:string,method:'GET'|'POST'|'DELETE'='GET',body?:unknown){check();if(!path.startsWith('/')||path.startsWith('//')||/[#\\\u0000-\u0020]/.test(path))fail('Invalid account request target',422);try{const response=await transport({url:publicOrigin+'/api/v1'+path,method,...(body===undefined?{}:{body:new TextEncoder().encode(JSON.stringify(body)),contentType:'application/json'})});check();return response;}catch(error){check();const details=error as{status?:number;outcome?:string};fail(details.outcome==='uncertain'?'The outcome is unknown. Reload the current account state before trying again.':'The account request was not confirmed. Reload before trying again.',details.status??502);}}
  const response=await request('/user/me');if(response.status!==200)fail('This signing identity is not bound to an allowed account on this instance.',response.status);const body=record(response.body),user=record(body.data),attrs=record(user.attributes),nostr=record(body.nostr);if(body.result!=='ok'||body.response!=='entity'||user.type!=='user'||typeof attrs.username!=='string'||!attrs.username||nostr.scope!=='instance-account'||nostr.identity!=='explicitly-bound-native-account')fail('Invalid bound account response');
  return{accountId:publicUuid(user.id),username:attrs.username,request,check};
 };
}
export const boundAccountAccess=createBoundAccountAccess();
