// Native-cookie security workspace; no signer or source-service credentials.
export interface AdminTarget {id:string;username:string;verified:boolean;disabled:boolean;retainedAccount:boolean;roles:string[];permissions:string[];bundles:string[];authorityVersion:number;customAuthority:boolean;restricted:boolean}
export interface AdminCommand {requestId:string;targetId:string;authorityVersion:number;operation:'roles'|'restrict'|'unrestrict'|'revoke-sessions'|'reactivate';bundles:string[];reason:string;replaceCustom:boolean;ownerRequested:boolean}
export type AdminPending={version:2;requestId:string;commandHash:string|null}|{version:2;blocked:true};
export interface AdminBundle {id:string;label:string;roles:string[];grants:string[]}
export interface AdminContext {actorId:string;username:string;roles:string[];permissions:string[];freshAuthentication:boolean;bundles:AdminBundle[];normalPermissions:string[];roleHierarchy:string;sourceOperatorAuthority:false}
export class AdminError extends Error {constructor(message:string,public status=0,public code='',public uncertain=false){super(message);}}
export interface AdminDependencies {fetch:typeof fetch;origin:()=>string;storage:Pick<Storage,'getItem'|'setItem'|'removeItem'>;requestId:()=>string}
const defaults:AdminDependencies={fetch:(...a)=>fetch(...a),origin:()=>window.location.origin,storage:{getItem:k=>sessionStorage.getItem(k),setItem:(k,v)=>sessionStorage.setItem(k,v),removeItem:k=>sessionStorage.removeItem(k)},requestId:()=>crypto.randomUUID()};
function object(v:any){if(!v||typeof v!=='object'||Array.isArray(v))throw new Error('Invalid admin response');return v;}
export function canAdmin(a:AdminContext|undefined,p:string){return !!a&&a.roles.includes('sysop')&&!a.roles.some(r=>['banned','restricted'].includes(r))&&a.permissions.includes(p);}
export function createAdminClient(account:string,assertCurrent=()=>{},injected:Partial<AdminDependencies>={}){
 const deps={...defaults,...injected},origin=new URL(deps.origin()).origin,controller=new AbortController(),prefix=`weebdex.admin.v1:${origin}:${account}:`;
 let busy=false,closed=false,intent:AdminCommand|undefined,prepared:{command:AdminCommand;commandHash:string}|undefined,retry:{command:AdminCommand;commandHash:string}|undefined;
 if(!account||account.length>128)throw new AdminError('Sign in with your native account.',401);
 const uuid=(v:any)=>typeof v==='string'&&v.length===36&&/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(v);
 const hash=(v:any)=>typeof v==='string'&&v.length===64&&/^[0-9a-f]{64}$/.test(v);
 const operations=['roles','restrict','unrestrict','revoke-sessions','reactivate'];
 const fields=['requestId','targetId','authorityVersion','operation','bundles','reason','replaceCustom','ownerRequested'];
 const clone=(c:AdminCommand):AdminCommand=>JSON.parse(JSON.stringify(c));
 const same=(a:AdminCommand,b:AdminCommand)=>fields.every(k=>JSON.stringify((a as any)[k])===JSON.stringify((b as any)[k]));
 function check(){assertCurrent();if(closed||controller.signal.aborted||new URL(deps.origin()).origin!==origin)throw new AdminError('The account or site changed. Reopen this workspace.',409);}
 function storageRead(key:string){try{return deps.storage.getItem(key);}catch{throw new AdminError('Recovery storage cannot be read. No new action will be sent; restore site storage before continuing.',409);}}
 function writeMarker(marker:AdminPending,expected:string|null){check();if(storageRead(prefix+'pending')!==expected)throw new AdminError('The pending action changed. Check its exact receipt before continuing.',409,'',true);const raw=JSON.stringify(marker);try{deps.storage.setItem(prefix+'pending',raw);}catch{throw new AdminError('This browser cannot preserve the recovery marker. Enable site session storage; nothing was sent.',409);}if(storageRead(prefix+'pending')!==raw)throw new AdminError('The recovery marker was not preserved. Nothing was sent; restore site storage before continuing.',409);}
 function legacy(v:any){return v&&Object.keys(v).length===fields.length&&fields.every(k=>Object.hasOwn(v,k))&&uuid(v.requestId)&&typeof v.targetId==='string'&&v.targetId.length>0&&v.targetId.length<=128&&!/[\u0000-\u001f\u007f]/.test(v.targetId)&&Number.isInteger(v.authorityVersion)&&v.authorityVersion>0&&v.authorityVersion<=2147483647&&operations.includes(v.operation)&&Array.isArray(v.bundles)&&v.bundles.length<=16&&v.bundles.every((x:any)=>typeof x==='string'&&/^[a-z-]{1,64}$/.test(x))&&typeof v.reason==='string'&&v.reason.trim().length>0&&v.reason.length<=2000&&typeof v.replaceCustom==='boolean'&&typeof v.ownerRequested==='boolean';}
 function pending():AdminPending|undefined{
  check();const raw=storageRead(prefix+'pending');if(raw===null)return;let v:any;try{if(raw.length<=16384)v=JSON.parse(raw);}catch{}
  if(v?.version===2&&Object.keys(v).length===2&&v.blocked===true)return {version:2,blocked:true};
  if(v?.version===2&&Object.keys(v).length===3&&uuid(v.requestId)&&(v.commandHash===null||hash(v.commandHash)))return {version:2,requestId:v.requestId,commandHash:v.commandHash};
  // Legacy plaintext is scrubbed, never retransmitted or silently treated as absent.
  const marker:AdminPending=legacy(v)?{version:2,requestId:v.requestId,commandHash:null}:{version:2,blocked:true};writeMarker(marker,raw);return marker;
 }
 function exactPending(){const marker=pending();if(!marker||'blocked'in marker)throw new AdminError('The pending marker cannot identify a safe recovery request. Do not repeat the action; ask an administrator to inspect security history.',409);return marker;}
 function unchanged(marker:AdminPending){check();if(JSON.stringify(pending())!==JSON.stringify(marker))throw new AdminError('The pending marker changed while waiting. Its fence was retained.',409,'',true);}
 function clear(marker:AdminPending){unchanged(marker);try{deps.storage.removeItem(prefix+'pending');}catch{throw new AdminError('The result is known but its recovery marker could not be cleared. Check it again before starting another action.',409,'',true);}if(storageRead(prefix+'pending')!==null)throw new AdminError('The result is known but the recovery fence remains. Check it again before starting another action.',409,'',true);intent=undefined;prepared=undefined;retry=undefined;}
 // Pre-review intent is not a dispatched business write. Drop its legacy plaintext.
 try{deps.storage.removeItem(prefix+'intent');}catch{throw new AdminError('Old private review fields could not be removed. Restore site storage before continuing.',409);}
 if(storageRead(prefix+'intent')!==null)throw new AdminError('Old private review fields remain in site storage. No action will be sent.',409);
 pending();
 async function send(path:string,method:'GET'|'POST'='GET',command?:AdminCommand|{commandHash:string}){
  check();if(!path.startsWith('/staff/admin/')||/[#\\\u0000-\u0020]/.test(path))throw new AdminError('Invalid security workspace request.');const url=origin+'/api'+path;if(new URL(url).href!==url)throw new AdminError('Security requests must remain on this site.');
  let dispatched=false,definitiveResponse=false;try{const bytes=command?JSON.stringify(command):undefined;if(bytes&&new TextEncoder().encode(bytes).length>16384)throw new AdminError('Shorten this security review.',400);
   check();dispatched=true;const response=await deps.fetch(url,{method,credentials:'same-origin',cache:'no-store',redirect:'error',headers:{Accept:'application/json','X-WeebDex-Expected-Account':account,...(bytes?{'Content-Type':'application/json'}:{})},body:bytes,signal:AbortSignal.any([controller.signal,AbortSignal.timeout(30000)])});check();
   if(response.headers.get('content-type')?.split(';')[0].trim()!=='application/json')throw Error('Unexpected response type');const reader=response.body?.getReader();if(!reader)throw Error('Missing response');let size=0;const chunks:Uint8Array[]=[];
   try{for(;;){const r=await reader.read();check();if(r.done)break;size+=r.value.length;if(size>1024*1024)throw Error('Response too large');chunks.push(r.value);}}catch(e){await reader.cancel().catch(()=>{});throw e;}finally{reader.releaseLock();}
   const all=new Uint8Array(size);let offset=0;for(const x of chunks){all.set(x,offset);offset+=x.length;}const body=object(JSON.parse(new TextDecoder('utf-8',{fatal:true}).decode(all)));
   if(!response.ok){definitiveResponse=true;throw new AdminError(typeof body.error==='string'?body.error.slice(0,1000):'The action was refused. Reload and review.',response.status,typeof body.code==='string'?body.code:'',method==='POST'&&response.status>=500);}
   if(body.actorId!==account)throw Error('Wrong account response');return body;
  }catch(e){if(e instanceof AdminError&&(!dispatched||definitiveResponse))throw e;throw new AdminError(method==='POST'&&dispatched?'The result is unconfirmed. Check this exact receipt; no action will be repeated automatically.':'The security view could not be loaded.',0,'',method==='POST'&&dispatched);}
 }

 function validReceipt(r:any,m:{requestId:string;commandHash:string|null}){return r.scope==='native-admin-receipt'&&r.requestId===m.requestId&&hash(r.commandHash)&&r.commandHash===m.commandHash&&typeof r.targetId==='string'&&operations.includes(r.operation)&&(typeof r.auditId==='string'||r.outcome==='cancelled-before-apply')&&Number.isSafeInteger(r.authorityVersion)&&r.authorityVersion>0;}
 function canRetry(){const m=pending();return !!retry&&!!m&&!('blocked'in m)&&m.requestId===retry.command.requestId&&m.commandHash===retry.commandHash;}
 return {check,dispose(){intent=undefined;prepared=undefined;retry=undefined;closed=true;controller.abort();},newRequestId:deps.requestId,
  context:()=>send('/staff/admin/context') as Promise<AdminContext>,
  users:(q='',after='')=>send('/staff/admin/users?'+new URLSearchParams({q,after}).toString().replace(/^\?/,'')),
  target:(id:string)=>send('/staff/admin/users/'+encodeURIComponent(id)),
  audit:(targetId='',before='')=>send('/staff/admin/audit?'+new URLSearchParams({targetId,...(before?{before}:{})}).toString().replace(/^\?/,'')),
  intent:()=>{check();return intent?clone(intent):undefined;},pending,canRetry,
  cancel(){check();if(pending())throw new AdminError('An unconfirmed action must be reconciled, not discarded.',409);intent=undefined;prepared=undefined;retry=undefined;},
  async review(c:AdminCommand){check();if(busy||pending())throw new AdminError('Check the earlier action receipt before reviewing another action.',409);intent=clone(c);prepared=undefined;busy=true;try{const result=await send('/staff/admin/users/'+encodeURIComponent(c.targetId)+'/review','POST',clone(c));if(result.scope!=='native-admin-review'||result.requestId!==c.requestId||!same(result.command??{},c)||!hash(result.commandHash))throw new AdminError('The reviewed command fingerprint could not be verified. Review again.');prepared={command:clone(c),commandHash:result.commandHash};return result;}finally{busy=false;}},
  async confirm(c:AdminCommand){check();if(busy||pending())throw new AdminError('An action is already pending. Check its exact receipt.',409);if(!prepared||!same(prepared.command,c))throw new AdminError('Review this exact action in the current view before confirming.',409);const exact=clone(prepared.command),marker={version:2 as const,requestId:exact.requestId,commandHash:prepared.commandHash};writeMarker(marker,null);retry={command:exact,commandHash:marker.commandHash};busy=true;try{const r=await send('/staff/admin/users/'+encodeURIComponent(exact.targetId)+'/commands','POST',exact);if(!validReceipt(r,marker))throw new AdminError('The receipt did not match. Check it before doing anything else.',0,'',true);clear(marker);return r;}catch(e){if(e instanceof AdminError&&!e.uncertain){unchanged(marker);try{deps.storage.removeItem(prefix+'pending');}catch{}retry=undefined;}throw e;}finally{busy=false;}},
  async reconcile(){const marker=exactPending();if(busy)throw new AdminError('A recovery request is already running.',409);busy=true;try{const suffix=marker.commandHash?'?commandHash='+marker.commandHash:'';const r=await send('/staff/admin/receipts/'+encodeURIComponent(marker.requestId)+suffix);unchanged(marker);if(r.scope!=='native-admin-receipt'||r.requestId!==marker.requestId)throw new AdminError('The recovery response does not match the pending request.',409);let bound=marker;
   if(marker.commandHash===null&&hash(r.commandHash)){bound={...marker,commandHash:r.commandHash};writeMarker(bound,JSON.stringify(marker));}
   else if(r.commandHash!==null&&r.commandHash!==undefined&&r.commandHash!==marker.commandHash)throw new AdminError('The command fingerprint does not match. The pending fence was retained.',409);
   if(r.pending===true)return r;if(!validReceipt(r,bound))throw new AdminError('The receipt does not match the pending fingerprint.',409);clear(bound);return r;
  }finally{busy=false;}},
  async settle(){const marker=exactPending();if(!marker.commandHash||busy)throw new AdminError('Check the exact server record before cancelling this older action.',409);busy=true;try{const r=await send('/staff/admin/receipts/'+encodeURIComponent(marker.requestId)+'/settle','POST',{commandHash:marker.commandHash});if(!validReceipt(r,marker))throw new AdminError('The original action could not be settled.',0,'',true);clear(marker);return r;}finally{busy=false;}},
  // An exact retry is available only while this view retains the original body.
  async retryExact(){const marker=exactPending();if(!retry||retry.command.requestId!==marker.requestId||retry.commandHash!==marker.commandHash||!marker.commandHash||busy)throw new AdminError('The private command is no longer in memory. Check or cancel the original request, then make a fresh review.',409);const exact=clone(retry.command);busy=true;try{const r=await send('/staff/admin/users/'+encodeURIComponent(exact.targetId)+'/commands','POST',exact);if(!validReceipt(r,marker))throw new AdminError('The exact action is still unconfirmed.',0,'',true);clear(marker);return r;}finally{busy=false;}}
 };
}
export type AdminClient=ReturnType<typeof createAdminClient>;
