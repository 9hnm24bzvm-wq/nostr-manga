// Ordinary profile controls use the browser account, never an external signer.
import {openSessionAccountAccess,type SessionAccountDependencies} from './session-account-access';
import {NativeAccountError,NativeAccountRequestError} from './native-account-scope';
import {createAccountSettingsClient} from './session-settings-client';
import {loadSessionFollow} from './session-follow-client';
import {createUserDirectorySearch} from './session-user-directory-core';
import {publicUuid,PublicApiContractError} from './validation';
export interface NativeProfileActionsView {
 accountId:string;username:string;targetId:string;targetName:string;self:boolean;
 settingsHref?:string;blocked:boolean;followed:boolean;canBlock:boolean;needsRecheck:boolean;unconfirmed:boolean;canResolve?:boolean;
}
// Active same-document writes cannot be acknowledged by a second mounted view.
const activeBlockCommands=new Set<string>();
function fail(message:string,status=409):never{throw new PublicApiContractError(message,status);}
export async function loadSessionProfileActions(input:string,requested:string,nativeAccount:string,assertCurrent=()=>{},injected:Partial<SessionAccountDependencies>={}){
 const targetId=publicUuid(input);if(targetId!==input||targetId!==publicUuid(requested))fail('Open the current profile before choosing account actions.',422);
 const scope=await openSessionAccountAccess(nativeAccount,assertCurrent,injected);scope.check();
 const {username,permissions,accountId}=scope,self=targetId===accountId,access=async()=>scope;
 const key=scope.key('profile-block:'+targetId),pending=()=>scope.fences.get(key)!==null;
 let busy=false,state:NativeProfileActionsView={accountId,username,targetId,targetName:'',self,blocked:false,followed:false,canBlock:false,needsRecheck:true,unconfirmed:pending()};
 let reconciliationContext:{marker:string;commandId:string;version:number;updatedAt:string}|undefined;
 let settings:Awaited<ReturnType<ReturnType<typeof createAccountSettingsClient>>>|undefined;
 let following:Awaited<ReturnType<typeof loadSessionFollow>>|undefined;
 const view=()=>Object.freeze({...state,unconfirmed:pending()});
 async function reload(){scope.check();if(busy)fail('A profile action is already pending.');busy=true;reconciliationContext=undefined;state={...state,needsRecheck:true,settingsHref:undefined,canBlock:false,canResolve:false};try{
  if(self){if(!permissions.includes('self.user.update'))fail('This account cannot edit its public profile.',403);scope.check();state={...state,targetName:username,settingsHref:'/user/'+targetId+'/settings',needsRecheck:false};}
  else {
   if(!permissions.includes('self.preferences.update'))fail('This account cannot change uploader preferences.',403);
   const result=await createUserDirectorySearch(access)(new URLSearchParams({'ids[]':targetId,limit:'1'}));scope.check();
   if(result.body.total!==1||result.body.data.length!==1||result.body.data[0].id!==targetId)fail('This stored native profile is no longer available.',404);
   settings??=await createAccountSettingsClient(access)(assertCurrent);following??=await loadSessionFollow('user',targetId,nativeAccount,assertCurrent,injected);
   const preferences=await settings.reload(),follow=await following.reload();scope.check();scope.check();
   state={...state,targetName:result.body.data[0].name,blocked:(preferences.settings.uploaders as string[]|undefined??[]).includes(targetId),followed:follow.followed,canBlock:true,needsRecheck:false};
  }
  return view();
 }catch(error){state={...state,needsRecheck:true,settingsHref:undefined,canBlock:false};throw error;}finally{busy=false;}}
 async function setBlocked(blocked:boolean){scope.check();if(typeof blocked!=='boolean'||busy||self||!settings||!following||state.needsRecheck||!state.canBlock||pending())fail('Reload account actions before changing this uploader. An unconfirmed change is never replayed.');if(blocked===state.blocked)fail('The uploader already has this observed block state.',422);busy=true;let ownedMarker:string|undefined;
  try {
   // Same UI exclusion as retained native controls, not an atomic follow/block lock.
   const follow=await following.reload();scope.check();state={...state,followed:follow.followed};if(blocked&&follow.followed)fail('Unfollow this user explicitly before blocking them. No follow was changed.',422);
   const before=settings.view();if(!before||before.needsRecheck)fail('Reload the current account preferences before blocking.');
   const uploaders=[...(before.settings.uploaders as string[]|undefined??[])].filter(id=>id!==targetId);if(blocked)uploaders.push(targetId);
   if(uploaders.length>100)fail('The account uploader blocklist is full.',422);
   if(pending())fail('Another uploader change is pending. No mutation was replayed.');
   const marker=JSON.stringify({commandId:crypto.randomUUID(),accountId,targetId,blocked,version:before.settings.version,updatedAt:before.updatedAt});
   scope.fences.set(key,marker);if(scope.fences.get(key)!==marker)fail('The block recovery fence could not be retained.');ownedMarker=marker;activeBlockCommands.add(marker);
   const receipt=await settings.save({uploaders});scope.check();
   if((receipt.settings.uploaders as string[]|undefined??[]).includes(targetId)!==blocked)fail('The exact uploader change was not confirmed.',502);
   // settings.save verifies actor, revision, timestamp, template, full changed
   // uploaders field and merge provenance. No successful GET reaches this branch.
   if(scope.fences.get(key)!==marker)fail('The pending uploader command changed.');scope.fences.delete(key);
   state={...state,blocked,needsRecheck:false};return view();
  }catch(error){if(ownedMarker!==undefined&&scope.fences.get(key)===ownedMarker&&(error instanceof NativeAccountError&&(error as NativeAccountError&{confirmedResponse?:boolean}).confirmedResponse===true&&error.status>=400&&error.status<500||error instanceof NativeAccountRequestError&&error.outcome==='unsent'))scope.fences.delete(key);state={...state,needsRecheck:true};throw error;}finally{if(ownedMarker)activeBlockCommands.delete(ownedMarker);busy=false;}
 }
 async function reconcile(){
  scope.check();if(busy)fail('Wait for the current account request to finish.');const marker=scope.fences.get(key);if(marker===null)fail('There is no unconfirmed change to check.');if(activeBlockCommands.has(marker))fail('Your change is still being sent. Wait for it to finish before checking its status.');
  let command:{commandId?:unknown;accountId?:unknown;targetId?:unknown;blocked?:unknown;version?:unknown;updatedAt?:unknown};try{command=JSON.parse(marker);}catch{fail('This saved change cannot be verified safely. Contact the site team for help.');}
  if(!command||typeof command!=='object'||typeof command.commandId!=='string'||!/^[0-9a-f-]{36}$/.test(command.commandId)||command.accountId!==accountId||command.targetId!==targetId||typeof command.blocked!=='boolean'||!Number.isSafeInteger(command.version)||Number(command.version)<1||typeof command.updatedAt!=='string'||!Number.isFinite(Date.parse(command.updatedAt)))fail('This saved change does not match the current account and uploader.');
  await reload();scope.check();const observed=settings?.view();
  if(scope.fences.get(key)!==marker||activeBlockCommands.has(marker))fail('Another change is being checked. Nothing was cleared.');
  // A greater persisted revision makes the old CAS command impossible to commit
  // later. Equal revision is not proof that a timed-out write has finished.
  if(!observed||observed.needsRecheck||observed.accountId!==accountId||observed.settings.version<Number(command.version))fail('The current setting could not be verified. Nothing was cleared.');
  if(observed.settings.version===Number(command.version)){reconciliationContext={marker,commandId:command.commandId,version:Number(command.version),updatedAt:observed.updatedAt};state={...state,canResolve:true};fail('The earlier change is still unconfirmed. Keep the current setting to safely finish checking it.');}
  scope.fences.delete(key);return view();
 }
 async function resolve(){
  scope.check();const context=reconciliationContext;if(busy||!settings||!context||!state.canResolve||scope.fences.get(key)!==context.marker||activeBlockCommands.has(context.marker))fail('Check the current status before continuing.');
  busy=true;activeBlockCommands.add(context.marker);state={...state,canResolve:false};
  try{const response=await scope.request('/settings/reconcile-block','POST',{expectedAccountId:accountId,targetId,commandId:context.commandId,expectedVersion:context.version,expectedUpdatedAt:context.updatedAt});scope.check();
   const envelope=response.body as {nostr?:Record<string,unknown>},meta=envelope?.nostr;if(!meta||meta.authentication!=='native-session'||meta.accountId!==accountId||meta.barrierEstablished!==true||meta.commandId!==context.commandId||meta.targetId!==targetId||meta.expectedVersion!==context.version||!Number.isSafeInteger(meta.barrierVersion)||Number(meta.barrierVersion)<=context.version||meta.mutation!=='version-barrier-only'||meta.changed!==false)fail('The earlier request was not confirmed as safely settled.');
   if(scope.fences.get(key)!==context.marker)fail('Another change is being checked. Nothing was cleared.');
   const observed=settings.installBarrier(response.body);scope.check();if(scope.fences.get(key)!==context.marker)fail('Another change is being checked. Nothing was cleared.');scope.fences.delete(key);reconciliationContext=undefined;
   state={...state,blocked:(observed.settings.uploaders as string[]|undefined??[]).includes(targetId),canResolve:false,needsRecheck:false};return view();
  }catch(error){state={...state,needsRecheck:true,canResolve:false};reconciliationContext=undefined;throw error;}finally{activeBlockCommands.delete(context.marker);busy=false;}
 }
 return{reload,setBlocked,reconcile,resolve,view,check:scope.check,dispose:()=>{following?.dispose();scope.dispose();}};
}

