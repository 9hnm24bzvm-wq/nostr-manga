// Explicit browser-account contribution authority. Never signs, acquires a key,
// or retries a write through another authentication scheme.
import {createNativeAccountScope,nativeAccountError,type NativeAccountDependencies} from './native-account-scope';
import {PublicUploadRequestError,type PublicUploadRequest,type PublicUploadResponse} from './nostr-upload-request';
import {publicUuid,record,PublicApiContractError} from './validation';
import type {TitleAccount} from './title-client';
export type NativeContributionDependencies=Partial<NativeAccountDependencies>;
const imageTypes=new Set(['image/png','image/jpeg','image/webp','image/gif']);
const headerNames=['retry-after','content-type','cache-control','vary','x-content-type-options','x-weebdex-instance-removal','x-weebdex-source-deletion-status','x-weebdex-source-deletion-request'] as const;
export function nativeContributionReceipt(body:unknown,account:string){const value=record(body),n=record(value.nostr);if(n.authentication!=='native-session'||n.scope!=='instance-account'||publicUuid(n.accountId)!==account)throw new PublicApiContractError('Contribution response changed its browser account authority',502);return value;}
export async function createNativeContributionScope(nativeAccount:string,assertCurrent:()=>void,injected:NativeContributionDependencies={}){
 const scope=await createNativeAccountScope(nativeAccount,assertCurrent,injected),origin=new URL(injected.origin?.()??window.location.origin).origin,fetcher=injected.fetch??fetch;
 const abort=new AbortController();let disposed=false;
 const check=()=>{scope.check();if(disposed)throw new PublicApiContractError('This contribution view closed',409);};
 const request=async(input:PublicUploadRequest):Promise<PublicUploadResponse>=>{
  let dispatched=false,received:Response|undefined,completed=false;
  const controller=new AbortController();
  const signal=AbortSignal.any([abort.signal,controller.signal,...(input.signal?[input.signal]:[]),AbortSignal.timeout(input.timeoutMs??60000)]);
  try{
   check();if(signal.aborted)throw Error('Request cancelled');
   const url=new URL(input.url);if(url.origin!==origin||url.href!==input.url||url.hash||url.username||url.password||!url.pathname.startsWith('/api/v1/account/')||/[%\\]/.test(url.pathname))throw Error('Contribution request escaped the native account namespace');
   if(!['GET','POST','PATCH','PUT','DELETE'].includes(input.method)||input.method==='GET'&&input.body)throw Error('Invalid contribution request method/body');
   const requestLimit=input.contentType?.startsWith('multipart/form-data;')?130*1024*1024:65536;
   if(input.body&&(!(input.body instanceof Uint8Array)||input.body.byteLength>requestLimit||input.requestLimit!==undefined&&input.body.byteLength>input.requestLimit)||input.body&&!input.contentType||input.contentType&&/[\r\n]/.test(input.contentType))throw Error('Contribution body exceeds its operation budget');
   const headers:Record<string,string>={Accept:input.responseType==='image'?'image/png,image/jpeg,image/webp,image/gif':'application/json','X-Weebdex-Expected-Account':nativeAccount};
   if(input.contentType)headers['Content-Type']=input.contentType;
   check();dispatched=true;received=await fetcher(url.href,{method:input.method,headers,body:input.body?new Uint8Array(input.body).buffer:undefined,credentials:'same-origin',redirect:'error',cache:'no-store',signal});check();if(signal.aborted)throw Error('Request cancelled');
   const responseHeaders=Object.fromEntries(headerNames.map(name=>[name,received!.headers.get(name)])) as PublicUploadResponse['headers'];
   const noBody=received.status===204,media=received.headers.get('content-type')?.split(';')[0].trim().toLowerCase();
   const image=input.responseType==='image'&&received.status===200;
   if(!noBody&&(image?!imageTypes.has(media??''):media!=='application/json'))throw Error('Invalid contribution response media');
   const maximum=Math.min(input.responseLimit??(image?32*1024*1024:4*1024*1024),image?32*1024*1024:4*1024*1024);
   if(!Number.isSafeInteger(maximum)||maximum<1)throw Error('Invalid response budget');
   const declared=received.headers.get('content-length');if(declared!==null&&(!/^\d+$/.test(declared)||Number(declared)>maximum))throw Error('Contribution response exceeds budget');
   const chunks:Uint8Array[]=[];let length=0;const reader=received.body?.getReader();
   if(reader){try{for(;;){const part=await reader.read();check();if(signal.aborted)throw Error('Request cancelled');if(part.done)break;length+=part.value.byteLength;if(length>maximum)throw Error('Contribution response exceeds budget');chunks.push(part.value);}}catch(error){await reader.cancel().catch(()=>{});throw error;}finally{reader.releaseLock();}}
   const bytes=new Uint8Array(length);let offset=0;for(const part of chunks){bytes.set(part,offset);offset+=part.length;}
   if(noBody&&length)throw Error('Unexpected empty-response payload');
   const body=noBody?undefined:image?bytes:JSON.parse(new TextDecoder('utf-8',{fatal:true}).decode(bytes));
   if(received.status>=200&&received.status<300&&!noBody&&!image)nativeContributionReceipt(body,scope.account.id);
   check();completed=true;return{status:received.status,body,bodyAvailable:true,headers:responseHeaders};
  }catch(error){throw new PublicUploadRequestError(dispatched&&input.method!=='GET'?'This contribution may have been accepted. Check its saved state; it will not be sent again automatically.':error instanceof Error?error.message:'Contribution request unavailable',dispatched?'uncertain':'unsent',received?.status);}
  finally{if(!completed){controller.abort();try{await received?.body?.cancel();}catch{}}}
 };
 const publicRequest=async(input:PublicUploadRequest):Promise<PublicUploadResponse>=>{
  check();const url=new URL(input.url);const resolver=url.pathname==='/api/v1/legacy/resolve'&&input.method==='POST'&&input.contentType==='application/json'&&!!input.body&&input.body.byteLength<=32768; if((!resolver&&(input.method!=='GET'||input.body))||input.responseType==='image'||url.origin!==origin||url.href!==input.url||url.hash||url.username||url.password||!/^\/api\/v1\/(?:manga(?:\/[^/]+)?|author(?:\/[^/]+)?|tag|group(?:\/[^/]+)?|chapter\/[^/]+|legacy\/resolve)$/.test(url.pathname))throw new PublicUploadRequestError('Only explicit public catalogue reads are allowed','unsent');
  // Public catalogue reads and the explicit typed alias-resolution POST never
  // carry a cookie, expected account, or signer.
  const response=await fetcher(url.href,{method:input.method,...(resolver?{body:new Uint8Array(input.body!).buffer,headers:{'Content-Type':'application/json'}}:{}),credentials:'omit',redirect:'error',cache:'no-store',signal:AbortSignal.any([abort.signal,...(input.signal?[input.signal]:[]),AbortSignal.timeout(30000)])});check();
  if(response.headers.get('content-type')?.split(';')[0].trim().toLowerCase()!=='application/json')throw Error('Invalid catalogue response');
  const reader=response.body?.getReader();if(!reader)throw Error('Missing catalogue response');const chunks:Uint8Array[]=[];let size=0;try{for(;;){const p=await reader.read();check();if(p.done)break;size+=p.value.length;if(size>4*1024*1024)throw Error('Catalogue response exceeds budget');chunks.push(p.value);}}catch(e){await reader.cancel().catch(()=>{});throw e;}finally{reader.releaseLock();}
  const bytes=new Uint8Array(size);let offset=0;for(const p of chunks){bytes.set(p,offset);offset+=p.length;}const body=JSON.parse(new TextDecoder('utf-8',{fatal:true}).decode(bytes));check();return{status:response.status,body,bodyAvailable:true,headers:Object.fromEntries(headerNames.map(name=>[name,response.headers.get(name)])) as PublicUploadResponse['headers']};
 };
 async function context(){const response=await request({url:origin+'/api/v1/account/contribution-context',method:'GET'});check();if(response.status!==200)throw nativeAccountError(response.status,response.body);const body=nativeContributionReceipt(response.body,scope.account.id),u=record(body.user),n=record(body.nostr);if(body.result!=='ok'||publicUuid(u.id)!==scope.account.id||n.nativeSessionAuthority!==true||n.catalogueScope!=='instance-local'||typeof u.username!=='string'||!u.username||!Number.isSafeInteger(u.version)||Number(u.version)<1||!Array.isArray(u.permissions)||u.permissions.some(v=>typeof v!=='string')||!Array.isArray(u.roles)||u.roles.some(v=>typeof v!=='string'))throw Error('Invalid contribution account');return{account:{id:scope.account.id,name:u.username,permissions:[...u.permissions],roles:[...u.roles]} as TitleAccount,catalogNamespace:publicUuid(n.catalogNamespace),body};}
 try{const initial=await context();return{...scope,origin,nativeAccount,check,request,publicRequest,context,account:initial.account,catalogNamespace:initial.catalogNamespace,contextBody:initial.body,dispose:()=>{disposed=true;abort.abort();scope.dispose();}};}catch(error){disposed=true;abort.abort();scope.dispose();throw error;}
}
export type NativeContributionScope=Awaited<ReturnType<typeof createNativeContributionScope>>;
