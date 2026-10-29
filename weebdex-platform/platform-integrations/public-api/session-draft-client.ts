import { createNativeContributionScope, type NativeContributionDependencies } from './native-contribution-scope';
import { createSessionTitleController } from './session-title-client';
import type { TitleAccount } from './title-client';
import { normalizeManga } from './normalize';
import { publicUuid, record, resource, PublicApiContractError } from './validation';
import type { MangaView, ViewFeed } from './types';
function fail(message:string,status=502):never{throw new PublicApiContractError(message,status);}
export function draftQuery(input:URLSearchParams,account:string):URLSearchParams {
 const allowed=['state','sort','order','page'];for(const key of input.keys())if(!allowed.includes(key)||input.getAll(key).length!==1)fail('Unsupported or repeated draft filter',422);
 const state=input.get('state')??'',sort=input.get('sort')??'created_at',direction=input.get('order')??'desc',page=input.get('page')??'1';
 if(!['','draft','submitted','rejected'].includes(state)||!['asc','desc'].includes(direction)||!/^\d+$/.test(page)||!Number.isSafeInteger(Number(page))||Number(page)<1||Number(page)>278)fail('Invalid draft filter or page',422);
 const sortMap:Record<string,string>={created_at:'createdAt',updated_at:'updatedAt',title:'title',year:'year'};if(!sortMap[sort])fail('Unsupported draft sort',422);
 return new URLSearchParams([['limit','36'],['offset',String((Number(page)-1)*36)],['order['+sortMap[sort]+']',direction],['includes[]','cover_art'],['includes[]','creator'],['expectedAccountId',publicUuid(account)],...(state?[['state',state]]:[])]);
}
export function draftGrants(account:TitleAccount,title:MangaView){const moderator=account.roles.some(role=>['moderator','staff','sysop'].includes(role)),owner=title.public_api.resource.relationships.some(row=>row.type==='creator'&&publicUuid(row.id)===account.id),create=account.permissions.includes('manga.create');return{edit:owner&&create&&!title.locked||moderator&&(account.permissions.includes('manga.update')||account.permissions.includes('manga.state.update')),submit:create&&(owner||moderator)&&(!title.locked||moderator),review:moderator&&account.permissions.includes('manga.state.update'),remove:owner&&create||moderator&&account.permissions.includes('manga.delete'),hide:moderator&&account.permissions.includes('manga.delete')};}
export async function createSessionDraftReader(nativeAccount:string,assertCurrent:()=>void,injected:NativeContributionDependencies={}){
 const scope=await createNativeContributionScope(nativeAccount,assertCurrent,injected);let busy=false;
 async function read(input:URLSearchParams):Promise<ViewFeed<MangaView>>{scope.check();if(busy)fail('A draft read is already running',409);const query=draftQuery(input,scope.account.id),offset=Number(query.get('offset'));busy=true;try{const response=await scope.request({url:scope.origin+'/api/v1/account/manga/draft?'+query.toString().replace(/^\?/,''),method:'GET'});if(response.status!==200)fail(`Drafts unavailable (HTTP ${response.status})`,response.status);const body=record(response.body);if(body.result!=='ok'||body.response!=='collection'||!Array.isArray(body.data)||body.data.length>36||body.limit!==36||body.offset!==offset||!Number.isSafeInteger(body.total)||Number(body.total)<0||body.data.length>Number(body.total))fail('Invalid bounded private draft page');const data=body.data.map(value=>{const row=resource(value,'manga');if(!Number.isSafeInteger(row.attributes.version)||Number(row.attributes.version)<1||!['draft','submitted','rejected'].includes(String(row.attributes.state))||(query.has('state')&&row.attributes.state!==query.get('state')))fail('Draft page changed its state/version filter');const view=normalizeManga(row);const moderator=scope.account.roles.some(role=>['moderator','staff','sysop'].includes(role));if(!(moderator&&scope.account.permissions.includes('manga.state.update'))&&!row.relationships.some(relation=>relation.type==='creator'&&publicUuid(relation.id)===scope.account.id))fail('Draft page escaped the captured account');return view;});if(new Set(data.map(row=>row.id)).size!==data.length)fail('Duplicate draft identity');return{data,total:Number(body.total),limit:36,offset,page:offset/36+1};}finally{busy=false;}}
 return{account:scope.account,check:scope.check,dispose:scope.dispose,read};
}
// A card click captures its visible revision and account. A later read may not
// silently upgrade that intent to a newer version or another canonical target.
export async function openSessionDraftCommand(nativeAccount:string,title:MangaView,account:TitleAccount,assertCurrent:()=>void,injected:NativeContributionDependencies={}){
 const target=publicUuid(title.id),version=title.version,state=title.state,accountId=publicUuid(account.id);
 const controller=await createSessionTitleController(nativeAccount,assertCurrent,target,injected);
 try{controller.assert();if(controller.account.id!==accountId)fail('The draft account changed. Reload the list.',409);const read=await controller.read();controller.assert();if(read.status!==200||!read.body||read.body.id!==target||read.body.version!==version||read.body.state!==state)fail('This draft changed. Reload the list before acting.',409);return controller;}catch(error){controller.dispose();throw error;}
}
