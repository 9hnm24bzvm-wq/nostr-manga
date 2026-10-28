// Anonymous public presentation only. Never a cookie actor or signing identity.
import {publicUuid, record, PublicApiContractError} from './validation';
export interface PublicMembership {id:string;name:string;version:number;isLeader:boolean;isOfficer:boolean;}
export interface MembershipPage {userId:string;data:PublicMembership[];nextCursor:string|null;}
const MAX_BYTES=65536, PAGE_SIZE=32;
function fail(message:string,status=502):never {throw new PublicApiContractError(message,status);}
function exact(value:Record<string,unknown>,fields:string[],label:string){if(Object.keys(value).length!==fields.length||Object.keys(value).some(key=>!fields.includes(key)))fail('Invalid '+label+' fields.');}
export function membershipCursor(input:unknown,userId:string):{version:1;userId:string;groupId:string;name:string} {
 if(typeof input!=='string'||!input||input.length>2048||!/^[A-Za-z0-9_-]+$/.test(input))fail('Invalid public membership cursor.');
 let value:Record<string,unknown>;try{const bytes=Uint8Array.from(atob(input.replace(/-/g,'+').replace(/_/g,'/')),char=>char.charCodeAt(0));value=record(JSON.parse(new TextDecoder('utf-8',{fatal:true}).decode(bytes)));}catch{fail('Invalid public membership cursor.');}
 exact(value,['version','userId','groupId','name'],'membership cursor');
 if(value.version!==1||value.userId!==userId||publicUuid(value.groupId)!==value.groupId||typeof value.name!=='string'||!value.name||[...value.name].length>200||new TextEncoder().encode(value.name).byteLength>800)fail('Membership cursor belongs to another target or changed page.',409);
 return value as {version:1;userId:string;groupId:string;name:string};
}
export function normalizeMembershipPage(input:unknown,userId:string,after:string|null=null):MembershipPage {
 const expected=publicUuid(userId),cursor=after===null?null:(membershipCursor(after,expected),after),body=record(input,'membership page');
 exact(body,['result','response','data','limit','after','nextCursor','nostr'],'membership page');
 const meta=record(body.nostr,'membership provenance');exact(meta,['scope','source','identity','requestedId','userId','consistency'],'membership provenance');
 if(body.result!=='ok'||body.response!=='membership_page'||body.limit!==PAGE_SIZE||body.after!==cursor||!Array.isArray(body.data)||body.data.length>PAGE_SIZE||meta.scope!=='public-native-user'||meta.source!=='persisted-visible-native-memberships'||meta.identity!=='native_public_user'||meta.requestedId!==expected||meta.userId!==expected||meta.consistency!=='live-keyset-v1')fail('Memberships no longer match this native public user. Refresh the profile.',409);
 const seen=new Set<string>();
 const data=body.data.map(value=>{
  const row=record(value,'membership');exact(row,['id','type','attributes','relationships','membership'],'membership');
  const id=publicUuid(row.id),attributes=record(row.attributes,'group attributes'),edge=record(row.membership,'target membership');
  exact(attributes,['name','version'],'group attributes');exact(edge,['isLeader','isOfficer'],'target membership');
  if(row.id!==id||seen.has(id)||row.type!=='scanlation_group'||!Array.isArray(row.relationships)||row.relationships.length||typeof attributes.name!=='string'||!attributes.name||[...attributes.name].length>200||new TextEncoder().encode(attributes.name).byteLength>800||!Number.isSafeInteger(attributes.version)||Number(attributes.version)<1||Number(attributes.version)>2147483647||typeof edge.isLeader!=='boolean'||typeof edge.isOfficer!=='boolean')fail('Invalid target-only public membership.');
  seen.add(id);return{id,name:attributes.name,version:Number(attributes.version),isLeader:edge.isLeader,isOfficer:edge.isOfficer};
 });
 if(body.nextCursor!==null){const next=membershipCursor(body.nextCursor,expected),last=data.at(-1);if(data.length!==PAGE_SIZE||next.groupId!==last?.id||next.name!==last.name||body.nextCursor===cursor)fail('Invalid public membership continuation.');}
 return{userId:expected,data,nextCursor:body.nextCursor as string|null};
}

// Same bounded ownership pattern as reviewed device-history transport, scoped
// independently to anonymous public memberships. Reopening cannot stack reads
// whose underlying fetch/body/cancellation ignored AbortSignal.
export const MEMBERSHIP_LIMITS={responseBytes:MAX_BYTES,fragments:8192,emptyFragments:128,deadlineMs:15000,cleanupMs:250} as const;
let active:object|undefined;
function unsettled(){return new PublicApiContractError('Membership cleanup is unconfirmed. Wait for the previous read to settle before refreshing.',503);}
export function createMembershipReader(fetcher:typeof fetch=(...args)=>fetch(...args)) {
 return async(userId:string,after:string|null=null,signal?:AbortSignal):Promise<MembershipPage>=>{
  const user=publicUuid(userId),cursor=after===null?null:(membershipCursor(after,user),after);
  if(signal?.aborted)fail('Membership read cancelled.',499);
  if(active)throw unsettled();
  const owner=active={},request=new AbortController();
  let reader:ReadableStreamDefaultReader<Uint8Array>|undefined,response:Response|undefined;
  let fetchWork:Promise<Response>|undefined,readWork:Promise<ReadableStreamReadResult<Uint8Array>>|undefined;
  let eof=false,timedOut=false,failure:unknown;
  const cancelled=()=>new PublicApiContractError(timedOut?'Membership read timed out. Refresh to try again.':'Membership read cancelled.',timedOut?503:499);
  const cancel=()=>request.abort(cancelled());
  let rejectAbort:(reason:unknown)=>void=()=>{};
  const aborted=new Promise<never>((_,reject)=>{rejectAbort=reject;});void aborted.catch(()=>{});
  const onAbort=()=>rejectAbort(request.signal.reason);
  request.signal.addEventListener('abort',onAbort,{once:true});signal?.addEventListener('abort',cancel,{once:true});if(signal?.aborted)cancel();
  const timer=setTimeout(()=>{timedOut=true;cancel();},MEMBERSHIP_LIMITS.deadlineMs);
  const check=()=>{if(request.signal.aborted)throw request.signal.reason;};
  const release=()=>{reader?.releaseLock();if(active===owner)active=undefined;};
  try {
   // The retained frontend patches URLSearchParams.toString() to prepend '?'.
   // Encode our two bounded fields explicitly, independent of global patches.
   check();const query='limit='+PAGE_SIZE+(cursor===null?'':'&after='+encodeURIComponent(cursor));
   fetchWork=Promise.resolve().then(()=>{check();return fetcher('/api/v1/nostr/user/'+user+'/memberships?'+query,{method:'GET',credentials:'omit',cache:'no-store',redirect:'error',signal:request.signal,headers:{Accept:'application/json'}});}).then(value=>{response=value;return value;});
   response=await Promise.race([fetchWork,aborted]);check();
   reader=response.body?.getReader();if(!reader)fail('The public membership response had no body.');
   if(response.status!==200)fail('Public memberships could not be loaded ('+response.status+'). Refresh the profile.',response.status);
   if(response.headers.get('content-type')?.split(';',1)[0].trim().toLowerCase()!=='application/json')fail('The public membership response was not JSON.');
   const declared=response.headers.get('content-length');if(declared!==null&&(!/^(0|[1-9][0-9]*)$/.test(declared)||Number(declared)>MAX_BYTES))fail('Public membership response exceeded its byte budget.',503);
   const data=new Uint8Array(MAX_BYTES);let bytes=0,fragments=0,empty=0;
   for(;;){
    check();if(++fragments>MEMBERSHIP_LIMITS.fragments)fail('Public membership response exceeded its fragment budget.',503);
    readWork=Promise.resolve().then(()=>{check();return reader!.read();}).then(part=>{if(part.done)eof=true;return part;});
    const part=await Promise.race([readWork,aborted]);check();if(part.done)break;
    if(!ArrayBuffer.isView(part.value)||Object.prototype.toString.call(part.value)!=='[object Uint8Array]')fail('Public membership response returned invalid bytes.');
    if(part.value.byteLength>data.length-bytes)fail('Public membership response exceeded its byte budget.',503);
    if(!part.value.byteLength&&++empty>MEMBERSHIP_LIMITS.emptyFragments)fail('Public membership response exceeded its empty-fragment budget.',503);
    data.set(part.value,bytes);bytes+=part.value.byteLength;
   }
   check();let body:unknown;try{body=JSON.parse(new TextDecoder('utf-8',{fatal:true}).decode(data.subarray(0,bytes)));}catch{fail('The public membership response was malformed.');}
   return normalizeMembershipPage(body,user,cursor);
  }catch(error){failure=error;throw error;}
  finally {
   clearTimeout(timer);signal?.removeEventListener('abort',cancel);request.signal.removeEventListener('abort',onAbort);
   if(eof){
    // EOF is terminal even if subsequent JSON/provenance validation fails.
    // Do not retroactively abort or cancel a healthy completed transport.
    try{release();}catch{throw unsettled();}
   }else{
    request.abort(failure);
    const cleanup=(async()=>{
     if(fetchWork)await fetchWork.catch(()=>undefined);
     if(!reader&&response?.body)reader=response.body.getReader();
     if(reader)await Promise.allSettled([Promise.resolve().then(()=>reader!.cancel()),...(readWork?[readWork]:[])]);
     release();
    })();
    // Observe late rejection and retain the active slot after the finite grace.
    // Only actual settlement/release can admit a replacement fetch.
    const settled=cleanup.then(()=>true,()=>false);let grace:ReturnType<typeof setTimeout>|undefined;
    try{if(!await Promise.race([settled,new Promise<boolean>(resolve=>{grace=setTimeout(()=>resolve(false),MEMBERSHIP_LIMITS.cleanupMs);})]))throw unsettled();}
    finally{if(grace!==undefined)clearTimeout(grace);}
   }
  }
 };
}
export const readMembershipPage=createMembershipReader();
