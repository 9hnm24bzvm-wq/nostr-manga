#!/usr/bin/env node
// Reconstructed strict native DTO normalization and actual composed save
// callback. Real jsdom constraint validation; native completion is a stand-in.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import vm from 'node:vm';
import {createRequire,stripTypeScriptTypes} from 'node:module';
import test from 'node:test';
import {createFrontendOverlays,frontendOverlayManifest} from '../../platform-integrations/frontend-overlays.mjs';
const root=new URL('../../',import.meta.url).pathname,frontend=path.join(root,'frontend');
const relative='src/lib/components/draft/manga/MangaEdit.svelte',file=path.join(frontend,relative);
const original=fs.readFileSync(file,'utf8'),plugin=createFrontendOverlays(frontend),composed=plugin.transform.handler(original,file).code;
const expression=composed.match(/year: (snapshot\.year[^,]+),/)[1];
const normalize=year=>vm.runInNewContext(stripTypeScriptTypes('('+expression+')'),{snapshot:{year}});
const modules=process.env.WEEBDEX_BROWSER_DOM_NODE_MODULES;assert(modules,'Use reviewed exact-lock DOM stage');
const {JSDOM}=createRequire(import.meta.url)(path.join(modules,'jsdom'));
const dom=new JSDOM('<form><input id="year" type="number" min="1900" max="2100"><button>Save</button></form>');
const input=dom.window.document.querySelector('input');
test('one hash-bound overlay normalizes only absent/zero year without changing original year bounds',()=>{
 const patches=frontendOverlayManifest.filter(row=>row.path===relative&&row.label==='native unknown publication year is empty in the title editor, never invalid numeric zero');assert.equal(patches.length,1);assert.equal(patches[0].count,1);assert.equal(original.split('year: snapshot.year ?? null!,').length-1,1);
 assert(original.includes('let draft = $state<MangaDraft>(structuredClone($state.snapshot(initial)))'));
 const metadata=fs.readFileSync(path.join(frontend,'src/lib/components/draft/manga/MetadataSection.svelte'),'utf8');assert.match(metadata,/id="year"[\s\S]*?min="1900"[\s\S]*?max="2100"/);
 for(const year of [undefined,null,0]){assert.equal(normalize(year),null);input.value=normalize(year)??'';assert.equal(input.checkValidity(),true);}
 for(const year of [1900,2026,2100]){assert.equal(normalize(year),year);input.value=String(normalize(year));assert.equal(input.checkValidity(),true);}
 for(const year of [-1,1899,2101]){assert.equal(normalize(year),year);input.value=String(normalize(year));assert.equal(input.checkValidity(),false);}
 input.value='0';assert.equal(input.checkValidity(),false,'old native0 genuinely blocks form submission');
});
test('actual save builder leaves unknown year unset on title-only edits and preserves explicit positive year',async()=>{
 const start=composed.indexOf('\tconst updateManga = async () => {'),end=composed.indexOf('\n\tconst processCovers',start);assert(start>0&&end>start);
 for(const [year,changed]of [[0,undefined],[null,undefined],[undefined,undefined],[1900,2026]]){
  let sent;const initial={title:'before',year:normalize(year)},draft={...initial,title:'after'};if(changed!==undefined)draft.year=changed;
  const context={manga:{id:'OwnedTitle',version:4,year},initial,draft,equal:(a,b)=>a===b,window:{location:{pathname:'/edit'}},pathname:'/edit',mutexState:0,STATE:{UpdatingEntry:1},StatusCodes:{OK:200},API:{Manga:{async Update(id,payload){assert.equal(id,'OwnedTitle');sent=structuredClone(payload);return{status:200,body:{id,version:5,...payload}};}}}};
  await vm.runInNewContext(stripTypeScriptTypes(composed.slice(start,end))+'\nupdateManga();',context);assert.equal(sent.title,'after');assert.equal(sent.version,4);assert.equal(Object.hasOwn(sent,'year'),changed!==undefined);if(changed!==undefined)assert.equal(sent.year,changed);
 }
 dom.window.close();
});
