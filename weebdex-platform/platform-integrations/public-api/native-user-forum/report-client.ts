// Native moderation only. This controller shares the already captured discussion
// account scope; source-event reports and browser-cookie authority are unrelated.
import {hash} from '../../local-library/protocol/codec.js';
import type {createSignedAccountScope} from '../signed-account-scope';
import {PublicUploadRequestError} from '../nostr-upload-request';
import {publicUuid,record,PublicApiContractError} from '../validation';
export type NativeReportTarget={type:'thread'}|{type:'reply';replyId:number;version:number};
export interface NativeReportReason {id:string;version:number;reason:string;detailsRequired:boolean;}
export interface NativeReportContext {target:NativeReportTarget;reasons:NativeReportReason[];}
type Scope=Awaited<ReturnType<typeof createSignedAccountScope>>;
function fail(message:string,status=502):never{throw new PublicApiContractError(message,status);}
function nativeId(value:unknown):string{if(typeof value!=='string'||!value||value.length>128||/[\\/\u0000-\u0020]/.test(value))fail('Invalid native report identifier');return value;}
function positive(value:unknown):number{if(!Number.isSafeInteger(value)||Number(value)<1)fail('Invalid native report target or reason version');return value as number;}
function target(input:unknown):NativeReportTarget{const value=record(input);if(value.type==='thread'&&Object.keys(value).length===1)return{type:'thread'};if(value.type==='reply'&&Object.keys(value).length===3)return{type:'reply',replyId:positive(value.replyId),version:positive(value.version)};return fail('Choose an actual native thread or reply',422);}
export function nativeReportDetails(value:unknown):string{if(typeof value!=='string'||value.length>40000||[...value].length>20000||value.includes('\0'))fail('Report details exceed the native text limit',422);const text=value.replace(/^\p{White_Space}+|\p{White_Space}+$/gu,'');if(new TextEncoder().encode(text).byteLength>20000)fail('Report details exceed 20,000 UTF-8 bytes',422);return text;}
/** One sealed native moderation controller with disjoint typed target scopes. */
export function createNativeDiscussionReports(kind:'user'|'manga'|'chapter'|'group',scope:Scope,userId:string,threadId:string,discussion:()=>object,validateTarget:(value:NativeReportTarget)=>void){
 if(!['user','manga','chapter','group'].includes(kind))fail('Invalid native report resource kind',422);
 const path='/nostr/'+kind+'/'+publicUuid(userId)+'/discussion/reports',key=scope.key(kind==='user'?'native-user-report-pending':'native-discussion-report-pending')+':'+(kind==='user'?'':kind+':')+userId+':'+nativeId(threadId);
 let busy=false,current:{view:NativeReportContext;discussion:object}|undefined;const reviewed=new WeakMap<object,NonNullable<typeof current>>();
 const pending=()=>scope.fences.get(key)!==null;
 const check=(expected?:object)=>{scope.check();const actual=discussion();if(expected&&expected!==actual)fail('The reviewed native discussion changed',409);return actual;};
 function provenance(input:unknown){const body=record(input),meta=record(body.nostr);if(body.result!=='ok'||meta.scope!=='instance-account'||meta.source!==(kind==='user'?'persisted-native-user-report':'persisted-native-discussion-report')||meta.identity!==(kind==='user'?'native_public_user':'native_catalogue_resource')||publicUuid(meta.accountId)!==scope.account.id||publicUuid(kind==='user'?meta.userId:meta.targetId)!==userId||kind!=='user'&&meta.targetKind!==kind||meta.threadId!==threadId)fail('The native report account, profile or thread changed',409);return{body,meta};}
 async function read(input:NativeReportTarget):Promise<NativeReportContext>{const captured=check(),chosen=target(input);validateTarget(chosen);if(busy)fail('A native report operation is already running',409);busy=true;current=undefined;try{
  const query=new URLSearchParams({expectedAccountId:scope.account.id,expectedThreadId:threadId,targetType:chosen.type,...(chosen.type==='reply'?{replyId:String(chosen.replyId),expectedReplyVersion:String(chosen.version)}:{})});
  const response=await scope.send(path+'/context?'+query);check(captured);if(response.status!==200)fail('Native report reasons unavailable (HTTP '+response.status+')',response.status);
  const {body}=provenance(response.body),data=record(body.data),actual=record(data.target);
  if(actual.threadId!==threadId)fail('Native report target thread changed',409);const returned=target(Object.fromEntries(Object.entries(actual).filter(([field])=>field!=='threadId')));if(JSON.stringify(returned)!==JSON.stringify(chosen))fail('Native report reply or version changed',409);
  if(!Array.isArray(data.reasons)||data.reasons.length>1000)fail('Invalid native report reasons');const seen=new Set<string>(),reasons:NativeReportReason[]=[];
  for(const raw of data.reasons){const value=record(raw),id=nativeId(value.id);if(seen.has(id)||typeof value.reason!=='string'||!value.reason||value.reason.length>8000||new TextEncoder().encode(value.reason).byteLength>8000||typeof value.detailsRequired!=='boolean')fail('Invalid actual native report reason');seen.add(id);reasons.push({id,version:positive(value.version),reason:value.reason,detailsRequired:value.detailsRequired});}
  await scope.verify();check(captured);validateTarget(chosen);current={discussion:captured,view:{target:chosen,reasons}};return JSON.parse(JSON.stringify(current.view)) as NativeReportContext;
 }finally{busy=false;}}
 function plan(reasonId:string,details:unknown){check(current?.discussion);if(busy||pending()||!current)fail('Reload native report reasons before reviewing. Unconfirmed reports are never repeated.',409);const reason=current.view.reasons.find(row=>row.id===reasonId);if(!reason)fail('Choose an actual enabled native report reason',422);const body=nativeReportDetails(details);if(reason.detailsRequired&&!body)fail('This native report reason requires details',422);const chosen=current.view.target;validateTarget(chosen);
  const payload:Record<string,unknown>={expectedAccountId:scope.account.id,[kind==='user'?'expectedUserId':'expectedTargetId']:userId,expectedThreadId:threadId,targetType:chosen.type,...(chosen.type==='reply'?{replyId:chosen.replyId,expectedReplyVersion:chosen.version}:{}),reasonId:reason.id,expectedReasonVersion:reason.version,details:body};Object.freeze(payload);reviewed.set(payload,current);return payload;
 }
 async function submit(payload:Record<string,unknown>){check(current?.discussion);if(busy||pending()||!current||reviewed.get(payload)!==current)fail('Review the exact current native report before sending',409);const captured=current;reviewed.delete(payload);busy=true;let installed=false,received=false;try{
  const marker=JSON.stringify({targetType:payload.targetType,replyId:payload.replyId,replyVersion:payload.expectedReplyVersion,reasonId:payload.reasonId,reasonVersion:payload.expectedReasonVersion});scope.fences.set(key,marker);installed=true;if(scope.fences.get(key)!==marker)fail('Native report pending safeguard could not be saved',409);
  const response=await scope.send(path,'POST',payload);received=true;check(captured.discussion);
  if(response.status>=400&&response.status<500){scope.fences.delete(key);installed=false;fail('Native report refused (HTTP '+response.status+'). Reload before a new review.',response.status);}
  if(response.status!==200)fail('Native report outcome is unknown. It was not repeated.');const {body,meta}=provenance(response.body);
  if(meta.requestMethod!=='POST'||meta.requestPath!=='/api/v1'+path||meta.requestHash!==hash(new TextEncoder().encode(JSON.stringify(payload))))fail('Native report receipt does not confirm the exact reviewed request');
  const data=record(body.data),report=record(data.report);if(Object.keys(data).length!==1||Object.keys(report).length!==2||report.status!=='waiting')fail('Invalid private native report receipt');const reportId=nativeId(report.id);
  await scope.verify();check(captured.discussion);scope.fences.delete(key);installed=false;return{id:reportId,status:'waiting' as const};
 }catch(error){if(installed&&!received&&error instanceof PublicUploadRequestError&&error.outcome==='unsent')scope.fences.delete(key);throw error;}finally{current=undefined;busy=false;}}
 return{read,plan,submit,pending,running:()=>busy,clear:()=>{current=undefined;}};
}

/** Preserve the existing user-only call surface, wire metadata and fence key. */
export function createNativeUserReports(scope:Scope,userId:string,threadId:string,discussion:()=>object,validateTarget:(value:NativeReportTarget)=>void){
 return createNativeDiscussionReports('user',scope,userId,threadId,discussion,validateTarget);
}
