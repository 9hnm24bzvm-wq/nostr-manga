#!/usr/bin/env node
// Real artifact omissions and a real denied write in an owned temporary cache.
// Never fabricate obsolete checker artifacts or mutate a real dependency cache.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {createHash} from 'node:crypto';
import {spawnSync} from 'node:child_process';
import {fileURLToPath} from 'node:url';
const root=path.resolve(path.dirname(fileURLToPath(import.meta.url)),'../..');
assert.ok(process.argv[2]&&process.argv[3],'Usage: test_semantic_validation_fail_closed.mjs GENERATION OUTER_QUALIFICATION');
const work=path.resolve(process.argv[2]),outer=path.resolve(process.argv[3]);
fs.mkdirSync(path.join(root,'.build'),{recursive:true});
const temporary=fs.mkdtempSync(path.join(root,'.build/semantic-negative-controls-'));
const validator=path.join(root,'tests/ci/test_composed_semantic_projection.mjs'),guard=path.join(root,'scripts/semantic-readonly-cache-guard.cjs'),results=[];
const hash=bytes=>createHash('sha256').update(bytes).digest('hex');
// One physical input clone is shared by all cases in this invocation. Only the
// owned evidence pointer switches; real generations and dependency caches stay read-only.
const inputRoots=['project','production-composed','production-input','production-stages'];
const generation=path.join(temporary,'generation');
let originalBefore,clonedBefore,completed=false,guardDeniedWriteReceipt=null,primaryFailure=null;
function snapshot(base,roots=['.']){
 const entries={};
 function visit(relative){
  const file=path.join(base,relative),info=fs.lstatSync(file,{bigint:true});
  const row={mode:Number(info.mode&0o777n),mtimeNs:String(info.mtimeNs),inode:String(info.ino),device:String(info.dev)};
  if(info.isSymbolicLink())Object.assign(row,{type:'symlink',target:fs.readlinkSync(file)});
  else if(info.isFile())Object.assign(row,{type:'file',bytes:Number(info.size),sha256:hash(fs.readFileSync(file))});
  else assert.ok(info.isDirectory(),'Unexpected generation object: '+file);
  if(info.isDirectory())row.type='directory';
  entries[relative]=row;
  if(info.isDirectory())for(const name of fs.readdirSync(file).sort())visit(relative==='.'?name:path.join(relative,name));
 }
 for(const relative of roots)visit(relative);
 return entries;
}
function retain(name,value){fs.writeFileSync(path.join(temporary,name+'.json'),JSON.stringify(value,null,2)+'\n');}
function evidenceView(destination){
 fs.mkdirSync(destination,{recursive:true});
 for(const item of fs.readdirSync(path.join(work,'evidence')))fs.symlinkSync(path.join(work,'evidence',item),path.join(destination,item));
}
function selectEvidence(destination){
 assert.ok(destination.startsWith(generation+path.sep),'Evidence view must stay in the owned clone');
 const pointer=path.join(generation,'evidence');
 if(fs.existsSync(pointer)){assert.ok(fs.lstatSync(pointer).isSymbolicLink());fs.unlinkSync(pointer);}
 fs.symlinkSync(destination,pointer);
}
try{
originalBefore=snapshot(work);
retain('original-generation-before',originalBefore);
const baseline=spawnSync(process.execPath,[validator,work,outer],{cwd:root,encoding:'utf8',timeout:30000});
retain('original-baseline-validation',{status:baseline.status,error:baseline.error?.message??null,stdout:baseline.stdout,stderr:baseline.stderr});
assert.equal(baseline.status,0,'Negative controls require a genuinely passing current baseline: '+baseline.stderr);
fs.mkdirSync(generation);
for(const item of inputRoots)fs.cpSync(path.join(work,item),path.join(generation,item),{recursive:true,dereference:false,verbatimSymlinks:true,force:false,errorOnExist:true});
clonedBefore=snapshot(generation,inputRoots);
for(const [relative,copied] of Object.entries(clonedBefore)){
 const original=originalBefore[relative];assert.ok(original,'Unexpected cloned input: '+relative);
 for(const field of ['type','mode','bytes','sha256','target'])assert.equal(copied[field],original[field],'Cloned input differs: '+relative+' '+field);
 if(copied.type==='file')assert.ok(copied.inode!==original.inode||copied.device!==original.device,'Clone must not hardlink real inputs: '+relative);
}
assert.deepEqual(Object.keys(clonedBefore).sort(),Object.keys(originalBefore).filter(relative=>inputRoots.some(item=>relative===item||relative.startsWith(item+'/'))).sort());
retain('cloned-inputs-before',clonedBefore);
const baselineEvidence=path.join(generation,'cases/baseline/evidence');evidenceView(baselineEvidence);selectEvidence(baselineEvidence);
const clonedBaseline=spawnSync(process.execPath,[validator,generation,outer],{cwd:root,encoding:'utf8',timeout:30000});
retain('cloned-baseline-validation',{status:clonedBaseline.status,error:clonedBaseline.error?.message??null,stdout:clonedBaseline.stdout,stderr:clonedBaseline.stderr});
assert.ok(!clonedBaseline.error&&clonedBaseline.status===0,'The physical cloned baseline must pass before any mutation: '+clonedBaseline.stderr);
const actualSummary=JSON.parse(fs.readFileSync(path.join(work,'evidence/summary.json'))),tsc=actualSummary.service_worker_receipt.command.find(value=>value.endsWith('/typescript/bin/tsc'));assert.ok(tsc);const physicalTsc=fs.realpathSync(tsc),nodeModules=physicalTsc.indexOf('/node_modules/');assert.ok(nodeModules>=0);const primaryRoot=physicalTsc.slice(0,nodeModules+'/node_modules'.length);
function fixture(name,expectedFailure,mutate){
 const destination=path.join(generation,'cases',name),qualification=path.join(destination,'qualification');
 evidenceView(path.join(destination,'evidence'));fs.mkdirSync(qualification);selectEvidence(path.join(destination,'evidence'));
 for(const item of fs.readdirSync(outer))if(item!=='cache-audit')fs.symlinkSync(path.join(outer,item),path.join(qualification,item));
 const auditPath='cache-audit/strict-physical';fs.mkdirSync(path.join(qualification,auditPath),{recursive:true});
 for(const item of fs.readdirSync(path.join(outer,auditPath)))fs.symlinkSync(path.join(outer,auditPath,item),path.join(qualification,auditPath,item));
 const edit=(base,file,callback)=>{const target=path.join(base,file),value=JSON.parse(fs.readFileSync(target));fs.unlinkSync(target);fs.writeFileSync(target,JSON.stringify(callback(value),null,2)+'\n');};
 const artifact=(file,callback)=>edit(path.join(destination,'evidence'),file,callback),proof=(file,callback)=>edit(qualification,file,callback);
 const audits=callback=>{let manifest=JSON.parse(fs.readFileSync(path.join(qualification,'cache-audit-summary-physical.json')));for(const record of manifest.recordsManifest.filter(x=>x.file.startsWith(auditPath+'/'))){edit(qualification,record.file,callback);const changed=JSON.parse(fs.readFileSync(path.join(qualification,record.file)));record.roots=changed.protected_roots.length;record.sha256=hash(fs.readFileSync(path.join(qualification,record.file)));}proof('cache-audit-summary-physical.json',()=>manifest);};
 const compressed=script=>{const child=spawnSync(process.env.PYTHON??'python3',['-c',script,qualification],{encoding:'utf8',timeout:10000});assert.equal(child.status,0,child.stderr);};
 mutate({artifact,proof,audits,compressed});
 const checked=spawnSync(process.execPath,[validator,generation,qualification],{cwd:root,encoding:'utf8',timeout:30000});
 fs.writeFileSync(path.join(destination,'validation-result.json'),JSON.stringify({probe:name,expectedFailure,status:checked.status,error:checked.error?.message??null,stdout:checked.stdout,stderr:checked.stderr},null,2)+'\n');
 assert.ok(checked.status!==0&&!checked.error,'Incomplete artifact fixture unexpectedly passed: '+name);
 assert.ok(checked.stderr.includes(expectedFailure),'Negative control failed for an unrelated reason: '+name+'\n'+checked.stderr);
 results.push({probe:name,status:'rejected',exit_code:checked.status,first_error:checked.stderr.split('\n').find(line=>line.includes('AssertionError'))??checked.stderr.split('\n')[0]});
 retain('completed-negative-results',results);
}
fixture('empty-canonical-inventory','Actual canonical inventory must contain 679 records',({artifact})=>artifact('canonical-source-hashes.json',()=>({})));
fixture('missing-map-manifest-entry','Every actual frontend source must have one manifest entry',({artifact})=>artifact('projection-manifest.json',items=>items.filter(x=>x.projection!=='src/app.d.ts')));
fixture('forged-composed-transform-output','Actual reviewed transform output differs:',({artifact})=>artifact('projection-manifest.json',items=>{const target=items.find(entry=>entry.stages.length>0);assert.ok(target);target.production_composed_sha256='0'.repeat(64);return items;}));
fixture('missing-map-validation-entry','Map validation identities must cover every manifest map',({artifact})=>artifact('source-map-validation.json',value=>({...value,maps:value.maps.slice(1)})));
fixture('empty-dependency-inventory','Actual dependency inventory must not be empty',({proof})=>{proof('physical-dependencies-before-summary.json',()=>({}));proof('physical-dependencies-after-summary.json',()=>({}));});
fixture('no-cache-process-audits','At least one actual strict cache-guard audit is required',({proof})=>proof('cache-audit-summary-physical.json',value=>({...value,recordsManifest:value.recordsManifest.filter(x=>!x.file.startsWith('cache-audit/strict-physical/'))})));
fixture('uncertain-owned-cleanup','Actual strict owner needs unforced, certain cleanup and terminal EOF',({proof})=>proof('strict-resource-receipt.json',value=>({...value,cleanup:{...value.cleanup,tracking_errors:1}})));
fixture('forced-owned-cleanup','Actual strict owner needs unforced, certain cleanup and terminal EOF',({proof})=>proof('strict-resource-receipt.json',value=>({...value,cleanup:{...value.cleanup,forced:true}})));
fixture('missing-owned-birth','Owned strict samples require exact process birth identities',({proof})=>proof('strict-resource-receipt.json',value=>({...value,samples:value.samples.map(sample=>({...sample,owned:sample.owned.map(({birth,...row})=>row)}))})));
fixture('empty-audit-protected-roots','Each strict audit must declare physical cache roots',({audits})=>audits(value=>({...value,protected_roots:[]})));
fixture('missing-physical-primary-root','Missing physical dependency root inventory:',({proof})=>{const remove=value=>{assert.ok(Object.hasOwn(value,primaryRoot),'Primary root must be present before negative removal');const next=Object.fromEntries(Object.entries(value).filter(([key])=>key!==primaryRoot));assert.equal(Object.keys(next).length,Object.keys(value).length-1);return next;};proof('physical-dependencies-before-summary.json',remove);proof('physical-dependencies-after-summary.json',remove);proof('physical-dependency-root-discovery.json',value=>{assert.ok(value.roots.includes(primaryRoot));return{...value,roots:value.roots.filter(key=>key!==primaryRoot)};});});
fixture('unprotected-physical-primary-root','Each strict audit must protect every actual physical dependency root:',({audits})=>audits(value=>{assert.ok(value.protected_roots.includes(primaryRoot),'Primary root must be protected before negative removal');const roots=value.protected_roots.filter(key=>key!==primaryRoot);assert.equal(roots.length,value.protected_roots.length-1);return{...value,protected_roots:roots};}));
fixture('changed-cache-metadata-snapshot','Physical dependency bytes or metadata changed between snapshots',({compressed})=>compressed(`import gzip,json,pathlib,sys
p=pathlib.Path(sys.argv[1])/'physical-dependencies-after.json.gz'
data=json.loads(gzip.decompress(p.read_bytes()));root=next(iter(data));entry=next(iter(data[root]));data[root][entry]['mtimeNs']+=1
p.unlink();p.write_bytes(gzip.compress(json.dumps(data,sort_keys=True,separators=(',',':')).encode(),mtime=0))`));
fixture('forged-matching-cache-metadata','Current physical dependency bytes or metadata differ:',({compressed})=>compressed(`import gzip,json,pathlib,sys
for phase in ['before','after']:
 p=pathlib.Path(sys.argv[1])/('physical-dependencies-'+phase+'.json.gz')
 data=json.loads(gzip.decompress(p.read_bytes()));root=next(iter(data));entry=next(iter(data[root]));data[root][entry]['mtimeNs']+=1
 p.unlink();p.write_bytes(gzip.compress(json.dumps(data,sort_keys=True,separators=(',',':')).encode(),mtime=0))`));
fixture('missing-actual-graph-import','Graph must cover every actual resolved import literal:',({artifact})=>artifact('production-module-graph.json',edges=>edges.slice(1)));
const protectedDirectory = path.join(temporary, 'protected-probe-cache'), auditDirectory = path.join(temporary, 'guard-audits');
fs.mkdirSync(protectedDirectory); fs.mkdirSync(auditDirectory);
const probeFile = path.join(protectedDirectory, 'must-not-exist');
const cleanEnvironment = { ...process.env }; delete cleanEnvironment.NODE_OPTIONS; delete cleanEnvironment.WEEBDEX_READONLY_CACHE_ROOTS; delete cleanEnvironment.WEEBDEX_READONLY_AUDIT_DIR;
function guardProbe(name, environment) {
  const checked = spawnSync(process.execPath, ['--require', guard, '-e', `require('node:fs').writeFileSync(${JSON.stringify(probeFile)}, 'forbidden')`], { cwd: root, env: { ...cleanEnvironment, ...environment }, encoding: 'utf8', timeout: 30_000 });
  retain(name,{probe:name,status:checked.status,error:checked.error?.message??null,stdout:checked.stdout,stderr:checked.stderr});
  assert.ok(checked.status !== 0 && !checked.error, `Unsafe cache guard configuration unexpectedly passed: ${name}`);
  const expectedFailure = { 'guard-missing-protected-roots': 'requires configured protected roots', 'guard-empty-protected-roots': 'requires a nonempty array', 'guard-missing-audit-directory': 'requires an existing absolute audit directory', 'guard-valid-root-blocks-write': 'Read-only dependency cache mutation denied' }[name];
  assert.ok(checked.stderr.includes(expectedFailure), `Guard control failed for an unrelated reason: ${name}`);
  assert.equal(fs.existsSync(probeFile), false, name);
  results.push({ probe: name, status: 'rejected', exit_code: checked.status });
  retain('completed-negative-results',results);
}
guardProbe('guard-missing-protected-roots', { WEEBDEX_READONLY_AUDIT_DIR: auditDirectory });
guardProbe('guard-empty-protected-roots', { WEEBDEX_READONLY_CACHE_ROOTS: '[]', WEEBDEX_READONLY_AUDIT_DIR: auditDirectory });
guardProbe('guard-missing-audit-directory', { WEEBDEX_READONLY_CACHE_ROOTS: JSON.stringify([protectedDirectory]) });
guardProbe('guard-valid-root-blocks-write', { WEEBDEX_READONLY_CACHE_ROOTS: JSON.stringify([protectedDirectory]), WEEBDEX_READONLY_AUDIT_DIR: auditDirectory });
const audits = fs.readdirSync(auditDirectory).map(name => JSON.parse(fs.readFileSync(path.join(auditDirectory, name))));
assert.equal(audits.length, 1, 'Configured negative write must produce one guard receipt');
assert.equal(audits[0].protected_write_attempts, 1);
assert.deepEqual(audits[0].protected_roots, [protectedDirectory]);
guardDeniedWriteReceipt=audits[0];
retain('guard-denied-write-receipt',guardDeniedWriteReceipt);
completed=true;
}catch(error){
 primaryFailure=error;
}finally{
 // Preserve a primary setup/control failure; postflight failures are separate.
 const postflightErrors=[];
 const describe=error=>({name:error?.name??'Error',message:error?.message??String(error)});
 const postflight=(phase,action)=>{try{action();}catch(error){postflightErrors.push({phase,error});}};
 let originalUnchanged=false,cloneUnchanged=false,cachesUnchanged=false;
 if(originalBefore)postflight('original-generation',()=>{
  const originalAfter=snapshot(work);retain('original-generation-after',originalAfter);
  assert.deepEqual(originalAfter,originalBefore,'Original strict generation bytes or metadata changed');
  originalUnchanged=true;
 });
 if(clonedBefore)postflight('cloned-inputs',()=>{
  const clonedAfter=snapshot(generation,inputRoots);retain('cloned-inputs-after',clonedAfter);
  assert.deepEqual(clonedAfter,clonedBefore,'Shared physical clone inputs changed during negative cases');
  cloneUnchanged=true;
 });
 postflight('physical-dependencies',()=>{
  const physical=spawnSync(process.env.PYTHON??'python3',[path.join(root,'tests/ci/semantic_dependency_snapshot.py'),outer],{encoding:'utf8',timeout:30000,maxBuffer:1024*1024});
  retain('physical-dependencies-after',{status:physical.status,error:physical.error?.message??null,stdout:physical.stdout,stderr:physical.stderr});
  assert.ok(!physical.error&&physical.status===0,'Original physical dependency bytes or metadata changed: '+physical.stderr);
  cachesUnchanged=true;
 });
 const report={status:completed&&!primaryFailure&&postflightErrors.length===0&&originalUnchanged&&cloneUnchanged&&cachesUnchanged?'passed':'failed',rejected_negative_controls:results.length,results,primary_failure:primaryFailure?describe(primaryFailure):null,postflight_errors:postflightErrors.map(({phase,error})=>({phase,...describe(error)})),original_generation_unchanged:originalUnchanged,original_generation_check_skipped:!originalBefore,cloned_inputs_unchanged:cloneUnchanged,cloned_inputs_check_skipped:!clonedBefore,physical_dependency_snapshot_unchanged:cachesUnchanged,real_dependency_caches_written:cachesUnchanged?false:null,guard_denied_write_receipt:guardDeniedWriteReceipt,one_physical_clone:true,source_generation:work,fixture_generation:generation};
 postflight('final-report',()=>retain('negative-controls',report));
 if(postflightErrors.length){report.status='failed';report.postflight_errors=postflightErrors.map(({phase,error})=>({phase,...describe(error)}));}
 console.log(JSON.stringify({...report,evidence:path.relative(root,path.join(temporary,'negative-controls.json'))}));
 if(primaryFailure)throw primaryFailure;
 if(postflightErrors.length)throw new AggregateError(postflightErrors.map(({error})=>error),'Negative-control postflight failed');
 assert.equal(report.status,'passed','Negative controls require complete original, cloned and cache continuity');
}
