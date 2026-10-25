// Instance lifecycle only. Never deletes source publications, signer keys or Library.
export const acknowledgement='DISABLE THIS ACCOUNT';
const maximumAccountVersion=2147483647,maximumDisableVersion=maximumAccountVersion-2;
const maximumResponseBytes=512*1024,maximumBlockers=600,maximumBlockersPerKind=100;
const utf8=new TextEncoder();
export function nativeClosureBase(value:string,current:string){const origin=new URL(current).origin,base=new URL(value,current);if(base.origin!==origin||base.pathname.replace(/\/$/,'')!=='/api'||base.search||base.hash||base.username||base.password)throw Error('Account disabling requires the native API on this exact instance origin');return origin+'/api';}
export interface ClosureAccess {accountId:string;origin:string;check():void;request(action:'read'|'prepare'|'confirm',body?:unknown):Promise<{status:number;body:unknown}>;}
export interface ClosureStorage {getItem(key:string):string|null;setItem(key:string,value:string):void;removeItem(key:string):void;}
export type Exclusive=<T>(key:string,work:()=>Promise<T>)=>Promise<T>;
export interface ClosureBlocker {kind:string;id:string;action:string;href:string;}
export interface ClosureView {lifecycleId:string;accountId:string;userId:string;username:string;version:number;blockers:ClosureBlocker[];}
function object(value:unknown):Record<string,any>{if(!value||typeof value!=='object'||Array.isArray(value))throw Error('Invalid account-disable response');return value as Record<string,any>;}
// A read-only snapshot may expose an exhausted INTEGER revision. It is not a capture.
function invariant(row:Record<string,any>,account:string){if(row.scope!=='instance-account-disable'||row.accountId!==account||row.userId!==account||!Number.isSafeInteger(row.version)||row.version<1||row.version>maximumAccountVersion)throw Error('The selected account or revision changed');}
function disableVersion(version:number){if(!Number.isSafeInteger(version)||version<1||version>maximumDisableVersion)throw Error('Account revision has no room for reversible disable and administrator reactivation. Contact the instance administrator.');}
function captured(row:Record<string,any>,account:string,version:number){invariant(row,account);disableVersion(row.version);if(row.version!==version)throw Error('The selected account or revision changed');}
function receipt(row:Record<string,any>,account:string,version:number){disableVersion(version);invariant(row,account);if(row.version!==version+1||row.version>maximumAccountVersion-1)throw Error('The disable receipt revision is not confirmed');}
// Post-parse normalized JSON budget, including unknown envelope fields. This is
// not a bound on raw XHR response allocation; the native transport has no raw cap.
function boundedObject(value:unknown):Record<string,any>{const row=object(value),serialized=JSON.stringify(row);if(typeof serialized!=='string'||serialized.length>maximumResponseBytes||utf8.encode(serialized).byteLength>maximumResponseBytes)throw Error('Account-disable response exceeds its byte budget');if('blockers' in row)blockers(row.blockers);return row;}
function policy(row:Record<string,any>){if(row.effect!=='reversible-disable'||row.automaticPurge!==false||row.reactivation!=='instance-administrator')throw Error('The account-disable policy changed');}
function blockers(value:unknown):ClosureBlocker[]{
 if(!Array.isArray(value)||value.length>maximumBlockers)throw Error('Invalid disable blockers');
 const counts=new Map<string,number>();
 return value.map(raw=>{
  const r=object(raw);if(['kind','id','action','href'].some(k=>typeof r[k]!=='string'||!r[k]))throw Error('Invalid disable blocker');
  let characters:number,href:string,ascii=false;
  switch(r.kind){
   case 'sole-group-leader':characters=128;href='/following/group';break;
   case 'publication-unresolved':characters=200;href='/upload';break;
   case 'source-request-unresolved':characters=10;href='/following/manga';ascii=true;break;
   case 'upload-active':case 'scheduled-work':characters=10;href='/upload';ascii=true;break;
   case 'draft-in-review':characters=128;href='/drafts/title';break;
   default:throw Error('Invalid disable blocker');
  }
  // Match PostgreSQL character limits, including astral Unicode, not UTF-16 length.
  if(r.id.length>characters*2||r.kind.length>25||r.action.length>116||utf8.encode(r.action).byteLength>116||r.href!==href||r.href.length>16)throw Error('Invalid disable blocker');
  let count=0;for(const character of r.id){const point=character.codePointAt(0)!;if(++count>characters||point===0||point>=0xd800&&point<=0xdfff)throw Error('Invalid disable blocker');}
  if(ascii&&!/^[A-Za-z0-9]{10}$/.test(r.id))throw Error('Invalid disable blocker');
  const total=(counts.get(r.kind)??0)+1;if(total>maximumBlockersPerKind)throw Error('Invalid disable blockers');counts.set(r.kind,total);
  return{kind:r.kind,id:r.id,action:r.action,href:r.href};
 });
}
export function createAccountClosure(access:ClosureAccess,storage:ClosureStorage,exclusive:Exclusive){
 const account=access.accountId,origin=new URL(access.origin).origin;let key:string|undefined;
 let view:ClosureView|undefined,challenge:Record<string,any>|undefined,pending=false;
 const check=()=>{access.check();if(access.accountId!==account||access.origin!==origin)throw Error('The selected account changed');};
 const safe=()=>{check();if(key&&storage.getItem(key)!==null)throw Error('A previous disable confirmation is unresolved or completed. Do not resend it. Contact the instance administrator with this account ID.');};
 async function run<T>(work:()=>Promise<T>):Promise<T>{if(pending)throw Error('An account operation is pending');pending=true;try{safe();return await exclusive(key??'weebdex.account-disable.preview:'+origin+':'+account,async()=>{safe();return work();});}finally{pending=false;}}
 async function response(action:'read'|'prepare'|'confirm',body?:unknown){check();const r=await access.request(action,body);check();if(r.status!==200)throw Error('Account operation refused ('+r.status+'). Reload blockers or sign in again.');return boundedObject(r.body);}
 return {
  async reload(){return run(async()=>{view=undefined;challenge=undefined;const r=await response('read');invariant(r,account);policy(r);if(r.result!=='ok'||typeof r.lifecycleId!=='string'||!/^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$/.test(r.lifecycleId)||typeof r.username!=='string'||!r.username||r.username.length>128)throw Error('Invalid account preview');key='weebdex.account-disable.v1:'+origin+':'+r.lifecycleId;safe();view={lifecycleId:r.lifecycleId,accountId:account,userId:account,username:r.username,version:r.version,blockers:blockers(r.blockers)};return structuredClone(view);});},
  async prepare(){return run(async()=>{if(!view||view.blockers.length)throw Error('Load and resolve this account’s blockers first');challenge=undefined;disableVersion(view.version);const r=await response('prepare',{expectedAccountId:account,expectedUserId:account,expectedLifecycleId:view.lifecycleId,version:view.version});captured(r,account,view.version);if(r.result==='blocked'){view.blockers=blockers(r.blockers);return{prepared:false,view:structuredClone(view)};}policy(r);if(r.lifecycleId!==view.lifecycleId||r.result!=='prepared'||r.acknowledge!==acknowledgement||r.expiresInSeconds!==600||typeof r.confirmationId!=='string'||!/^[a-f0-9-]{36}$/.test(r.confirmationId)||typeof r.confirmationToken!=='string'||!/^[A-Za-z0-9_-]{43}$/.test(r.confirmationToken))throw Error('Invalid one-use account confirmation');challenge=Object.freeze({...r});return{prepared:true,view:structuredClone(view)};});},
  async confirm(phrase:string){return run(async()=>{if(!view||!challenge||!key||phrase!==acknowledgement)throw Error('Prepare and explicitly acknowledge this exact account first');const c=challenge;challenge=undefined;disableVersion(view.version);
   const capture=Object.freeze({expectedAccountId:account,expectedUserId:account,expectedLifecycleId:view.lifecycleId,version:view.version});
   // Durable non-secret marker precedes dispatch; another tab/reload must not replay.
   const marker=JSON.stringify({accountId:account,lifecycleId:view.lifecycleId,version:view.version,confirmationId:c.confirmationId,state:'unknown'});storage.setItem(key,marker);if(storage.getItem(key)!==marker)throw Error('Durable confirmation protection is unavailable');
   try{check();const reply=await access.request('confirm',{capture,confirmationId:c.confirmationId,confirmationToken:c.confirmationToken,acknowledge:phrase});check();
    if(reply.status>=400&&reply.status<500){storage.removeItem(key);view=undefined;throw Error('Nothing was disabled by this request ('+reply.status+'). Load the current account and resolve its blockers before preparing again.');}
    const r=boundedObject(reply.body);receipt(r,account,capture.version);policy(r);if(r.lifecycleId!==c.lifecycleId||reply.status!==200||r.result!=='ok'||r.disabled!==true||r.confirmationId!==c.confirmationId)throw Error('The disable receipt is not confirmed');storage.setItem(key,JSON.stringify({accountId:account,lifecycleId:r.lifecycleId,version:r.version,confirmationId:c.confirmationId,state:'confirmed'}));return r;
   }catch(error){view=undefined;throw error;}
  });},
 };
}
export const browserExclusive:Exclusive=async(key,work)=>{if(!navigator.locks)throw Error('This browser cannot safely coordinate account confirmation across tabs. Use a browser with Web Locks support.');return navigator.locks.request(key,{mode:'exclusive',ifAvailable:true},lock=>{if(!lock)throw Error('Another tab is handling this account confirmation');return work();});};
