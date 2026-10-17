// Personal ratings belong to an explicitly bound instance account. A selected
// signing pubkey is request authentication, never a native Session/account ID.
import {capabilities,selectedSigner} from '../local-library/publication';
import {proveService} from '../local-library/protocol/nostr.js';
import {createNostrUploadRequest,type UploadRequestContext,type PublicUploadRequest,type PublicUploadResponse} from './nostr-upload-request';
import {publicUuid,record,PublicApiContractError} from './validation';
export interface PersonalRating{mangaId:string;accountId:string;username:string;rating:number|null;version:number;needsRecheck:boolean;}
type Identity=ReturnType<typeof selectedSigner>;
type Request=(input:PublicUploadRequest)=>Promise<PublicUploadResponse>;
export interface RatingDependencies{capabilities:typeof capabilities;selectedSigner:typeof selectedSigner;proveService:typeof proveService;publicOrigin():string;request(context:UploadRequestContext):Request;}
const defaults:RatingDependencies={capabilities,selectedSigner,proveService,publicOrigin:()=>window.location.origin,request:createNostrUploadRequest};
function fail(detail:string,status=502):never{throw new PublicApiContractError(detail,status);}
function integer(value:unknown,minimum=0):number{if(!Number.isSafeInteger(value)||Number(value)<minimum)fail('Invalid persisted rating version');return Number(value);}
function account(input:unknown){const body=record(input,'bound rating account'),row=record(body.data,'bound rating account'),a=record(row.attributes,'bound rating account'),nostr=record(body.nostr,'bound rating account');if(body.result!=='ok'||body.response!=='entity'||row.type!=='user'||typeof a.username!=='string'||!a.username||nostr.scope!=='instance-account'||nostr.identity!=='explicitly-bound-native-account')fail('The selected identity has no verified bound instance account');return{id:publicUuid(row.id),username:a.username};}
export function createPersonalRatingClient(injected:Partial<RatingDependencies>={}){
 const deps={...defaults,...injected};
 return async(mangaInput:string,assertCurrent:()=>void=()=>{})=>{
  const mangaId=publicUuid(mangaInput),identity:Identity=deps.selectedSigner(),publicOrigin=new URL(deps.publicOrigin()).origin;
  const check=()=>{assertCurrent();identity.check();if(new URL(deps.publicOrigin()).origin!==publicOrigin)fail('Rating destination changed',409);};check();
  const policy=await deps.capabilities();check();
  if(!policy.available||typeof policy.origin!=='string'||typeof policy.service_key!=='string'||!/^[0-9a-f]{64}$/.test(policy.service_key)||!policy.catalog_namespace)fail('This instance has no available signed-account service',503);
  const namespace=publicUuid(policy.catalog_namespace),service=policy.service_key,proofOrigin=policy.origin;
  const proof=await deps.proveService(proofOrigin,service);check();if(proof.origin!==proofOrigin||proof.serviceKey!==service)fail('Rating service proof changed');
  const scope={origin:publicOrigin,serviceKey:service,catalogNamespace:namespace};
  const request=deps.request({scope,identity,assertCurrent:check,beforeSend:async()=>{check();const current=await deps.capabilities();check();if(current.origin!==proofOrigin||current.service_key!==service||current.catalog_namespace!==namespace||!current.available)fail('Rating service or catalogue changed',409);}});
  async function send(path:string,method:'GET'|'POST'|'DELETE',body?:unknown){check();let response:PublicUploadResponse;try{response=await request({url:publicOrigin+'/api/v1'+path,method,...(body===undefined?{}:{body:new TextEncoder().encode(JSON.stringify(body)),contentType:'application/json'})});}catch(error){check();const details=error as {status?:number;outcome?:string};fail(details.outcome==='uncertain'?'Your rating outcome is unknown. Reload it before trying again.':'Your rating was not confirmed. Reload it before trying again.',details.status??502);}check();if(response.status<200||response.status>=300)fail(response.status===403?'This signing identity is not bound to an allowed instance account.':response.status===409?'Your rating changed. Reload it before saving again.':'The instance could not confirm your rating ('+response.status+').',response.status);return response.body;}
  const user=account(await send('/user/me','GET'));check();
  let state:PersonalRating={mangaId,accountId:user.id,username:user.username,rating:null,version:0,needsRecheck:true},busy=false;
  function view(){return Object.freeze({...state});}
  function provenance(input:unknown){const body=record(input,'personal rating'),nostr=record(body.nostr,'rating provenance');if(body.result!=='ok'||nostr.scope!=='instance-account'||nostr.source!=='persisted-native-account-ratings'||publicUuid(nostr.accountId)!==user.id)fail('The bound rating account changed',409);return{body,nostr};}
  async function reload(){check();if(busy)fail('A rating request is still pending',409);busy=true;try{const{body,nostr}=provenance(await send('/rating?manga%5B%5D='+encodeURIComponent(mangaId),'GET')),versions=record(nostr.versions,'rating versions'),ratings=record(body.ratings,'personal rating');if(nostr.zeroVersionMeans!=='no-persisted-rating-record'||Object.keys(versions).length!==1||!(mangaId in versions)||Object.keys(ratings).some(key=>key!==mangaId))fail('Rating response escaped the exact selected title');const version=integer(versions[mangaId]);let rating:number|null=null;if(mangaId in ratings){const row=record(ratings[mangaId],'personal rating'),meta=record(row.nostr,'rating observation');rating=integer(row.rating,1);if(rating>10||integer(meta.version,1)!==version||typeof row.createdAt!=='string'||!Number.isFinite(Date.parse(row.createdAt))||typeof meta.updatedAt!=='string'||!Number.isFinite(Date.parse(meta.updatedAt)))fail('Invalid actual personal rating observation');}check();state={...state,rating,version,needsRecheck:false};return view();}catch(error){state={...state,needsRecheck:true};throw error;}finally{busy=false;}}
  async function mutate(rating:number|null){check();if(busy||state.needsRecheck)fail('Reload the current rating before saving',409);if(rating!==null&&(!Number.isSafeInteger(rating)||rating<1||rating>10))fail('Choose a rating from 1 to 10',422);busy=true;const before=state;try{const payload={...(rating===null?{}:{rating}),version:before.version,expectedAccountId:user.id},method=rating===null?'DELETE':'POST';const{nostr}=provenance(await send('/rating/'+mangaId,method,payload));if(publicUuid(nostr.mangaId)!==mangaId||typeof nostr.changed!=='boolean'||nostr.changed!==(before.rating!==rating))fail('Rating receipt identifies a different title or inconsistent state change');const version=integer(nostr.version);if(version!==before.version+(nostr.changed?1:0))fail('Rating receipt changed its revision unexpectedly');check();state={...state,rating,version,needsRecheck:false};return view();}catch(error){state={...state,needsRecheck:true};throw error;}finally{busy=false;}}
  return{reload,save:(rating:number)=>mutate(rating),remove:()=>mutate(null),view,check};
 };
}
export const loadPersonalRating=createPersonalRatingClient();
