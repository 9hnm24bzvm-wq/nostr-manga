import {localPageDriverPath,restoreLocalPageDriverSource} from './reviewed-local-page-driver-source-successor.mjs';
import {updatesIntegrationPaths,restoreBundleSource} from './reviewed-bundle-source-successors.mjs';
import {nativeProfileActionsPaths,restoreNativeProfileActionsSource} from "./native-profile-actions-source-association.mjs";
// Exact finite comparisons only. No application build, browser, database or HTTP.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {createHash} from 'node:crypto';
import test from 'node:test';
import {readIntegrationAssociation,restoreIntegratedSource,integrationSuccessorPaths,integrationAssociationSha256} from './reviewed-integration-source-successors.mjs';
import {assertDiscussionFeatures,assertDiscussionManifest} from './native-discussion-overlay-fixture.mjs';
import {publicApiApprovedFeatureExceptions,publicApiOverlayManifest} from '../../platform-integrations/public-api/overlays.mjs';
const root=fileURLToPath(new URL('../../../',import.meta.url));
const metadata=fs.readFileSync(new URL('../../docs/fixtures/integrated-schedule-catalogue-membership-source-association.json',import.meta.url));
const hash=value=>createHash('sha256').update(value).digest('hex');
const encoded=value=>Buffer.from(JSON.stringify(value,null,2)+'\n');
const data=readIntegrationAssociation();
test('exact six current product rows and one comparison driver preserve prior reviewed bytes, not execution claims',()=>{
 assert.equal(hash(metadata),integrationAssociationSha256);
 assert.equal(hash(fs.readFileSync(new URL('../../docs/fixtures/integrated-discussion-title-source-association.json',import.meta.url))),data.prior_association_sha256);
 assert.equal(data.prior_association_sha256,'f1cb600e66092923ef0b60c31fbb525329b5c3467dc3537dd5b14259a046c7b9');
 assert.equal(data.integrated_product_source,'11ea3ea9f593c134f2ba478bb84fa683f4b14149');
 assert.equal(data.browser_executed,false);assert.equal(data.http_executed,false);assert.equal(data.capacity_qualified,false);
 assert.deepEqual(integrationSuccessorPaths,['platform-integrations/local-library/overlays.mjs','platform-integrations/public-api/anchors.json','platform-integrations/public-api/overlays.mjs','platform-integrations/public-api/source-hashes.json','platform-integrations/public-api/PublicReportInspection.svelte','platform-integrations/local-library/MangaFollow.ts','tests/ci/test_signed_feature_source_pins.py'].map(p=>'weebdex-platform/'+p));
 assert.deepEqual(data.rows.map(r=>r.kind),[...Array(6).fill('product_source'),'comparison_driver']);
 assert.match(data.scope,/never runtime\/build substitution or historical execution relabelling/);
 for(const row of data.rows){
  const physical=fs.readFileSync(path.join(root,row.path)),associated=row.path===localPageDriverPath?restoreLocalPageDriverSource(row.path,physical):physical,current=nativeProfileActionsPaths.includes(row.path)?restoreNativeProfileActionsSource(row.path,associated):updatesIntegrationPaths.includes(row.path)?restoreBundleSource('updates-integration',row.path,associated):associated,previous=restoreIntegratedSource(row.path,associated);
  assert(Buffer.isBuffer(previous));assert.equal(current.length,row.current_bytes);assert.equal(hash(current),row.current_sha256);
  assert.equal(previous.length,row.previous_bytes);assert.equal(hash(previous),row.previous_sha256);assert.notDeepEqual(previous,current);
  assert.equal(restoreIntegratedSource(row.path,associated.toString('utf8')),previous.toString('utf8'));
  assert.deepEqual(fs.readFileSync(path.join(root,row.path)),physical);
 }
});
for(const row of data.rows)test('every current byte binding and literal reversal fails closed: '+row.path,()=>{
 const physical=fs.readFileSync(path.join(root,row.path)),associated=row.path===localPageDriverPath?restoreLocalPageDriverSource(row.path,physical):physical,current=nativeProfileActionsPaths.includes(row.path)?restoreNativeProfileActionsSource(row.path,associated):updatesIntegrationPaths.includes(row.path)?restoreBundleSource('updates-integration',row.path,associated):associated,previous=restoreIntegratedSource(row.path,associated),changed=Buffer.from(current);changed[0]^=1;
 for(const candidate of [previous,changed,Buffer.concat([current,Buffer.from(' ')]),current.subarray(1),null,{},new Uint8Array(current)])assert.throws(()=>restoreIntegratedSource(row.path,candidate));
 for(const edit of row.literal_reversals){
  const text=current.toString('utf8');assert.equal(text.split(edit.current).length,2);
  assert.throws(()=>restoreIntegratedSource(row.path,text.replace(edit.current,()=>edit.previous)));
  assert.throws(()=>restoreIntegratedSource(row.path,text+edit.current));
 }
 for(const file of [row.path+'unknown','',null,'../'+row.path])assert.throws(()=>restoreIntegratedSource(file,current));
});
test('full metadata digest rejects every altered claim, row field, literal, ordering and encoding',()=>{
 const candidates=[Buffer.concat([metadata,Buffer.from(' ')]),metadata.subarray(1),metadata.toString('utf8'),encoded(Object.fromEntries(Object.entries(data).reverse()))];
 for(const key of Object.keys(data))for(const operation of ['delete','change']){const value=structuredClone(data);if(operation==='delete')delete value[key];else value[key]='unknown';candidates.push(encoded(value));}
 for(let i=0;i<data.rows.length;i++){
  for(const key of Object.keys(data.rows[i])){const value=structuredClone(data);delete value.rows[i][key];candidates.push(encoded(value));}
  for(let j=0;j<data.rows[i].literal_reversals.length;j++)for(const key of ['current','previous']){const value=structuredClone(data);value.rows[i].literal_reversals[j][key]+=' ';candidates.push(encoded(value));}
 }
 for(const mutate of [v=>v.unknown=true,v=>v.rows.reverse(),v=>v.rows.pop(),v=>v.rows.push(v.rows[0]),v=>v.rows[0].path='unknown',v=>v.rows[0].literal_reversals.reverse()]){const value=structuredClone(data);mutate(value);candidates.push(encoded(value));}
 const current=fs.readFileSync(path.join(root,data.rows[0].path));
 for(const candidate of candidates){assert.throws(()=>readIntegrationAssociation(candidate));assert.throws(()=>restoreIntegratedSource(data.rows[0].path,current,candidate));}
});
test('exact feature delta rejects same-count substitutions, duplicate/new paths and changed original eighteen',()=>{
 assertDiscussionFeatures(publicApiApprovedFeatureExceptions);assertDiscussionManifest(publicApiOverlayManifest);
 for(const index of [0,18,19,20,21]){const rows=structuredClone(publicApiApprovedFeatureExceptions);rows[index].after+=' ';assert.throws(()=>assertDiscussionFeatures(rows));}
 const duplicate=structuredClone(publicApiApprovedFeatureExceptions);duplicate[19]=duplicate[18];assert.throws(()=>assertDiscussionFeatures(duplicate));
 for(const index of [0,59,60,61]){const rows=structuredClone(publicApiOverlayManifest);rows[index].path+='unknown';assert.throws(()=>assertDiscussionManifest(rows));}
 const changed=structuredClone(publicApiOverlayManifest);changed.find(row=>row.path==='src/routes/(app)/forums/threads/[id]/+page.svelte').activation='always on';assert.throws(()=>assertDiscussionManifest(changed));
});
test('comparison helpers are not imported into current runtime or preparation inputs',()=>{
 function visit(directory){for(const entry of fs.readdirSync(directory,{withFileTypes:true})){const file=path.join(directory,entry.name);if(entry.isDirectory())visit(file);else if(/\.(?:mjs|ts|svelte|py|rs)$/.test(file))assert.doesNotMatch(fs.readFileSync(file,'utf8'),/reviewed[-_]integration[-_]source[-_]successors/,file);}}
 for(const directory of ['platform-integrations','frontend/src','scripts','backend/src'])visit(path.join(root,'weebdex-platform',directory));
});
