// Normal browser account only; external signing authority is never used.
import {openSessionAccountAccess,type SessionAccountAccess,type SessionAccountDependencies} from './session-account-access';
import {createAccountSettingsClient,type AccountSettingsView} from './session-settings-client';
import {publicUuid,record,resource,PublicApiContractError} from './validation';
export type AccountBlockKind='groups'|'uploaders';
export interface AccountBlockReference{id:string;name:string;version?:number;unavailable?:boolean;}
const kinds:AccountBlockKind[]=['groups','uploaders'];
function fail(message:string,status=502):never{throw new PublicApiContractError(message,status);}
function kind(value:AccountBlockKind){if(!kinds.includes(value))fail('Unsupported account blocklist',422);return value;}
function ids(view:AccountSettingsView,key:AccountBlockKind):string[]{return [...(view.settings[key] as string[]|undefined??[])];}
export function createAccountBlocklistClient(access:SessionAccountAccess){return async(assertCurrent=()=>{})=>{
 const account=await access(assertCurrent);account.check();
 const settings=await createAccountSettingsClient(async()=>account)(assertCurrent);
 let current:AccountSettingsView|undefined,busy=false,requiresReload=true;
 let saved:Record<AccountBlockKind,string[]>={groups:[],uploaders:[]},draft:Record<AccountBlockKind,string[]>={groups:[],uploaders:[]};
 const known:Record<AccountBlockKind,Map<string,AccountBlockReference>>={groups:new Map(),uploaders:new Map()},candidates:Record<AccountBlockKind,Map<string,AccountBlockReference>>={groups:new Map(),uploaders:new Map()};
 const check=()=>{account.check();assertCurrent();};
 const dirty=()=>kinds.some(key=>JSON.stringify(saved[key])!==JSON.stringify(draft[key]));
 const rows=(key:AccountBlockKind):AccountBlockReference[]=>draft[key].map(id=>({...known[key].get(id)??{id,name:'Unavailable reference',unavailable:true}}));
 const view=()=>current?{accountId:account.accountId,username:account.username,version:current.settings.version,unavailable:current.unavailable,needsRecheck:requiresReload||!!settings.view()?.needsRecheck,dirty:dirty(),groups:rows('groups'),uploaders:rows('uploaders')}:undefined;
 function ready(){check();if(busy||requiresReload||!current||settings.view()?.needsRecheck)fail('Reload the current account blocklists before editing.',409);}
 async function lookup(key:AccountBlockKind,query:URLSearchParams,requested?:Set<string>){
  // The retained layout prefixes URLSearchParams.toString() with '?'. Keep
  // exactly one delimiter for both typed searches and saved-ID hydration.
  const response=await account.request((key==='groups'?'/group':'/user')+'?'+query.toString().replace(/^\?/,''));check();
  if(response.status!==200)fail('Account filter references could not be loaded ('+response.status+').',response.status);
  const cache=new Set((response.headers?.['cache-control']??'').toLowerCase().split(',').map(value=>value.trim())),vary=new Set((response.headers?.vary??'').toLowerCase().split(',').map(value=>value.trim()));
  if(key==='uploaders'&&(!cache.has('no-store')||!cache.has('private')||!vary.has('cookie')||!vary.has('x-weebdex-expected-account')))fail('Private account directory cache policy was not confirmed.');
  const body=record(response.body,'account reference collection');
  if(body.result!=='ok'||body.response!=='collection'||!Array.isArray(body.data)||body.offset!==0||body.limit!==Number(query.get('limit'))||!Number.isSafeInteger(body.total)||Number(body.total)<body.data.length||body.data.length>Number(body.limit))fail('Invalid account reference collection');
  if(key==='uploaders'){const meta=record(body.nostr);if(meta.scope!=='instance-account'||meta.authentication!=='native-session'||publicUuid(meta.accountId)!==account.accountId||meta.source!=='persisted-native-account-library'||meta.requires!=='native-session-account')fail('The account directory binding changed. Reload account blocklists.',409);}
  const seen=new Set<string>();return body.data.map(value=>{const row=resource(value,key==='groups'?'scanlation_group':'user'),id=publicUuid(row.id),name=row.attributes[key==='groups'?'name':'username'],version=row.attributes.version;if(row.id!==id||seen.has(id)||requested&&!requested.has(id)||typeof name!=='string'||!name.trim()||name.length>500||!Number.isSafeInteger(version)||Number(version)<1||Number(version)>2147483647)fail('Invalid or mismatched account filter reference');seen.add(id);return{id,name,version:Number(version)};});
 }
 async function reload(){check();if(busy)fail('An account blocklist request is pending.',409);busy=true;requiresReload=true;try{const next=await settings.reload();check();const hydrated:Record<AccountBlockKind,AccountBlockReference[]>={groups:[],uploaders:[]};for(const key of kinds){const selected=ids(next,key);if(selected.length){const query=new URLSearchParams({limit:'100'});for(const id of selected)query.append('ids[]',id);hydrated[key]=await lookup(key,query,new Set(selected));check();}}current=next;for(const key of kinds){saved[key]=ids(next,key);draft[key]=[...saved[key]];known[key].clear();candidates[key].clear();for(const row of hydrated[key])known[key].set(row.id,row);}requiresReload=false;return view()!;}finally{busy=false;}}
 async function search(key:AccountBlockKind,input:string){kind(key);ready();const text=input.trim();if(text.length<2||text.length>100||/[\u0000-\u001f]/.test(text))fail('Enter 2 to 100 characters to search account references.',422);busy=true;candidates[key].clear();try{const rows=await lookup(key,new URLSearchParams({limit:'10',[key==='groups'?'name':'username']:text}));check();for(const row of rows)candidates[key].set(row.id,row);return rows.map(row=>({...row}));}catch(error){requiresReload=true;throw error;}finally{busy=false;}}
 function add(key:AccountBlockKind,id:string){kind(key);ready();const row=candidates[key].get(publicUuid(id));if(!row||row.id!==id)fail('Select a verified current search result.',422);if(!draft[key].includes(id)){if(draft[key].length>=100)fail('An account blocklist supports at most 100 visible references.',422);draft[key].push(id);known[key].set(id,{...row});}return view()!;}
 function remove(key:AccountBlockKind,id:string){kind(key);ready();id=publicUuid(id);draft[key]=draft[key].filter(value=>value!==id);return view()!;}
 async function save(){ready();const patch:Record<string,string[]>={};for(const key of kinds)if(JSON.stringify(saved[key])!==JSON.stringify(draft[key]))patch[key]=[...draft[key]];if(!Object.keys(patch).length)fail('There are no account blocklist changes to save.',422);busy=true;try{current=await settings.save(patch);check();for(const key of kinds){saved[key]=ids(current,key);draft[key]=[...saved[key]];}return view()!;}catch(error){requiresReload=true;throw error;}finally{busy=false;}}
 return{reload,search,add,remove,save,view,check};
};}
export async function loadAccountBlocklists(nativeAccount:string,assertCurrent=()=>{},injected:Partial<SessionAccountDependencies>={}){const account=await openSessionAccountAccess(nativeAccount,assertCurrent,injected);try{const client=await createAccountBlocklistClient(async()=>account)(assertCurrent);return{...client,dispose:account.dispose};}catch(error){account.dispose();throw error;}}
