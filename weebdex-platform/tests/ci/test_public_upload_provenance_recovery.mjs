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
import { createBoundedReaderOverlays, boundedReaderOverlayManifest, boundedReaderApprovedMarkupExceptions, assertBoundedReaderExceptionScope } from '../../platform-integrations/bounded-reader/overlays.mjs';
import { assertBadgeExceptionScope } from '../../platform-integrations/bounded-reader/badge-overlays.mjs';
import { associateSupersededUploadRecipe } from '../../scripts/composed-upload-provenance.mjs';
const root=path.resolve(path.dirname(fileURLToPath(import.meta.url)),'../..'),frontend=path.join(root,'frontend');
const hash=value=>createHash('sha256').update(value).digest('hex'),normalize=value=>value.replace(/\r\n/g,'\n').trim();
const recipePath='platform-integrations/frontend-behavior/upload-replacement.fragment.ts',target='src/lib/components/upload/UploadChapterPages.svelte';
const readerPatches=JSON.parse(fs.readFileSync(path.join(root,'platform-integrations/bounded-reader/anchors.json'),'utf8'));
const badgePatches=JSON.parse(fs.readFileSync(path.join(root,'platform-integrations/bounded-reader/badge-anchors.json'),'utf8'));
const boundedTargets=new Set(boundedReaderOverlayManifest.map(patch=>patch.path));
const productionOrder=['weebdex-reviewed-script-overlays','weebdex-reviewed-reader-overlays','weebdex-local-library-overlays','retained-public-uuid-catalogue-staging','recovered-public-signed-upload','bounded-live-semantic-reader'];
const recipes=[];function walk(directory){for(const entry of fs.readdirSync(directory,{withFileTypes:true})){assert.equal(entry.isSymbolicLink(),false);const file=path.join(directory,entry.name);if(entry.isDirectory())walk(file);else if(entry.name.endsWith('.fragment.ts'))recipes.push(file);}}walk(path.join(root,'platform-integrations'));
function profile(enabled){
  const plugins=[createFrontendOverlays(frontend),createReaderOverlays(frontend),createLocalLibraryOverlays(frontend),createPublicApiOverlays(frontend,{enabled}),createPublicUploadOverlays(frontend,{enabled}),createBoundedReaderOverlays(frontend)];
  assert.deepEqual(plugins.map(plugin=>plugin.name),productionOrder);
  const names=[...new Set([...frontendOverlayManifest,...readerOverlayManifest,...localLibraryOverlayManifest,...(enabled?publicApiOverlayManifest:[]),...publicUploadOverlayManifest,...boundedReaderOverlayManifest].map(row=>row.path))];
  const snapshots=new Map(),manifest=[];
  for(const relative of names){let code=fs.readFileSync(path.join(frontend,relative),'utf8');const stages=[];
    for(const plugin of plugins){const result=plugin.transform.handler(code,path.join(frontend,relative));if(result){code=result.code;const snapshot=`production-stages/${plugin.name}/${relative}`;snapshots.set(snapshot,code);stages.push({plugin:plugin.name,sha256:hash(code),snapshot});}}
    manifest.push({canonical:relative,projection:relative,production_composed_sha256:hash(code),stages,code});
  }
  return {manifest,snapshots,plugins,readSnapshot:name=>snapshots.get(name)};
}
test('current sixth-stage source pins and combined manifest preserve separate exception scopes',()=>{
  const pins={
    'overlays.mjs':'a7d89c85f2fdf435c8606f82c5519e532c2717fed698e72fcca81eaaf201c4aa',
    'anchors.json':'feafb34479f4d81413f44cc24d786beb431e9bebffc6d6be24de1753139d2fae',
    'badge-overlays.mjs':'e36bdf2b2e00c6e6a485f3fbc7d6f63cf05c59f59c4847e87854cdf3561bb960',
    'badge-anchors.json':'a81b80af1b637794ea23834b713ee06c4bfa5161a009708ef01f7badebea0ca4'
  };
  for(const [file,digest] of Object.entries(pins))assert.equal(hash(fs.readFileSync(path.join(root,'platform-integrations/bounded-reader',file))),digest,file);
  assert.equal(readerPatches.length,34);assert.equal(readerPatches.filter(patch=>patch.markup).length,12);
  assert.equal(badgePatches.length,16);assert.equal(badgePatches.filter(patch=>patch.markup).length,5);
  assert.deepEqual(boundedReaderOverlayManifest,[...readerPatches,...badgePatches].map(({before,after,...metadata})=>metadata));
  assert.deepEqual(boundedReaderApprovedMarkupExceptions,[...readerPatches,...badgePatches].filter(patch=>patch.markup));
  assertBadgeExceptionScope();assertBoundedReaderExceptionScope();
  for(const upload of publicUploadOverlayManifest)assert.equal(boundedTargets.has(upload.path),false,upload.path);
});
test('all six historical fragment recipes remain upload-only',()=>{
  assert.deepEqual(recipes.map(file=>path.relative(root,file)).sort(),['admission','edit-resume','new-resume','process','replacement','scan'].map(name=>'platform-integrations/frontend-behavior/upload-'+name+'.fragment.ts').sort());
});
const profiles=new Map();
for(const enabled of [false,true])test(`all legacy recipes retain honest production ancestry, public uploads ${enabled?'enabled':'disabled'}`,()=>{
  const state=profile(enabled);profiles.set(enabled,state);const inactive=[];
  for(const file of recipes){const fragment=fs.readFileSync(file),value=normalize(String(fragment));const lines=[...new Set(value.split('\n').map(line=>line.trim()).filter(line=>line.length>12))];assert.ok(lines.length);
    const destinations=state.manifest.filter(row=>normalize(row.code).includes(value));
    const derivations=state.manifest.filter(row=>row.stages.length&&lines.filter(line=>new Set(normalize(row.code).split('\n').map(line=>line.trim())).has(line)).length/lines.length>0.5);
    for(const row of [...destinations,...derivations])assert.ok(row.projection.startsWith('src/lib/components/upload/'),'Recipe ancestry cannot move to reader/badge targets: '+row.projection);
    const exact=destinations.length>0,derived=derivations.length>0;
    if(!exact&&!derived){const result=associateSupersededUploadRecipe({fragmentPath:path.relative(root,file),fragmentBytes:fragment,entry:state.manifest.find(row=>row.projection===target),readSnapshot:state.readSnapshot});assert.ok(result,path.relative(root,file));assert.equal(result.active_in_final,false);inactive.push(result.path);}
  }
  assert.deepEqual(inactive,enabled?[recipePath]:[]);
  assert.deepEqual(state.manifest.filter(row=>row.stages.some(stage=>stage.plugin==='bounded-live-semantic-reader')).map(row=>row.projection).sort(),[...boundedTargets].sort());
  const snapshotNames=[];
  for(const entry of state.manifest){
    assert.equal(entry.production_composed_sha256,hash(entry.code),entry.projection+' final bytes');
    for(const stage of entry.stages){
      assert.equal(stage.snapshot,`production-stages/${stage.plugin}/${entry.projection}`);
      assert.equal(hash(state.readSnapshot(stage.snapshot)),stage.sha256,entry.projection+' actual stage bytes');
      snapshotNames.push(stage.snapshot);
    }
    if(entry.stages.length)assert.equal(entry.stages.at(-1).sha256,entry.production_composed_sha256,entry.projection+' final stage');
    if(boundedTargets.has(entry.projection)){
      assert.equal(entry.stages.filter(stage=>stage.plugin==='bounded-live-semantic-reader').length,1);
      assert.equal(entry.stages.at(-1).plugin,'bounded-live-semantic-reader');
      assert.equal(state.readSnapshot(entry.stages.at(-1).snapshot),entry.code,'Do not normalize current reader/badge snapshots');
    }
    if(!enabled)assert.equal(entry.stages.some(stage=>['retained-public-uuid-catalogue-staging','recovered-public-signed-upload'].includes(stage.plugin)),false);
  }
  assert.deepEqual([...state.snapshots.keys()].sort(),snapshotNames.sort(),'Every real stage snapshot must be retained');
  for(const spec of publicUploadOverlayManifest){
    const entry=state.manifest.find(row=>row.projection===spec.path);assert.ok(entry);
    assert.equal(entry.stages.some(stage=>stage.plugin==='bounded-live-semantic-reader'),false);
    assert.equal(state.plugins.at(-1).transform.handler(entry.code,path.join(frontend,spec.path)),null,'Sixth stage must not alter upload output');
    if(enabled){assert.equal(entry.production_composed_sha256,spec.output_sha256);assert.equal(entry.stages.at(-1).plugin,'recovered-public-signed-upload');}
    else assert.equal(entry.production_composed_sha256,spec.composed_sha256);
  }
});
test('disabled public-only upload target retains an exact original zero-stage provenance entry',()=>{
  const relative='src/routes/(app)/chapter/[id]/edit/+layout.svelte';
  const spec=publicUploadOverlayManifest.find(row=>row.path===relative);assert.ok(spec);
  const state=profiles.get(false);assert.ok(state,'The disabled profile must complete its ancestry checks');
  const entries=state.manifest.filter(row=>row.projection===relative);assert.equal(entries.length,1);
  const entry=entries[0],original=fs.readFileSync(path.join(frontend,relative),'utf8');
  assert.equal(hash(original),spec.source_sha256);assert.equal(spec.source_sha256,spec.composed_sha256);
  assert.equal(entry.code,original);assert.equal(entry.production_composed_sha256,spec.source_sha256);
  assert.deepEqual(entry.stages,[],'Inactive public-only target must not invent an ancestry stage');
  for(const plugin of state.plugins)assert.equal(plugin.transform.handler(original,path.join(frontend,relative)),null);
  assert.equal([...state.snapshots.keys()].some(name=>name.endsWith('/'+relative)),false);
});
const fixture=()=>{const state=profiles.get(true)??profile(true);return {fragmentPath:recipePath,fragmentBytes:fs.readFileSync(path.join(root,recipePath)),entry:structuredClone(state.manifest.find(row=>row.projection===target)),readSnapshot:state.readSnapshot};};
test('exact replacement supersession is explicitly inactive and retains both real source stages',()=>{const result=associateSupersededUploadRecipe(fixture());assert.equal(result.active_in_final,false);assert.match(result.qualification,/public signed atomic replacement adapter/);assert.match(result.qualification,/does not qualify actual HTTP or browser/);assert.ok(result.historical_origin.matching_recipe_lines/result.historical_origin.distinctive_recipe_lines>0.5);assert.match(result.superseded_by.input_sha256,/^[0-9a-f]{64}$/);});
for(const [name,change]of[
 ['wrong fragment bytes',v=>{v.fragmentBytes=Buffer.from('changed');}],
 ['wrong target',v=>{v.entry.projection='src/other.svelte';}],
 ['duplicate public stage',v=>{v.entry.stages.push({...v.entry.stages.at(-1)});}],
 ['reader stage incorrectly appended to upload',v=>{v.entry.stages.push({...v.entry.stages.at(-1),plugin:'bounded-live-semantic-reader'});}],
 ['stale prior hash',v=>{v.entry.stages.at(-2).sha256='0'.repeat(64);}],
 ['wrong final hash',v=>{v.entry.production_composed_sha256='0'.repeat(64);}],
 ['escaped snapshot',v=>{v.entry.stages.at(-2).snapshot='../outside';}],
 ['tampered snapshot bytes',v=>{v.readSnapshot=()=> 'tampered';}],
])test(`supersession rejects ${name}`,()=>{const value=fixture();change(value);assert.throws(()=>associateSupersededUploadRecipe(value));});
test('unrelated recipes cannot borrow the inactive replacement record',()=>{const value=fixture();value.fragmentPath='platform-integrations/frontend-behavior/other.fragment.ts';assert.equal(associateSupersededUploadRecipe(value),null);});
test('global final-source fragment threshold and source-map hash checks remain unchanged',()=>{const source=fs.readFileSync(path.join(root,'scripts/check-composed-frontend.mjs'),'utf8');assert.match(source,/entry.exact_fraction > 0.5/);assert.match(source,/sourceHash !== entry.production_composed_sha256/);assert.match(source,/if \(!destinations.length && !derived.length\)/);});
