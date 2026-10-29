import {createNativeContributionScope,type NativeContributionDependencies} from './native-contribution-scope';
import {createEditorSearch,type EditorChoice} from './editor-fields';
import {PublicUploadRequestError} from './nostr-upload-request';
import {publicUuid,record,PublicApiContractError} from './validation';
export type TeamRequest={id:string;groupId:string;groupName:string;userId:string;username:string;kind:'invitation'|'application';version:number;message:string;expiresAt:string;createdBy:string};
export type OwnGroup={id:string;name:string;role:'leader'|'officer'|'member'};
export type TeamView={group:{id:string;name:string;version:number};members:OwnGroup[];hasMoreMembers:boolean;ownRole:string|null;canManage:boolean;requests:{data:TeamRequest[];hasMore:boolean;limit:100;offset:number}};
export type ChapterCredit={role:'translator'|'editor'|'typesetter';name:string};
function fail(message:string):never{throw new PublicApiContractError(message,502);}
function positive(value:unknown):number{if(typeof value!=='number'||!Number.isSafeInteger(value)||value<1)fail('Invalid current version');return value;}
function textField(value:unknown):string{if(typeof value!=='string')fail('Invalid team request');return value;}
function requestKind(value:unknown):TeamRequest['kind']{if(value!=='invitation'&&value!=='application')fail('Invalid team request kind');return value;}
function memberRole(value:unknown):OwnGroup['role']{if(value!=='leader'&&value!=='officer'&&value!=='member')fail('Invalid group membership');return value;}
export function requestPage(input:unknown){
 const value=record(input),rows=value.data,offset=value.offset??0;
 if(!Array.isArray(rows)||rows.length>100||typeof value.hasMore!=='boolean'||value.limit!==100||typeof offset!=='number'||!Number.isSafeInteger(offset)||offset<0||offset>500||offset%100!==0)fail('Invalid team request page');
 return{data:rows.map((row:unknown):TeamRequest=>{const r=record(row);return{id:publicUuid(r.id),groupId:publicUuid(r.groupId),userId:publicUuid(r.userId),createdBy:publicUuid(r.createdBy),version:positive(r.version),groupName:textField(r.groupName),username:textField(r.username),message:textField(r.message),expiresAt:textField(r.expiresAt),kind:requestKind(r.kind)};}),hasMore:value.hasMore,limit:100 as const,offset};
}
export function ownGroupPage(input:unknown){
 const value=record(input),rows=value.data;
 if(!Array.isArray(rows)||rows.length>100||typeof value.hasMore!=='boolean'||value.limit!==100)fail('Invalid own-group page');
 return{data:rows.map((row:unknown):OwnGroup=>{const r=record(row);return{id:publicUuid(r.id),name:textField(r.name),role:memberRole(r.role)};}),hasMore:value.hasMore};
}
export function creditList(input:unknown):ChapterCredit[]{
 if(!Array.isArray(input)||input.length>30)fail('Use at most 30 chapter credits');
 return input.map((value:unknown):ChapterCredit=>{const r=record(value),role=r.role,name=r.name;
  if(Object.keys(r).some(k=>!['role','name'].includes(k))||(role!=='translator'&&role!=='editor'&&role!=='typesetter')||typeof name!=='string'||!name.trim()||name.length>120||/[\u0000-\u001f\u007f-\u009f]/.test(name))fail('Each credit needs a translator, editor or typesetter role and a name (1–120 characters).');
  return{role,name:name.trim()};
 });
}
export async function createNativeTeamController(nativeAccount:string,assertCurrent:()=>void,injected:NativeContributionDependencies={}){
 const scope=await createNativeContributionScope(nativeAccount,assertCurrent,injected);let busy=false;
 const key=(target:string)=>scope.key('team-command')+':'+scope.catalogNamespace+':'+target;
 async function request(path:string,body?:Record<string,unknown>){scope.check();const r=await scope.request({url:scope.origin+'/api/v1/account'+path,method:body?'POST':'GET',...(body?{body:new TextEncoder().encode(JSON.stringify({...body,expectedAccountId:scope.account.id})),contentType:'application/json'}:{})});scope.check();if(r.status!==200){const v=record(r.body);throw new PublicApiContractError(Array.isArray(v.errors)&&typeof v.errors[0]?.detail==='string'?v.errors[0].detail:`Team request refused (HTTP ${r.status})`,r.status);}return record(r.body);}
 async function mutate(target:string,path:string,payload:Record<string,unknown>,expected?:{id:string;version:number;status:string}){scope.check();if(busy||scope.fences.get(key(target))!==null)throw new PublicApiContractError('A team change is in progress or unconfirmed. Check the team before trying another change.',409);busy=true;let fenced=false;const marker=JSON.stringify({path,kind:payload.kind??null,action:payload.action??null,version:payload.version,recipient:payload.recipient??null});try{scope.fences.set(key(target),marker);fenced=true;if(scope.fences.get(key(target))!==marker)throw new PublicUploadRequestError('Could not protect this team change','unsent');const value=await request(path,payload);if(!['pending','accepted','declined','revoked','cancelled'].includes(String(value.status))||value.role!=='member')fail('Team decision receipt was not confirmed');publicUuid(value.id);positive(value.version);if(expected?(value.id!==expected.id||value.version!==expected.version||value.status!==expected.status):(value.version!==1||value.status!=='pending'))fail('Team receipt changed the requested action');scope.fences.delete(key(target));fenced=false;return value;}catch(e){if(fenced&&(e instanceof PublicUploadRequestError&&e.outcome==='unsent'||e instanceof PublicApiContractError&&e.status>=400&&e.status<500))scope.fences.delete(key(target));throw e;}finally{busy=false;}}
 const search=createEditorSearch(async({path,signal})=>scope.request({url:scope.origin+'/api/v1/account'+path,method:'GET',signal}),scope.check);
 return{account:scope.account,check:scope.check,dispose:scope.dispose,pending:(target:string)=>scope.fences.get(key(target))!==null,
  searchUsers:(query:string,signal?:AbortSignal):Promise<EditorChoice[]>=>search('user',query,signal),
  ownGroups:async()=>ownGroupPage(await request('/groups')),
  inbox:async()=>requestPage(await request('/group-requests')),
  team:async(id:string,offset=0):Promise<TeamView>=>{id=publicUuid(id);if(!Number.isSafeInteger(offset)||offset<0||offset>500||offset%100!==0)fail('Invalid request offset');const r=await request(`/group/${id}/team${offset?'?offset='+offset:''}`),g=record(r.group);if(publicUuid(g.id)!==id||typeof g.name!=='string'||typeof r.canManage!=='boolean'||!Array.isArray(r.members)||r.members.length>100||typeof r.hasMoreMembers!=='boolean'||r.ownRole!==null&&!['leader','officer','member'].includes(String(r.ownRole)))fail('Invalid team details');const members=ownGroupPage({data:r.members,hasMore:r.hasMoreMembers,limit:100}).data;return{group:{id,name:g.name as string,version:positive(g.version)},members,hasMoreMembers:r.hasMoreMembers as boolean,ownRole:r.ownRole as string|null,canManage:r.canManage as boolean,requests:requestPage(r.requests)};},
  invite:(id:string,version:number,recipient:string,message:string)=>mutate(publicUuid(id),`/group/${publicUuid(id)}/requests`,{kind:'invitation',version:positive(version),recipient:publicUuid(recipient),message}),
  apply:(id:string,version:number,message:string)=>mutate(publicUuid(id),`/group/${publicUuid(id)}/requests`,{kind:'application',version:positive(version),message}),
  decide:(item:TeamRequest,action:'accept'|'decline'|'revoke'|'cancel')=>mutate(publicUuid(item.groupId),`/group/${publicUuid(item.groupId)}/requests/${publicUuid(item.id)}/decision`,{action,version:positive(item.version)},{id:publicUuid(item.id),version:positive(item.version)+1,status:({accept:'accepted',decline:'declined',revoke:'revoked',cancel:'cancelled'} as const)[action]}),
 };
}
export type NativeTeamController=Awaited<ReturnType<typeof createNativeTeamController>>;
