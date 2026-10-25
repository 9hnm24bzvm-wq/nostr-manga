// Instance-local moderation reports, authenticated by a request-scoped signer.
import { createSignedAccountScope } from './signed-account-scope';
import { PublicUploadRequestError } from './nostr-upload-request';
import { publicUuid, uuidPattern, record, PublicApiContractError } from './validation';
import type { TitleDependencies } from './title-client';
export const reportCategories=['manga','chapter','scanlation_group','user','author'] as const;
export type ReportCategory=typeof reportCategories[number];
export interface SignedReportReason {id:string;reason:string;details_required:boolean;version:number;}
function fail(message:string,status=502):never{throw new PublicApiContractError(message,status);}
// UUID-shaped catalogue subjects must never fall through to native-ID writes,
// even if their public provenance is absent/malformed. Validation is separate.
export function publicReportSubject(entity:unknown,category:unknown):boolean {return reportCategories.includes(category as ReportCategory)&&!!entity&&typeof entity==='object'&&('public_api' in entity||('id' in entity&&typeof entity.id==='string'&&uuidPattern.test(entity.id)));}
export async function createSignedReportController(categoryInput:unknown,targetInput:unknown,assertCurrent:()=>void,injected:Partial<TitleDependencies>={}) {
 if(!reportCategories.includes(categoryInput as ReportCategory))fail('This report category requires the native forum workflow',422);
 const category=categoryInput as ReportCategory,target=publicUuid(targetInput),scope=await createSignedAccountScope(assertCurrent,injected);
 if(!scope.account.permissions.includes('report.create')){scope.dispose();fail('This bound account cannot create reports',403);}
 const key=scope.key('report-pending')+':'+category+':'+target;
 let busy=false,reasons:SignedReportReason[]|undefined,sent=false;
 const pending=()=>scope.fences.get(key)!==null;
 async function read(){scope.check();if(busy)fail('A report request is already in progress',409);busy=true;try{
  const response=await scope.send('/report/reasons/'+category);if(response.status!==200)fail(`Report reasons unavailable (HTTP ${response.status})`,response.status);
  const body=record(response.body);if(body.result!=='ok'||body.response!=='collection'||!Array.isArray(body.data)||body.data.length>1000||body.offset!==0||body.total!==body.data.length||body.limit!==body.data.length)fail('Invalid complete report reason collection');
  const next=body.data.map(value=>{const row=record(value),attrs=record(row.attributes),localized=record(attrs.reason);if(row.type!=='report_reason'||attrs.category!==category||typeof localized.en!=='string'||!localized.en.trim()||localized.en.length>1000||typeof attrs.detailsRequired!=='boolean'||!Number.isSafeInteger(attrs.version)||Number(attrs.version)<1)fail('Invalid actual report reason/category/version');return{id:publicUuid(row.id),reason:localized.en,details_required:attrs.detailsRequired,version:Number(attrs.version)};});
  if(new Set(next.map(row=>row.id)).size!==next.length)fail('Duplicate report reason identity');reasons=next;return next.map(row=>({...row}));
 }finally{busy=false;}}
 async function submit(reasonInput:string,details=''){scope.check();if(busy||pending()||sent)fail('A report is in progress, already sent or unconfirmed. Nothing was replayed.',409);const reason=reasons?.find(row=>row.id===publicUuid(reasonInput));if(!reason)fail('Choose an actual reason from this subject category',422);if(typeof details!=='string'||details.includes('\0')||reason.details_required&&!details.trim())fail('Provide the required report explanation',422);
  const body={category,reason:reason.id,objectId:target,details:details.trim(),expectedAccountId:scope.account.id,expectedReasonVersion:reason.version};if(new TextEncoder().encode(JSON.stringify(body)).byteLength>8192)fail('Report exceeds the signed 8 KiB budget',422);
  busy=true;let installed=false;try{const marker=JSON.stringify({category,target,reason:reason.id,reasonVersion:reason.version});scope.fences.set(key,marker);installed=true;if(scope.fences.get(key)!==marker)fail('Report recovery fence was not persisted',409);
   const response=await scope.send('/report','POST',body);
   if(response.status>=400&&response.status<500){scope.fences.delete(key);installed=false;if(response.status===409)reasons=undefined;fail(`Report refused (HTTP ${response.status}). Reload reasons before reporting again.`,response.status);}
   if(response.status!==201||record(response.body).result!=='ok')fail('Report outcome is unconfirmed. Inspect your reports before another submission.');
   scope.fences.delete(key);installed=false;sent=true;
  }catch(error){if(installed&&error instanceof PublicUploadRequestError&&error.outcome==='unsent')scope.fences.delete(key);throw error;}finally{busy=false;}
 }
 return{account:scope.account,check:scope.check,dispose:scope.dispose,pending,read,submit};
}
