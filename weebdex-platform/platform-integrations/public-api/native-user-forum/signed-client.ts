// Signed native-account operations on one actual persisted user discussion.
// Existing source-event comments and native cookie routes are never substituted.
import {hash} from '../../local-library/protocol/codec.js';
import {createSignedAccountScope} from '../signed-account-scope';
import {PublicUploadRequestError} from '../nostr-upload-request';
import type {TitleDependencies} from '../title-client';
import {publicUuid,record,PublicApiContractError} from '../validation';
import {normalizeDiscussionPage,type PublicDiscussionPage} from './data';
import {createNativeDiscussionReports} from './report-client';
export type DiscussionOperation='create'|'update'|'remove'|'delete'|'thread_vote'|'reply_vote';
export interface DiscussionIntent {operation:DiscussionOperation;replyId?:number;parentId?:number;body?:string;vote?:-1|0|1;}
export interface ReplyActions {update:boolean;remove:boolean;delete:boolean;vote:boolean;}
export interface SignedDiscussionView {feed:PublicDiscussionPage;thread:Record<string,unknown>;threadVote:-1|0|1;actions:{create:boolean;threadVote:boolean;replies:Record<string,ReplyActions>};}
function fail(message:string,status=502):never{throw new PublicApiContractError(message,status);}
const operations:DiscussionOperation[]=['create','update','remove','delete','thread_vote','reply_vote'];
export function discussionBody(value:unknown):string{if(typeof value!=='string'||[...value].length>50000||value.includes('\0'))fail('Comment body must contain at most 50,000 characters',422);const text=value.replace(/^\p{White_Space}+|\p{White_Space}+$/gu,'');if(!text)fail('Comment body cannot be empty',422);if(new TextEncoder().encode(text).byteLength>50000)fail('Comment body exceeds the native 50,000-byte storage limit',422);return text;}
export type NativeDiscussionKind='manga'|'chapter'|'group';
/** Shared exact account, review capability, CAS and receipt fence. */
export async function createSignedDiscussion(kind:'user'|NativeDiscussionKind,userInput:string,threadInput:string,assertCurrent=()=>{},injected:Partial<TitleDependencies>={},signal?:AbortSignal){
 if(!['user','manga','chapter','group'].includes(kind))fail('Invalid native discussion target kind',422);
 const userId=publicUuid(userInput),threadId=threadInput;
 if(typeof threadId!=='string'||!threadId||threadId.length>128||/[\\/\u0000-\u0020]/.test(threadId))fail('Invalid actual discussion thread',422);
 const scope=await createSignedAccountScope(assertCurrent,injected,signal),key=scope.key(kind==='user'?'native-user-discussion-pending':'native-discussion-pending')+':'+(kind==='user'?'':kind+':')+userId+':'+threadId,path='/nostr/'+kind+'/'+userId+'/discussion';
 let current:SignedDiscussionView|undefined,busy=false;const reviewed=new WeakMap<object,SignedDiscussionView>();
 const pending=()=>scope.fences.get(key)!==null;
 const reports=createNativeDiscussionReports(kind,scope,userId,threadId,()=>{if(kind!=='user'&&!scope.account.permissions.includes('report.create'))fail('This selected account cannot create native reports',403);if(busy||!current||kind!=='user'&&pending())fail('Reload the current native discussion before reporting',409);return current;},target=>{if(target.type==='reply'&&!current?.feed.data.some(reply=>reply.id===target.replyId&&reply.version===target.version&&reply.state==='active'))fail('Choose an actual active reply from this discussion page',409);});
 function provenance(input:unknown){const body=record(input),meta=record(body.nostr);if(body.result!=='ok'||meta.scope!=='instance-account'||meta.source!==(kind==='user'?'persisted-native-user-discussion':'persisted-native-discussion')||meta.identity!==(kind==='user'?'native_public_user':'native_catalogue_resource')||publicUuid(meta.accountId)!==scope.account.id||publicUuid(kind==='user'?meta.userId:meta.targetId)!==userId||kind!=='user'&&meta.targetKind!==kind||meta.threadId!==threadId)fail('The discussion account, profile or stored thread changed',409);return{body,meta};}
 async function read(page=1,order:'asc'|'desc'='desc'):Promise<SignedDiscussionView>{scope.check();if(busy||reports?.running())fail('A discussion operation is already running',409);if(!Number.isSafeInteger(page)||page<1||page>1000000||!['asc','desc'].includes(order))fail('Invalid discussion page or order',422);busy=true;current=undefined;try{
  const query=new URLSearchParams({expectedAccountId:scope.account.id,expectedThreadId:threadId,page:String(page),order}).toString().replace(/^\?/,'');
  const response=await scope.send(path+'?'+query);scope.check();if(response.status!==200)fail('Signed discussion unavailable (HTTP '+response.status+')',response.status);
  const {body}=provenance(response.body),data=record(body.data),thread=record(data.thread),actions=record(data.actions),replyActions=record(actions.replies),feed=normalizeDiscussionPage(data.feed,page,true);
  if(thread.id!==threadId||typeof thread.locked!=='boolean'||![-1,0,1].includes(data.threadVote as number)||typeof actions.create!=='boolean'||typeof actions.threadVote!=='boolean')fail('Invalid current signed discussion context');
  const parsed:Record<string,ReplyActions>={};if(Object.keys(replyActions).length!==feed.data.length)fail('Discussion permissions escaped their exact reply page');
  for(const reply of feed.data){const row=record(replyActions[String(reply.id)]);for(const field of ['update','remove','delete','vote'])if(typeof row[field]!=='boolean')fail('Invalid actual reply permission');parsed[String(reply.id)]={update:row.update as boolean,remove:row.remove as boolean,delete:row.delete as boolean,vote:row.vote as boolean};}
  await scope.verify();scope.check();current={thread:{id:threadId,locked:thread.locked},feed,threadVote:data.threadVote as -1|0|1,actions:{create:actions.create as boolean,threadVote:actions.threadVote as boolean,replies:parsed}};return JSON.parse(JSON.stringify(current)) as SignedDiscussionView;
 }finally{busy=false;}}
 function plan(input:DiscussionIntent){scope.check();if(busy||reports?.running()||kind!=='user'&&reports.pending()||pending()||!current)fail('Reload the current discussion before changing it. Unconfirmed actions are never replayed.',409);if(!input||!operations.includes(input.operation)||Object.keys(input).some(key=>!['operation','replyId','parentId','body','vote'].includes(key)))fail('Invalid discussion action',422);
  const {operation}=input,payload:Record<string,unknown>={expectedAccountId:scope.account.id,[kind==='user'?'expectedUserId':'expectedTargetId']:userId,expectedThreadId:threadId,operation};
  const reply=input.replyId===undefined?undefined:current.feed.data.find(row=>row.id===input.replyId);
  const requiresReply=['update','remove','delete','reply_vote'].includes(operation);
  if(requiresReply&&!reply||!requiresReply&&input.replyId!==undefined)fail('Choose an actual current reply for this operation',422);
  if(reply){const action=operation==='reply_vote'?'vote':operation;if(!current.actions.replies[String(reply.id)]?.[action as keyof ReplyActions])fail('This account cannot perform that reply action',403);payload.replyId=reply.id;payload.version=reply.version;}
  if(operation==='create'&&!current.actions.create||operation==='thread_vote'&&!current.actions.threadVote)fail('This account cannot perform that discussion action',403);
  if(['create','update'].includes(operation))payload.body=discussionBody(input.body);else if(input.body!==undefined)fail('Unexpected body for discussion action',422);
  if(input.parentId!==undefined){if(operation!=='create'||!current.feed.data.some(row=>row.id===input.parentId&&row.state==='active'))fail('The parent must be an active reply on this discussion page',422);payload.parentId=input.parentId;}
  if(operation==='thread_vote'||operation==='reply_vote'){if(![-1,0,1].includes(input.vote as number))fail('Invalid discussion vote',422);if(input.vote!==0&&(current.thread.locked||reply&&reply.state!=='active'))fail('Only clearing an existing vote is available on this locked or inactive discussion',403);payload.vote=input.vote;payload.expectedVote=reply?reply.vote??0:current.threadVote;}else if(input.vote!==undefined)fail('Unexpected vote for discussion action',422);
  Object.freeze(payload);reviewed.set(payload,current);return payload;
 }
 async function mutate(payload:Record<string,unknown>){scope.check();if(busy||reports?.running()||kind!=='user'&&reports.pending()||pending()||!current||reviewed.get(payload)!==current)fail('Review the current exact discussion action before sending',409);reviewed.delete(payload);busy=true;let installed=false,received=false;try{
  const marker=JSON.stringify({operation:payload.operation,replyId:payload.replyId,version:payload.version});scope.fences.set(key,marker);installed=true;if(scope.fences.get(key)!==marker)fail('Discussion pending safeguard could not be saved',409);
  const response=await scope.send(path,'POST',payload);received=true;scope.check();
  if(response.status>=400&&response.status<500){scope.fences.delete(key);installed=false;fail('Discussion action refused (HTTP '+response.status+'). Reload before trying a new action.',response.status);}
  if(response.status!==200)fail('Discussion outcome is unknown. Inspect the discussion; the action was not replayed.');
  const {body,meta}=provenance(response.body);if(meta.operation!==payload.operation||typeof meta.changed!=='boolean'||meta.requestMethod!=='POST'||meta.requestPath!=='/api/v1'+path||meta.requestHash!==hash(new TextEncoder().encode(JSON.stringify(payload))))fail('Discussion receipt does not confirm the exact reviewed command');
  const data=record(body.data);
  if(['create','update','remove','reply_vote'].includes(String(payload.operation))){
   const reply=normalizeDiscussionPage({total:1,limit:20,page:1,data:[data.reply]},1,true).data[0];
   if(payload.operation==='create'&&(reply.version!==1||reply.state!=='active'||reply.body!==payload.body||!meta.changed)||payload.operation!=='create'&&reply.id!==payload.replyId)fail('Discussion receipt returned a different reply');
   if(payload.operation==='update'&&(reply.version!==Number(payload.version)+1||reply.body!==payload.body||reply.state!=='active'||!meta.changed))fail('Discussion edit receipt does not match its reviewed revision/body');
   if(payload.operation==='remove'&&(!['deleted_by_user','deleted_by_moderator','deleted_by_system'].includes(reply.state)||reply.body!==undefined||reply.version!==Number(payload.version)+(meta.changed?1:0)))fail('Discussion removal receipt is inconsistent');
   if(payload.operation==='reply_vote'&&(reply.version!==payload.version||reply.vote!==payload.vote))fail('Discussion vote receipt does not match its reviewed reply/version/vote');
  }else if(Object.keys(data).length||payload.operation==='delete'&&!meta.changed)fail('Discussion receipt shape is inconsistent');
  await scope.verify();scope.check();
  scope.fences.delete(key);installed=false;return{changed:meta.changed};
 }catch(error){if(installed&&!received&&error instanceof PublicUploadRequestError&&error.outcome==='unsent')scope.fences.delete(key);throw error;}finally{current=undefined;busy=false;}}
 return{account:scope.account,check:scope.check,dispose:scope.dispose,pending,read,plan,mutate,reports};
}

/** Preserve the public native-user surface and its separately scoped reports. */
export async function createSignedUserDiscussion(userInput:string,threadInput:string,assertCurrent=()=>{},injected:Partial<TitleDependencies>={},signal?:AbortSignal){
 const client=await createSignedDiscussion('user',userInput,threadInput,assertCurrent,injected,signal);
 return {...client,reports:client.reports!};
}
