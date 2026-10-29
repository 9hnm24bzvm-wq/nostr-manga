import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import {createHash} from 'node:crypto';
import {composed,composedBaseline} from './composed-account-library-fixture.mjs';
import {sixPersonaOverlayManifest,sixPersonaPatches,assertSixPersonaScope,applySixPersonaOutput,restoreSixPersonaOutput,createSixPersonaOverlays} from '../../platform-integrations/six-persona/overlays.mjs';
const hash=value=>createHash('sha256').update(value).digest('hex');

test('reviewed combined scope has exact targets and no unregistered patches',()=>{
 assertSixPersonaScope();assert.equal(new Set(sixPersonaOverlayManifest.map(row=>row.path)).size,sixPersonaOverlayManifest.length);
 for(const row of sixPersonaOverlayManifest)assert.ok(sixPersonaPatches.some(patch=>patch.path===row.path));
});
for(const row of sixPersonaOverlayManifest)test(`current composed source is reversible to the qualified baseline: ${row.path}`,async()=>{
 const previous=await composedBaseline(row.path),current=await composed(row.path);
 assert.equal(hash(previous),row.input_sha256);assert.equal(hash(current),row.output_sha256);
 assert.equal(applySixPersonaOutput(previous,row.path),current);
 assert.equal(restoreSixPersonaOutput(current,row.path),previous);
 assert.throws(()=>applySixPersonaOutput(previous+' ',row.path),/predecessor changed/);
 assert.throws(()=>restoreSixPersonaOutput(current+' ',row.path),/output changed/);
});
test('stage does not transform disabled, query, or unregistered input, and rejects changed canonical source',async()=>{
 const root=fs.mkdtempSync(path.join(os.tmpdir(),'six-persona-source-'));
 try{
  const row=sixPersonaOverlayManifest[0],id=path.join(root,row.path),previous=await composedBaseline(row.path);
  assert.equal(createSixPersonaOverlays(root,{enabled:false}).transform.handler(previous,id),null);
  assert.equal(createSixPersonaOverlays(root,{enabled:true}).transform.handler(previous,id+'?raw'),null);
  assert.equal(createSixPersonaOverlays(root,{enabled:true}).transform.handler('unknown',path.join(root,'unregistered.svelte')),null);
  assert.equal(applySixPersonaOutput('unknown','unregistered.svelte'),'unknown');
  fs.mkdirSync(path.dirname(id),{recursive:true});fs.writeFileSync(id,'changed original');
  assert.throws(()=>createSixPersonaOverlays(root,{enabled:true}).transform.handler(previous,id),/canonical source changed/);
 }finally{fs.rmSync(root,{recursive:true,force:true});}
});

test('inactive upload recipe remains bound through real historical and current production snapshots',async()=>{
 const {fileURLToPath}=await import('node:url');
 const root=fileURLToPath(new URL('../../',import.meta.url)),frontend=path.join(root,'frontend');
 const target='src/lib/components/upload/UploadChapterPages.svelte',file=path.join(frontend,target),snapshots=new Map(),stages=[];
 let source=fs.readFileSync(file,'utf8');
 for(const [module,name]of [['frontend-overlays.mjs','createFrontendOverlays'],['frontend-reader-overlays.mjs','createReaderOverlays'],['local-library/overlays.mjs','createLocalLibraryOverlays'],['public-api/overlays.mjs','createPublicApiOverlays'],['public-api/upload-overlays.mjs','createPublicUploadOverlays'],['public-api/upload-overlays.mjs','createNativeUploadOverlays'],['bounded-reader/overlays.mjs','createBoundedReaderOverlays'],['local-library/account-overlays.mjs','createAccountLibraryOverlays'],['six-persona/overlays.mjs','createSixPersonaOverlays']]){
  const exports=await import(path.join(root,'platform-integrations',module)),plugin=exports[name](frontend,{enabled:true}),result=plugin.transform.handler(source,file);
  if(result){source=result.code;const snapshot=`production-stages/${plugin.name}/${target}`;snapshots.set(snapshot,source);stages.push({plugin:plugin.name,snapshot,sha256:hash(source)});}
 }
 const {associateCurrentUploadRecipe}=await import('../../scripts/composed-six-persona-provenance.mjs');
 const options={fragmentPath:'platform-integrations/frontend-behavior/upload-replacement.fragment.ts',fragmentBytes:fs.readFileSync(path.join(root,'platform-integrations/frontend-behavior/upload-replacement.fragment.ts')),entry:{canonical:target,projection:target,production_composed_sha256:hash(source),stages},readSnapshot:name=>snapshots.get(name)};
 const receipt=associateCurrentUploadRecipe(options);
 assert.equal(receipt.active_in_final,false);assert.equal(receipt.native_successor.exact_reverse_to_historical_snapshot,true);assert.equal(receipt.six_persona_successor.exact_reverse_to_qualified_snapshot,true);
 assert.throws(()=>associateCurrentUploadRecipe({...options,readSnapshot:name=>snapshots.get(name)+' '}),/restoration changed/);
 assert.throws(()=>associateCurrentUploadRecipe({...options,entry:{...options.entry,stages:[...stages,stages.at(-1)]}}),/ambiguous or nonfinal/);
});
