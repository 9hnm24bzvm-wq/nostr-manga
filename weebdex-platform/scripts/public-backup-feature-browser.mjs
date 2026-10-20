// Prospective real-UI/IndexedDB acceptance in a fresh, owned public fixture.
// No production hooks, route/response synthesis, arbitrary DB clearing or IDs in results.
import { createHash } from 'node:crypto';
const UUID=/^[0-9a-f]{8}-[0-9a-f]{4}-[1-8][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;
const STORES=Object.freeze(['records','meta','publicAliases','idMigrationArchives','idMigrationIssues','idMigrationTombstones']);
const LIST='a'.repeat(32),FOREIGN='77777777-7777-8777-8777-777777777777',MAX_FILE=1024*1024;
const owners=new WeakMap(), failureCategories=new WeakMap(), failureFacts=new WeakMap();
// Only owned, fixed facts cross into the public report. Never classify from an
// exception message, URL, arbitrary property, stack, ID or database contents.
function failure(code){const error=Error(code);failureCategories.set(error,code==='backup_merge_prerequisite'?'prerequisite':['backup_case_deadline','backup_initialization_deadline','backup_download_timeout','backup_download_open_timeout'].includes(code)?'timeout':'check');return error;}
function demand(condition,code){if(!condition)throw failure(code);}
function mark(h,stage){if(h.backupTrace)h.backupTrace.stage=stage;}
function category(error){
 if(error!==null&&(typeof error==='object'||typeof error==='function')){
  const known=failureCategories.get(error);if(known)return known;
  try{const name=Object.getOwnPropertyDescriptor(error,'name');if(name&&'value'in name&&name.value==='TimeoutError')return 'timeout';}catch{}
 }
 return 'operation';
}
function traced(run){return async context=>{const trace={stage:'preconditions'};try{return await run({...context,backupTrace:trace});}catch(error){if(error!==null&&(typeof error==='object'||typeof error==='function'))failureFacts.set(error,Object.freeze({stage:trace.stage,category:category(error)}));throw error;}};}
export function backupFailureDiagnostic(error){const value=error!==null&&(typeof error==='object'||typeof error==='function')?failureFacts.get(error):null;return value?{stage:value.stage,category:value.category}:{stage:'unavailable',category:'operation'};}

function canonical(value){if(Array.isArray(value))return '['+value.map(canonical).join(',')+']';if(value&&typeof value==='object')return '{'+Object.keys(value).sort().map(key=>JSON.stringify(key)+':'+canonical(value[key])).join(',')+'}';return JSON.stringify(value);}
const equal=(a,b)=>canonical(a)===canonical(b);
const ordered=rows=>[...rows].sort((a,b)=>canonical(a).localeCompare(canonical(b)));
function archive(record,ownerId,scope){const content={ownerId,scope,record};return {digest:createHash('sha256').update(canonical(content)).digest('hex'),...content};}
function privateState(raw){return {scope:raw.scope,ownerId:raw.ownerId,marker:raw.marker,revision:raw.revision,count:raw.count,records:ordered(raw.records),aliases:ordered(raw.aliases),archives:ordered(raw.archives),issues:ordered(raw.issues),tombstones:ordered(raw.tombstones),migration_reason:raw.migration_reason??null};}
function validState(raw,origin){demand(raw&&equal(Object.keys(raw).sort(),['aliases','archives','count','issues','marker','migration_reason','ownerId','records','revision','scope','tombstones'].sort()),'backup_device_shape');demand(raw.scope?.origin===origin&&UUID.test(raw.scope.catalogNamespace)&&equal(Object.keys(raw.scope).sort(),['catalogNamespace','origin','serviceKey'])&&(raw.scope.serviceKey===null||/^[0-9a-f]{64}$/.test(raw.scope.serviceKey))&&[null,undefined].includes(raw.migration_reason),'backup_device_scope');demand(UUID.test(raw.ownerId)&&raw.marker?.version===2&&raw.marker.ownerId===raw.ownerId&&equal(Object.keys(raw.marker).sort(),['ownerId','scope','version'])&&equal(raw.marker.scope,raw.scope),'backup_device_owner');demand(Number.isSafeInteger(raw.revision)&&raw.revision>=0&&raw.count===raw.records?.length,'backup_device_revision');for(const key of ['records','aliases','archives','issues','tombstones'])demand(Array.isArray(raw[key])&&raw[key].length<=16,'backup_device_budget');return privateState(raw);}
function sameState(actual,expected,code){demand(equal(privateState(actual),privateState(expected)),code);}
function exportState(backup,state){demand(backup?.format==='weebdex-device-library'&&backup.version===2&&backup.id_format==='public_uuid_with_unresolved_legacy','backup_export_format');demand(typeof backup.exported_at==='string'&&Number.isFinite(Date.parse(backup.exported_at)),'backup_export_date');demand(equal(Object.keys(backup).sort(),['format','version','id_format','scope','ownerId','revision','exported_at','records','aliases','archives','issues','tombstones'].sort()),'backup_export_fields');for(const key of ['scope','ownerId','revision'])demand(equal(backup[key],state[key]),'backup_export_binding');for(const key of ['records','aliases','archives','issues','tombstones'])demand(equal(ordered(backup[key]),ordered(state[key])),'backup_export_content');}
function environment(h,seconds){const url=new URL(h.origin);demand(url.origin===h.origin&&url.protocol==='http:'&&['127.0.0.1','localhost'].includes(url.hostname)&&url.port,'backup_fixture_origin');demand(h.fixture?.public_upload?.fixture_only===true&&h.fixture.public_upload.enabled===true&&h.fixture.public_upload.scenario==='public-upload-v1','backup_fixture_profile');demand(h.page&&h.context&&typeof h.page.context==='function'&&h.page.context()===h.context&&typeof h.insist==='function','backup_fixture_context');const deadline=Date.now()+seconds*1000;return {...h,deadline,origin:url.origin};}
function left(h,cap=12000){const remaining=h.deadline-Date.now();demand(remaining>0,'backup_case_deadline');return Math.max(1,Math.min(cap,remaining));}
async function ready(h,locator,enabled=true){await locator.waitFor({state:'visible',timeout:left(h)});demand(await locator.count()===1,'backup_control_unique');while(enabled&&!await locator.isEnabled()){left(h);await h.page.waitForTimeout(Math.min(80,left(h)));}left(h);return locator;}
async function click(h,locator){await ready(h,locator);await locator.click({timeout:left(h)});left(h);}
const panel=h=>h.page.locator('[data-local-library-panel]');
const dialog=h=>h.page.getByRole('dialog',{name:'Review library import',exact:true});
async function open(h,reload=false){const response=reload?await h.page.reload({waitUntil:'domcontentloaded',timeout:left(h)}):await h.page.goto(h.origin+'/settings/library',{waitUntil:'domcontentloaded',timeout:left(h)});demand(response?.status()===200,'backup_settings_http');demand(new URL(h.page.url()).origin===h.origin&&new URL(h.page.url()).pathname==='/settings/library','backup_settings_route');await ready(h,panel(h).getByRole('button',{name:'Export device backup',exact:true}));}
// Static page function: opens only the known already-created device DB. A new
// or upgraded DB is aborted; seed adds only four known rows after an empty CAS.
async function deviceOperation({origin,seed,expected,timeoutMs=3000}){
 if(!Number.isSafeInteger(timeoutMs)||timeoutMs<1||timeoutMs>3000)throw Error('backup_device_timeout_bound');
 if(location.origin!==origin)throw Error('backup_device_origin');
 const name='weebdex-device-library-v1:'+encodeURIComponent(origin);
 return new Promise((resolve,reject)=>{
  let db,tx,done=false;const finish=(error,value)=>{if(done)return;done=true;clearTimeout(timer);db?.close();if(error)reject(Error(error));else resolve(value);};
  const timer=setTimeout(()=>{try{tx?.abort();}catch{}finish('backup_device_timeout');},timeoutMs);
  const query=indexedDB.open(name);query.onupgradeneeded=()=>{try{query.transaction.abort();query.result?.close();}catch{}finish('backup_device_uninitialized');};query.onblocked=()=>finish('backup_device_blocked');query.onerror=()=>finish('backup_device_open');
  query.onsuccess=()=>{
   db=query.result;if(done){db.close();return;}if(db.version!==2||!['records','meta','publicAliases','idMigrationArchives','idMigrationIssues','idMigrationTombstones'].every(key=>db.objectStoreNames.contains(key)))return finish('backup_device_schema');
   try{tx=db.transaction(['records','meta','publicAliases','idMigrationArchives','idMigrationIssues','idMigrationTombstones'],seed?'readwrite':'readonly');}catch{return finish('backup_device_transaction');}
   const meta=tx.objectStore('meta'),requests={records:tx.objectStore('records').getAll(),scope:meta.get('instanceScope'),ownerId:meta.get('deviceOwnerId'),marker:meta.get('publicIdOwner'),revision:meta.get('revision'),count:meta.get('count'),migration_reason:meta.get('scopeMigrationReason'),aliases:tx.objectStore('publicAliases').getAll(),archives:tx.objectStore('idMigrationArchives').getAll(),issues:tx.objectStore('idMigrationIssues').getAll(),tombstones:tx.objectStore('idMigrationTombstones').getAll()};
   let value,pending=Object.keys(requests).length;
   for(const request of Object.values(requests))request.onsuccess=()=>{if(done)return;if(--pending)return;value=Object.fromEntries(Object.entries(requests).map(([key,row])=>[key,row.result]));value.migration_reason??=null;
    if(seed){const current=[value.scope,value.ownerId,value.marker,value.revision,value.count,value.records,value.aliases,value.archives,value.issues,value.tombstones,value.migration_reason],before=[expected.scope,expected.ownerId,expected.marker,expected.revision,expected.count,expected.records,expected.aliases,expected.archives,expected.issues,expected.tombstones,expected.migration_reason];
     if(JSON.stringify(current)!==JSON.stringify(before)||value.count!==0||value.revision!==0||[value.records,value.aliases,value.archives,value.issues,value.tombstones].some(rows=>rows.length)||seed.length!==4){try{tx.abort();}catch{}return finish('backup_seed_not_empty');}
     for(const row of seed)tx.objectStore('records').add(row);meta.put(seed.length,'count');meta.put(1,'revision');
    }
   };
   tx.oncomplete=()=>finish(null,value);tx.onabort=()=>finish('backup_device_aborted');tx.onerror=()=>{};
  };
 });
}
async function readDevice(h){left(h);let value;try{value=await h.page.evaluate(deviceOperation,{origin:h.origin,timeoutMs:left(h,3000)});}catch{throw failure('backup_device_read_failed');}left(h);return value;}
async function durable(h){return validState(await readDevice(h),h.origin);}
async function initialized(h){const deadline=Math.min(h.deadline,Date.now()+12000);for(;;){const value=await readDevice(h);demand(value?.scope?.origin===h.origin&&UUID.test(value.ownerId)&&[null,undefined].includes(value.migration_reason),'backup_initial_device_scope');if(value.marker)return validState(value,h.origin);demand(Date.now()<deadline,'backup_initialization_deadline');left(h);await h.page.waitForTimeout(Math.min(80,left(h)));}}
// APIResponse.body materializes bytes first; this is an admission limit, not a streaming memory bound.
async function json(response,h){demand(response.status()===200,'backup_fixture_read_status');const bytes=await response.body();left(h);demand(bytes.length<=65536,'backup_fixture_read_budget');try{return JSON.parse(bytes.toString('utf8'));}catch{throw failure('backup_fixture_read_json');}}
async function fixtureIds(h,scope){const resources=['manga','chapter'].map(kind=>({kind,id:h.fixture.ids?.[kind]}));demand(resources.every(row=>typeof row.id==='string'&&/^[A-Za-z0-9-]{10,36}$/.test(row.id)),'backup_fixture_ids');const body=await json(await h.context.request.post(h.origin+'/api/v1/legacy/resolve',{data:{catalog_namespace:scope.catalogNamespace,resources},maxRedirects:0,timeout:left(h,8000)}),h);demand(body.result==='ok'&&Array.isArray(body.data)&&body.data.length===2&&Array.isArray(body.unresolved)&&body.unresolved.length===0,'backup_fixture_resolution');const result={};for(const row of body.data){demand(resources.some(ref=>ref.kind===row.kind&&ref.id===row.legacy_id)&&!result[row.kind]&&UUID.test(row.id),'backup_fixture_resolution_scope');result[row.kind]=row.id;}demand(result.manga&&result.chapter,'backup_fixture_resolution_missing');const chapter=await json(await h.context.request.get(h.origin+'/api/v1/chapter/'+result.chapter,{maxRedirects:0,timeout:left(h,8000)}),h);demand(chapter.result==='ok'&&chapter.response==='entity'&&chapter.data?.type==='chapter'&&chapter.data.id===result.chapter&&Array.isArray(chapter.data.relationships),'backup_fixture_chapter');const parents=chapter.data.relationships.filter(row=>row.type==='manga');demand(parents.length===1&&parents[0].id===result.manga,'backup_fixture_parent');return result;}
function seedRows(ids){return [{family:'series',key:ids.manga,id:ids.manga,status:'reading',notification:true,followedAt:'2026-10-01T00:00:00Z',updated_at:100},{family:'chapters',key:ids.chapter,id:ids.chapter,mangaId:ids.manga,completed:true,page:3,readAt:'2026-10-01T00:01:00Z',updated_at:101},{family:'lists',key:LIST,id:LIST,name:'Fixture backup list',updated_at:99},{family:'members',key:LIST+':'+ids.manga,list:LIST,series:ids.manga,position:0,updated_at:102}];}
async function downloadStream(h,file){return new Promise((resolve,reject)=>{let expired=false;const timer=setTimeout(()=>{expired=true;reject(failure('backup_download_open_timeout'));},left(h));Promise.resolve().then(()=>file.createReadStream()).then(stream=>{clearTimeout(timer);if(expired)stream?.destroy();else resolve(stream);},()=>{clearTimeout(timer);if(!expired)reject(failure('backup_download_open'));});});}
async function download(h,locator,state){const waiting=h.page.waitForEvent('download',{timeout:left(h)});waiting.catch(()=>{});await click(h,locator);const file=await waiting;left(h);const stream=await downloadStream(h,file);demand(stream,'backup_download_stream');let timer;try{const bytes=await Promise.race([(async()=>{const chunks=[];let count=0;for await(const chunk of stream){left(h);count+=chunk.length;demand(count<=MAX_FILE,'backup_download_budget');chunks.push(chunk);}return Buffer.concat(chunks);})(),new Promise((_,reject)=>{timer=setTimeout(()=>{stream.destroy();reject(failure('backup_download_timeout'));},left(h));})]);left(h);let parsed;try{parsed=JSON.parse(bytes.toString('utf8'));}catch{throw failure('backup_download_json');}exportState(parsed,state);return parsed;}finally{clearTimeout(timer);stream.destroy();}}
async function preview(h,backup){await ready(h,panel(h).getByLabel('Preview device backup',{exact:true}));await panel(h).getByLabel('Preview device backup',{exact:true}).setInputFiles({name:'owned-fixture-device-backup.json',mimeType:'application/json',buffer:Buffer.from(JSON.stringify(backup))},{timeout:left(h)});await ready(h,dialog(h));}
async function prepare(h,mode){await click(h,dialog(h).getByRole('button',{name:mode==='merge'?'Review backup merge':'Review backup replacement',exact:true}));await ready(h,dialog(h).getByRole('button',{name:mode==='merge'?'Confirm reviewed merge':'Confirm reviewed replacement',exact:true}));}
async function confirm(h,mode){await click(h,dialog(h).getByRole('button',{name:mode==='merge'?'Confirm reviewed merge':'Confirm reviewed replacement',exact:true}));await dialog(h).waitFor({state:'hidden',timeout:left(h)});await ready(h,panel(h).getByText(mode==='merge'?'Backup merged locally.':'Device library restored from the reviewed backup.',{exact:true}));}
function mergeExpected(before,incoming){const records=before.records.map(row=>row.family==='series'?incoming.records.find(candidate=>candidate.family==='series'):row);return {...before,records,revision:before.revision+1,archives:[archive(before.records.find(row=>row.family==='series'),before.ownerId,before.scope)]};}
async function mergeRoundtrip(context){
 const h=environment(context,35);demand(!owners.has(h.page),'backup_case_reused');
 mark(h,'open-settings');await open(h);
 mark(h,'initialize-device');const before=await initialized(h);demand(before.revision===0&&before.count===0&&['records','aliases','archives','issues','tombstones'].every(key=>before[key].length===0),'backup_fresh_device_required');
 mark(h,'resolve-fixture');const ids=await fixtureIds(h,before.scope),rows=seedRows(ids);
 mark(h,'seed-device');try{await h.page.evaluate(deviceOperation,{origin:h.origin,seed:rows,expected:before,timeoutMs:left(h,3000)});}catch{throw failure('backup_owned_seed_failed');}
 mark(h,'verify-seed');const seeded=await durable(h);sameState(seeded,{...before,records:rows,count:4,revision:1},'backup_seed_durable');
 mark(h,'reload-seed');await open(h,true);
 mark(h,'export-backup');const original=await download(h,panel(h).getByRole('button',{name:'Export device backup',exact:true}),seeded),incoming=structuredClone(original);const series=incoming.records.find(row=>row.family==='series');series.status='completed';series.updated_at=110;
 mark(h,'preview-merge');await preview(h,incoming);
 mark(h,'review-merge');await prepare(h,'merge');demand(await dialog(h).getByRole('checkbox').count()===0,'backup_same_owner_consent');
 mark(h,'verify-review-unchanged');sameState(await durable(h),seeded,'backup_prepare_mutated');
 mark(h,'download-pre-restore');await download(h,dialog(h).getByRole('button',{name:'Download pre-restore backup',exact:true}),seeded);
 mark(h,'confirm-merge');await confirm(h,'merge');
 mark(h,'verify-merge');const expected=mergeExpected(seeded,incoming);sameState(await durable(h),expected,'backup_merge_durable');
 mark(h,'reload-merge');await open(h,true);sameState(await durable(h),expected,'backup_merge_reload');
 owners.set(h.page,{original,merged:expected,ids});return {fixture_owned:true,ui_exported:true,ui_reviewed:true,ui_confirmed:true,durable_verified:true,reload_verified:true};
}
function foreignBackup(state){demand(state.original.ownerId!==FOREIGN,'backup_foreign_owner_collision');const input=structuredClone(state.original),source=input.records.find(row=>row.family==='series'),list=input.records.find(row=>row.family==='lists');input.ownerId=FOREIGN;input.records=[{...list,name:'Fixture foreign restored list',updated_at:120}];input.archives=[archive(source,FOREIGN,input.scope)];return input;}
async function acknowledge(h){const checkbox=dialog(h).getByRole('checkbox',{name:'I reviewed this backup from another browser device. Restore its library data while keeping this device’s owner.',exact:true});await ready(h,checkbox);demand(!await checkbox.isChecked(),'backup_foreign_consent_not_reset');for(const name of ['Review backup merge','Review backup replacement']){const button=await ready(h,dialog(h).getByRole('button',{name,exact:true}),false);demand(!await button.isEnabled(),'backup_foreign_consent_bypass');}await checkbox.check({timeout:left(h)});demand(await checkbox.isChecked(),'backup_foreign_consent_missing');}
function replacementExpected(before,input){const archived=new Map(before.archives.map(row=>[row.digest,row]));for(const row of input.archives)archived.set(row.digest,row);for(const row of before.records){const record=archive(row,before.ownerId,before.scope);archived.set(record.digest,record);}return {...before,records:input.records,count:1,revision:before.revision+1,archives:[...archived.values()]};}
async function foreignReplacement(context){
 const h=environment(context,25);mark(h,'merge-prerequisite');const state=owners.get(h.page);demand(state,'backup_merge_prerequisite');owners.delete(h.page);
 mark(h,'verify-owned-state');sameState(await durable(h),state.merged,'backup_owned_state_changed');const input=foreignBackup(state);
 mark(h,'preview-replacement');await preview(h,input);
 mark(h,'acknowledge-foreign');await acknowledge(h);
 mark(h,'review-replacement');await prepare(h,'replace');
 mark(h,'verify-review-unchanged');sameState(await durable(h),state.merged,'backup_foreign_prepare_mutated');
 mark(h,'download-pre-restore');await download(h,dialog(h).getByRole('button',{name:'Download pre-restore backup',exact:true}),state.merged);
 mark(h,'cancel-replacement');await click(h,dialog(h).getByRole('button',{name:'Cancel import',exact:true}));await dialog(h).waitFor({state:'hidden',timeout:left(h)});
 mark(h,'verify-cancel');sameState(await durable(h),state.merged,'backup_cancel_mutated');
 mark(h,'reload-cancel');await open(h,true);sameState(await durable(h),state.merged,'backup_cancel_reload');
 mark(h,'preview-replacement-again');await preview(h,input);
 mark(h,'acknowledge-foreign-again');await acknowledge(h);
 mark(h,'review-replacement-again');await prepare(h,'replace');sameState(await durable(h),state.merged,'backup_preconfirm_changed');
 mark(h,'confirm-replacement');await confirm(h,'replace');
 mark(h,'verify-replacement');const expected=replacementExpected(state.merged,input);sameState(await durable(h),expected,'backup_replacement_durable');
 mark(h,'reload-replacement');await open(h,true);sameState(await durable(h),expected,'backup_replacement_reload');
 mark(h,'export-replacement');await download(h,panel(h).getByRole('button',{name:'Export device backup',exact:true}),expected);
 return {foreign_consent_checked:true,cancel_preserved:true,replacement_confirmed:true,owner_preserved:true,provenance_verified:true,reload_verified:true};
}
export const publicBackupFeatureCases=Object.freeze([Object.freeze({id:'backup-v2-merge-roundtrip',run:traced(mergeRoundtrip)}),Object.freeze({id:'backup-v2-foreign-replacement-cancel',run:traced(foreignReplacement)})]);
