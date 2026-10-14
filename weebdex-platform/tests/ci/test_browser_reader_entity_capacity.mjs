#!/usr/bin/env node
// Real composed source and exact journey bodies run with explicit DOM/socket/
// transport stand-ins. No browser, listener, network, database or native denial.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import vm from 'node:vm';
import { EventEmitter } from 'node:events';
import { createRequire, stripTypeScriptTypes } from 'node:module';
import test from 'node:test';
import { runJourneys, ordinaryReaderDiagnosticChecks } from '../../scripts/browser-journeys.mjs';
import { ui11To16DiagnosticChecks } from '../../scripts/browser-ui11-16-journeys.mjs';
import { createJourneyCheck } from '../../scripts/journey-checks.mjs';
import { createFrontendOverlays } from '../../platform-integrations/frontend-overlays.mjs';
import { createReaderOverlays } from '../../platform-integrations/frontend-reader-overlays.mjs';
import { createLocalLibraryOverlays } from '../../platform-integrations/local-library/overlays.mjs';
const root = path.resolve(new URL('../../', import.meta.url).pathname), frontend = path.join(root, 'frontend');
const read = value => fs.readFileSync(path.join(root, value), 'utf8');
const ordinary = read('scripts/browser-journeys.mjs'), ui = read('scripts/browser-ui11-16-journeys.mjs');
const AsyncFunction = Object.getPrototypeOf(async function() {}).constructor;
const readerIds = ['open','double','deep-link','first-images','reload','reloaded-images','selected-page','quality','quality-images','single'].map(id => 'reader-spread-' + id);
const entityIds = ['entity-seed-pagination','entity-fixture', ...['group','author','user'].flatMap(kind => ['desc','asc'].flatMap(order => ['prerequisite','open','native-last-page','page-one-control','page-one-route','native-first-page'].map(id => `entity-${kind}-${order}-${id}`))), ...['withdrawal','open','empty-visible','native-empty','page-one-control','native-first-page'].map(id => 'entity-empty-' + id)];
const capacityIds = ['capacity-fixture','capacity-connections','capacity-saturated','capacity-live','capacity-cleanup'];
const insist = (condition, message) => assert(condition, message);
const until = async (read, valid, message) => { const value = await read(); assert(await valid(value), message); return value; };
function composed(relative) {
  const file = path.join(frontend, relative); let source = fs.readFileSync(file, 'utf8');
  for (const plugin of [createFrontendOverlays(frontend), createReaderOverlays(frontend), createLocalLibraryOverlays(frontend)]) source = plugin.transform.handler(source, file)?.code ?? source;
  return source;
}
test('new inventories are fixed, exact and frozen', () => {
  assert.deepEqual(ordinaryReaderDiagnosticChecks.reader, readerIds);
  assert.deepEqual(ui11To16DiagnosticChecks['ui14-entity-pagination'], entityIds);
  for (const ids of [ordinaryReaderDiagnosticChecks.reader,ui11To16DiagnosticChecks['ui14-entity-pagination']]) { assert(Object.isFrozen(ids)); assert.equal(new Set(ids).size,ids.length); }
});
test('actual retained Double normalization differs from real page2 initialization and quality rebuild', async () => {
  const provider = composed('src/lib/components/reader/reader-provider.svelte');
  const settings = composed('src/lib/components/reader/reader-settings.svelte');
  const spreadComponent = composed('src/lib/components/reader/reader-pages-double.svelte');
  const makeSpreads = provider.slice(provider.indexOf('\tconst makeSpreads ='), provider.indexOf('\n</script>'));
  const changeSpread = settings.slice(settings.indexOf('\tconst changeSpread ='),settings.indexOf('\n\tconst changeScale ='));
  const effect = provider.slice(provider.indexOf('\tlet previousOffset:'),provider.indexOf('\n</script>',provider.indexOf('\tlet previousOffset:')));
  const selectedExpression = spreadComponent.match(/const page = \$derived\(([^;]+)\);/)[1];
  assert.match(selectedExpression,/spread\.find\(candidate => candidate === reader\.currentPage\)/);
  const preferences = { double:false,offset:0 }, chapter = { data:[0,1,2].map(i=>({name:`original${i}`,dimensions:[600,900]})), data_optimized:[0,1,2].map(i=>({name:`optimized${i}`,dimensions:[300,450]})) };
  const revoked = [], reader = { pages:[],spreads:[],key:0,scrollIntoView() {},get currentSpread() { return this.spreads.find(spread=>spread.includes(this.currentPage)); } };
  const sandbox = vm.createContext({ reader,chapter,$readerPrefs:preferences,$dataSaver:false,page:{state:{},params:{page:'2'}},writable:()=>({}),URL:{revokeObjectURL:value=>revoked.push(value)},$effect:operation=>{sandbox.effect=operation;} });
  vm.runInContext(stripTypeScriptTypes(makeSpreads + '\n' + effect + '\n' + changeSpread + '\nthis.changeSpread = changeSpread;'),sandbox);
  sandbox.effect(); assert.equal(reader.currentPage.index,1);
  await sandbox.changeSpread(true); assert.equal(preferences.double,true); assert.equal(reader.currentPage.index,0,'Double gesture intentionally selects the first member');
  // A fresh provider for /2 runs the same initial effect while Double persists.
  const fresh = vm.createContext({ reader,chapter,$readerPrefs:preferences,$dataSaver:false,page:{state:{},params:{page:'2'}},writable:()=>({}),URL:{revokeObjectURL:value=>revoked.push(value)},$effect:operation=>{fresh.effect=operation;} });
  vm.runInContext(stripTypeScriptTypes(makeSpreads + '\n' + effect),fresh); fresh.effect();
  assert.equal(reader.currentPage.index,1); fresh.spread=reader.spreads[0];
  assert.equal(vm.runInContext(selectedExpression,fresh),reader.currentPage,'Real composed double visibility follows the selected second member');
  reader.pages.forEach((page,i)=>page.blob=`blob:old${i}`); fresh.$dataSaver=true; fresh.effect(); fresh.spread=reader.spreads[0];
  assert.equal(reader.currentPage.index,1); assert.equal(reader.currentPage.data.name,'optimized1');
  assert.equal(vm.runInContext(selectedExpression,fresh),reader.currentPage); assert.equal(revoked.length,3);
});
for (const failAt of [undefined,...readerIds]) test('ordinary reader actual body records failure honestly: '+(failAt??'complete'),async()=>{
  const report={}, record=createJourneyCheck(report), calls=[]; let index=-1, selected=1;
  const page={
    getByRole(role,{name,exact}) { assert.equal(role,'button');assert.equal(exact,true);return {async click(){calls.push(name);if(name==='Double') selected=1;}}; },
    locator(selector) { assert.equal(selector,'img[src^="blob:"]');return {async evaluateAll(){return 2;}}; },
    async reload(options){assert.deepEqual(options,{waitUntil:'domcontentloaded'});calls.push('reload');},url(){return 'http://127.0.0.1:1/chapter/c/'+selected;}
  };
  const failure=Error('private reader state'), steps=[];
  await runJourneys({phase:'reader',page,context:{},origin:'http://127.0.0.1:1',fixture:{ids:{chapter:'c'}},report,redactions:new Set(),insist,
    async open(route){assert.equal(route,'/chapter/c/2');selected=2;calls.push('open2');},
    async step(_name,operation){if(++index!==1)return;try{await operation();steps.push('passed');}catch(error){assert.equal(error,failure);steps.push('failed');}},
    check:(id,operation)=>record(id,async()=>{if(id===failAt)throw failure;return operation();})});
  assert.deepEqual(steps,[failAt?'failed':'passed']);
  const reached=failAt?readerIds.slice(0,readerIds.indexOf(failAt)+1):readerIds;
  assert.deepEqual(report.journey_checks.map(row=>row.check_id),reached);
  assert.deepEqual(report.journey_checks.map(row=>row.status),reached.map(id=>id===failAt?'failed':'passed'));
  if(!failAt)assert.deepEqual(calls,['open2','Double','open2','reload','Optimized','Single']);
  assert.doesNotMatch(JSON.stringify(report),/private reader|127\.0\.0\.1/);
});
test('installed Bits UI aria label is forwarded onto retained paginator anchors',()=>{
  assert(process.env.WEEBDEX_FRONTEND_ROOT,'Use verified exact-lock dependency root');
  assert.equal(fs.readFileSync(path.join(process.env.WEEBDEX_FRONTEND_ROOT,'yarn.lock'),'utf8'),read('frontend/yarn.lock'),'Installed Bits UI is bound to the exact retained frontend lockfile');
  const require=createRequire(path.join(process.env.WEEBDEX_FRONTEND_ROOT,'package.json'));
  const bits=path.dirname(require.resolve('bits-ui'));
  const state=fs.readFileSync(path.join(bits,'bits/pagination/pagination.svelte.js'),'utf8');
  assert.match(state,/"aria-label": `Page \$\{this\.opts\.page\.current\.value\}`/);
  const page=fs.readFileSync(path.join(bits,'bits/pagination/components/pagination-page.svelte'),'utf8');
  assert.match(page,/mergeProps\(restProps, pageState\.props, \{ type \}\)/);
  const retained=read('frontend/src/lib/components/GenericPagination.svelte');
  assert.match(retained,/<a href=\{makePath\(page.value\)\} \{\.\.\.props\}>/);
  assert.match(retained,/\{page.value\}/);
  assert.doesNotMatch(ui.slice(ui.indexOf('if (phase === "ui14-entity-pagination")'),ui.indexOf('if (phase === "ui14-account-failures"')),/name: "1"/);
});
const entityStart=ui.indexOf('  if (phase === "ui14-entity-pagination") {')+'  if (phase === "ui14-entity-pagination") {'.length;
const entityEnd=ui.indexOf('\n  if (phase === "ui14-account-failures"',entityStart);
const entityBody=ui.slice(entityStart,entityEnd).replace(/\n  }\s*$/,'');
for(const failAt of [undefined,...entityIds])test('actual entity branch scopes controls and closes observers: '+(failAt??'complete'),async()=>{
  const report={},record=createJourneyCheck(report),failure=Error('private entity state');let current,withdrawn=false,active,created=0,closed=0;const steps=[];
  const data={group_id:'g',author_id:'a',user_id:'u',total:101};
  const page={locator(selector){assert.equal(selector,'footer');return{getByRole(role,options){assert.equal(role,'link');assert.deepEqual(options,{name:'Page 1',exact:true});return{async waitFor(){},async click(){current.searchParams.delete('page');active.rows.push({status:200,body:{page:1,total:withdrawn?100:101,data:Array(100).fill({})}});}};}};},
    getByText(text,options){assert.equal(text,'No results found');assert.equal(options.exact,true);return{async waitFor(){assert(withdrawn);}};},async waitForURL(predicate){assert(predicate(current));}};
  const bindings={check:(id,operation)=>record(id,async()=>{if(id===failAt)throw failure;return operation();}),ensurePagination:async()=>{},dependency:name=>{assert.equal(name,'entity');return data;},insist,page,until,requestPath(){},
    async privateControl(action,body){assert.equal(action,'ui-regressions-withdraw');assert.deepEqual(body,{scenario:'entity'});withdrawn=true;return{ui_regressions:{entity:{current_total:100}}};},
    observe(){created++;return active={rows:[],async settle(){},async close(){closed++;}};},
    async open(route){current=new URL(route,'http://127.0.0.1:1');active.rows.push({status:200,body:{page:2,total:withdrawn?100:101,data:withdrawn?[]:[{}]}});},
    async step(_name,operation){try{await operation();steps.push('passed');}catch(error){assert.equal(error,failure);steps.push('failed');}}};
  const execute=new AsyncFunction(...Object.keys(bindings),entityBody)(...Object.values(bindings));
  if(failAt&&['entity-seed-pagination','entity-fixture'].includes(failAt))await assert.rejects(execute,error=>error===failure);else await execute;
  assert.equal(created,closed);assert.equal(steps.filter(status=>status==='failed').length,failAt&&!['entity-seed-pagination','entity-fixture'].includes(failAt)?1:0);
  const failed=report.journey_checks.filter(row=>row.status==='failed');assert.deepEqual(failed.map(row=>row.check_id),failAt?[failAt]:[]);
  if(!failAt)assert.deepEqual(report.journey_checks.map(row=>row.check_id),entityIds);
  assert.doesNotMatch(JSON.stringify(report),/private entity|127\.0\.0\.1/);
});
const capacityStart=ui.indexOf('const withCapacity = async (operation, probe = (_suffix, action) => action()) => {')+'const withCapacity = async (operation, probe = (_suffix, action) => action()) => {'.length;
const capacityEnd=ui.indexOf('\n  };\n\n  // Native platform fault gates',capacityStart);
assert(capacityStart>0&&capacityEnd>capacityStart);const capacityBody=ui.slice(capacityStart,capacityEnd);
for(const failAt of [undefined,...capacityIds,'wrong-native-status','socket-before-connect','watchdog-expired','operation'])test('actual capacity helper preserves finite fault and teardown: '+(failAt??'complete'),async()=>{
  const report={},record=createJourneyCheck(report),requests=[],sockets=[],failure=Error('private socket state');let watchdog,cleared=false,operations=0;
  const transport={request(url,options){assert.equal(url,'http://127.0.0.1:1/api/client');assert.deepEqual(options,{method:'POST',headers:{Origin:'http://127.0.0.1:1','Content-Type':'application/json','Content-Length':'65536'},agent:false});
    const socket=new EventEmitter();socket.closed=false;socket.destroy=()=>{if(!socket.closed){socket.closed=true;socket.emit('close');}};sockets.push(socket);
    const request=new EventEmitter();request.write=value=>assert.equal(value,'{"name":"');request.destroy=()=>socket.destroy();request.setTimeout=duration=>assert.equal(duration,10000);requests.push(request);
    queueMicrotask(()=>{request.emit('socket',socket);if(failAt==='socket-before-connect'&&requests[0]===request)request.emit('error',failure);else socket.emit('connect');});return request;}};
  const bindings={operation:async()=>{operations++;if(failAt==='operation')throw failure;},probe:(id,operation)=>record(id,async()=>{if(id===failAt)throw failure;return operation();}),origin:'http://127.0.0.1:1',fixture:{fixture_only:true},insist,http:transport,
    setTimeout(operation,duration){assert.equal(duration,9000);watchdog=operation;return 1;},clearTimeout(id){assert.equal(id,1);cleared=true;},
    async api(route){assert.equal(route,'/api/manga?limit=1');if(failAt==='watchdog-expired')watchdog();return{status:failAt==='wrong-native-status'?200:429,body:{code:'capacity_busy'}};},
    async until(read,valid,message,timeout){assert.equal(timeout,3000);return until(read,valid,message);}};
  const execute=new AsyncFunction(...Object.keys(bindings),capacityBody)(...Object.values(bindings));
  if(failAt)await assert.rejects(execute);else await execute;
  assert(sockets.every(socket=>socket.closed));if(failAt!=='capacity-fixture')assert(cleared);
  if(!failAt||['wrong-native-status','socket-before-connect','watchdog-expired','operation','capacity-saturated','capacity-live','capacity-cleanup'].includes(failAt))assert.equal(requests.length,32);
  if(!failAt)assert.deepEqual(report.journey_checks.map(row=>row.check_id),capacityIds);
  if(['wrong-native-status','watchdog-expired'].includes(failAt))assert.equal(operations,0);
  const ids=report.journey_checks.filter(row=>row.status==='failed').map(row=>row.check_id);
  if(failAt==='wrong-native-status')assert.deepEqual(ids,['capacity-saturated']);
  if(failAt==='watchdog-expired')assert.deepEqual(ids,['capacity-live']);
  assert.doesNotMatch(JSON.stringify(report),/private socket|127\.0\.0\.1/);
});
