// Private preview reads never enter public catalogue, reader or library transports.
import {createNativeContributionScope,type NativeContributionDependencies} from './native-contribution-scope';
import {nativeAccountError} from './native-account-scope';
import {normalizeManga} from './normalize';
import {reviewTimeline} from './draft-feedback';
import {publicUuid,record,resource,PublicApiContractError} from './validation';
export async function createSessionDraftPreview(nativeAccount:string,target:string,assertCurrent:()=>void,injected:NativeContributionDependencies={}){
 const id=publicUuid(target),scope=await createNativeContributionScope(nativeAccount,assertCurrent,injected);
 let reading=false;
 async function read(){
  scope.check();if(reading)throw new PublicApiContractError('A draft preview read is already running',409);reading=true;
  try{
   const query=new URLSearchParams([['includes[]','cover_art'],['includes[]','author'],['includes[]','artist'],['includes[]','creator']]).toString().replace(/^\?/,'');
   const response=await scope.request({url:`${scope.origin}/api/v1/account/manga/draft/${id}?${query}`,method:'GET'});scope.check();
   if(response.status!==200)throw nativeAccountError(response.status,response.body);
   const body=record(response.body),row=resource(body.data,'manga'),a=row.attributes;
   if(body.result!=='ok'||body.response!=='entity'||row.id!==id||!['draft','submitted','rejected'].includes(String(a.state))||!Number.isSafeInteger(a.version)||Number(a.version)<1)throw new PublicApiContractError('The private draft changed. Reload its preview.',409);
   const titles=record(a.title,'draft title');
   if(!Object.values(titles).some(value=>typeof value==='string'&&value.trim())||typeof a.originalLanguage!=='string'||!a.originalLanguage)throw new PublicApiContractError('The private draft metadata could not be verified.',502);
   const reviewer=scope.account.roles.some(role=>['moderator','staff','sysop'].includes(role))&&scope.account.permissions.includes('manga.state.update');
   const owner=scope.account.permissions.includes('manga.create')&&row.relationships.some(r=>r.type==='creator'&&r.id===scope.account.id);
   if(!reviewer&&!owner)throw nativeAccountError(403);
   reviewTimeline(a.reviewTimeline);
   return normalizeManga(row);
  }finally{reading=false;}
 }
 return{account:scope.account,check:scope.check,read,dispose:scope.dispose};
}
