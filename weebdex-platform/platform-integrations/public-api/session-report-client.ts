// Private moderation reports from the normal signed-in browser account.
import { createNativeAccountScope, nativeAccountError, NativeAccountRequestError, type NativeAccountDependencies } from './native-account-scope';
import { publicUuid, uuidPattern, record, PublicApiContractError } from './validation';
export const reportCategories=['manga','chapter','scanlation_group','user','author'] as const;
export type ReportCategory=typeof reportCategories[number];
export interface SignedReportReason {id:string;reason:string;details_required:boolean;version:number;}
function fail(message:string,status=502):never{throw new PublicApiContractError(message,status);}
// UUID-shaped catalogue subjects must never fall through to native-ID writes,
// even if their public provenance is absent/malformed. Validation is separate.
export function publicReportSubject(entity:unknown,category:unknown):boolean {return reportCategories.includes(category as ReportCategory)&&!!entity&&typeof entity==='object'&&('public_api' in entity||('id' in entity&&typeof entity.id==='string'&&uuidPattern.test(entity.id)));}
export async function createSessionReportController(categoryInput:unknown,targetInput:unknown,nativeAccount:string,assertCurrent:()=>void,injected:Partial<NativeAccountDependencies>={}) {
 if(!reportCategories.includes(categoryInput as ReportCategory))fail('This report category requires the native forum workflow',422);
 const category=categoryInput as ReportCategory,target=publicUuid(targetInput),scope=await createNativeAccountScope(nativeAccount,assertCurrent,injected);
 if(!scope.account.permissions.includes('report.create')){scope.dispose();throw nativeAccountError(403);}
 function provenance(input:unknown){const value=record(input),authority=record(value.nostr);if(authority.scope!=='instance-account'||authority.authentication!=='native-session'||publicUuid(authority.accountId)!==scope.account.id)fail('The report account changed. Reopen this report before trying again.',409);return value;}
 const key=scope.key('report-pending')+':'+category+':'+target;
 let busy=false,reasons:SignedReportReason[]|undefined,sent=false;
 const pending=()=>scope.fences.get(key)!==null;
 async function read(){scope.check();if(busy)fail('A report request is already in progress',409);busy=true;try{
  const response=await scope.send('/report/reasons/'+category);if(response.status===401||response.status===403)throw nativeAccountError(response.status,response.body);if(response.status!==200)fail('Report reasons could not be loaded. Please try again.',response.status);
  const body=provenance(response.body);if(body.result!=='ok'||body.response!=='collection'||!Array.isArray(body.data)||body.data.length>1000||body.offset!==0||body.total!==body.data.length||body.limit!==body.data.length)fail('Invalid complete report reason collection');
  const next=body.data.map(value=>{const row=record(value),attrs=record(row.attributes),localized=record(attrs.reason);if(row.type!=='report_reason'||attrs.category!==category||typeof localized.en!=='string'||!localized.en.trim()||localized.en.length>1000||typeof attrs.detailsRequired!=='boolean'||!Number.isSafeInteger(attrs.version)||Number(attrs.version)<1)fail('Invalid actual report reason/category/version');return{id:publicUuid(row.id),reason:localized.en,details_required:attrs.detailsRequired,version:Number(attrs.version)};});
  if(new Set(next.map(row=>row.id)).size!==next.length)fail('Duplicate report reason identity');reasons=next;return next.map(row=>({...row}));
 }finally{busy=false;}}
 async function submit(reasonInput:string,details=''){scope.check();if(busy||pending()||sent)fail('A report is in progress, already sent or unconfirmed. Nothing was replayed.',409);const reason=reasons?.find(row=>row.id===publicUuid(reasonInput));if(!reason)fail('Choose an actual reason from this subject category',422);if(typeof details!=='string'||details.includes('\0')||reason.details_required&&!details.trim())fail('Provide the required report explanation',422);
  const body={category,reason:reason.id,objectId:target,details:details.trim(),expectedAccountId:scope.account.id,expectedReasonVersion:reason.version};if(new TextEncoder().encode(JSON.stringify(body)).byteLength>8192)fail('Your report is too long; shorten its explanation',422);
  busy=true;let installed=false,marker:string|undefined;const clearOwned=()=>{if(marker!==undefined&&scope.fences.get(key)===marker)scope.fences.delete(key);};try{marker=JSON.stringify({commandId:crypto.randomUUID(),category,target,reason:reason.id,reasonVersion:reason.version});scope.fences.set(key,marker);installed=true;if(scope.fences.get(key)!==marker)fail('Report recovery fence was not persisted',409);
   const response=await scope.send('/report','POST',body);
   if(response.status>=400&&response.status<500){clearOwned();installed=false;if(response.status===409)reasons=undefined;if(response.status===401||response.status===403)throw nativeAccountError(response.status,response.body);fail('The report was not accepted. Reload the reasons and review your report before trying again.',response.status);}
   if(response.status!==201||provenance(response.body).result!=='ok')fail('Report outcome is unconfirmed. Inspect your reports before another submission.');
   clearOwned();installed=false;sent=true;
  }catch(error){if(installed&&error instanceof NativeAccountRequestError&&error.outcome==='unsent')clearOwned();throw error;}finally{busy=false;}
 }
 return{account:scope.account,check:scope.check,dispose:scope.dispose,pending,read,submit};
}
