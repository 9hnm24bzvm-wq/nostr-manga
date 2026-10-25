// Synthetic transport fixtures only. Actual source controller/account scope runs.
import {fixture,actor,loadSignedFixtureModule} from './report-draft-fixture.mjs';
export const nativeInspection=await loadSignedFixtureModule('native-report-inspection');
export const nativeReportRow=(patch={})=>({id:'report_6',category:'thread',objectId:'NativeThread',status:'waiting',createdAt:'2026-10-22T00:00:00.123456Z',details:'Private submitted explanation',reason:{id:'thread_spam',reason:'Actual native reason',version:2,detailsRequired:true},...patch});
export const nativePage=(data=[nativeReportRow()],page=1,total=data.length)=>({status:200,body:{result:'ok',response:'collection',nostr:{scope:'instance-account',source:'persisted-native-personal-reports',accountId:actor,nativeSessionAuthority:false},data,total,page,limit:5,offset:(page-1)*5}});
export const nativeDetail=(row=nativeReportRow())=>({status:200,body:{result:'ok',response:'entity',nostr:nativePage().body.nostr,data:row}});
export const isNativeInspection=request=>new URL(request.url).pathname.startsWith('/api/v1/nostr/account/reports/native')&&request.method==='GET';
export function nativeFixture(options={}){const f=fixture({...options,handler:async request=>{const result=await options.handler?.(request);if(result!==undefined)return result;if(isNativeInspection(request)){const url=new URL(request.url);return url.pathname.endsWith('/native')?nativePage(undefined,Number(url.searchParams.get('page'))):nativeDetail();}}});return{...f,open:signal=>nativeInspection.createNativeReportReader(f.check,f.deps,signal)};}
