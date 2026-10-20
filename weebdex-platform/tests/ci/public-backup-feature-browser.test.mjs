// Finite source/adapter controls only. No Playwright/browser, real IndexedDB,
// fixture HTTP, app build, install or acceptance run is started here.
import fs from 'node:fs';import path from 'node:path';import vm from 'node:vm';
import {createHash} from 'node:crypto';import {stripTypeScriptTypes} from 'node:module';import {fileURLToPath} from 'node:url';
import {Readable} from 'node:stream';import test from 'node:test';import assert from 'node:assert/strict';
const root=path.resolve(path.dirname(fileURLToPath(import.meta.url)),'../..'),file=path.join(root,'scripts/public-backup-feature-browser.mjs'),source=fs.readFileSync(file,'utf8');
const context=vm.createContext({console,URL,Buffer,TextEncoder,TextDecoder,Uint8Array,structuredClone,Date,setTimeout,clearTimeout,queueMicrotask});
const control=new vm.SourceTextModule(source+'\nexport {canonical,archive,privateState,validState,sameState,exportState,environment,left,deviceOperation,seedRows,mergeExpected,foreignBackup,replacementExpected,downloadStream,fixtureIds,initialized};',{context,identifier:file});
const cache=new Map();function synthetic(name,values){if(!cache.has(name))cache.set(name,new vm.SyntheticModule(Object.keys(values),function(){for(const[key,value]of Object.entries(values))this.setExport(key,value);},{context,identifier:name}));return cache.get(name);}
await control.link(specifier=>{assert.equal(specifier,'node:crypto');return synthetic(specifier,{createHash});});await control.evaluate();const api=control.namespace;
async function load(file){if(!cache.has(file)){const code=fs.readFileSync(file,'utf8'),module=new vm.SourceTextModule(file.endsWith('.ts')?stripTypeScriptTypes(code,{mode:'transform'}):code,{context,identifier:file});cache.set(file,module);await module.link((specifier,importer)=>{if(specifier==='@noble/hashes/sha2.js')return synthetic(specifier,{sha256:bytes=>new Uint8Array(createHash('sha256').update(bytes).digest())});const target=path.resolve(path.dirname(importer.identifier),specifier);return load(target+(path.extname(target)?'':'.ts'));});}return cache.get(file);}
const stateModule=await load(path.join(root,'platform-integrations/public-api/device-id-state.ts'));await stateModule.evaluate();const state=stateModule.namespace;
const origin='http://127.0.0.1:61234',owner='00000000-0000-8000-8000-000000000000',scope={origin,serviceKey:null,catalogNamespace:'55555555-5555-8555-8555-555555555555'},ids={manga:'11111111-1111-8111-8111-111111111111',chapter:'22222222-2222-8222-8222-222222222222'};
function empty(){return {scope:structuredClone(scope),ownerId:owner,marker:{version:2,ownerId:owner,scope:structuredClone(scope)},revision:0,count:0,records:[],aliases:[],archives:[],issues:[],tombstones:[],migration_reason:null};}
function exported(raw){return {format:'weebdex-device-library',version:2,id_format:'public_uuid_with_unresolved_legacy',scope:raw.scope,ownerId:raw.ownerId,revision:raw.revision,exported_at:'2026-10-20T00:00:00Z',records:raw.records,aliases:raw.aliases,archives:raw.archives,issues:raw.issues,tombstones:raw.tombstones};}
function h(overrides={}){const browser={};return {origin,fixture:{ids:{manga:'SeedManga1',chapter:'SeedChapt1'},public_upload:{fixture_only:true,enabled:true,scenario:'public-upload-v1'}},page:{context:()=>browser},context:browser,insist:assert.ok,deadline:Date.now()+1000,...overrides};}
function model(initial,{upgrade=false,blocked=false,version=2,hang=false}={}){
 let durable=structuredClone(initial),closed=0,opened=0,aborts=0,transactions=0,touched=[],query;
 const stores=['records','meta','publicAliases','idMigrationArchives','idMigrationIssues','idMigrationTombstones'],fields={records:'records',publicAliases:'aliases',idMigrationArchives:'archives',idMigrationIssues:'issues',idMigrationTombstones:'tombstones'},metaFields={instanceScope:'scope',deviceOwnerId:'ownerId',publicIdOwner:'marker',revision:'revision',count:'count',scopeMigrationReason:'migration_reason'};
 const db={version,objectStoreNames:{contains:name=>stores.includes(name)},close(){closed++;},transaction(names,mode){transactions++;assert.deepEqual([...names].sort(),[...stores].sort());let draft=structuredClone(durable),pending=0,aborted=false,completed=false;
  const tx={abort(){if(aborted||completed)return;aborted=true;aborts++;queueMicrotask(()=>tx.onabort?.());},objectStore(name){assert.ok(stores.includes(name));const request=(value,write)=>{pending++;const row={};queueMicrotask(()=>{if(aborted)return;try{write?.();row.result=structuredClone(value);row.onsuccess?.();}catch{tx.abort();}pending--;queueMicrotask(()=>{if(pending||aborted||completed)return;completed=true;if(mode==='readwrite')durable=draft;tx.oncomplete?.();});});return row;};
   return {getAll(){return request(draft[fields[name]]);},get(key){assert.equal(name,'meta');return request(draft[metaFields[key]]);},add(row){assert.equal(name,'records');touched.push('records:add');return request(undefined,()=>{assert.equal(draft.records.some(old=>old.family===row.family&&old.key===row.key),false);draft.records.push(structuredClone(row));});},put(value,key){assert.equal(name,'meta');assert.ok(['count','revision'].includes(key));touched.push('meta:'+key);return request(undefined,()=>{draft[metaFields[key]]=value;});}};}};return tx;}};
 const indexedDB={open(name){opened++;assert.equal(name,'weebdex-device-library-v1:'+encodeURIComponent(origin));query={result:db,transaction:{abort(){aborts++;}}};if(!hang)queueMicrotask(()=>{if(upgrade)query.onupgradeneeded?.();else if(blocked)query.onblocked?.();else query.onsuccess?.();});return query;}};
 return {indexedDB,db,state:()=>structuredClone(durable),counts:()=>({opened,closed,aborts,transactions,touched:[...touched]}),late(){query.onsuccess?.();}};
}
async function deviceCall(m,args){const sandbox=vm.createContext({location:{origin},indexedDB:m.indexedDB,setTimeout,clearTimeout,queueMicrotask,structuredClone});const operation=vm.runInContext('('+api.deviceOperation.toString()+')',sandbox);return operation(args);}

test('two fixed exported cases are frozen in the required fresh-context order',()=>{assert.deepEqual(structuredClone(api.publicBackupFeatureCases.map(row=>row.id)),['backup-v2-merge-roundtrip','backup-v2-foreign-replacement-cancel']);assert.equal(Object.isFrozen(api.publicBackupFeatureCases),true);assert.ok(api.publicBackupFeatureCases.every(row=>Object.isFrozen(row)&&typeof row.run==='function'));});
test('exact frozen f2f panel/store source and backup-library block bind the selectors and restore API',()=>{
 const panel=fs.readFileSync(path.join(root,'platform-integrations/local-library/LocalLibraryPanel.svelte'));assert.equal(createHash('sha256').update(panel).digest('hex'),'f62dd91277fbb7f6e511601601072bd1657b56b68c40c18c8dec4847fc16ec5a');
 const store=fs.readFileSync(path.join(root,'platform-integrations/local-library/device-store.ts'));assert.equal(createHash('sha256').update(store).digest('hex'),'aedc80e3cb1a529f46643b02a7bb4998c6e5f2f1bb4e5b7f2b2a3a43453014ff');
 for(const label of ['Export device backup','Preview device backup','Review library import','Review backup merge','Review backup replacement','Confirm reviewed','Download pre-restore backup','Cancel import'])assert.ok(panel.toString().includes(label));
 assert.match(panel.toString(),/restoreWatchers.add\(changed\)/);assert.match(panel.toString(),/foreignOwnerConsent/);assert.doesNotMatch(source,/page\.route|\.clear\(|deleteDatabase|addInitScript|window\.[A-Za-z].*=|localStorage/);
 const lib=fs.readFileSync(path.join(root,'platform-integrations/local-library/library.ts'),'utf8'),block=lib.slice(lib.indexOf('export type ImportReview'),lib.indexOf('// Reviewed CSV/imported rows'));
 assert.equal(createHash('sha256').update(block).digest('hex'),'6efdad9a38a1fa76b1988c0a174a6b7faa450ebcd851d039d5ba86bab0ae7ef7');
});
test('fixture boundary refuses nonlocal/malformed origins, wrong profile and a foreign browser context',()=>{for(const origin of ['https://example.org','http://127.0.0.1:61234/path','http://name@127.0.0.1:61234','http://127.0.0.1'])assert.throws(()=>api.environment(h({origin}),35));assert.throws(()=>api.environment(h({fixture:{public_upload:{enabled:true,fixture_only:false}}}),35));assert.throws(()=>api.environment(h({context:{}}),35));assert.equal(api.environment(h(),35).origin,origin);});
test('durable snapshot validation refuses owner/scope/marker/revision/fence/collection mismatches',()=>{for(const change of [row=>row.ownerId='bad',row=>row.scope.origin='http://localhost:1',row=>row.marker.ownerId=ids.manga,row=>row.revision=-1,row=>row.count=1,row=>row.migration_reason='fenced',row=>row.archives=Array(17).fill({}),row=>row.scope.serviceKey='bad']){const row=empty();change(row);assert.throws(()=>api.validState(row,origin));}assert.equal(api.validState(empty(),origin).ownerId,owner);});
test('export checks reject extra fields, changed binding, incomplete data and wrong version',()=>{const raw=empty();api.exportState(exported(raw),raw);for(const change of [row=>row.privateKey='synthetic-disallowed',row=>row.version=1,row=>row.ownerId=ids.manga,row=>row.records=[{}],row=>row.exported_at='bad']){const backup=exported(empty());change(backup);assert.throws(()=>api.exportState(backup,raw));}});
test('independent fixture archive digest matches actual production canonicalization and validation',()=>{const row=api.seedRows(ids)[0],record=api.archive(row,owner,scope);assert.equal(record.digest,state.libraryDigest({ownerId:owner,scope,record:row}));const raw={...empty(),records:api.seedRows(ids),revision:1,count:4,archives:[record]};const checked=state.validatePublicDeviceBackup(exported(raw));assert.equal(checked.archives[0].digest,record.digest);});
test('foreign replacement derives only known rows from first export and preserves foreign archive provenance',()=>{const initial={...empty(),records:api.seedRows(ids),revision:1,count:4},original=exported(initial),foreign=api.foreignBackup({original}),checked=state.validatePublicDeviceBackup(foreign);assert.equal(checked.records.length,1);assert.equal(checked.records[0].id,'a'.repeat(32));assert.notEqual(checked.ownerId,owner);assert.equal(checked.archives[0].ownerId,checked.ownerId);assert.deepEqual(checked.archives[0].record,original.records[0]);assert.equal(original.records.length,4);});
test('merge/replacement expected states require changed durable revisions and exact retained preimages',()=>{const before={...empty(),records:api.seedRows(ids),revision:1,count:4},incoming=exported(structuredClone(before));incoming.records[0].status='completed';incoming.records[0].updated_at=110;const merged=api.mergeExpected(before,incoming);assert.equal(merged.revision,2);assert.equal(merged.archives.length,1);assert.equal(merged.archives[0].record.status,'reading');const input=api.foreignBackup({original:exported(before)}),result=api.replacementExpected(merged,input);assert.equal(result.ownerId,owner);assert.equal(result.revision,3);assert.equal(result.archives.length,6);assert.equal(result.records.length,1);assert.throws(()=>api.sameState(result,merged,'closed-refusal'),/closed-refusal/);});
test('static browser read resolves only on the modeled transaction completion and closes its own connection',async()=>{const m=model(empty()),value=await deviceCall(m,{origin});assert.equal(value.revision,0);assert.deepEqual(m.state(),empty());assert.equal(m.counts().closed,1);assert.deepEqual(m.counts().touched,[]);});
test('static browser seed CAS only adds four fixture rows and count/revision in one modeled transaction',async()=>{const before=empty(),m=model(before),rows=api.seedRows(ids);await deviceCall(m,{origin,seed:rows,expected:before});assert.equal(m.state().records.length,4);assert.equal(m.state().revision,1);assert.equal(m.state().count,4);assert.deepEqual(m.counts().touched,['records:add','records:add','records:add','records:add','meta:count','meta:revision']);assert.equal(m.counts().closed,1);});
test('seed refuses stale/foreign-populated state, missing schema or an uninitialized database without writes',async()=>{for(const setup of [()=>({state:{...empty(),revision:1}}),()=>({state:{...empty(),records:[api.seedRows(ids)[0]],count:1}}),()=>({state:empty(),options:{version:1}}),()=>({state:empty(),options:{upgrade:true}})]){const{state:before,options}=setup(),m=model(before,options);await assert.rejects(()=>deviceCall(m,{origin,seed:api.seedRows(ids),expected:empty()}));assert.deepEqual(m.state(),structuredClone(before));assert.deepEqual(m.counts().touched,[]);}});
test('static database operation refuses a foreign page origin before opening any database',async()=>{const m=model(empty());await assert.rejects(()=>deviceCall(m,{origin:'http://localhost:1'}),/backup_device_origin/);assert.equal(m.counts().opened,0);});
test('typed fixture resolver uses only the two exact namespace-bound references and checks actual chapter parent',async()=>{
 const calls=[],reply=body=>({status:()=>200,body:async()=>Buffer.from(JSON.stringify(body))});
 const run=h({context:{request:{
  post:async(url,options)=>{calls.push({url,options});return reply({result:'ok',data:[{kind:'manga',legacy_id:'SeedManga1',id:ids.manga},{kind:'chapter',legacy_id:'SeedChapt1',id:ids.chapter}],unresolved:[]});},
  get:async url=>{calls.push({url});return reply({result:'ok',response:'entity',data:{id:ids.chapter,type:'chapter',relationships:[{id:ids.manga,type:'manga'}]}});}
 }}});
 const actual=await api.fixtureIds(run,scope);assert.deepEqual(structuredClone(actual),ids);assert.equal(calls[0].options.data.catalog_namespace,scope.catalogNamespace);assert.equal(calls[0].options.maxRedirects,0);assert.equal(calls[1].url,origin+'/api/v1/chapter/'+ids.chapter);
});
test('typed resolver rejects unresolved, duplicated/wrong typed IDs and wrong-parent resources without a fallback',async()=>{
 for(const wrongParent of[false,true]){
  const reply=body=>({status:()=>200,body:async()=>Buffer.from(JSON.stringify(body))});
  const run=h({context:{request:{
   post:async()=>reply({result:'ok',data:[{kind:'manga',legacy_id:'SeedManga1',id:ids.manga},{kind:wrongParent?'chapter':'manga',legacy_id:wrongParent?'SeedChapt1':'SeedManga1',id:ids.chapter}],unresolved:[]}),
   get:async()=>reply({result:'ok',response:'entity',data:{id:ids.chapter,type:'chapter',relationships:[{id:ids.chapter,type:'manga'}]}})
  }}});
  await assert.rejects(()=>api.fixtureIds(run,scope));
 }
});
test('reader stream opening is bounded and destroys a late stream instead of retaining it',async()=>{let deliver,destroyed=0;const run=h({deadline:Date.now()+5}),pending=api.downloadStream(run,{createReadStream:()=>new Promise(resolve=>{deliver=resolve;})});await assert.rejects(()=>pending,/backup_download_open_timeout/);deliver({destroy(){destroyed++;}});await Promise.resolve();await Promise.resolve();assert.equal(destroyed,1);});
test('case deadlines and outputs are fixed, bounded and contain no database/key/URL values',()=>{assert.match(source,/environment\(context,35\)/);assert.match(source,/environment\(context,25\)/);assert.match(source,/timeoutMs=3000/);assert.match(source,/timeoutMs>3000/);assert.match(source,/MAX_FILE=1024\*1024/);assert.match(source,/owners.delete\(h.page\)/);const returns=[...source.matchAll(/return \{((?:[a-z_]+:true,?)+)\};/g)].map(row=>row[1]);assert.equal(returns.length,2);for(const value of returns)assert.doesNotMatch(value,/ownerId|event_id|scope|url|digest|records:/);});

test('actual initialization reader waits for the app-owned UUID marker without admitting pending scope',async()=>{
 const pending={...empty(),marker:undefined,scope:{...scope,catalogNamespace:null}};let reads=0,waits=0;const run=h({page:{evaluate:async()=>++reads===1?pending:empty(),waitForTimeout:async()=>{waits++;}}});
 const result=await api.initialized(run);assert.equal(result.scope.catalogNamespace,scope.catalogNamespace);assert.equal(reads,2);assert.equal(waits,1);
});
test('static operation refuses invalid database timeout bounds before touching IndexedDB',async()=>{for(const timeoutMs of[0,3001,NaN]){const m=model(empty());await assert.rejects(()=>deviceCall(m,{origin,timeoutMs}),/backup_device_timeout_bound/);assert.equal(m.counts().opened,0);}});

// A finite Playwright-shaped driver executes the two exported run() functions.
// Its controls/confirmation are stand-ins, never claimed as a rendered UI or
// production commit. Static device operations still execute the model above.
function journeyDriver(initial=empty()){
 let raw=structuredClone(initial),url=origin+'/',modal=false,input,prepared,ack=false,notice='',waiting,downloaded=0,confirmations=0,cancels=0;
 const callLog=[],reply=body=>({status:()=>200,body:async()=>Buffer.from(JSON.stringify(body))});
 const browser={request:{post:async(_url,options)=>{callLog.push('resolve');assert.equal(options.data.catalog_namespace,scope.catalogNamespace);return reply({result:'ok',data:[{kind:'manga',legacy_id:'SeedManga1',id:ids.manga},{kind:'chapter',legacy_id:'SeedChapt1',id:ids.chapter}],unresolved:[]});},get:async()=>reply({result:'ok',response:'entity',data:{id:ids.chapter,type:'chapter',relationships:[{id:ids.manga,type:'manga'}]}})}};
 function complete(){const archives=new Map(raw.archives.map(row=>[row.digest,row]));for(const row of input.archives)archives.set(row.digest,row);let rows;
  if(prepared==='merge'){rows=raw.records.map(old=>{const incoming=input.records.find(row=>row.family===old.family&&row.key===old.key);if(incoming&&incoming.updated_at>old.updated_at){const archived=state.archiveRecord(old,raw.ownerId,raw.scope);archives.set(archived.digest,structuredClone(archived));return structuredClone(incoming);}return old;});}
  else{rows=structuredClone(input.records);for(const old of raw.records){const candidate=rows.find(row=>row.family===old.family&&row.key===old.key);if(!candidate||api.canonical(candidate)!==api.canonical(old)){const archived=state.archiveRecord(old,raw.ownerId,raw.scope);archives.set(archived.digest,structuredClone(archived));}}}
  raw={...raw,records:rows,count:rows.length,revision:raw.revision+1,archives:[...archives.values()]};confirmations++;notice=prepared==='merge'?'Backup merged locally.':'Device library restored from the reviewed backup.';modal=false;prepared=undefined;
 }
 function locator(kind,name){const control={
  async count(){return kind==='checkbox'?Number(modal&&input.ownerId!==raw.ownerId):1;},
  async waitFor({state:requested}){if(kind==='dialog')assert.equal(modal,requested!=='hidden');else if(requested==='visible'){if(kind==='button'&&name.startsWith('Confirm reviewed'))assert.equal(name,prepared==='merge'?'Confirm reviewed merge':'Confirm reviewed replacement');if(kind==='text')assert.equal(name,notice);}},
  async isEnabled(){return !(kind==='button'&&name.startsWith('Review backup')&&input.ownerId!==raw.ownerId&&!ack);},
  async click(){callLog.push(name);assert.equal(await control.isEnabled(),true);if(name==='Export device backup'||name==='Download pre-restore backup'){assert.ok(waiting,'Listener precedes real download click');downloaded++;const snapshot=exported(structuredClone(raw));waiting({createReadStream:async()=>Readable.from([Buffer.from(JSON.stringify(snapshot))])});waiting=undefined;}
   else if(name==='Review backup merge'||name==='Review backup replacement'){assert.equal(modal,true);prepared=name.endsWith('merge')?'merge':'replace';}
   else if(name.startsWith('Confirm reviewed'))complete();else if(name==='Cancel import'){assert.equal(modal,true);modal=false;prepared=undefined;ack=false;cancels++;}},
  async isChecked(){return ack;},async check(){assert.equal(modal,true);ack=true;},
  async setInputFiles(file){input=structuredClone(state.validatePublicDeviceBackup(JSON.parse(file.buffer.toString())));modal=true;prepared=undefined;ack=false;},
  getByRole(role,options={}){return locator(role,options.name??'');},getByLabel(label){assert.equal(label,'Preview device backup');return locator('file',label);},getByText(text){return locator('text',text);}
 };return control;}
 const page={context:()=>browser,url:()=>url,async goto(next){url=next;return{status:()=>200};},async reload(){return{status:()=>200};},locator:selector=>{assert.equal(selector,'[data-local-library-panel]');return locator('panel','');},getByRole:(role,options)=>{assert.equal(role,'dialog');assert.equal(options.name,'Review library import');return locator('dialog',options.name);},async waitForTimeout(){},waitForEvent(event){assert.equal(event,'download');assert.equal(waiting,undefined);return new Promise(resolve=>{waiting=resolve;});},async evaluate(fn,args){assert.equal(fn,api.deviceOperation);const m=model(raw);const value=await deviceCall(m,args);raw=m.state();return value;}};
 return {h:h({page,context:browser}),snapshot:()=>structuredClone(raw),counts:()=>({downloaded,confirmations,cancels}),log:callLog};
}
test('both actual exported cases complete in order under the finite driver and return only their fixed proof booleans',async()=>{const driver=journeyDriver();const results=[];for(const entry of api.publicBackupFeatureCases)results.push(await entry.run(driver.h));assert.ok(results.every(row=>Object.values(row).every(value=>value===true)));assert.deepEqual(driver.counts(),{downloaded:4,confirmations:2,cancels:1});const final=driver.snapshot();assert.equal(final.count,1);assert.equal(final.revision,3);assert.equal(final.archives.length,6);assert.equal(final.ownerId,owner);assert.equal(driver.log.filter(value=>value==='Review backup replacement').length,2);});
test('actual first case refuses a nonempty fixture database without seed or confirmation',async()=>{const initial={...empty(),records:[structuredClone(api.seedRows(ids)[0])],count:1},driver=journeyDriver(initial);await assert.rejects(()=>api.publicBackupFeatureCases[0].run(driver.h),/backup_fresh_device_required/);assert.deepEqual(driver.snapshot(),initial);assert.equal(driver.counts().confirmations,0);assert.equal(driver.log.length,0);});
test('actual second case cannot run without a successful first case in that same page',async()=>{const driver=journeyDriver();await assert.rejects(()=>api.publicBackupFeatureCases[1].run(driver.h),/backup_merge_prerequisite/);assert.deepEqual(driver.snapshot(),empty());assert.equal(driver.counts().confirmations,0);});

test('actual first backup failure reports its owned initialization stage and fixed check category',async()=>{
 const initial={...empty(),records:[structuredClone(api.seedRows(ids)[0])],count:1},driver=journeyDriver(initial);
 let original;try{await api.publicBackupFeatureCases[0].run(driver.h);}catch(error){original=error;}
 assert.match(original.message,/backup_fresh_device_required/);
 assert.deepEqual(structuredClone(api.backupFailureDiagnostic(original)),{stage:'initialize-device',category:'check'});
 assert.deepEqual(driver.snapshot(),initial);assert.equal(driver.counts().confirmations,0);
});
test('dependent replacement refusal has a distinct prerequisite diagnostic without reads or mutations',async()=>{
 const driver=journeyDriver();let original;try{await api.publicBackupFeatureCases[1].run(driver.h);}catch(error){original=error;}
 assert.deepEqual(structuredClone(api.backupFailureDiagnostic(original)),{stage:'merge-prerequisite',category:'prerequisite'});
 assert.equal(driver.counts().confirmations,0);assert.deepEqual(driver.snapshot(),empty());
});
test('actual opening operation preserves original failure and never reads exception text or getters',async()=>{
 for(const error of [Object.defineProperty(new Error('secret URL and private value'), 'name',{get(){throw Error('must not inspect getter');}}),new Proxy({}, {get(){throw Error('must not inspect properties');},getOwnPropertyDescriptor(){throw Error('hostile descriptor');}})]){
  const driver=journeyDriver();driver.h.page.goto=async()=>{throw error;};let caught;
  try{await api.publicBackupFeatureCases[0].run(driver.h);}catch(value){caught=value;}
  assert.equal(caught,error);const diagnostic=api.backupFailureDiagnostic(error);
  assert.deepEqual(structuredClone(diagnostic),{stage:'open-settings',category:'operation'});
  assert.equal(JSON.stringify(diagnostic).includes('secret'),false);
 }
});
test('timeout classification uses only a fixed own data property and unknown primitives fail closed',async()=>{
 const driver=journeyDriver(),error=Object.defineProperty(new Error('not exported'),'name',{value:'TimeoutError'});
 driver.h.page.goto=async()=>{throw error;};await assert.rejects(()=>api.publicBackupFeatureCases[0].run(driver.h));
 assert.deepEqual(structuredClone(api.backupFailureDiagnostic(error)),{stage:'open-settings',category:'timeout'});
 for(const raw of [null,undefined,'private string',42,Symbol('private')])assert.deepEqual(structuredClone(api.backupFailureDiagnostic(raw)),{stage:'unavailable',category:'operation'});
});
test('actual merge traversal retains the precise preview/review/confirmation boundary on failure',async()=>{
 for(const [method,event,expected] of [['setInputFiles',null,'preview-merge'],['click','Review backup merge','review-merge'],['click','Confirm reviewed merge','confirm-merge']]){
  const driver=journeyDriver(),failure=Error('private failure details'),wrap=locator=>new Proxy(locator,{get(target,key){const value=target[key];if(['getByRole','getByLabel','getByText'].includes(key))return (...args)=>{const child=value(...args);const name=key==='getByRole'?args[1]?.name:args[0];return wrap(new Proxy(child,{get(inner,property){if(property===method&&(!event||name===event))return async()=>{throw failure;};return inner[property];}}));};return value;}});
  const panel=driver.h.page.locator,role=driver.h.page.getByRole;driver.h.page.locator=(...args)=>wrap(panel(...args));driver.h.page.getByRole=(...args)=>wrap(role(...args));
  let caught;try{await api.publicBackupFeatureCases[0].run(driver.h);}catch(error){caught=error;}
  assert.equal(caught,failure);assert.deepEqual(structuredClone(api.backupFailureDiagnostic(caught)),{stage:expected,category:'operation'});
  assert.equal(driver.counts().confirmations,0);
 }
});
