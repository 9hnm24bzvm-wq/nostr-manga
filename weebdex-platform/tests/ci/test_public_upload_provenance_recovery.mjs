// Complete source-only recipe/stage preflight, using the real data transforms.
// Does not materialize sources, import application code, or run a compiler/runtime.
import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { createHash } from 'node:crypto';
import { createFrontendOverlays, frontendOverlayManifest } from '../../platform-integrations/frontend-overlays.mjs';
import { createReaderOverlays, readerOverlayManifest } from '../../platform-integrations/frontend-reader-overlays.mjs';
import { createLocalLibraryOverlays, localLibraryOverlayManifest } from '../../platform-integrations/local-library/overlays.mjs';
import { createPublicApiOverlays, publicApiOverlayManifest } from '../../platform-integrations/public-api/overlays.mjs';
import { createPublicUploadOverlays, publicUploadOverlayManifest } from '../../platform-integrations/public-api/upload-overlays.mjs';
import { associateSupersededUploadRecipe } from '../../scripts/composed-upload-provenance.mjs';
const root=path.resolve(path.dirname(fileURLToPath(import.meta.url)),'../..'),frontend=path.join(root,'frontend');
const hash=value=>createHash('sha256').update(value).digest('hex'),normalize=value=>value.replace(/\r\n/g,'\n').trim();
const recipePath='platform-integrations/frontend-behavior/upload-replacement.fragment.ts',target='src/lib/components/upload/UploadChapterPages.svelte';
const recipes=[];function walk(directory){for(const entry of fs.readdirSync(directory,{withFileTypes:true})){assert.equal(entry.isSymbolicLink(),false);const file=path.join(directory,entry.name);if(entry.isDirectory())walk(file);else if(entry.name.endsWith('.fragment.ts'))recipes.push(file);}}walk(path.join(root,'platform-integrations'));
function profile(enabled){
  const plugins=[createFrontendOverlays(frontend),createReaderOverlays(frontend),createLocalLibraryOverlays(frontend),createPublicApiOverlays(frontend,{enabled}),createPublicUploadOverlays(frontend,{enabled})];
  const names=[...new Set([...frontendOverlayManifest,...readerOverlayManifest,...localLibraryOverlayManifest,...(enabled?[...publicApiOverlayManifest,...publicUploadOverlayManifest]:[])].map(row=>row.path))];
  const snapshots=new Map(),manifest=[];
  for(const relative of names){let code=fs.readFileSync(path.join(frontend,relative),'utf8');const stages=[];
    for(const plugin of plugins){const result=plugin.transform.handler(code,path.join(frontend,relative));if(result){code=result.code;const snapshot=`production-stages/${plugin.name}/${relative}`;snapshots.set(snapshot,code);stages.push({plugin:plugin.name,sha256:hash(code),snapshot});}}
    manifest.push({canonical:relative,projection:relative,production_composed_sha256:hash(code),stages,code});
  }
  return {manifest,snapshots,readSnapshot:name=>snapshots.get(name)};
}
const profiles=new Map();
for(const enabled of [false,true])test(`all legacy recipes retain honest production ancestry, public uploads ${enabled?'enabled':'disabled'}`,()=>{
  const state=profile(enabled);profiles.set(enabled,state);const inactive=[];
  for(const file of recipes){const fragment=fs.readFileSync(file),value=normalize(String(fragment));const lines=[...new Set(value.split('\n').map(line=>line.trim()).filter(line=>line.length>12))];assert.ok(lines.length);
    const exact=state.manifest.some(row=>normalize(row.code).includes(value));
    const derived=state.manifest.some(row=>row.stages.length&&lines.filter(line=>new Set(normalize(row.code).split('\n').map(line=>line.trim())).has(line)).length/lines.length>0.5);
    if(!exact&&!derived){const result=associateSupersededUploadRecipe({fragmentPath:path.relative(root,file),fragmentBytes:fragment,entry:state.manifest.find(row=>row.projection===target),readSnapshot:state.readSnapshot});assert.ok(result,path.relative(root,file));assert.equal(result.active_in_final,false);inactive.push(result.path);}
  }
  assert.deepEqual(inactive,enabled?[recipePath]:[]);
  if(enabled)for(const spec of publicUploadOverlayManifest){const entry=state.manifest.find(row=>row.projection===spec.path);assert.equal(entry.production_composed_sha256,spec.output_sha256);assert.equal(entry.stages.at(-1).plugin,'recovered-public-signed-upload');}
});
const fixture=()=>{const state=profiles.get(true)??profile(true);return {fragmentPath:recipePath,fragmentBytes:fs.readFileSync(path.join(root,recipePath)),entry:structuredClone(state.manifest.find(row=>row.projection===target)),readSnapshot:state.readSnapshot};};
test('exact replacement supersession is explicitly inactive and retains both real source stages',()=>{const result=associateSupersededUploadRecipe(fixture());assert.equal(result.active_in_final,false);assert.match(result.qualification,/public signed atomic replacement adapter/);assert.match(result.qualification,/does not qualify actual HTTP or browser/);assert.ok(result.historical_origin.matching_recipe_lines/result.historical_origin.distinctive_recipe_lines>0.5);assert.match(result.superseded_by.input_sha256,/^[0-9a-f]{64}$/);});
for(const [name,change]of[
 ['wrong fragment bytes',v=>{v.fragmentBytes=Buffer.from('changed');}],
 ['wrong target',v=>{v.entry.projection='src/other.svelte';}],
 ['duplicate public stage',v=>{v.entry.stages.push({...v.entry.stages.at(-1)});}],
 ['stale prior hash',v=>{v.entry.stages.at(-2).sha256='0'.repeat(64);}],
 ['wrong final hash',v=>{v.entry.production_composed_sha256='0'.repeat(64);}],
 ['escaped snapshot',v=>{v.entry.stages.at(-2).snapshot='../outside';}],
 ['tampered snapshot bytes',v=>{v.readSnapshot=()=> 'tampered';}],
])test(`supersession rejects ${name}`,()=>{const value=fixture();change(value);assert.throws(()=>associateSupersededUploadRecipe(value));});
test('unrelated recipes cannot borrow the inactive replacement record',()=>{const value=fixture();value.fragmentPath='platform-integrations/frontend-behavior/other.fragment.ts';assert.equal(associateSupersededUploadRecipe(value),null);});
test('global final-source fragment threshold and source-map hash checks remain unchanged',()=>{const source=fs.readFileSync(path.join(root,'scripts/check-composed-frontend.mjs'),'utf8');assert.match(source,/entry.exact_fraction > 0.5/);assert.match(source,/sourceHash !== entry.production_composed_sha256/);assert.match(source,/if \(!destinations.length && !derived.length\)/);});
