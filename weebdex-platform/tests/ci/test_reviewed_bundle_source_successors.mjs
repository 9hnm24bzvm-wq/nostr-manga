import {readReaderUxAssociation,readReaderUxFixpassAssociation,restoreReaderUxFixpassPredecessor,restoreReaderUxPredecessor} from './reviewed-card-spinner-source-successor.mjs';
import {restoreHistorySource} from './reviewed-history-source-successors.mjs';
import {readBoundedReaderAssociation,restoreBoundedReaderSource,boundedReaderAssociationSha256,boundedReaderSuccessorPaths} from './reviewed-bounded-reader-source-successors.mjs';
// Source-bound comparison controls. No build, browser, network, or historical execution claim.
import fs from 'node:fs';
import vm from 'node:vm';
import {preUpdatesOverlayComparison} from './reviewed-preparation-counterfactual.mjs';
import path from 'node:path';
import {createHash} from 'node:crypto';
import {fileURLToPath} from 'node:url';
import assert from 'node:assert/strict';
import test from 'node:test';
import {readBundleAssociation,restoreBundleSource,bundleAssociationSha256} from './reviewed-bundle-source-successors.mjs';
import {restoreNativeProfileActionsSource} from './native-profile-actions-source-association.mjs';
import {readModeratorCompositionAssociation,restoreModeratorCompositionSource} from './moderator-composition-source-association.mjs';
import {restoreModeratorTransportSource} from './reviewed-moderator-transport-source-successors.mjs';
const root=fileURLToPath(new URL('../../../',import.meta.url));
const metadata=fs.readFileSync(new URL('../../docs/fixtures/reviewed-bundle-source-successors.json',import.meta.url));
const hash=value=>createHash('sha256').update(value).digest('hex');
const encoded=value=>Buffer.from(JSON.stringify(value,null,2)+'\n');
// Source-only historical inputs: first undo the exact second pass, then the
// exact published first pass where applicable. Runtime tests read live files.
const firstReaderPaths = new Set(readReaderUxAssociation().rows.map(row => row.path));
const secondReaderPaths = new Set(readReaderUxFixpassAssociation().rows.map(row => row.path));
function beforeReaderUx(file, value) {
 return firstReaderPaths.has(file) ? restoreReaderUxPredecessor(file, value)
  : secondReaderPaths.has(file) ? restoreReaderUxFixpassPredecessor(file, value) : value;
}
const liveSourceSnapshots = new Map();
function readCompared(file, encoding) {
 const live = fs.readFileSync(path.join(root, file));
 if (liveSourceSnapshots.has(file)) assert.deepEqual(live, liveSourceSnapshots.get(file), 'Physical source changed during comparison: '+file);
 else liveSourceSnapshots.set(file, Buffer.from(live));
 const value = beforeReaderUx(file, live);
 assert.notEqual(value, null, 'An added source cannot impersonate a historical predecessor: '+file);
 return encoding ? value.toString(encoding) : value;
}
function assertUnwritten(file) { assert.deepEqual(fs.readFileSync(path.join(root, file)), liveSourceSnapshots.get(file)); }
const data=readBundleAssociation();
function stageInput(row,physical){
 if(row.path==='weebdex-platform/scripts/prepare-frontend-integration.mjs')return restoreBoundedReaderSource('bounded-reader-to-frozen-main:scripts/prepare-frontend-integration.mjs',row.path,physical);
 if(row.stage==='pre-catalogue-generator')return restoreNativeProfileActionsSource(row.path,physical);
 if(row.stage==='pre-generic-discussion-budget')return restoreModeratorTransportSource(row.path,physical,'raw');
 return physical;
}
test('five explicitly named source stages preserve exact immutable predecessors without execution claims',()=>{
 assert.equal(hash(metadata),bundleAssociationSha256);
 assert.equal(data.physical_source_commit,'5a4cc2530e4dd81f3f1888b39588a6afd27be98d');
 assert.equal(data.rows.length,5);
 for(const key of ['browser_executed','http_executed','capacity_qualified'])assert.equal(data[key],false);
 assert.deepEqual(data.rows.map(row=>row.stage),['updates-integration','updates-integration','updates-composition','pre-catalogue-generator','pre-generic-discussion-budget']);
 assert.equal(data.rows[3].previous_sha256,'9e2427f87e8542c449bad010c869d767ab826cec05e73b03620066375753b70b');
 assert.equal(data.rows[4].previous_sha256,'b9079ea5b27f19ecdbbaaea5f9316804b563fa4400c04c19b26a5ed966640411');
 for(const row of data.rows){
  const physical=readCompared(row.path),current=stageInput(row,physical),previous=restoreBundleSource(row.stage,row.path,current);
  assert(Buffer.isBuffer(previous));assert.equal(current.length,row.current_bytes);assert.equal(hash(current),row.current_sha256);
  assert.equal(previous.length,row.previous_bytes);assert.equal(hash(previous),row.previous_sha256);
  assert.equal(restoreBundleSource(row.stage,row.path,current.toString()),previous.toString());
  assertUnwritten(row.path);
 }
});
for(const row of data.rows)test('exact stage rejects historical, edited, partial, duplicated and wrong-stage input: '+row.stage+' '+row.path,()=>{
 const physical=readCompared(row.path),current=stageInput(row,physical),previous=restoreBundleSource(row.stage,row.path,current),changed=Buffer.from(current);changed[0]^=1;
 for(const bad of [previous,changed,current.subarray(1),Buffer.concat([current,Buffer.from(' ')]),null,{},new Uint8Array(current)])assert.throws(()=>restoreBundleSource(row.stage,row.path,bad));
 for(const edit of row.literal_reversals){
  const source=current.toString();assert.equal(source.split(edit.current).length,2);
  assert.throws(()=>restoreBundleSource(row.stage,row.path,source.replace(edit.current,()=>edit.previous)));
  assert.throws(()=>restoreBundleSource(row.stage,row.path,source+edit.current));
 }
 for(const stage of ['',null,'unknown',...data.rows.map(r=>r.stage).filter(stage=>stage!==row.stage)])assert.throws(()=>restoreBundleSource(stage,row.path,current));
 for(const file of ['',null,row.path+'unknown','../'+row.path])assert.throws(()=>restoreBundleSource(row.stage,file,current));
 if(row.stage.startsWith('pre-'))assert.throws(()=>restoreBundleSource(row.stage,row.path,physical),'Each explicit upstream layer is required');
});
test('full metadata digest binds every claim, stage, source commit, count, literal, order and encoding',()=>{
 const candidates=[Buffer.concat([metadata,Buffer.from(' ')]),metadata.subarray(1),metadata.toString(),encoded(Object.fromEntries(Object.entries(data).reverse()))];
 for(const key of Object.keys(data)){const copy=structuredClone(data);delete copy[key];candidates.push(encoded(copy));}
 for(let i=0;i<data.rows.length;i++){
  for(const key of Object.keys(data.rows[i])){const copy=structuredClone(data);delete copy.rows[i][key];candidates.push(encoded(copy));}
  for(let j=0;j<data.rows[i].literal_reversals.length;j++)for(const key of ['current','previous']){const copy=structuredClone(data);copy.rows[i].literal_reversals[j][key]+=' ';candidates.push(encoded(copy));}
 }
 for(const mutate of [copy=>copy.rows.reverse(),copy=>copy.rows.pop(),copy=>copy.rows.push(copy.rows[0]),copy=>copy.rows[0].literal_reversals.reverse(),copy=>copy.http_executed=true,copy=>copy.unknown=true]){const copy=structuredClone(data);mutate(copy);candidates.push(encoded(copy));}
 const row=data.rows[0],current=readCompared(row.path);
 for(const bytes of candidates){assert.throws(()=>readBundleAssociation(bytes));assert.throws(()=>restoreBundleSource(row.stage,row.path,current,bytes));}
});
test('preparation follows reader then updates then moderator then profile, never skipped or reordered layers',()=>{
 const file='weebdex-platform/scripts/prepare-frontend-integration.mjs',physical=readCompared(file),reader=restoreBoundedReaderSource('bounded-reader-to-frozen-main:scripts/prepare-frontend-integration.mjs',file,physical),updates=restoreBundleSource('updates-composition',file,reader),moderator=restoreModeratorCompositionSource(file,reader),profile=restoreNativeProfileActionsSource(file,moderator);
 assert.deepEqual([physical.length,reader.length,updates.length,moderator.length,profile.length],[31867,30561,29891,29847,29805]);
 assert.equal(hash(physical),'e55efc65d6efda7ee894aa29d5671ba3c35ee827d9f3b7cababf1125092353f4');
 assert.equal(hash(reader),'d498597c8b214f6e4f5025eb6216e0c2b1516d4d1bb27933a727f0c3cfedc38c');
 assert.equal(hash(updates),'bdb676c70e3d9eb4a5488b602ad6a778cc25be3b0a196a95fa87d4642fa10543');
 assert.equal(hash(moderator),'38011a1d691495466ac05ba441cf9cdc0bbe5d4b743569bf72539d6687344aba');
 assert.equal(hash(profile),'56d8e9005ab1ea6af8e299adc25ac210da7d58f794b00d45d6b7996f4b4d7812');
 for(const bad of [physical,updates,moderator,profile])assert.throws(()=>restoreModeratorCompositionSource(file,bad));
 for(const bad of [reader,updates,moderator,profile])assert.throws(()=>restoreBoundedReaderSource('bounded-reader-to-frozen-main:scripts/prepare-frontend-integration.mjs',file,bad));
 assert.throws(()=>restoreBundleSource('updates-composition',file,physical));
 assertUnwritten(file);
 for(const bad of [physical,reader,updates,profile])assert.throws(()=>restoreNativeProfileActionsSource(file,bad));
 const row=readModeratorCompositionAssociation().rows.find(row=>row.path===file);
 for(const edit of row.literal_reversals){assert.equal(updates.toString().split(edit.current).length,2);assert.throws(()=>restoreModeratorCompositionSource(file,updates.toString().replace(edit.current,()=>edit.previous)));}
});
test('isolated predecessor overlay comparison refuses current preparation and edited predecessors',async()=>{
 const file='weebdex-platform/scripts/prepare-frontend-integration.mjs',physical=readCompared(file,'utf8'),comparison=restoreBoundedReaderSource('bounded-reader-to-frozen-main:scripts/prepare-frontend-integration.mjs',file,physical),old=restoreModeratorCompositionSource(file,comparison);
 for(const bad of [physical,old+' ',old.slice(1),null,{},Buffer.from(old)])await assert.rejects(()=>preUpdatesOverlayComparison(bad,vm.createContext({})),/Unknown predecessor preparation comparison bytes/);
});
test('comparison helpers never enter current runtime or build imports',()=>{
 function visit(directory){for(const entry of fs.readdirSync(directory,{withFileTypes:true})){const file=path.join(directory,entry.name);if(entry.isDirectory())visit(file);else if(/\.(?:mjs|ts|svelte|py|rs)$/.test(file))assert.doesNotMatch(fs.readFileSync(file,'utf8'),/(?:reviewed[-_]bounded[-_]reader[-_]source[-_]successors|bounded-reader-source-associations|native-forum-reader-before|mounted-reader-before|reviewed[-_]bundle[-_]source[-_]successors|moderator[-_]composition[-_]source[-_]association|reviewed[-_]preparation[-_]counterfactual)/,file);}}
 for(const directory of ['platform-integrations','frontend/src','scripts','backend/src'])visit(path.join(root,'weebdex-platform',directory));
});

const readerMetadata=fs.readFileSync(new URL('../../docs/fixtures/bounded-reader-source-associations.json',import.meta.url));
const readerData=readBoundedReaderAssociation();
test('five bounded reader comparison stages bind exact combined inputs and nineteen ordered seams',()=>{
 assert.equal(hash(readerMetadata),boundedReaderAssociationSha256);
 assert.equal(readerData.physical_source_commit,'9c221a556ed4dbe3fcbda889d67b3f403c5a37d7');
 assert.equal(readerData.physical_source_tree,'8661c3cb5a6efe1fb71f1da847f04552645b2763');
 assert.equal(readerData.previous_source_commit,'f7c598cb57f9a2afbecc107c1d81bb4b30ac4102');
 assert.equal(readerData.row_count,5);assert.equal(readerData.literal_reversal_count,19);
 assert.deepEqual(readerData.rows.map(row=>row.literal_reversals.length),[7,2,2,3,5]);
 assert.deepEqual(boundedReaderSuccessorPaths,['scripts/prepare-frontend-integration.mjs','scripts/compile-frontend-source.mjs','scripts/check-composed-frontend.mjs','scripts/vite.platform.config.mjs','platform-integrations/local-library/library.ts'].map(file=>'weebdex-platform/'+file));
 for(const flag of ['tests_executed','build_executed','browser_executed','http_executed','capacity_qualified','external_mounted_preparation_applied'])assert.equal(readerData[flag],false);
 for(const row of readerData.rows){
  assert.equal(row.stage,'bounded-reader-to-frozen-main:'+row.path.slice('weebdex-platform/'.length));
  const physical=readCompared(row.path),previous=restoreBoundedReaderSource(row.stage,row.path,physical);
  assert(Buffer.isBuffer(previous));assert.equal(physical.length,row.current_bytes);assert.equal(hash(physical),row.current_sha256);
  assert.equal(previous.length,row.previous_bytes);assert.equal(hash(previous),row.previous_sha256);
  assert.equal(restoreBoundedReaderSource(row.stage,row.path,physical.toString('utf8')),previous.toString('utf8'));
  assert.deepEqual(row.literal_reversals.map(edit=>edit.ordinal),row.literal_reversals.map((_,index)=>index+1));
  assertUnwritten(row.path);
 }
});
for(const row of readerData.rows)test('bounded stage rejects every unknown, skipped, duplicate or reordered input: '+row.path,()=>{
 const physical=readCompared(row.path),source=physical.toString(),previous=restoreBoundedReaderSource(row.stage,row.path,physical),changed=Buffer.from(physical);changed[0]^=1;
 for(const bad of [previous,changed,physical.subarray(1),physical.subarray(0,-1),Buffer.concat([physical,Buffer.from(' ')]),null,{},[],0,new String(source),new Uint8Array(physical)])assert.throws(()=>restoreBoundedReaderSource(row.stage,row.path,bad));
 for(const edit of row.literal_reversals){
  assert.equal(source.split(edit.current).length,2);
  for(const bad of [source.replace(edit.current,()=>edit.previous),source+edit.current,source.replace(edit.current,()=>edit.current+' ')])assert.throws(()=>restoreBoundedReaderSource(row.stage,row.path,bad));
 }
 const [first,second]=row.literal_reversals,marker='__READER_COMPARISON_SWAP_SENTINEL__';assert(!source.includes(marker));
 const reordered=source.replace(first.current,()=>marker).replace(second.current,()=>first.current).replace(marker,()=>second.current);
 assert.throws(()=>restoreBoundedReaderSource(row.stage,row.path,reordered));
 for(const stage of ['',null,'unknown','consumer-to-frozen-main:'+row.path,...readerData.rows.filter(other=>other!==row).map(other=>other.stage)])assert.throws(()=>restoreBoundedReaderSource(stage,row.path,physical));
 for(const file of ['',null,'../'+row.path,row.path+'unknown',row.path.slice('weebdex-platform/'.length),...boundedReaderSuccessorPaths.filter(file=>file!==row.path)])assert.throws(()=>restoreBoundedReaderSource(row.stage,file,physical));
 assertUnwritten(row.path);
});
test('bounded metadata encoding, every nested fact and literal ordering are indivisibly digest-bound',()=>{
 const row=readerData.rows[0],physical=readCompared(row.path);
 const reject=bytes=>{assert.throws(()=>readBoundedReaderAssociation(bytes));assert.throws(()=>restoreBoundedReaderSource(row.stage,row.path,physical,bytes));};
 for(const bytes of [readerMetadata.toString(),null,{},new Uint8Array(readerMetadata),readerMetadata.subarray(1),Buffer.concat([readerMetadata,Buffer.from(' ')]),Buffer.from(JSON.stringify(readerData)),encoded(Object.fromEntries(Object.entries(readerData).reverse()))])reject(bytes);
 function visit(value,keys=[]){
  for(const key of Object.keys(value)){
   const route=[...keys,key],copy=structuredClone(readerData);let parent=copy;for(const segment of keys)parent=parent[segment];delete parent[key];reject(encoded(copy));
   if(value[key]!==null&&typeof value[key]==='object')visit(value[key],route);
   else{const altered=structuredClone(readerData);let target=altered;for(const segment of keys)target=target[segment];target[key]=typeof value[key]==='string'?value[key]+' ':typeof value[key]==='number'?value[key]+1:!value[key];reject(encoded(altered));}
  }
 }
 visit(readerData);
 for(const mutate of [copy=>copy.rows.reverse(),copy=>copy.rows.pop(),copy=>copy.rows.push(copy.rows[0]),copy=>copy.rows[0].literal_reversals.reverse(),copy=>copy.rows[0].literal_reversals.push(copy.rows[0].literal_reversals[0]),copy=>copy.rows[0].literal_reversals.pop(),copy=>copy.unknown=true,copy=>copy.rows[0].unknown=true,copy=>copy.rows[0].literal_reversals[0].unknown=true]){const copy=structuredClone(readerData);mutate(copy);reject(encoded(copy));}
});
test('physical dependencies, old metadata and capsule dispositions are checked independently of reconstruction',()=>{
 // Strict consumer contract successors bind old metadata and new physical bytes separately.
 const strictSuccessors=new Map([
  ["weebdex-platform/platform-integrations/bounded-reader/presentation.ts", {before: [1455, "f609e06d87da8529b40c7f21ceddcf8165ca47158161d769f1f125c6e11f4d39"], after: [1462, "2756117714dfa19955938a7e072b8ffe8d8549b446db3d327b969bd97cf3ab61"]}],
  ["weebdex-platform/platform-integrations/bounded-reader/EditionSelector.svelte", {before: [481, "d3d28a1164341dfce9eb9cfad7df90b77c1c3cfae4f2211828b1bf881da0673c"], after: [555, "1f384b4e5348091db37a0462150e890390ff9e05ed752199abf755a7d9b8e607"]}],
  ["weebdex-platform/platform-integrations/bounded-reader/anchors.json", {before: [48213, "57a4c1020b4afcb7f66c345d3d0ff070d24cb649a90623a3b3ccd504d56d3e23"], after: [48293, "feafb34479f4d81413f44cc24d786beb431e9bebffc6d6be24de1753139d2fae"]}],
 ]);
 const assertPhysical=pin=>{const physical=readCompared(pin.path),strict=strictSuccessors.get(pin.path);if(strict){assert.deepEqual([pin.bytes,pin.sha256],strict.before,pin.path);assert.deepEqual([physical.length,hash(physical)],strict.after,pin.path);return;}if(pin.path==='weebdex-platform/scripts/integrated-load/mounted-reader.mjs'){/* Historical association metadata stays immutable; current test-harness successor is explicit. */assert.deepEqual([pin.bytes,pin.sha256],[44683,'b6d7b0a15681feada198897b35a4c497dde8b800226a011eba85fe8fe910e977']);assert.deepEqual([physical.length,hash(physical)],[45414,'badb60c9f59b93f2267a5a50bda41364acc7086d7bd2cff4e38a0d8153c998ee']);return;}assert.equal(physical.length,pin.bytes,pin.path);assert.equal(hash(physical),pin.sha256,pin.path);};
 assert.equal(readerData.actual_dependency_pins.length,37);
 for(const pin of readerData.actual_dependency_pins)assertPhysical(pin);
 assert.equal(readerData.preserved_metadata.length,11);for(const pin of readerData.preserved_metadata)assertPhysical(pin);
 assertPhysical(readerData.preserved_local_page_metadata);
 assert.equal(readerData.preserved_local_page_metadata.bytes,73175);assert.equal(readerData.preserved_local_page_metadata.sha256,'c62216c07016789f0cbc25dd0277ebddb4ae1a707edefcb22cb0a4d749263d1c');
 const [forum,mounted]=readerData.capsule_dispositions;assert.equal(readerData.capsule_dispositions.length,2);
 assert.equal(forum.existing_capsule.bytes,7346);assert.equal(forum.existing_capsule.sha256,'1bf6022f610dc312936a20a92e566788191e2210171a1893e3276b526435c4d0');assertPhysical(forum.existing_capsule);
 for(const pin of forum.current_runtime_inputs)assertPhysical(pin);
 assert.deepEqual(forum.current_runtime_inputs.map(pin=>[pin.bytes,pin.sha256]),[[2553,'748fc2997a528057400d4d9368aa58b8ab8c0fd1c3f5e4adacf9b5eb11d6b819'],[16458,'afe757c8eb7391fdb8b77d74bcf885c56849d782559efdb9625976724f5b1b71']]);
 assert.equal(mounted.historical_source.bytes,21816);assert.equal(mounted.historical_source.sha256,'b99b2a0ebbe1e0b0ffb40f4836091ad8423f0450b27cf0427bd23899462bf851');
 assert.equal(mounted.current_source.bytes,44683);assert.equal(mounted.current_source.sha256,'b6d7b0a15681feada198897b35a4c497dde8b800226a011eba85fe8fe910e977');assertPhysical(mounted.current_source);
 assert.match(mounted.disposition,/retained in Git only/);assert.equal(Object.hasOwn(mounted,'proposed_capsule_path_if_needed_later'),false);
 for(const file of [forum.historical_source.path,mounted.historical_source.path])assert(!boundedReaderSuccessorPaths.includes(file),'No runtime capsule recipe');
 const guard=readerData.unchanged_language_guard,physical=fs.readFileSync(path.join(root,guard.path),'utf8');
 assert.equal(physical.split(guard.start_marker_utf8).length,2);assert.equal(physical.split(guard.end_marker_utf8).length,2);
 const slice=physical.slice(physical.indexOf(guard.start_marker_utf8),physical.indexOf(guard.end_marker_utf8));
 assert.equal(Buffer.byteLength(slice),801);assert.equal(hash(slice),'213da1c168ac55a52403f2520959fa2ea8e7a3d87d8b36e4d559dafc822bd9c0');
});
test('library reader predecessor precedes history exactly once without swallowing another layer',()=>{
 const file='weebdex-platform/platform-integrations/local-library/library.ts',stage='bounded-reader-to-frozen-main:platform-integrations/local-library/library.ts',physical=readCompared(file),reader=restoreBoundedReaderSource(stage,file,physical),history=restoreHistorySource(file,reader);
 assert.deepEqual([physical.length,reader.length,Buffer.byteLength(history)],[37875,35991,35984]);
 assert.deepEqual([hash(physical),hash(reader),hash(history)],['aba0286cf8cb386d703fa2580f937722fe4e68f383a14b73f3a5ebd5830e1838','18a90ef68078943f7fbc25bcac80479f0311079f745ef924acd538a00c45cb0e','7a731e22f8242e278068779393ff1b3b75e3e9306d608598bdb066d5e18a3eef']);
 assert.throws(()=>restoreHistorySource(file,physical));
 for(const input of [reader,history])assert.throws(()=>restoreBoundedReaderSource(stage,file,input));
 assertUnwritten(file);
});

test('historical comparison layers never accept edited current UX bytes or rewrite live source files',()=>{
 for(const [file,live] of liveSourceSnapshots){
  assertUnwritten(file);
  if(firstReaderPaths.has(file)||secondReaderPaths.has(file)){
   assert.throws(()=>beforeReaderUx(file,Buffer.concat([live,Buffer.from(' ')])));
   const prior=beforeReaderUx(file,live);assert.throws(()=>beforeReaderUx(file,prior));
  }
 }
});
