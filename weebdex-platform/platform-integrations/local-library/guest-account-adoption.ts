// Explicit, bounded, private copy. A frozen review is an opaque capability;
// neither source device data nor signing/publication APIs are ever mutated.
import type {DeviceSnapshot} from './device-store';
import type {NativeAccountLibrary,AccountReadingStatus} from './native-account-library-client';
import {accountReadingStatuses} from './native-account-library-client';
import {uuidPattern} from '../public-api/validation';
export const GUEST_ADOPTION_BATCH = 25;
type Command = {kind:'title';id:string;status:AccountReadingStatus;notification:boolean}|{kind:'progress';id:string;mangaId:string;page:number;completed:boolean;version:number}|{kind:'history';id:string;chapter:string}|{kind:'list';name:string;ids:string[]};
export type GuestAdoptionItem=Readonly<{kind:'title'|'chapter'|'list';mangaId?:string;name?:string;page?:number;completed?:boolean}>;
export type GuestAdoptionReview=Readonly<{accountName:string;accountId:string;page:number;pages:number;selected:number;copy:number;keep:number;unavailable:number;titles:number;chapters:number;lists:number;listNames:readonly string[];items:readonly GuestAdoptionItem[]}>;
export type GuestAdoptionReceipt={completed:number;total:number;titles:number;chapters:number;lists:number};
const reviews=new WeakMap<GuestAdoptionReview,{client:NativeAccountLibrary;commands:Command[];revision:number;source:string}>();
const stamp=(snapshot:DeviceSnapshot)=>JSON.stringify([snapshot.version,snapshot.scope,snapshot.revision,snapshot.version===2?snapshot.ownerId:null]);
const validId=(id:unknown):id is string=>typeof id==='string'&&uuidPattern.test(id);
const sourceRows=(snapshot:DeviceSnapshot)=>snapshot.records.filter(row=>['series','chapters','lists'].includes(row.family));
function safeName(name:unknown){return typeof name==='string'&&name.trim().length>0&&name.trim().length<=200&&!name.includes('\0')?name.trim():undefined;}
export async function prepareGuestAccountAdoption(client:NativeAccountLibrary,snapshot:DeviceSnapshot,page:number,check:()=>void):Promise<GuestAdoptionReview>{
 check();if(!Number.isSafeInteger(page)||page<1)throw Error('Choose a valid group of device saves.');
 const rows=sourceRows(snapshot),pages=Math.max(1,Math.ceil(rows.length/GUEST_ADOPTION_BATCH));if(page>pages)throw Error('Your device saves changed. Start a fresh preview.');
 const selected=rows.slice((page-1)*GUEST_ADOPTION_BATCH,page*GUEST_ADOPTION_BATCH),commands:Command[]=[];let revision:number|undefined,keep=0,unavailable=0;
 function observe(value:number){check();if(revision!==undefined&&revision!==value)throw Error('Your account library changed during the preview. Preview again before copying.');revision=value;}
 // This is a bounded metadata page, not a whole catalogue/account-title scan.
 const lists=await client.lists(new URLSearchParams({page:'1',limit:'100'}));check();observe(lists.revision);if(lists.total!==lists.data.length)throw Error('Account lists could not be checked completely. Nothing was copied.');
 const names=new Set(lists.data.map(row=>row.name.trim().toLocaleLowerCase())),plannedNames:string[]=[];let availableListSlots=Math.max(0,100-lists.total);
 for(const row of selected){
  check();
  try{
   if(row.family==='series'){
    if(!validId(row.id)||!accountReadingStatuses.includes(row.status as AccountReadingStatus)||typeof row.notification!=='boolean'){unavailable++;continue;}
    const found=await client.followStates([row.id]);check();observe(found.revision);if(found.data[0].saved){keep++;continue;}
    commands.push({kind:'title',id:row.id,status:row.status as AccountReadingStatus,notification:row.notification});
   }else if(row.family==='chapters'){
    if(!validId(row.id)||!validId(row.mangaId)||typeof row.completed!=='boolean'){unavailable++;continue;}
    const found=await client.progress(row.id);check();observe(found.revision);if(found.data.mangaId!==row.mangaId){unavailable++;continue;}
    if(found.data.version>0||found.data.page!==null||found.data.completed){keep++;continue;}
    if(Number.isSafeInteger(row.page)&&Number(row.page)>=1)commands.push({kind:'progress',id:row.id,mangaId:row.mangaId,page:Number(row.page)-1,completed:row.completed,version:found.data.version});
    else if(row.completed){const history=await client.historyState([row.mangaId],1,[row.id]);check();observe(history.revision);if(history.total>0){keep++;continue;}commands.push({kind:'history',id:row.mangaId,chapter:row.id});}
    else unavailable++;
   }else if(row.family==='lists'){
    const name=safeName(row.name);if(!name||typeof row.id!=='string'||!row.id){unavailable++;continue;}
    if(names.has(name.toLocaleLowerCase())){keep++;continue;}
    const members=snapshot.records.filter(item=>item.family==='members'&&item.list===row.id).sort((a,b)=>(a.position??0)-(b.position??0)||a.key.localeCompare(b.key));
    const ids=members.map(item=>item.series);if(!availableListSlots||ids.length>100||ids.some(id=>!validId(id))||new Set(ids).size!==ids.length){unavailable++;continue;}
    if(ids.length){const visible=await client.followStates(ids as string[]);check();observe(visible.revision);}
    commands.push({kind:'list',name,ids:ids as string[]});names.add(name.toLocaleLowerCase());plannedNames.push(name);availableListSlots--;
   }
  }catch(error){check();if([404,422].includes(Number((error as {status?:number})?.status))){unavailable++;continue;}throw error;}
 }
 check();const review=Object.freeze({accountName:client.account.name,accountId:client.account.id,page,pages,selected:selected.length,copy:commands.length,keep,unavailable,titles:commands.filter(row=>row.kind==='title').length,chapters:commands.filter(row=>['progress','history'].includes(row.kind)).length,lists:commands.filter(row=>row.kind==='list').length,listNames:Object.freeze(plannedNames),items:Object.freeze(commands.map(command=>Object.freeze(command.kind==='list'?{kind:'list' as const,name:command.name}:command.kind==='title'?{kind:'title' as const,mangaId:command.id}:command.kind==='progress'?{kind:'chapter' as const,mangaId:command.mangaId,page:command.page+1,completed:command.completed}:{kind:'chapter' as const,mangaId:command.id,completed:true})))});
 reviews.set(review,{client,commands,revision:revision!,source:stamp(snapshot)});return review;
}
export async function copyGuestAccountAdoption(review:GuestAdoptionReview,options:{check:()=>void;source:()=>Promise<DeviceSnapshot>;cancelled:()=>boolean;onReceipt:(receipt:GuestAdoptionReceipt)=>void}):Promise<GuestAdoptionReceipt>{
 const selected=reviews.get(review);if(!selected)throw Error('Preview these device saves again before copying.');reviews.delete(review);const {client,commands}=selected;let revision=selected.revision;
 const result:GuestAdoptionReceipt={completed:0,total:commands.length,titles:0,chapters:0,lists:0};
 const check=()=>{options.check();client.check();if(options.cancelled())throw Error('Copy stopped. Confirmed saves are kept; your device library is unchanged.');};
 check();const source=await options.source();check();if(stamp(source)!==selected.source)throw Error('Your device library changed. Preview it again before copying.');
 for(const command of commands){
  check();const currentSource=await options.source();check();if(stamp(currentSource)!==selected.source)throw Error('Your device library changed. Confirmed copies are kept; preview again before continuing.');let receipt;
  if(command.kind==='title')receipt=await client.setFollow(command.id,revision,command.status,command.notification);
  else if(command.kind==='progress')receipt=await client.saveProgress(command.id,revision,command.page,command.completed,command.version);
  else if(command.kind==='history')receipt=await client.updateHistory(command.id,revision,[command.chapter],[]);
  else receipt=await client.createList(revision,command.name,command.ids);
  // Acknowledged receipts remain accurate even when Cancel was clicked during
  // the in-flight request. Cancellation can stop only subsequent commands.
  options.check();client.check();revision=receipt.revision;result.completed++;if(command.kind==='title')result.titles++;else if(command.kind==='list')result.lists++;else result.chapters++;options.onReceipt({...result});
  check();
 }
 return result;
}
