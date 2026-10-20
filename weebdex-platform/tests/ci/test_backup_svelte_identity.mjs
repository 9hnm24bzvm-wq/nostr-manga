// Actual Svelte runtime plus actual panel/library/DeviceStore/restore functions.
// Only two opaque assignments use compiler-evidenced lowering, not a full DOM
// compilation. IndexedDB/store/environment adapters remain the finite model.
import fs from 'node:fs';
import path from 'node:path';
import vm from 'node:vm';
import assert from 'node:assert/strict';
import test from 'node:test';
import { createHash } from 'node:crypto';
import { fileURLToPath, pathToFileURL } from 'node:url';
const hash=s=>createHash('sha256').update(s).digest('hex');
const here=path.dirname(fileURLToPath(import.meta.url));
const runtimeRoot=process.env.WEEBDEX_SVELTE_REVIEW_RUNTIME_ROOT;
assert.ok(runtimeRoot&&path.isAbsolute(runtimeRoot),'Provide verified Svelte 5.55.1 runtime sources');
const pins=JSON.parse(fs.readFileSync(path.join(here,'svelte-review-runtime-pins.json')));
for(const [name,digest] of Object.entries(pins.files))assert.equal(hash(fs.readFileSync(path.join(runtimeRoot,name))),digest,name);
assert.equal(JSON.parse(fs.readFileSync(path.join(runtimeRoot,'package.json'))).version,'5.55.1');
const assignment=fs.readFileSync(path.join(runtimeRoot,'src/compiler/phases/3-transform/client/visitors/AssignmentExpression.js'),'utf8');
assert.ok(assignment.includes("binding.kind !== 'raw_state'"));
const utils=fs.readFileSync(path.join(runtimeRoot,'src/compiler/phases/3-transform/client/utils.js'),'utf8');
const predicate=utils.slice(utils.indexOf('export function should_proxy('),utils.indexOf('/**',utils.indexOf('export function should_proxy(')));
const shouldProxy=new Function(predicate.replace('export ','')+';return should_proxy;')();
// Both exact panel result bindings below have an awaited initializer.
const currentPanel=fs.readFileSync(path.join(here,'../../platform-integrations/local-library/LocalLibraryPanel.svelte'),'utf8');
assert.equal((currentPanel.match(/const result = await /g)||[]).length,2);
assert.equal(shouldProxy({type:'Identifier',name:'result'},{get:()=>({reassigned:false,initial:{type:'AwaitExpression'}})}),true);

const source=fs.readFileSync(path.join(here,'device-backup-restore.test.mjs'),'utf8');
let prefix=source.slice(0,source.indexOf('if (baselineRoot) test('));
assert.equal(hash(prefix),'89d2a0997bccb518dc33ed39201f644f9572e807308246b08bccea609ee255c0');
prefix=prefix.replaceAll('import.meta.url',JSON.stringify(pathToFileURL(path.join(here,'device-backup-restore.test.mjs')).href));
prefix=prefix.replace('async panel() {','async panel({ assign, variant } = {}) {');
prefix=prefix.replace("const source = fs.readFileSync(path.join(selectedBase, 'local-library/LocalLibraryPanel.svelte'), 'utf8');", "let source = fs.readFileSync(path.join(selectedBase, 'local-library/LocalLibraryPanel.svelte'), 'utf8'); if (variant === 'original') { source = source.replace('$state.raw<DeviceBackupRestoreReview>', '$state<DeviceBackupRestoreReview>').replace('$state.raw<ImportReview>', '$state<ImportReview>').replace('  // Opaque frozen review handles are WeakMap authorities; never deep-proxy them.\\n', ''); assert.equal(createHash('sha256').update(source).digest('hex'), 'f62dd91277fbb7f6e511601601072bd1657b56b68c40c18c8dec4847fc16ec5a'); } if (variant === 'preview-only') source = source.replace('$state.raw<DeviceBackupRestoreReview>', '$state<DeviceBackupRestoreReview>');");
prefix=prefix.replace('const script = stripTypeScriptTypes','let script = stripTypeScriptTypes');
prefix=prefix.replace('let cleanup;',`let cleanup;
      if (assign) for (const [name,type] of [['imported','ImportReview'],['deviceRestore','DeviceBackupRestoreReview']]) {
        const raw = source.includes('$state.raw<'+type+'>');
        const needle = name+' = result';
        assert.equal(script.split(needle).length-1,1);
        script = script.replace(needle,name+' = __assign("'+name+'", result, '+(!raw)+')');
      }`);
prefix=prefix.replace('$state: Object.assign(value => value, { raw: value => value }),', '$state: Object.assign(value => value,{raw:value=>value}), __assign:assign,');
const fixtureModule=await import('data:text/javascript;base64,'+Buffer.from(prefix+'\nexport {fixture, exported, series, other, foreignOwner, unchanged};').toString('base64'));
async function actualRuntime(context){
 Object.assign(context,{queueMicrotask,setTimeout,clearTimeout});
 const modules=new Map();
 function module(name){
  if(modules.has(name))return modules.get(name);
  let value;
  if(name==='esm-env')value=new vm.SyntheticModule(['DEV','BROWSER'],function(){this.setExport('DEV',false);this.setExport('BROWSER',true);},{context,identifier:name});
  else value=new vm.SourceTextModule(fs.readFileSync(path.join(runtimeRoot,name),'utf8'),{context,identifier:name});
  modules.set(name,value);return value;
 }
 const entry=new vm.SourceTextModule("export {state,set} from './reactivity/sources.js';export {get} from './runtime.js';",{context,identifier:'src/internal/client/entry.js'});
 await entry.link((specifier,importer)=>module(specifier==='esm-env'?specifier:specifier==='#client/constants'?'src/internal/client/constants.js':path.posix.normalize(path.posix.join(path.posix.dirname(importer.identifier),specifier))));
 await entry.evaluate();const api=entry.namespace,signals=new Map();
 return (name,value,proxy)=>{if(!signals.has(name))signals.set(name,api.state(undefined));const signal=signals.get(name);api.set(signal,value,proxy);return api.get(signal);};
}
async function scenario(variant){const f=await fixtureModule.fixture({records:[fixtureModule.series()]});const assign=await actualRuntime(f.context);return {f,panel:await f.panel({assign,variant})};}
test('exact original panel deep state loses frozen preview WeakMap authority in actual Svelte', {timeout:5000},async()=>{
 const {f,panel}=await scenario('original');let original;
 await panel.preview(async()=>original=await f.library.previewImport(f.file(await fixtureModule.exported(f))));
 assert.ok(Object.isFrozen(original));assert.notEqual(panel.imported,original);const before=await f.snapshot();
 await panel.run('prepare-merge',()=>panel.prepareRestore('merge'));assert.match(panel.error,/Preview this device backup/);assert.equal(panel.restore,undefined);await fixtureModule.unchanged(f,before);
});
test('preview-only raw counterfactual exposes second prepared handle identity loss', {timeout:5000},async()=>{
 const {f,panel}=await scenario('preview-only');await panel.preview(async()=>f.library.previewImport(f.file(await fixtureModule.exported(f))));await panel.prepareRestore('merge');const before=await f.snapshot();
 await assert.rejects(()=>panel.confirmRestore(),/Prepare and confirm a fresh/);await fixtureModule.unchanged(f,before);
});
test('raw opaque handles preserve actual-runtime foreign consent and explicit confirmation', {timeout:5000},async()=>{
 const {f,panel}=await scenario();let original;const file=f.file({...await fixtureModule.exported(f),ownerId:fixtureModule.foreignOwner,records:[fixtureModule.series(fixtureModule.other)]});
 await panel.preview(async()=>original=await f.library.previewImport(file));assert.equal(panel.imported,original);const before=await f.snapshot();
 await assert.rejects(()=>panel.prepareRestore('replace'),/foreign-device owner/);panel.consent(true);await panel.prepareRestore('replace');await fixtureModule.unchanged(f,before);
 const prepared=panel.restore;await assert.rejects(()=>f.library.applyDeviceBackupImport({...prepared},()=>{}),/fresh/);await panel.confirmRestore();assert.equal((await f.snapshot()).records[0].id,fixtureModule.other);
 await assert.rejects(()=>f.library.applyDeviceBackupImport(prepared,()=>{}),/fresh/);await panel.preview(()=>f.library.previewImport(file));await assert.rejects(()=>panel.prepareRestore('merge'),/foreign-device owner/);
});
test('actual-runtime raw handles still reject close and dispose before commit', {timeout:5000},async()=>{
 for(const dispose of [false,true]){const {f,panel}=await scenario();await panel.preview(async()=>f.library.previewImport(f.file(await fixtureModule.exported(f))));await panel.prepareRestore('merge');const before=await f.snapshot();if(dispose)panel.dispose();else panel.closeReview(false);await assert.rejects(()=>panel.confirmRestore());await fixtureModule.unchanged(f,before);}
});
test('actual-runtime raw handles merge newer records only after explicit confirmation', {timeout:5000},async()=>{
 const {f,panel}=await scenario();const incoming={...await fixtureModule.exported(f),records:[fixtureModule.series(undefined,110,'completed')]};
 await panel.preview(()=>f.library.previewImport(f.file(incoming)));const before=await f.snapshot();await panel.prepareRestore('merge');await fixtureModule.unchanged(f,before);
 await panel.confirmRestore();const after=await f.snapshot();assert.equal(after.records[0].status,'completed');assert.equal(after.revision,before.revision+1);assert.equal(after.archives.length,1);assert.equal(after.ownerId,before.ownerId);
});
test('actual-runtime raw preview still rejects copied authority and foreign catalogue scope', {timeout:5000},async()=>{
 const {f,panel}=await scenario();await panel.preview(async()=>f.library.previewImport(f.file(await fixtureModule.exported(f))));const before=await f.snapshot();
 await assert.rejects(()=>f.library.prepareDeviceBackupImport({...panel.imported},'merge',false,()=>{}),/Preview/);await fixtureModule.unchanged(f,before);
 const backup=await fixtureModule.exported(f);await panel.preview(()=>f.library.previewImport(f.file({...backup,scope:{...backup.scope,catalogNamespace:fixtureModule.other}})));
 await assert.rejects(()=>panel.prepareRestore('merge'),/different verified catalogue/);await fixtureModule.unchanged(f,before);
});
