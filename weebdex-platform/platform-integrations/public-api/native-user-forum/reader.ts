// Public reads only. A native thread is selected by the actual persisted user
// context, never by a signing key or cookie account. No mutation API is exported.
import {PublicApiClient} from '../client';
import {publicUuid,PublicApiContractError} from '../validation';
import {normalizeDiscussionPage,type PublicDiscussionPage} from './data';
export {normalizeDiscussionPage,type PublicReply,type PublicDiscussionPage} from './data';
export interface DiscussionDependencies {api:PublicApiClient;fetch:typeof fetch;origin():string;setTimer(callback:()=>void,ms:number):ReturnType<typeof setTimeout>;clearTimer(timer:ReturnType<typeof setTimeout>):void;}
const defaults:DiscussionDependencies={api:new PublicApiClient(),fetch:(...args)=>fetch(...args),origin:()=>window.location.origin,setTimer:(callback,ms)=>setTimeout(callback,ms),clearTimer:timer=>clearTimeout(timer)};
function fail(message:string,status=502):never{throw new PublicApiContractError(message,status);}
function nativeId(value:unknown):string{if(typeof value!=='string'||!value||value.length>128||/[\\/\u0000-\u0020]/.test(value))fail('Invalid native discussion identity');return value;}
export function createNativeUserDiscussionReader(injected:Partial<DiscussionDependencies>={}){
 const deps={...defaults,...injected};
 return async(userId:string,threadId:string,page=1,signal?:AbortSignal,assertCurrent=()=>{},order:'asc'|'desc'='desc'):Promise<PublicDiscussionPage>=>{
  const user=publicUuid(userId),thread=nativeId(threadId),origin=new URL(deps.origin()).origin;
  if(!Number.isSafeInteger(page)||page<1||page>1000000)fail('Discussion page exceeds the public read budget',422);
  if(!['asc','desc'].includes(order))fail('Invalid public discussion order',422);
  const owned=new Set<Promise<unknown>>();
  function own<T>(promise:Promise<T>):Promise<T>{owned.add(promise);void promise.then(()=>owned.delete(promise),()=>owned.delete(promise));return promise;}
  const abort=new AbortController();let timedOut=false,completed=false,reader:ReadableStreamDefaultReader<Uint8Array>|undefined,response:Response|undefined;
  const cancel=()=>abort.abort();signal?.addEventListener('abort',cancel,{once:true});if(signal?.aborted)cancel();
  const cancelled=()=>new PublicApiContractError(timedOut?'Discussion read exceeded its 30-second deadline':'Discussion read cancelled',timedOut?504:499);
  const onAbort=()=>rejectAbort(cancelled());let rejectAbort:(reason:unknown)=>void=()=>{};
  const aborted=new Promise<never>((_resolve,reject)=>{rejectAbort=reject;});void aborted.catch(()=>{});abort.signal.addEventListener('abort',onAbort,{once:true});
  const timer=deps.setTimer(()=>{timedOut=true;abort.abort();},30000);
  const check=()=>{assertCurrent();if(abort.signal.aborted)throw cancelled();if(new URL(deps.origin()).origin!==origin)fail('Discussion destination changed',409);};
  async function guarded<T>(operation:()=>Promise<T>):Promise<T>{check();const value=await Promise.race([own(Promise.resolve().then(()=>{check();return operation();})),aborted]);check();return value;}
  async function context(){const response=await guarded(()=>deps.api.nativeForumContexts([{kind:'user',id:user}],abort.signal));if(response.status!==200)fail('The profile discussion could not be checked',response.status);const row=response.body?.data.find(row=>row.kind==='user'&&row.id===user);if(!row?.thread||row.thread.id!==thread)fail('The visible profile discussion changed. Reload this profile.',409);}
  try{
   check();await context();
   response=await guarded(async()=>{const value=await deps.fetch(origin+'/api/thread/'+encodeURIComponent(thread)+'/replies?limit=20&page='+page+'&order='+order,{method:'GET',credentials:'omit',cache:'no-store',redirect:'error',signal:abort.signal,headers:{Accept:'application/json'}});response=value;if(abort.signal.aborted){if(value.body)await own(value.body.cancel());throw cancelled();}return value;});
   if(response.status!==200)fail('Profile comments could not be loaded',response.status);
   if(response.headers.get('content-type')?.split(';',1)[0].trim().toLowerCase()!=='application/json')fail('Profile comments returned a non-JSON response');
   const limit=8*1024*1024,declared=response.headers.get('content-length');
   if(declared!==null&&(!/^(0|[1-9][0-9]*)$/.test(declared)||Number(declared)>limit))fail('Profile comments exceed the bounded response size');
   reader=response.body?.getReader();if(!reader)fail('Profile comments returned no response body');
   let length=0,text='';const decoder=new TextDecoder('utf-8',{fatal:true});
   while(true){const part=await guarded(()=>reader!.read());if(part.done)break;if(!(part.value instanceof Uint8Array)||part.value.byteLength>limit-length)fail('Profile comments exceed the bounded response size');length+=part.value.byteLength;text+=decoder.decode(part.value,{stream:true});}
   text+=decoder.decode();check();const feed=normalizeDiscussionPage(JSON.parse(text),page);await context();check();completed=true;return feed;
  }finally{
   // Failure paths cancel the underlying fetch/context request and stream before
   // releasing ownership. Successful EOF plus final context never aborts or
   // cancels a healthy completed read. Observe every eventual rejection, even
   // when a faulty dependency ignores AbortSignal and outlives the grace bound.
   deps.clearTimer(timer);
   let settled=true;
   if(!completed){abort.abort();
   if(reader)own(reader.cancel());else if(response?.body)own(response.body.cancel());
   let settlementTimer:ReturnType<typeof setTimeout>|undefined;
   settled=await Promise.race([Promise.allSettled([...owned]).then(()=>true),new Promise<boolean>(resolve=>{settlementTimer=setTimeout(()=>resolve(false),250);})]);
   if(settlementTimer!==undefined)clearTimeout(settlementTimer);
   }
   signal?.removeEventListener('abort',cancel);abort.signal.removeEventListener('abort',onAbort);
   if(reader){try{reader.releaseLock();}catch{/* no successful result can escape unconfirmed cleanup */}}
   if(!settled)fail('Discussion read stopped, but underlying cancellation settlement is unconfirmed.',timedOut?504:499);
  }
 };
}
export const readNativeUserDiscussion=createNativeUserDiscussionReader();

