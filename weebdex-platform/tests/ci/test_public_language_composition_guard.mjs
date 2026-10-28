// Finite source/composition checks only. No browser, build or HTTP acceptance.
import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import {createHash} from 'node:crypto';
import {fileURLToPath} from 'node:url';
import {createPublicApiOverlays, publicApiApprovedLanguageExceptions, publicApiApprovedFilterExceptions, assertPublicApiExceptionScope} from '../../platform-integrations/public-api/overlays.mjs';
import {prepareFrontendIntegration} from '../../scripts/prepare-frontend-integration.mjs';
import {createFrontendOverlays} from '../../platform-integrations/frontend-overlays.mjs';
import {createReaderOverlays} from '../../platform-integrations/frontend-reader-overlays.mjs';
import {createLocalLibraryOverlays} from '../../platform-integrations/local-library/overlays.mjs';
const root=path.resolve(path.dirname(fileURLToPath(import.meta.url)),'../..');
const frontend=path.join(root,'frontend');
const relative='src/lib/components/MangaSearchFilter.svelte';
const hash=value=>createHash('sha256').update(value).digest('hex');
const original=fs.readFileSync(path.join(frontend,relative),'utf8');
const retained=value=>value.replace(/<script\b[^>]*>[\s\S]*?<\/script>/g,'<script></script>');

test('only the two exact reviewed language selector replacements enter the markup registry',()=>{
 assertPublicApiExceptionScope();
 assert.equal(hash(JSON.stringify(publicApiApprovedLanguageExceptions)),'ce6ba35bc27ca8eb40bc7e0dc0e8df3eab07c6160cab3662664827482a8fd443');
 assert.equal(publicApiApprovedLanguageExceptions.length,2);
 assert.equal(publicApiApprovedLanguageExceptions[0].after,'\t\t\t<PublicLanguageFilter bind:value={search.availableTranslatedLang} label="Translated languages" />');
 assert.match(publicApiApprovedLanguageExceptions[0].before,/disabled=\{!search.hasChapters\}/);
 for(const row of publicApiApprovedLanguageExceptions){assert.equal(row.path,relative);assert.equal(row.sha256,hash(original));assert.equal(row.count,1);assert.equal(row.composed_before,undefined);}
 assert.deepEqual(publicApiApprovedLanguageExceptions.map(row=>row.approved_public_language_exception),['public_wire_translated_language_codes','public_wire_original_language_codes']);
});

test('unknown bytes, labels, paths, counts, duplicate or partial language exceptions are refused',()=>{
 const saved=structuredClone(publicApiApprovedLanguageExceptions);
 const restore=()=>publicApiApprovedLanguageExceptions.splice(0,Infinity,...structuredClone(saved));
 try{
  for(const patch of [{before:saved[0].before+' '},{after:saved[0].after+' '},{sha256:'0'.repeat(64)},{label:'unreviewed'},{path:'src/lib/components/MangaSearch.svelte'},{count:2},{composed_before:true},{approved_public_language_exception:'arbitrary'}]){
   Object.assign(publicApiApprovedLanguageExceptions[0],patch);
   assert.throws(assertPublicApiExceptionScope,/Public language exception scope changed/);restore();
  }
  publicApiApprovedLanguageExceptions.push({...saved[0]});assert.throws(assertPublicApiExceptionScope,/Public language exception scope changed/);restore();
  publicApiApprovedLanguageExceptions.pop();assert.throws(assertPublicApiExceptionScope,/Public language exception scope changed/);restore();
  publicApiApprovedLanguageExceptions.reverse();assert.throws(assertPublicApiExceptionScope,/Public language exception scope changed/);
 }finally{restore();}
 assertPublicApiExceptionScope();
});

test('unchanged actual reverse guard admits exact selectors, while the old missing registry and arbitrary markup fail',()=>{
 const preparation=fs.readFileSync(path.join(root,'scripts/prepare-frontend-integration.mjs'),'utf8');
 const guard=preparation.slice(preparation.indexOf('\t\tlet normalizedResult = preUpload;'),preparation.indexOf('\t\treturn { path: relative,'));
 // Privately verified byte-identical to the failed 312dee79 preparation guard.
 assert.equal(hash(guard),'213da1c168ac55a52403f2520959fa2ea8e7a3d87d8b36e4d559dafc822bd9c0');
 const check=new Function('hash','original','prePublic','preUpload','exceptions','relative',guard+'\nreturn normalizedResult;');
 let prePublic=original;
 for(const plugin of [createFrontendOverlays(frontend),createReaderOverlays(frontend),createLocalLibraryOverlays(frontend)])prePublic=plugin.transform.handler(prePublic,path.join(frontend,relative))?.code??prePublic;
 const composed=createPublicApiOverlays(frontend,{enabled:true}).transform.handler(prePublic,path.join(frontend,relative)).code;
 assert.notEqual(retained(composed),retained(original));
 assert.equal(retained(check(hash,original,prePublic,composed,[...publicApiApprovedLanguageExceptions,...publicApiApprovedFilterExceptions.filter(row=>row.path===relative)],relative)),retained(original));
 assert.throws(()=>check(hash,original,prePublic,composed,[],relative),/Overlay changed original HTML\/CSS/);
 assert.throws(()=>check(hash,original,prePublic,composed,publicApiApprovedLanguageExceptions.slice(0,1),relative),/Overlay changed original HTML\/CSS/);
 assert.throws(()=>check(hash,original,prePublic,composed+'<p>unreviewed</p>',publicApiApprovedLanguageExceptions,relative),/Overlay changed original HTML\/CSS/);
 assert.throws(()=>check(hash,original+' ',original,composed,[...publicApiApprovedLanguageExceptions,...publicApiApprovedFilterExceptions.filter(row=>row.path===relative)],relative),/Approved event handler anchor changed/);
});

test('actual preparation keeps native gate off and records only the exact public language extension when on',()=>{
 const directory=fs.mkdtempSync(path.join(os.tmpdir(),'weebdex-language-guard-'));
 const copy=path.join(directory,'frontend'),prior=process.env.PUBLIC_MANGADEX_CATALOGUE;
 try{
  fs.cpSync(frontend,copy,{recursive:true,filter:file=>!file.split(path.sep).some(part=>['node_modules','.svelte-kit','build'].includes(part))});
  for(const gate of ['0','1']){
   process.env.PUBLIC_MANGADEX_CATALOGUE=gate;
   const receipt=prepareFrontendIntegration(copy,{evidenceDirectory:path.join(directory,'evidence-'+gate)});
   assert.deepEqual(receipt.approved_public_language_exceptions,gate==='1'?publicApiApprovedLanguageExceptions:[]);
   assert.equal(receipt.external_component_extensions.some(row=>row.path==='platform-integrations/public-api/PublicLanguageFilter.svelte'),gate==='1');
   assert.equal(receipt.external_component_extensions.some(row=>row.path==='platform-integrations/public-api/PublicTitleGroupFilter.svelte'),gate==='1');
   assert.equal(receipt.original_files_written,false);
   assert.equal(fs.readFileSync(path.join(copy,relative),'utf8'),original);
  }
 }finally{
  if(prior===undefined)delete process.env.PUBLIC_MANGADEX_CATALOGUE;else process.env.PUBLIC_MANGADEX_CATALOGUE=prior;
  fs.rmSync(directory,{recursive:true,force:true});
 }
});
