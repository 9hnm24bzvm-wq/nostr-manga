// Exact current preparation checks only; not browser or HTTP qualification.
import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {assertReaderFixpassExceptionScope,prepareFrontendIntegration} from '../../scripts/prepare-frontend-integration.mjs';
import {frontendApprovedReadToggleExceptions,frontendApprovedRegistrationRecoveryExceptions,frontendApprovedReaderSimplicityExceptions,frontendApprovedDiscoveryCopyExceptions} from '../../platform-integrations/frontend-overlays.mjs';
const root=fileURLToPath(new URL('../../',import.meta.url));
const scopes={read_toggle:frontendApprovedReadToggleExceptions,registration_recovery:frontendApprovedRegistrationRecoveryExceptions,reader_simplicity:frontendApprovedReaderSimplicityExceptions,discovery_copy:frontendApprovedDiscoveryCopyExceptions};
test('new reader fixpass exception registries are complete and independently exact-bound',()=>{
 assertReaderFixpassExceptionScope();
 assert.equal(scopes.read_toggle.length,1);assert.equal(scopes.registration_recovery.length,3);
 assert.equal(scopes.reader_simplicity.length,19);assert.equal(scopes.discovery_copy.length,12);
 for(const entries of Object.values(scopes))for(const row of entries){assert.ok(row.before);assert.ok(row.after);assert.notEqual(row.before,row.after);assert.ok(row.count>0);assert.match(row.sha256,/^[0-9a-f]{64}$/);}
});
for(const [name,entries]of Object.entries(scopes))test('exact '+name+' scope refuses edited, extra, missing, partial, reordered and wrongly typed exception data',()=>{
 const saved=structuredClone(entries),restore=()=>entries.splice(0,Infinity,...structuredClone(saved));
 try{
  for(const patch of [{path:'src/routes/arbitrary.svelte'},{sha256:'0'.repeat(64)},{count:0},{count:saved[0].count+1},{before:saved[0].before+' '},{after:saved[0].after+' '},{extra:true},{composed_before:true}]){
   Object.assign(entries[0],patch);assert.throws(assertReaderFixpassExceptionScope,/Unreviewed reader fixpass/);restore();
  }
  for(const key of Object.keys(saved[0])){delete entries[0][key];assert.throws(assertReaderFixpassExceptionScope,/Unreviewed reader fixpass/);restore();}
  entries.push(structuredClone(saved[0]));assert.throws(assertReaderFixpassExceptionScope,/Unreviewed reader fixpass/);restore();
  entries.pop();assert.throws(assertReaderFixpassExceptionScope,/Unreviewed reader fixpass/);restore();
  if(entries.length>1){entries.reverse();assert.throws(assertReaderFixpassExceptionScope,/Unreviewed reader fixpass/);}
 }finally{restore();}
 assertReaderFixpassExceptionScope();
});
test('actual preparation records exact fixpass controls with both public gate states while preserving original attachment bytes',()=>{
 const temporary=fs.mkdtempSync(path.join(os.tmpdir(),'weebdex-reader-fixpass-')),frontend=path.join(temporary,'frontend'),prior=process.env.PUBLIC_MANGADEX_CATALOGUE;
 try{
  fs.cpSync(path.join(root,'frontend'),frontend,{recursive:true,filter:file=>!file.split(path.sep).some(part=>['node_modules','.svelte-kit','build'].includes(part))});
  const paths=[...new Set(Object.values(scopes).flat().map(row=>row.path))];
  const original=new Map(paths.map(relative=>[relative,fs.readFileSync(path.join(frontend,relative))]));
  for(const gate of ['0','1']){
   process.env.PUBLIC_MANGADEX_CATALOGUE=gate;
   const receipt=prepareFrontendIntegration(frontend,{evidenceDirectory:path.join(temporary,'evidence-'+gate)});
   for(const [name,entries]of Object.entries(scopes))assert.deepEqual(receipt['approved_'+name+'_exceptions'],entries);
   for(const [relative,bytes]of original)assert.deepEqual(fs.readFileSync(path.join(frontend,relative)),bytes,relative);
   assert.equal(receipt.original_files_written,false);
   assert.equal(receipt.external_component_extensions.some(row=>row.path==='platform-integrations/public-api/FeedCommentsLink.svelte'),gate==='1');
   assert.ok(receipt.external_component_extensions.some(row=>row.path==='platform-integrations/local-library/BackupReviewSummary.svelte'));
  }
 }finally{if(prior===undefined)delete process.env.PUBLIC_MANGADEX_CATALOGUE;else process.env.PUBLIC_MANGADEX_CATALOGUE=prior;fs.rmSync(temporary,{recursive:true,force:true});}
});
