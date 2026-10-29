// Same persisted rating and revision contract as advanced signing clients, but
// authenticated only by the normal browser account (never a selected key).
import { createNativeAccountScope, nativeAccountError, type NativeAccountDependencies } from './native-account-scope';
import { publicUuid, record, PublicApiContractError } from './validation';
import type { PersonalRating } from './rating-client';
function fail(message: string, status=502): never { throw new PublicApiContractError(message, status); }
function integer(value: unknown, minimum=0): number { if (!Number.isSafeInteger(value) || Number(value)<minimum) fail('Invalid persisted rating version'); return Number(value); }
export async function loadSessionRating(mangaInput: string, nativeAccount: string, assertCurrent: () => void, injected: Partial<NativeAccountDependencies> = {}) {
  const mangaId=publicUuid(mangaInput), scope=await createNativeAccountScope(nativeAccount,assertCurrent,injected), user=scope.account, check=scope.check;
  async function send(path: string, method: 'GET'|'POST'|'DELETE', body?: unknown) {
    const response=await scope.send(path,method,body); check();
    if (response.status!==200) throw nativeAccountError(response.status,response.body);
    return response.body;
  }
  let state:PersonalRating={mangaId,accountId:user.id,username:user.name,rating:null,version:0,needsRecheck:true},busy=false;
  function view(){return Object.freeze({...state});}
  function provenance(input:unknown){const body=record(input,'personal rating'),nostr=record(body.nostr,'rating provenance');if(body.result!=='ok'||nostr.scope!=='instance-account'||nostr.authentication!=='native-session'||nostr.source!=='persisted-native-account-ratings'||publicUuid(nostr.accountId)!==user.id)fail('The bound rating account changed',409);return{body,nostr};}
  async function reload(){check();if(busy)fail('A rating request is still pending',409);busy=true;try{const{body,nostr}=provenance(await send('/rating?manga%5B%5D='+encodeURIComponent(mangaId),'GET')),versions=record(nostr.versions,'rating versions'),ratings=record(body.ratings,'personal rating');if(nostr.zeroVersionMeans!=='no-persisted-rating-record'||Object.keys(versions).length!==1||!(mangaId in versions)||Object.keys(ratings).some(key=>key!==mangaId))fail('Rating response escaped the exact selected title');const version=integer(versions[mangaId]);let rating:number|null=null;if(mangaId in ratings){const row=record(ratings[mangaId],'personal rating'),meta=record(row.nostr,'rating observation');rating=integer(row.rating,1);if(rating>10||integer(meta.version,1)!==version||typeof row.createdAt!=='string'||!Number.isFinite(Date.parse(row.createdAt))||typeof meta.updatedAt!=='string'||!Number.isFinite(Date.parse(meta.updatedAt)))fail('Invalid actual personal rating observation');}check();state={...state,rating,version,needsRecheck:false};return view();}catch(error){state={...state,needsRecheck:true};throw error;}finally{busy=false;}}
  async function mutate(rating:number|null){check();if(busy||state.needsRecheck)fail('Reload the current rating before saving',409);if(rating!==null&&(!Number.isSafeInteger(rating)||rating<1||rating>10))fail('Choose a rating from 1 to 10',422);busy=true;const before=state;try{const payload={...(rating===null?{}:{rating}),version:before.version,expectedAccountId:user.id},method=rating===null?'DELETE':'POST';const{nostr}=provenance(await send('/rating/'+mangaId,method,payload));if(publicUuid(nostr.mangaId)!==mangaId||typeof nostr.changed!=='boolean'||nostr.changed!==(before.rating!==rating))fail('Rating receipt identifies a different title or inconsistent state change');const version=integer(nostr.version);if(version!==before.version+(nostr.changed?1:0))fail('Rating receipt changed its revision unexpectedly');check();state={...state,rating,version,needsRecheck:false};return view();}catch(error){state={...state,needsRecheck:true};throw error;}finally{busy=false;}}
  return{reload,save:(rating:number)=>mutate(rating),remove:()=>mutate(null),view,check,dispose:scope.dispose};
}
