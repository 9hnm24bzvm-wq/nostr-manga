// Explicit read-only inspection. No report write, reconciliation guess, or fence
// deletion belongs here: a similar row (or empty page) proves no request outcome.
import { createSignedAccountScope } from './signed-account-scope';
import { reportCategories, type ReportCategory } from './report-client';
import { publicUuid, record, PublicApiContractError } from './validation';
import type { TitleDependencies } from './title-client';
export const reportStatuses=['waiting','accepted','refused','autoresolved'] as const;
export type ReportStatus=typeof reportStatuses[number];
// Native-created catalogue reports can contain 20,000 Unicode scalar values.
// Ten rows fit the existing 1 MiB signed response budget even at four bytes each.
export const reportPageSize=10,reportMaxPage=1001;
export interface InspectedReport {id:string;objectId:string;category:ReportCategory;status:ReportStatus;createdAt:string;details:string;reasonId:string;reason:string;reasonVersion:number;}
export interface InspectedReportPage {data:InspectedReport[];total:number;page:number;limit:number;offset:number;}
function fail(message:string,status=502):never{throw new PublicApiContractError(message,status);}
export function reportInspectionQuery(input:URLSearchParams,accountId?:string):URLSearchParams {
 const allowed=['status','category','objectId','page'];
 for(const key of input.keys())if(!allowed.includes(key)||input.getAll(key).length!==1)fail('Unsupported or repeated report inspection filter',422);
 const status=input.get('status')??'',category=input.get('category')??'',target=input.get('objectId'),page=input.get('page')??'1';
 if(status&&!reportStatuses.includes(status as ReportStatus)||category&&!reportCategories.includes(category as ReportCategory)||!/^[1-9]\d*$/.test(page)||!Number.isSafeInteger(Number(page))||Number(page)>reportMaxPage)fail('Invalid report status, category or bounded page',422);
 const query=new URLSearchParams([['scope','mine'],['limit',String(reportPageSize)],['offset',String((Number(page)-1)*reportPageSize)],['order[createdAt]','desc'],['includes[]','reason']]);
 if(accountId!==undefined)query.set('expectedAccountId',publicUuid(accountId));
 if(status)query.set('status',status);if(category)query.set('category',category);if(target!==null)query.set('objectId',publicUuid(target));return query;
}
export function reportInspectionHref(category:unknown,target:unknown):string {
 if(!reportCategories.includes(category as ReportCategory))fail('Native forum reports use the native report view',422);
 return '/reports/signed?'+new URLSearchParams({category:category as string,objectId:publicUuid(target)}).toString().replace(/^\?/,'');
}
function keys(value:unknown,expected:string[],label:string):Record<string,unknown>{const row=record(value,label);if(Object.keys(row).some(key=>!expected.includes(key))||expected.some(key=>!Object.hasOwn(row,key)))fail('Invalid '+label+' fields');return row;}
function decode(bodyInput:unknown,query:URLSearchParams,accountId:string):InspectedReportPage {
 const body=keys(bodyInput,['result','response','data','total','limit','offset'],'report collection'),offset=Number(query.get('offset'));
 if(body.result!=='ok'||body.response!=='collection'||!Array.isArray(body.data)||body.limit!==reportPageSize||body.offset!==offset||!Number.isSafeInteger(body.total)||Number(body.total)<0||body.data.length!==Math.min(reportPageSize,Math.max(0,Number(body.total)-offset)))fail('Invalid bounded personal report page');
 const data=body.data.map(value=>{
  const row=keys(value,['id','type','attributes','relationships'],'report'),attrs=keys(row.attributes,['details','objectId','status','createdAt'],'report attributes');
  if(row.type!=='report'||!Array.isArray(row.relationships)||row.relationships.length!==2)fail('Invalid personal report relationships');
  const user=row.relationships.find(value=>record(value).type==='user'),reason=row.relationships.find(value=>record(value).type==='report_reason');
  const submitter=keys(user,['id','type'],'report submitter'),reasonRow=keys(reason,['id','type','attributes'],'report reason relationship');
  if(publicUuid(submitter.id)!==accountId)fail('Report page escaped the captured personal account');
  const reasonAttrs=keys(reasonRow.attributes,['reason','detailsRequired','category','version'],'report reason attributes'),localized=keys(reasonAttrs.reason,['en'],'report reason label');
  if(!reportCategories.includes(reasonAttrs.category as ReportCategory)||typeof reasonAttrs.detailsRequired!=='boolean'||!Number.isSafeInteger(reasonAttrs.version)||Number(reasonAttrs.version)<1||typeof localized.en!=='string'||!localized.en.trim()||localized.en.length>1000)fail('Invalid report category or reason');
  if(!reportStatuses.includes(attrs.status as ReportStatus)||typeof attrs.details!=='string'||attrs.details.includes('\0')||[...attrs.details].length>20000||typeof attrs.createdAt!=='string'||!/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d{1,9})?(?:Z|[+-]\d{2}:\d{2})$/.test(attrs.createdAt)||!Number.isFinite(Date.parse(attrs.createdAt))||new Date(attrs.createdAt.slice(0,10)+'T00:00:00Z').toISOString().slice(0,10)!==attrs.createdAt.slice(0,10))fail('Invalid report status, details or creation time');
  const objectId=publicUuid(attrs.objectId);
  if(query.has('category')&&query.get('category')!==reasonAttrs.category||query.has('status')&&query.get('status')!==attrs.status||query.has('objectId')&&query.get('objectId')!==objectId)fail('Report page escaped the selected filters');
  return{id:publicUuid(row.id),objectId,category:reasonAttrs.category as ReportCategory,status:attrs.status as ReportStatus,createdAt:attrs.createdAt,details:attrs.details,reasonId:publicUuid(reasonRow.id),reason:localized.en,reasonVersion:Number(reasonAttrs.version)};
 });
 if(new Set(data.map(row=>row.id)).size!==data.length||data.some((row,index)=>index>0&&Date.parse(row.createdAt)>Date.parse(data[index-1].createdAt)))fail('Invalid report identity or ordering');
 return{data,total:Number(body.total),page:offset/reportPageSize+1,limit:reportPageSize,offset};
}
export async function createSignedReportReader(assertCurrent:()=>void,injected:Partial<TitleDependencies>={},signal?:AbortSignal){
 const scope=await createSignedAccountScope(assertCurrent,injected,signal);let busy=false;
 async function read(input:URLSearchParams):Promise<InspectedReportPage>{scope.check();if(busy)fail('A report inspection is already running',409);const query=reportInspectionQuery(input,scope.account.id);busy=true;
  try{const response=await scope.send('/report?'+query.toString().replace(/^\?/,''));if(response.status!==200)fail(`Personal reports unavailable (HTTP ${response.status}). No native-session fallback was used.`,response.status);
   // Revalidate even an empty response; owner validation on rows alone cannot
   // detect a binding or service change during a private read.
   await scope.verify();const result=decode(response.body,query,scope.account.id);scope.check();return result;
  }finally{busy=false;}
 }
 return{account:scope.account,check:scope.check,dispose:scope.dispose,read};
}
