// Actual feature dispatcher with finite case/step stand-ins; no browser runs.
import assert from 'node:assert/strict';
import test from 'node:test';
import fs from 'node:fs';
import vm from 'node:vm';
import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
const file = new URL('../../scripts/public-feature-recovery-browser.mjs', import.meta.url);
const source = fs.readFileSync(file, 'utf8');
const ids = ['backup-v2-merge-roundtrip','backup-v2-foreign-replacement-cancel','public-search-complete-tag-options','public-search-default-sort-ascending','public-search-empty-rating-selection','chapter-edit-entry-signed'];
async function fixture({ fail = [], changedId, prior = [], terminal = true, extraStep = false } = {}) {
  const calls = [], context = vm.createContext({}), report = {steps: [...prior]}, errors = new Map();
  const rows = ids.map((id,index) => ({id: changedId?.[0] === index ? changedId[1] : id, async run() { calls.push(id); if (fail.includes(index)) { const error = Error('private fixture failure ' + index); errors.set(index,error); throw error; } return {finite: true}; }}));
  const module = new vm.SourceTextModule(source, {context,identifier:file.href});
  const parts = {
    'node:assert/strict': {default:assert},
    './public-backup-feature-browser.mjs': {publicBackupFeatureCases:rows.slice(0,2),backupFailureDiagnostic:()=>({stage:'unavailable',category:'operation'})},
    './public-search-feature-browser.mjs': {publicSearchFeatureCases:rows.slice(2,5)},
    './public-chapter-feature-browser.mjs': {publicChapterFeatureCases:rows.slice(5)},
  };
  await module.link(name => new vm.SyntheticModule(Object.keys(parts[name]),function(){for(const [key,value] of Object.entries(parts[name]))this.setExport(key,value);},{context}));
  await module.evaluate();
  let saves = 0;
  const input = {phase:'public-feature-recovery', report, save(){saves++;}, async step(name,action) {
    const row = {name};report.steps.push(row);
    try {row.observation=await action();if(terminal)row.status='passed';}
    catch(error){row.error=error;row.status='failed';}
    if(extraStep)report.steps.push({name:'unexpected',status:'passed'});
  }};
  return {run:()=>module.namespace.runPublicFeatureRecovery(input),input,report,calls,errors,get saves(){return saves;},ids:module.namespace.publicFeatureCaseIds};
}
test('all six fixed feature cases execute in backup-first order with terminal public facts only',async()=>{
  const f=await fixture();await f.run();assert.deepEqual(f.calls,ids);assert.equal(f.report.public_features.completed,true);
  assert.deepEqual(JSON.parse(JSON.stringify(f.report.public_features.checks)),ids.map((id,index)=>({id,status:'passed',...(index<2?{stage:'complete',category:'none'}:{})})));
  assert.equal(f.report.public_features.actual_context_closed,false);assert.equal(f.saves,8);
  assert(!JSON.stringify(f.report.public_features).includes('observation'));
});
test('an early failed feature preserves its actual error and does not suppress later independent cases',async()=>{
  const f=await fixture({fail:[0]});await f.run();assert.deepEqual(f.calls,ids);assert.equal(f.report.public_features.completed,true);
  assert.equal(f.report.steps[0].error,f.errors.get(0));assert.equal(f.report.public_features.checks[0].status,'failed');
  assert(f.report.public_features.checks.slice(1).every(row=>row.status==='passed'));assert(!JSON.stringify(f.report.public_features).includes('private'));
});
test('multiple failed cases are retained without promotion',async()=>{
  const f=await fixture({fail:[1,4,5]});await f.run();
  assert.deepEqual(Array.from(f.report.public_features.checks,row=>row.status),['passed','failed','passed','passed','failed','failed']);
});
test('an unresolved step cannot advance its prefix or fabricate completion',async()=>{
  const f=await fixture({terminal:false});await assert.rejects(f.run);assert.equal(f.calls.length,1);
  assert.equal(f.report.public_features.checks.length,0);assert.equal(f.report.public_features.completed,false);
});
test('a case cannot inject a second terminal step into the fixed registry',async()=>{
  const f=await fixture({extraStep:true});await assert.rejects(f.run);assert.equal(f.calls.length,1);assert.equal(f.report.public_features.checks.length,0);
});
test('unknown or duplicate registry IDs refuse before any case action',async()=>{
  for(const changedId of [[0,'private invented case'],[1,ids[0]]]){const f=await fixture({changedId});await assert.rejects(f.run);assert.deepEqual(f.calls,[]);assert.equal(f.report.public_features,undefined);}
});
test('the feature phase requires its exact name and a fresh step ledger',async()=>{
  const wrong=await fixture();wrong.input.phase='public-upload-recovery';await assert.rejects(wrong.run);assert.deepEqual(wrong.calls,[]);
  const prior=await fixture({prior:[{status:'passed'}]});await assert.rejects(prior.run);assert.deepEqual(prior.calls,[]);
});
test('browser dispatcher admits the new phase and records actual outer context closure',()=>{
  const text=fs.readFileSync(new URL('../../scripts/browser-acceptance.mjs',import.meta.url),'utf8');
  assert(text.includes('phase === publicFeatureRecoveryPhase ? runPublicFeatureRecovery : phase === publicUploadRecoveryPhase ? runPublicUploadRecovery'));
  const close=text.indexOf('await context.close();'),feature=text.indexOf('report.public_features.actual_context_closed = report.cleanup.context_closed;');
  assert(close>0&&feature>close);assert(text.includes('chromiumSandbox: true'));assert(text.includes('serviceWorkers: phase === "pwa" ? "allow" : "block"'));
});
test('actual prospective module exports compose exactly the fixed six-case registry without allocation',async()=>{
  const [backup,search,chapter,dispatcher]=await Promise.all([
    import('../../scripts/public-backup-feature-browser.mjs'),
    import('../../scripts/public-search-feature-browser.mjs'),
    import('../../scripts/public-chapter-feature-browser.mjs'),
    import('../../scripts/public-feature-recovery-browser.mjs'),
  ]);
  const arrays=[backup.publicBackupFeatureCases,search.publicSearchFeatureCases,chapter.publicChapterFeatureCases];
  assert(arrays.every(Object.isFrozen));const rows=arrays.flat();
  assert(rows.every(row=>Object.isFrozen(row)&&typeof row.run==='function'));
  assert.deepEqual(rows.map(row=>row.id),ids);assert.deepEqual([...dispatcher.publicFeatureCaseIds],ids);
});
for(const fail of [[],[0,5]])test('actual dispatcher to actual closed Python projector, failures='+fail.join(','),async()=>{
  const f=await fixture({fail});await f.run();
  // Explicit finite outer-cleanup stand-in. No actual browser/process is owned.
  f.report.phase='public-feature-recovery';f.report.cleanup={context_closed:true};
  f.report.public_features.actual_context_closed=true;
  const helper=fileURLToPath(new URL('../../scripts/ci/public-feature-diagnostics.py',import.meta.url));
  const program="import importlib.util,json,sys; s=importlib.util.spec_from_file_location('features',sys.argv[1]); m=importlib.util.module_from_spec(s); s.loader.exec_module(m); v=m.project(json.load(sys.stdin)); m.validate(v); print(json.dumps({'value':v,'passed':m.passed(v)}))";
  const raw=execFileSync('python3',['-B','-c',program,helper],{input:JSON.stringify(f.report),encoding:'utf8',timeout:5000,maxBuffer:65536,env:{...process.env,PYTHONDONTWRITEBYTECODE:'1'}});
  const value=JSON.parse(raw);assert.equal(value.passed,fail.length===0);assert.equal(value.value.record_valid,true);
  assert.deepEqual(value.value.checks,ids.map((id,index)=>({id,status:fail.includes(index)?'failed':'passed',...(index<2?{stage:fail.includes(index)?'unavailable':'complete',category:fail.includes(index)?'operation':'none'}:{})})));
  assert.equal(value.value.completed,true);assert.equal(value.value.raw_messages_exported,false);
  assert(!raw.includes('private fixture'));
});

test('a tampered terminal status cannot promote a caught backup failure',async()=>{
 const f=await fixture({fail:[0]});const original=f.input.step;f.input.step=async(name,action)=>{await original(name,action);f.report.steps.at(-1).status='passed';};
 await assert.rejects(f.run,/Backup outcome must match its owned diagnostic/);
 assert.equal(f.report.public_features.checks.length,0);assert.equal(f.report.public_features.completed,false);
});
test('a tampered failure status cannot fabricate a failure stage for a successful backup',async()=>{
 const f=await fixture();const original=f.input.step;f.input.step=async(name,action)=>{await original(name,action);f.report.steps.at(-1).status='failed';};
 await assert.rejects(f.run,/Backup outcome must match its owned diagnostic/);
 assert.equal(f.report.public_features.checks.length,0);assert.equal(f.report.public_features.completed,false);
});
