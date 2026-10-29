// Native browser session only. No public identity, key, relay or operator secret.
export interface Authority {scope:'native-staff-context';actorId:string;username:string;roles:string[];permissions:string[];verified:boolean;moderator:boolean;staff:boolean;sourceOperatorAuthority:false}
export class WorkspaceError extends Error {constructor(message:string,public status=0,public uncertain=false,public confirmedRefusal=false){super(message);}}
export interface Dependencies {fetch:typeof fetch;origin:()=>string;storage:Pick<Storage,'getItem'|'setItem'|'removeItem'>;requestId:()=>string}
const defaults:Dependencies={fetch:(...args)=>fetch(...args),origin:()=>window.location.origin,storage:{getItem:k=>sessionStorage.getItem(k),setItem:(k,v)=>sessionStorage.setItem(k,v),removeItem:k=>sessionStorage.removeItem(k)},requestId:()=>crypto.randomUUID()};
export function can(authority:Authority|undefined,grant:string,staff=false){return !!authority?.verified&&(staff?authority.staff:authority.moderator)&&authority.permissions.includes(grant)&&!authority.roles.some(r=>r==='banned'||r==='restricted');}
export function object(value:unknown):Record<string,any>{if(!value||typeof value!=='object'||Array.isArray(value))throw new WorkspaceError('The site returned an invalid response. Reload this workspace.');return value as Record<string,any>;}
export async function createStaffClient(account:string,assertCurrent=()=>{},injected:Partial<Dependencies>={}){
 const deps={...defaults,...injected},origin=new URL(deps.origin()).origin,controller=new AbortController();let closed=false,busy=false;
 if(!/^[A-Za-z0-9_:-]{1,128}$/.test(account))throw new WorkspaceError('Sign in to continue.',401);
 function check(){assertCurrent();if(closed||controller.signal.aborted||new URL(deps.origin()).origin!==origin)throw new WorkspaceError('This account view changed. Open it again.',409);}
 const prefix=`weebdex.staff-action.v1:${origin}:${account}:`;
 type PendingReceipt={path:string;raw:string;requestId?:string;invalid?:boolean};
 function fence(path:string):PendingReceipt|undefined{let raw:string|null;try{raw=deps.storage.getItem(prefix+path);}catch{throw new WorkspaceError('The browser action safeguard could not be read. No new action will be sent.',400);}if(raw===null)return undefined;try{const value=object(JSON.parse(raw));if(typeof value.requestId!=='string'||!/^[A-Za-z0-9_-]{16,80}$/.test(value.requestId)||value.path!==path)throw Error('Invalid marker');return{path,requestId:value.requestId,raw};}catch{return{path,raw,invalid:true};}}
 function clearExact(path:string,marker:string){try{if(deps.storage.getItem(prefix+path)===marker)deps.storage.removeItem(prefix+path);}catch{throw new WorkspaceError('The receipt was checked, but the browser safeguard could not be cleared. Keep this reference and contact the instance operator.',0,true);}}
 async function send(path:string,method:'GET'|'POST'|'PUT'='GET',payload?:unknown){
  let dispatched=false,confirmedRefusal=false;
  try{check();if(!/^\/(staff|my\/reports)\//.test(path)||/[#\\\u0000-\u0020]/.test(path))throw new WorkspaceError('Invalid workspace request.');
   const destination=origin+'/api'+path,url=new URL(destination);if(url.href!==destination||url.origin!==origin)throw new WorkspaceError('Workspace requests must remain on this site.');
   const bytes=payload===undefined?undefined:JSON.stringify(payload);if((bytes?new TextEncoder().encode(bytes).length:0)>32768)throw new WorkspaceError('This review is too large. Shorten the explanation.',400);
   check();dispatched=true;const response=await deps.fetch(destination,{method,credentials:'same-origin',cache:'no-store',redirect:'error',headers:{Accept:'application/json','X-Weebdex-Expected-Account':account,...(bytes?{'Content-Type':'application/json'}:{})},body:bytes,signal:AbortSignal.any([controller.signal,AbortSignal.timeout(30000)])});check();
   if(response.headers.get('content-type')?.split(';')[0].trim()!=='application/json')throw Error('Unexpected response type');
   const reader=response.body?.getReader();if(!reader)throw Error('Missing response');const chunks:Uint8Array[]=[];let size=0;
   try{for(;;){const row=await reader.read();check();if(row.done)break;size+=row.value.length;if(size>1024*1024)throw Error('Response exceeds budget');chunks.push(row.value);}}catch(error){await reader.cancel().catch(()=>{});throw error;}finally{reader.releaseLock();}
   const buffer=new Uint8Array(size);let offset=0;for(const chunk of chunks){buffer.set(chunk,offset);offset+=chunk.length;}const body=object(JSON.parse(new TextDecoder('utf-8',{fatal:true}).decode(buffer)));
   if(!response.ok){const detail=typeof body.error==='string'?body.error:typeof body.errors?.[0]?.detail==='string'?body.errors[0].detail:undefined;if(!detail||detail.length>4000)throw Error('Invalid refusal response');const refusal=response.status>=400&&response.status<500&&response.status!==408;confirmedRefusal=refusal;throw new WorkspaceError(detail.slice(0,1000),response.status,method!=='GET'&&!refusal,refusal);}
   if(body.actorId!==account)throw Error('Account receipt mismatch');check();return body;
  }catch(error){if(error instanceof WorkspaceError&&(confirmedRefusal||!dispatched||method==='GET'))throw error;throw new WorkspaceError(method!=='GET'&&dispatched?'The result is not confirmed. Check its receipt before doing anything again.':'This view could not be loaded. Please try again.',error instanceof WorkspaceError?error.status:0,dispatched&&method!=='GET');}
 }
 const authority=await send('/staff/context') as Authority;
 if(authority.scope!=='native-staff-context'||!Array.isArray(authority.roles)||authority.roles.some(role=>typeof role!=='string')||!Array.isArray(authority.permissions)||authority.permissions.some(grant=>typeof grant!=='string')||typeof authority.username!=='string'||typeof authority.verified!=='boolean'||typeof authority.staff!=='boolean'||typeof authority.moderator!=='boolean'||authority.sourceOperatorAuthority!==false)throw new WorkspaceError('The account authority could not be checked.');
 return {authority,check,dispose(){closed=true;controller.abort();},read:(path:string)=>send(path),pending:fence,
  async mutate(path:string,payload:Record<string,unknown>,method:'POST'|'PUT'='POST'){
   check();if(busy)throw new WorkspaceError('An action is still in progress.',409);if(fence(path))throw new WorkspaceError('An earlier result is still unconfirmed. Check its receipt first.',409,true);
   const requestId=typeof payload.requestId==='string'?payload.requestId:deps.requestId();if(!/^[A-Za-z0-9_-]{16,80}$/.test(requestId))throw new WorkspaceError('The action reference is invalid. Reopen this review.',400);const command={...payload,requestId},marker=JSON.stringify({requestId,path,markerId:deps.requestId()});busy=true;
   // Persist before dispatch, so closing this page cannot unlock a duplicate.
   try{deps.storage.setItem(prefix+path,marker);if(deps.storage.getItem(prefix+path)!==marker)throw Error('Receipt marker was not retained');}catch{busy=false;throw new WorkspaceError('This browser cannot retain the action receipt. No action was sent; enable session storage and reopen the review.',400);}
   try{const result=await send(path,method,command);if(result.requestId!==requestId||result.scope!=='native-moderation')throw new WorkspaceError('The result receipt could not be verified. Check its receipt.',0,true);clearExact(path,marker);return result;}
   catch(error){if(error instanceof WorkspaceError&&!error.uncertain)clearExact(path,marker);throw error;}finally{busy=false;}
  },
  async reconcile(path:string){const pending=fence(path);if(!pending)throw new WorkspaceError('There is no pending receipt to check.',400);if(pending.invalid||!pending.requestId)throw new WorkspaceError('The saved receipt reference is damaged. No action will be repeated; ask the instance operator to reconcile this report.',409,true);const result=await send('/staff/receipts/'+encodeURIComponent(pending.requestId));if(result.requestId!==pending.requestId||result.scope!=='native-moderation')throw new WorkspaceError('The receipt did not match this action.');clearExact(path,pending.raw);return result;},
  // Profile CAS has its own readback contract; it never silently replays.
  async saveProfile(path:string,payload:unknown){check();if(busy)throw new WorkspaceError('An action is still in progress.',409);busy=true;try{return await send(path,'PUT',payload);}finally{busy=false;}}
 };
}
export type StaffClient=Awaited<ReturnType<typeof createStaffClient>>;
export function effectText(context:Record<string,any>,action:string){
 const report=object(context.report),target=context.target??{},effect=action==='delete'?context.effect:action==='accept'?context.acceptEffect:{};
 const label=target.label||report.objectId,reference=report.objectId;
 if(action==='delete')return `Remove report ${report.id} from report lists. This does not remove, hide or change its subject (${reference}). Its decision history is retained.`;
 if(action==='refuse')return `Refuse this report about ${label} (${reference}). The reported content and group authority will not change.`;
 if(effect?.action==='grant_group_leadership')return `Grant local leadership of ${label} (${reference}) to ${context.claimant?.name||effect.claimant_id} (${effect.claimant_id}). This changes this group's local management permissions, not original publishing authority.`;
 if(effect?.action==='instance_soft_hide')return `Hide ${label} (${reference}) on this instance. Readers here will no longer see it. This does not delete the original source or copies elsewhere. Any local restoration needs a separate authorized review.`;
 return `Accept this report about ${label} (${reference}) and record your reason. No automatic content removal or authority change is attached to this report type.`;
}
