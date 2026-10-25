// Direct retained-profile actions are explicitly signed, never cookie authority.
// Reads cannot erase a possibly committed Block/Unblock command's recovery fence.
import {createSignedAccountScope} from './signed-account-scope';
import {createAccountSettingsClient} from './settings-client';
import {createAccountFollowClient} from './account-follow-client';
import {createUserDirectorySearch} from './user-directory';
import {publicUuid,PublicApiContractError} from './validation';
export interface NativeProfileActionsView {
 accountId:string;username:string;targetId:string;targetName:string;self:boolean;
 settingsHref?:string;blocked:boolean;followed:boolean;canBlock:boolean;needsRecheck:boolean;unconfirmed:boolean;
}
function fail(message:string,status=409):never{throw new PublicApiContractError(message,status);}
export function createNativeProfileActionsClient(openScope=createSignedAccountScope){return async(input:string,requested:string,assertCurrent=()=>{})=>{
 const targetId=publicUuid(input);if(targetId!==input||targetId!==publicUuid(requested))fail('Open the canonical native profile before choosing account actions.',422);
 const scope=await openScope(assertCurrent);scope.check();
 const {name:username,permissions}=scope.account,accountId=publicUuid(scope.account.id),self=targetId===accountId;
 const access=async()=>({accountId,username,check:scope.check,request:async(path:string,method:'GET'|'POST'|'PUT'|'DELETE'='GET',body?:unknown)=>{
  if(method!=='GET'&&method!=='POST')fail('Unsupported profile action.',422);
  const response=await scope.send(path,method,body);
  if(path.startsWith('/user?')){const cache=new Set((response.headers?.['cache-control']??'').toLowerCase().split(',').map(value=>value.trim())),vary=new Set((response.headers?.vary??'').toLowerCase().split(',').map(value=>value.trim()));if(!cache.has('private')||!cache.has('no-store')||!vary.has('authorization'))fail('The private native account directory was not verified.',502);}
  return response;
 }});
 const key=scope.key('profile-block:'+targetId),pending=()=>scope.fences.get(key)!==null;
 let busy=false,state:NativeProfileActionsView={accountId,username,targetId,targetName:'',self,blocked:false,followed:false,canBlock:false,needsRecheck:true,unconfirmed:pending()};
 let settings:Awaited<ReturnType<ReturnType<typeof createAccountSettingsClient>>>|undefined;
 let following:Awaited<ReturnType<ReturnType<typeof createAccountFollowClient>>>|undefined;
 const view=()=>Object.freeze({...state,unconfirmed:pending()});
 async function reload(){scope.check();if(busy)fail('A profile action is already pending.');busy=true;state={...state,needsRecheck:true,settingsHref:undefined,canBlock:false};try{
  if(self){if(!permissions.includes('self.user.update'))fail('This account cannot edit its public profile.',403);await scope.verify();state={...state,targetName:username,settingsHref:'/user/'+targetId+'/settings?account='+accountId,needsRecheck:false};}
  else {
   if(!permissions.includes('self.preferences.update'))fail('This account cannot change uploader preferences.',403);
   const result=await createUserDirectorySearch(access)(new URLSearchParams({'ids[]':targetId,limit:'1'}));scope.check();
   if(result.body.total!==1||result.body.data.length!==1||result.body.data[0].id!==targetId)fail('This stored native profile is no longer available.',404);
   settings??=await createAccountSettingsClient(access)(assertCurrent);following??=await createAccountFollowClient(access)('user',targetId,assertCurrent);
   const preferences=await settings.reload(),follow=await following.reload();await scope.verify();scope.check();
   state={...state,targetName:result.body.data[0].name,blocked:(preferences.settings.uploaders as string[]|undefined??[]).includes(targetId),followed:follow.followed,canBlock:true,needsRecheck:false};
  }
  return view();
 }catch(error){state={...state,needsRecheck:true,settingsHref:undefined,canBlock:false};throw error;}finally{busy=false;}}
 async function setBlocked(blocked:boolean){scope.check();if(typeof blocked!=='boolean'||busy||self||!settings||!following||state.needsRecheck||!state.canBlock||pending())fail('Reload account actions before changing this uploader. An unconfirmed change is never replayed.');if(blocked===state.blocked)fail('The uploader already has this observed block state.',422);busy=true;
  try {
   // Same UI exclusion as retained native controls, not an atomic follow/block lock.
   const follow=await following.reload();scope.check();state={...state,followed:follow.followed};if(blocked&&follow.followed)fail('Unfollow this user explicitly before blocking them. No follow was changed.',422);
   const before=settings.view();if(!before||before.needsRecheck)fail('Reload the current account preferences before blocking.');
   const uploaders=[...(before.settings.uploaders as string[]|undefined??[])].filter(id=>id!==targetId);if(blocked)uploaders.push(targetId);
   if(uploaders.length>100)fail('The account uploader blocklist is full.',422);
   if(pending())fail('Another uploader change is pending. No mutation was replayed.');
   const marker=JSON.stringify({accountId,targetId,blocked,version:before.settings.version,updatedAt:before.updatedAt});
   scope.fences.set(key,marker);if(scope.fences.get(key)!==marker)fail('The block recovery fence could not be retained.');
   const receipt=await settings.save({uploaders});scope.check();
   if((receipt.settings.uploaders as string[]|undefined??[]).includes(targetId)!==blocked)fail('The exact uploader change was not confirmed.',502);
   // settings.save verifies actor, revision, timestamp, template, full changed
   // uploaders field and merge provenance. No successful GET reaches this branch.
   if(scope.fences.get(key)!==marker)fail('The pending uploader command changed.');scope.fences.delete(key);
   state={...state,blocked,needsRecheck:false};return view();
  }catch(error){state={...state,needsRecheck:true};throw error;}finally{busy=false;}
 }
 return{reload,setBlocked,view,check:scope.check,dispose:scope.dispose};
};}
export const loadNativeProfileActions=createNativeProfileActionsClient();
