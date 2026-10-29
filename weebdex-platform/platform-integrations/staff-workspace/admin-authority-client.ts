// Native-cookie security workspace; no signer or source-service credentials.
export interface AdminTarget {id:string;username:string;verified:boolean;disabled:boolean;retainedAccount:boolean;roles:string[];permissions:string[];bundles:string[];authorityVersion:number;customAuthority:boolean;restricted:boolean}
export interface AdminCommand {requestId:string;targetId:string;authorityVersion:number;operation:'roles'|'restrict'|'unrestrict'|'revoke-sessions'|'reactivate';bundles:string[];reason:string;replaceCustom:boolean;ownerRequested:boolean}
export interface AdminBundle {id:string;label:string;roles:string[];grants:string[]}
export interface AdminContext {actorId:string;username:string;roles:string[];permissions:string[];freshAuthentication:boolean;bundles:AdminBundle[];normalPermissions:string[];roleHierarchy:string;sourceOperatorAuthority:false}
export class AdminError extends Error {constructor(message:string,public status=0,public code='',public uncertain=false){super(message);}}
export interface AdminDependencies {fetch:typeof fetch;origin:()=>string;storage:Pick<Storage,'getItem'|'setItem'|'removeItem'>;requestId:()=>string}
const defaults:AdminDependencies={fetch:(...a)=>fetch(...a),origin:()=>window.location.origin,storage:{getItem:k=>sessionStorage.getItem(k),setItem:(k,v)=>sessionStorage.setItem(k,v),removeItem:k=>sessionStorage.removeItem(k)},requestId:()=>crypto.randomUUID()};
function object(v:any){if(!v||typeof v!=='object'||Array.isArray(v))throw new Error('Invalid admin response');return v;}
export function canAdmin(a:AdminContext|undefined,p:string){return !!a&&a.roles.includes('sysop')&&!a.roles.some(r=>['banned','restricted'].includes(r))&&a.permissions.includes(p);}
export function createAdminClient(account:string,assertCurrent=()=>{},injected:Partial<AdminDependencies>={}){
 const deps={...defaults,...injected},origin=new URL(deps.origin()).origin,controller=new AbortController(),prefix=`weebdex.admin.v1:${origin}:${account}:`;let busy=false,closed=false;
 if(!account||account.length>128)throw new AdminError('Sign in with your native account.',401);
 function check(){assertCurrent();if(closed||controller.signal.aborted||new URL(deps.origin()).origin!==origin)throw new AdminError('The account or site changed. Reopen this workspace.',409);}
 function readSaved(kind:string):AdminCommand|undefined{check();const raw=deps.storage.getItem(prefix+kind);if(!raw)return;try{const v=object(JSON.parse(raw));if(typeof v.requestId==='string'&&typeof v.targetId==='string'&&Number.isSafeInteger(v.authorityVersion)&&typeof v.operation==='string')return v as AdminCommand;}catch{}throw new AdminError('A saved security action cannot be read. Do not repeat it; ask another administrator to inspect security history.',409);}
 async function send(path:string,method:'GET'|'POST'='GET',command?:AdminCommand){
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
 function saved(kind:string,v:AdminCommand){check();try{deps.storage.setItem(prefix+kind,JSON.stringify(v));}catch{throw new AdminError('This browser cannot preserve the recovery receipt. Enable site session storage before trying again; nothing was sent.',409);}}
 function validReceipt(r:any,c:AdminCommand){return r.scope==='native-admin-receipt'&&r.requestId===c.requestId&&r.targetId===c.targetId&&r.operation===c.operation&&(typeof r.auditId==='string'||r.outcome==='cancelled-before-apply')&&Number.isSafeInteger(r.authorityVersion);}
 return {check,dispose(){closed=true;controller.abort();},newRequestId:deps.requestId,
  context:()=>send('/staff/admin/context') as Promise<AdminContext>,
  users:(q='',after='')=>send('/staff/admin/users?'+new URLSearchParams({q,after})),
  target:(id:string)=>send('/staff/admin/users/'+encodeURIComponent(id)),
  audit:(targetId='',before='')=>send('/staff/admin/audit?'+new URLSearchParams({targetId,...(before?{before}:{})})),
  intent:()=>readSaved('intent'),pending:()=>readSaved('pending'),
  cancel(){check();if(readSaved('pending'))throw new AdminError('An unconfirmed action must be reconciled, not discarded.',409);deps.storage.removeItem(prefix+'intent');},
  async review(c:AdminCommand){check();if(busy||readSaved('pending'))throw new AdminError('Check the earlier action receipt before reviewing another action.',409);saved('intent',c);busy=true;try{const result=await send('/staff/admin/users/'+encodeURIComponent(c.targetId)+'/review','POST',c);if(result.scope!=='native-admin-review'||result.requestId!==c.requestId||JSON.stringify(result.command)!==JSON.stringify(c)){
    // JSON field order is not authority; compare every fixed command field.
    if(result.scope!=='native-admin-review'||Object.keys(c).some(k=>JSON.stringify(result.command?.[k])!==JSON.stringify((c as any)[k])))throw new AdminError('The reviewed command could not be verified. Reload it.');
   }return result;}finally{busy=false;}},
  async confirm(c:AdminCommand){check();if(busy||readSaved('pending'))throw new AdminError('An action is already pending. Check its exact receipt.',409);saved('pending',c);busy=true;try{const r=await send('/staff/admin/users/'+encodeURIComponent(c.targetId)+'/commands','POST',c);if(!validReceipt(r,c))throw new AdminError('The action receipt did not match. Check it before doing anything else.',0,'',true);deps.storage.removeItem(prefix+'pending');deps.storage.removeItem(prefix+'intent');return r;}catch(e){if(e instanceof AdminError&&!e.uncertain)deps.storage.removeItem(prefix+'pending');throw e;}finally{busy=false;}},
  async reconcile(){const c=readSaved('pending');if(!c)throw new AdminError('There is no uncertain action.',400);const r=await send('/staff/admin/receipts/'+encodeURIComponent(c.requestId));if(r.pending===true)return r;if(!validReceipt(r,c))throw new AdminError('The receipt does not match the saved action.');deps.storage.removeItem(prefix+'pending');deps.storage.removeItem(prefix+'intent');return r;},
  async settle(){const c=readSaved('pending');if(!c||busy)throw new AdminError('No uncertain action is available.',409);busy=true;try{const r=await send('/staff/admin/receipts/'+encodeURIComponent(c.requestId)+'/settle','POST',c);if(!validReceipt(r,c))throw new AdminError('The original action could not be settled.',0,'',true);deps.storage.removeItem(prefix+'pending');deps.storage.removeItem(prefix+'intent');return r;}finally{busy=false;}},
  // Explicit retry is safe only with the unchanged actor/body/request ID. It never
  // creates a new command, and authority/CAS/review expiry are rechecked server-side.
  async retryExact(){const c=readSaved('pending');if(!c||busy)throw new AdminError('No retryable exact action is available.',409);busy=true;try{const r=await send('/staff/admin/users/'+encodeURIComponent(c.targetId)+'/commands','POST',c);if(!validReceipt(r,c))throw new AdminError('The exact action is still unconfirmed.',0,'',true);deps.storage.removeItem(prefix+'pending');deps.storage.removeItem(prefix+'intent');return r;}finally{busy=false;}}
 };
}
export type AdminClient=ReturnType<typeof createAdminClient>;
