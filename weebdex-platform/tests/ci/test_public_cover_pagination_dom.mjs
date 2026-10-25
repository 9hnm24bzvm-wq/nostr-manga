// Actual mounted Svelte cover editor + actual reader/controller + signed request
// transport. JSDOM/HTTP/signatures and image URL rendering are explicit stand-ins.
import fs from 'node:fs';import path from 'node:path';import {createRequire}from'node:module';import{pathToFileURL}from'node:url';import test from'node:test';import assert from'node:assert/strict';
import{paginationFixture,coverId,deferred,api,actor,root,json,plain,image}from'./cover-pagination-fixture.mjs';
const dependencies=process.env.WEEBDEX_FRONTEND_ROOT,require=createRequire(path.join(dependencies,'package.json')),{compile}=require('svelte/compiler'),domModules=process.env.WEEBDEX_BROWSER_DOM_NODE_MODULES;
assert.equal(require('svelte/package.json').version,'5.55.1');assert.equal(require(path.join(domModules,'jsdom/package.json')).version,'27.0.1');
const{JSDOM}=require(path.join(domModules,'jsdom')),{window}=new JSDOM('<main></main>',{url:'https://fixture.invalid/title/'+actor+'/edit',pretendToBeVisual:true});
for(const key of['window','document','Node','Element','HTMLElement','HTMLInputElement','HTMLTextAreaElement','HTMLSelectElement','Text','Comment','Event','MouseEvent','CustomEvent','MutationObserver','requestAnimationFrame','cancelAnimationFrame','getComputedStyle'])globalThis[key]=window[key];
const svelte=pathToFileURL(path.join(dependencies,'node_modules/svelte/src/index-client.js')).href,{mount,unmount,flushSync}=await import(svelte);
fs.mkdirSync(path.join(root,'.build'),{recursive:true});const generation=fs.mkdtempSync(path.join(root,'.build/cover-pagination-dom-'));fs.symlinkSync(path.join(dependencies,'node_modules'),path.join(generation,'node_modules'));
const sourceRoot=process.env.WEEBDEX_COVER_PAGINATION_CONTROL_SOURCE??path.join(root,'platform-integrations/public-api');
const source=fs.readFileSync(path.join(sourceRoot,'PublicCoverEditor.svelte'),'utf8').replace("from 'svelte'",'from '+JSON.stringify(svelte)).replace(/ import \{ createSignedCoverController,[^\n]+from '\.\/cover-client';/," const { createSignedCoverController, coverCommandCapabilities }=globalThis.__coverPaginationEnvironment;");
const file=path.join(generation,'PublicCoverEditor.mjs');fs.writeFileSync(file,compile(source,{filename:'PublicCoverEditor.svelte',generate:'client',runes:true}).js.code);const Editor=(await import(pathToFileURL(file))).default;
// Exact one-expression integration counterfactual: nullable selection already
// renders empty, but the old dirty comparator still substitutes title.language.
const dirtyCurrent="locale!==(selected.locale??'')",dirtyPrevious='locale!==(selected.locale??title.language)';
assert.equal(source.split(dirtyCurrent).length,2,'one exact reviewed nullable dirty comparator');
const mismatchFile=path.join(generation,'PublicCoverEditor-old-null-locale-mismatch.mjs');fs.writeFileSync(mismatchFile,compile(source.replace(dirtyCurrent,dirtyPrevious),{filename:'PublicCoverEditor-old-null-locale-mismatch.svelte',generate:'client',runes:true}).js.code);const OldNullLocaleMismatch=(await import(pathToFileURL(mismatchFile))).default;
const originalCreate=URL.createObjectURL,originalRevoke=URL.revokeObjectURL,originalConfirm=globalThis.confirm;let urls=0,revoked=[];URL.createObjectURL=()=>`blob:finite-${++urls}`;URL.revokeObjectURL=value=>revoked.push(value);
test.after(()=>{window.close();fs.rmSync(generation,{recursive:true,force:true});URL.createObjectURL=originalCreate;URL.revokeObjectURL=originalRevoke;globalThis.confirm=originalConfirm;delete globalThis.__coverPaginationEnvironment;});
async function settle(){for(let i=0;i<12;i++){await new Promise(resolve=>setImmediate(resolve));flushSync();}}
const button=(host,text)=>{const node=[...host.querySelectorAll('button')].find(node=>node.textContent.trim()===text);assert.ok(node,'button '+text);return node;};
function field(host,label){const node=[...host.querySelectorAll('label')].find(node=>node.firstChild?.textContent.trim()===label);assert.ok(node,'field '+label);return node.querySelector('input,textarea,select');}
function input(node,value){node.value=value;node.dispatchEvent(new window.Event(node.tagName==='SELECT'?'change':'input',{bubbles:true}));flushSync();}
async function ui(count=205,{nullLocales=[],editor=Editor}={}){const f=paginationFixture(count),states=[],host=document.createElement('main');let changed=0;revoked=[];globalThis.confirm=()=>true;for(const index of nullLocales)f.rows()[index-1].attributes.locale=null;
 globalThis.__coverPaginationEnvironment={createSignedCoverController:(check,title,expected)=>api.createSignedCoverController(check,title,expected,f.deps),coverCommandCapabilities:api.coverCommandCapabilities};document.body.append(host);
 const instance=mount(editor,{target:host,props:{title:f.title,accountId:actor,assertCurrent:()=>{if(!f.state.active)throw Error('Route changed');},onChanged:async()=>{changed++;},onState:state=>states.push(plain(state))}});await settle();return{...f,host,states,changed:()=>changed,async dispose(){await unmount(instance);host.remove();}};}

test('mounted nullable-locale selection and page navigation never invent unsaved language changes',async()=>{const f=await ui(101,{nullLocales:[1,101]});try{
 let asked=0;globalThis.confirm=()=>{asked++;return false;};input(field(f.host,'Saved cover'),coverId(1));assert.equal(field(f.host,'Cover language').value,'');assert.match(field(f.host,'Saved cover').selectedOptions[0].textContent,/unknown locale/);
 button(f.host,'Read current covers').click();await settle();assert.equal(f.reads().length,2);assert.equal(field(f.host,'Saved cover').value,coverId(1));
 button(f.host,'Next covers').click();await settle();assert.equal(f.reads().length,3);assert.match(f.host.textContent,/Showing 101-101 of 101 covers/);input(field(f.host,'Saved cover'),coverId(101));assert.equal(field(f.host,'Cover language').value,'');
 button(f.host,'Previous covers').click();await settle();assert.equal(f.reads().length,4);assert.equal(field(f.host,'Saved cover').value,'');assert.equal(field(f.host,'Cover language').value,f.title.language);assert.equal(asked,0);assert.equal(f.writes().length,0);
 }finally{await f.dispose();}});

test('mounted null locale is omitted from unrelated metadata update and remains null after acknowledgement',async()=>{const f=await ui(101,{nullLocales:[1]});try{
 input(field(f.host,'Saved cover'),coverId(1));assert.equal(field(f.host,'Cover language').value,'');input(field(f.host,'Description'),'Annotation without a language change');button(f.host,'Save cover metadata').click();await settle();
 assert.equal(f.writes().length,1);const patch=JSON.parse(new TextDecoder().decode(f.writes()[0].body));assert.equal(patch.description,'Annotation without a language change');assert.equal(Object.hasOwn(patch,'locale'),false);assert.equal(f.rows()[0].attributes.locale,null);assert.equal(f.changed(),1);assert.equal(field(f.host,'Cover language').value,'');assert.match(f.host.textContent,/Cover version 2/);
 let asked=0;globalThis.confirm=()=>{asked++;return false;};button(f.host,'Next covers').click();await settle();assert.equal(asked,0);assert.match(f.host.textContent,/Showing 101-101 of 101 covers/);assert.equal(f.writes().length,1);
 }finally{await f.dispose();}});

test('exact old nullable dirty-comparison mismatch produces the false discard prompt and blocks page navigation',async()=>{const f=await ui(101,{nullLocales:[1],editor:OldNullLocaleMismatch});try{
 input(field(f.host,'Saved cover'),coverId(1));assert.equal(field(f.host,'Cover language').value,'');let asked=0;globalThis.confirm=()=>{asked++;return false;};button(f.host,'Next covers').click();await settle();assert.equal(asked,1);assert.equal(f.reads().length,1);assert.equal(field(f.host,'Saved cover').value,coverId(1));assert.match(f.host.textContent,/Showing 1-100 of 101 covers/);assert.equal(f.writes().length,0);
 }finally{await f.dispose();}});

test('mounted editor navigates 205 actual controller rows, selects older main, updates and previews exact original',async()=>{const f=await ui();try{
 assert.equal(f.reads().length,1);assert.match(f.host.textContent,/Showing 1-100 of 205 covers/);assert.equal(field(f.host,'Saved cover').options.length,101);assert.equal(button(f.host,'Previous covers').disabled,true);
 button(f.host,'Next covers').click();await settle();assert.match(f.host.textContent,/Showing 101-200 of 205 covers/);button(f.host,'Next covers').click();await settle();assert.match(f.host.textContent,/Showing 201-205 of 205 covers/);assert.equal(button(f.host,'Next covers').disabled,true);
 input(field(f.host,'Saved cover'),coverId(205));assert.equal(field(f.host,'Volume').value,'205');assert.equal(f.host.querySelector('input[type=checkbox]').checked,true);assert.match(field(f.host,'Saved cover').selectedOptions[0].textContent,/main/);
 input(field(f.host,'Description'),'Older main annotation');button(f.host,'Save cover metadata').click();await settle();assert.equal(f.writes().length,1);assert.equal(f.changed(),1);assert.equal(field(f.host,'Saved cover').value,coverId(205));assert.equal(field(f.host,'Description').value,'Older main annotation');assert.match(f.host.textContent,/Cover version 2/);assert.match(f.host.textContent,/Showing 201-205 of 205 covers/);
 button(f.host,'Load signed original preview').click();await settle();assert.equal(f.host.querySelector('img').getAttribute('src').startsWith('blob:finite-'),true);
 button(f.host,'Previous covers').click();await settle();assert.equal(field(f.host,'Saved cover').value,'');assert.equal(f.host.querySelector('img'),null);assert.equal(revoked.length,1);assert.equal(field(f.host,'Description').value,'');assert.equal(f.states.at(-1).busy,false);
 }finally{await f.dispose();}});

test('unsaved selection, reload and paging cancellation preserve fields and issue no replacement read',async()=>{const f=await ui();try{
 input(field(f.host,'Saved cover'),coverId(1));input(field(f.host,'Description'),'Unsaved intent');let asked=0;globalThis.confirm=()=>{asked++;return false;};
 input(field(f.host,'Saved cover'),coverId(2));button(f.host,'Read current covers').click();button(f.host,'Next covers').click();await settle();assert.equal(asked,3);assert.equal(f.reads().length,1);assert.equal(field(f.host,'Saved cover').value,coverId(1));assert.equal(field(f.host,'Description').value,'Unsaved intent');
 globalThis.confirm=()=>true;button(f.host,'Next covers').click();await settle();assert.equal(f.reads().length,2);assert.equal(field(f.host,'Saved cover').value,'');assert.equal(field(f.host,'Description').value,'');assert.equal(f.writes().length,0);
 }finally{await f.dispose();}});

test('reload preserves exact surviving selection, but clears a removed selection and stale original preview',async()=>{const f=await ui(101);try{
 button(f.host,'Next covers').click();await settle();input(field(f.host,'Saved cover'),coverId(101));button(f.host,'Read current covers').click();await settle();assert.equal(field(f.host,'Saved cover').value,coverId(101));button(f.host,'Load signed original preview').click();await settle();
 f.setRows(f.rows().filter(row=>row.id!==coverId(101)));const before=f.reads().length;button(f.host,'Read current covers').click();await settle();assert.equal(f.reads().length,before+2);assert.match(f.host.textContent,/Showing 1-100 of 100 covers/);assert.equal(field(f.host,'Saved cover').value,'');assert.equal(field(f.host,'Volume').value,'');assert.equal(f.host.querySelector('img'),null);assert.equal(revoked.length,1);assert.equal(f.writes().length,0);
 }finally{await f.dispose();}});

test('actual approved removal of sole last-page cover returns to last nonempty page without replay',async()=>{const f=await ui(101);try{
 button(f.host,'Next covers').click();await settle();input(field(f.host,'Saved cover'),coverId(101));button(f.host,'Load signed original preview').click();await settle();const before=f.reads().length;
 button(f.host,'Approve instance cover removal').click();await settle();assert.equal(f.writes().length,1);assert.equal(f.writes()[0].init.method,'DELETE');assert.equal(f.reads().length,before+2);assert.equal(f.changed(),1);assert.match(f.host.textContent,/Showing 1-100 of 100 covers/);assert.equal(field(f.host,'Saved cover').value,'');assert.equal(button(f.host,'Next covers').disabled,true);assert.equal(revoked.length,1);
 button(f.host,'Read current covers').click();await settle();assert.equal(f.writes().length,1);
 }finally{await f.dispose();}});

test('failed page revokes editable rows but leaves explicit reload available',async()=>{const f=await ui();try{
 input(field(f.host,'Saved cover'),coverId(1));f.hook(request=>{if(request.path.endsWith('/covers'))return json({},503);});button(f.host,'Next covers').click();await settle();assert.match(f.host.textContent,/HTTP 503/);assert.equal(field(f.host,'Saved cover').value,'');assert.equal(field(f.host,'Saved cover').disabled,true);assert.equal(f.host.querySelector('fieldset').disabled,true);assert.equal(button(f.host,'Read current covers').disabled,false);
 f.hook(undefined);button(f.host,'Read current covers').click();await settle();assert.match(f.host.textContent,/Showing 1-100 of 205 covers/);assert.equal(field(f.host,'Saved cover').disabled,false);assert.equal(f.writes().length,0);
 }finally{await f.dispose();}});

test('repeated page clicks are serialized and unmounted delayed responses cannot install a stale page',async()=>{const f=await ui(),gate=deferred();let disposed=false;try{
 f.hook(async request=>{if(request.path.endsWith('/covers'))await gate.promise;});button(f.host,'Next covers').click();await settle();button(f.host,'Next covers').click();assert.equal(f.reads().length,2);assert.equal(field(f.host,'Saved cover').disabled,true);assert.equal(f.states.at(-1).busy,true);
 await f.dispose();disposed=true;gate.release();await settle();assert.equal(f.writes().length,0);assert.equal(f.host.textContent,'');
 }finally{gate.release();if(!disposed)await f.dispose();}});

test('rapidly shrinking live offsets use at most one correction then require explicit reload without false counts',async()=>{const f=await ui();try{
 button(f.host,'Next covers').click();await settle();button(f.host,'Next covers').click();await settle();f.setRows(f.rows().slice(0,102));let calls=0;f.hook(request=>{if(request.path.endsWith('/covers')&&++calls===2)f.setRows(f.rows().slice(0,50));});const before=f.reads().length;
 button(f.host,'Read current covers').click();await settle();assert.equal(f.reads().length,before+2);assert.match(f.host.textContent,/Cover pages changed again/);assert.doesNotMatch(f.host.textContent,/Showing/);assert.equal(field(f.host,'Saved cover').disabled,true);assert.equal(button(f.host,'Read current covers').disabled,false);
 f.hook(undefined);button(f.host,'Read current covers').click();await settle();assert.match(f.host.textContent,/Showing 1-50 of 50 covers/);assert.equal(f.writes().length,0);
 }finally{await f.dispose();}});


test('new exact original uploaded from an older page selects its first-page acknowledgement without fetching history',async()=>{const f=await ui();try{
 button(f.host,'Next covers').click();await settle();button(f.host,'Next covers').click();await settle();
 const input=f.host.querySelector('input[type=file]');Object.defineProperty(input,'files',{value:[image]});input.dispatchEvent(new window.Event('change',{bubbles:true}));flushSync();const before=f.reads().length;
 button(f.host,'Upload exact original cover').click();await settle();assert.equal(f.writes().length,1);assert.equal(f.writes()[0].init.method,'POST');assert.equal(f.reads().length,before+1);assert.equal(new URL(f.reads().at(-1).url).searchParams.get('offset'),'0');assert.match(f.host.textContent,/Showing 1-100 of 206 covers/);assert.equal(field(f.host,'Saved cover').value,coverId(206));assert.equal(f.changed(),1);assert.equal(f.host.querySelector('input[type=file]'),null);
 }finally{await f.dispose();}});

for (const [label,inputText,expected] of [['NEL','\u0085Annotation\u0085','Annotation'],['BOM','\ufeffAnnotation\ufeff','\ufeffAnnotation\ufeff']]) test('mounted nullable-locale '+label+' metadata edit keeps intent and accepts normalized receipt',async()=>{
 const f=await ui(1,{nullLocales:[1]});try{
  input(field(f.host,'Saved cover'),coverId(1));input(field(f.host,'Description'),inputText);
  f.hook((request,rows)=>{if(request.init.method!=='PUT')return;const patch=JSON.parse(new TextDecoder().decode(request.body));
   assert.equal(Object.hasOwn(patch,'locale'),false);assert.equal(patch.description,expected);
   rows[0].attributes.description=expected;rows[0].attributes.version++;
   return json({result:'ok',response:'entity',data:rows[0]});});
  button(f.host,'Save cover metadata').click();await settle();
  assert.equal(f.writes().length,1);assert.equal(f.changed(),1);assert.equal(field(f.host,'Cover language').value,'');
  assert.equal(field(f.host,'Description').value,expected);assert.equal(f.rows()[0].attributes.locale,null);assert.match(f.host.textContent,/Cover version 2/);
  assert.equal(f.states.at(-1).pending,false);
 }finally{await f.dispose();}
});
