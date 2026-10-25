// Shared, exact signer/service/account scope for reports and private draft reads.
// A cookie session, cached profile or public UUID never grants authority.
import { capabilities, selectedSigner } from '../local-library/publication';
import { proveService, origin } from '../local-library/protocol/nostr.js';
import { createNostrUploadRequest, PublicUploadRequestError } from './nostr-upload-request';
import type { TitleAccount, TitleDependencies } from './title-client';
import type { PublicUploadResponse } from './nostr-upload-request';
import { publicUuid, record, PublicApiContractError } from './validation';
const defaults:TitleDependencies={capabilities,selectedSigner,proveService,publicOrigin:()=>window.location.origin,createRequest:createNostrUploadRequest,fences:{get:key=>sessionStorage.getItem(key),set:(key,value)=>sessionStorage.setItem(key,value),delete:key=>sessionStorage.removeItem(key)}};
function fail(message:string,status=502):never{throw new PublicApiContractError(message,status);}
export async function createSignedAccountScope(assertCurrent:()=>void,injected:Partial<TitleDependencies>={},signal?:AbortSignal) {
 const abort=new AbortController();
 const cancel=()=>abort.abort();
 signal?.addEventListener('abort',cancel,{once:true});
 if(signal?.aborted)cancel();
 let disposed=false;
 const dispose=()=>{disposed=true;cancel();signal?.removeEventListener('abort',cancel);};
 try {
  const deps={...defaults,...injected},identity=deps.selectedSigner();
  function current(){if(disposed||abort.signal.aborted)fail('This account view is closed or cancelled',409);identity.check();assertCurrent();}
  current();
  function scope(policy:Awaited<ReturnType<typeof capabilities>>){if(!policy.available||!policy.schema_ready||!policy.origin||!policy.service_key||!/^[0-9a-f]{64}$/.test(policy.service_key)||!policy.catalog_namespace)fail('The configured signed account service is unavailable',503);return{origin:origin(deps.publicOrigin()),proofOrigin:origin(policy.origin),serviceKey:policy.service_key,catalogNamespace:publicUuid(policy.catalog_namespace)};}
  const captured=scope(await deps.capabilities());current();
  const proof=await deps.proveService(captured.proofOrigin,captured.serviceKey);current();if(proof.origin!==captured.proofOrigin||proof.serviceKey!==captured.serviceKey)fail('Signed account service proof changed');
  function check(){current();if(origin(deps.publicOrigin())!==captured.origin)fail('Account destination changed',409);}
  async function refresh(){check();if(JSON.stringify(scope(await deps.capabilities()))!==JSON.stringify(captured))fail('The account service or catalogue changed',409);check();}
  const request=deps.createRequest({scope:captured,identity,assertCurrent:check,beforeSend:async requestSignal=>{if(requestSignal.aborted)fail('Account request cancelled',409);await refresh();}});
  function facts(response:PublicUploadResponse):TitleAccount {
   if(response.status!==200)fail(`Signed account authorization refused (HTTP ${response.status})`,response.status);
   const body=record(response.body),user=record(body.user),nostr=record(body.nostr);
   if(body.result!=='ok'||nostr.pubkey!==identity.owner||nostr.serviceKey!==captured.serviceKey||publicUuid(nostr.catalogNamespace)!==captured.catalogNamespace||nostr.authentication!=='NIP-98'||nostr.requestScoped!==true||nostr.nativeSessionAuthority!==false||nostr.catalogueScope!=='instance-local')fail('The bound account does not match this exact signer/service/catalogue');
   if(typeof user.username!=='string'||!user.username||user.username.length>500||!Number.isSafeInteger(user.version)||Number(user.version)<1)fail('Invalid current bound account');
   for(const key of ['permissions','roles'])if(!Array.isArray(user[key])||(user[key] as unknown[]).some(value=>typeof value!=='string'))fail('Invalid bound account grants');
   return Object.freeze({id:publicUuid(user.id),name:user.username,permissions:Object.freeze([...(user.permissions as string[])]) as unknown as string[],roles:Object.freeze([...(user.roles as string[])]) as unknown as string[]});
  }
  const accountRequest=(signal=abort.signal)=>request({url:captured.origin+'/api/v1/nostr/account-context',method:'GET',signal});
  const account=facts(await accountRequest());check();
  async function verify(signal=abort.signal){await refresh();const actual=facts(await accountRequest(signal));check();if(actual.id!==account.id)throw new PublicUploadRequestError('The signer is now bound to another account. Reopen this view.','unsent',409);}
  // Additive immutable observation for action-specific authority captures.
  // Keep account and verify semantics unchanged for existing consumers.
  async function observeAccount(signal=abort.signal){
   await refresh();const response=await accountRequest(signal);check();const actual=facts(response);
   const version=Number(record(record(response.body).user).version);
   if(actual.id!==account.id)throw new PublicUploadRequestError('The signer is now bound to another account. Reopen this view.','unsent',409);
   await refresh();check();return Object.freeze({...actual,version});
  }
  const command=deps.createRequest({scope:captured,identity,assertCurrent:check,beforeSend:async requestSignal=>{if(requestSignal.aborted)fail('Account request cancelled',409);await verify(requestSignal);}});
  return {account,check,verify,observeAccount,dispose,fences:deps.fences,key:(operation:string)=>`weebdex.signed-${operation}.v1:${captured.origin}:${captured.serviceKey}:${captured.catalogNamespace}:${identity.owner}:${account.id}`,send:async(path:string,method:'GET'|'POST'='GET',body?:unknown)=>{
   check();if(!path.startsWith('/')||path.startsWith('//')||/[#\\\u0000-\u0020]/.test(path))fail('Invalid signed request target',422);
   const response=await command({url:captured.origin+'/api/v1'+path,method,signal:abort.signal,...(body===undefined?{}:{body:new TextEncoder().encode(JSON.stringify(body)),contentType:'application/json'})});check();return response;
  }};
 } catch(error){dispose();throw error;}
}
