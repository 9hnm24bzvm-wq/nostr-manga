// A title follow means chapter updates, not permission to remove a library entry.
// The existing account-library CAS protects the reviewed status against racing
// status/list/progress edits. Its uncertain-write fences are shared with saves.
import {createNativeAccountLibrary,type AccountFollowState} from '../local-library/native-account-library-client';
import type {NativeAccountDependencies} from './native-account-scope';
import {publicUuid,PublicApiContractError} from './validation';
import type {AccountFollowView} from './session-follow-client';
export async function loadSessionTitleUpdates(input:string,nativeAccount:string,assertCurrent=()=>{},injected:Partial<NativeAccountDependencies>={}){
 const targetId=publicUuid(input),library=await createNativeAccountLibrary(nativeAccount,assertCurrent,injected);
 let saved:AccountFollowState|undefined,revision:number|undefined,busy=false;
 let state:AccountFollowView={kind:'manga',targetId,accountId:library.account.id,username:library.account.name,followed:false,needsRecheck:true};
 const view=()=>Object.freeze({...state});
 const refuse=()=>{throw new PublicApiContractError('Check this title’s updates before changing them.',409);};
 async function reload(){library.check();if(busy)refuse();busy=true;try{const result=await library.followStates([targetId]);library.check();saved=result.data[0];revision=result.revision;state={...state,followed:saved.saved&&saved.notification===true,version:revision,needsRecheck:false};return view();}catch(cause){state={...state,needsRecheck:true};throw cause;}finally{busy=false;}}
 async function mutate(followed:boolean){library.check();if(busy||state.needsRecheck||!saved||revision===undefined)refuse();library.requireConfirmed('follow-'+targetId);busy=true;try{
   // No new record is needed to switch off updates on an unsaved title.
   if(!followed&&!saved!.saved)return view();
   const result=await library.setFollow(targetId,revision!,saved!.status??'plan_to_read',followed);library.check();saved=result.data;revision=result.revision;state={...state,followed:saved.notification===true,version:revision,needsRecheck:false};return view();
  }catch(cause){state={...state,needsRecheck:true};throw cause;}finally{busy=false;}}
 return{reload,follow:()=>mutate(true),unfollow:()=>mutate(false),view,check:library.check,dispose:library.dispose};
}
