// Actual pinned Svelte client component/runes/lifecycle in JSDOM. Account,
// transport and catalogue reads are explicit stand-ins. This is not a browser,
// real signer, HTTP, PostgreSQL, layout or original-source acceptance test.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {createRequire} from 'node:module';
import {fileURLToPath,pathToFileURL} from 'node:url';
import test,{after} from 'node:test';
const root=path.resolve(path.dirname(fileURLToPath(import.meta.url)),'../..');
const dependencies=process.env.WEEBDEX_FRONTEND_ROOT,domModules=process.env.WEEBDEX_BROWSER_DOM_NODE_MODULES;
assert(dependencies&&domModules,'Supply the existing pinned frontend and JSDOM dependency roots');
const require=createRequire(path.join(dependencies,'package.json'));
assert.equal(require('svelte/package.json').version,'5.55.1');assert.equal(require('typescript/package.json').version,'5.9.3');
assert.equal(require(path.join(domModules,'jsdom/package.json')).version,'27.0.1');
const {compile}=require('svelte/compiler'),ts=require('typescript'),{JSDOM}=require(path.join(domModules,'jsdom'));
const {window}=new JSDOM('<main></main>',{pretendToBeVisual:true,url:'http://127.0.0.1:1'});
for(const key of ['window','document','Node','Element','HTMLElement','HTMLButtonElement','HTMLInputElement','HTMLTextAreaElement','HTMLSelectElement','HTMLMediaElement','HTMLFormElement','Text','Comment','Event','CustomEvent','MutationObserver','requestAnimationFrame','cancelAnimationFrame','getComputedStyle'])globalThis[key]=window[key];
const client=pathToFileURL(path.join(dependencies,'node_modules/svelte/src/index-client.js')).href;
const {mount,unmount,flushSync}=await import(client);
const sourceFile=path.join(root,'platform-integrations/public-api/PublicChapterEditor.svelte');
const original=fs.readFileSync(sourceFile,'utf8');
const source=original.replace(/<script lang="ts">([\s\S]*?)<\/script>/,(_match,script)=>{
 const ast=ts.createSourceFile('PublicChapterEditor.ts',script,ts.ScriptTarget.Latest,true,ts.ScriptKind.TS);
 for(const item of [...ast.statements].reverse()){
  if(!ts.isImportDeclaration(item))continue;
  const specifier=item.moduleSpecifier.text,clause=item.importClause;let replacement='';
  if(specifier==='svelte')replacement=item.getText(ast).replace(/(['"])svelte\1/,JSON.stringify(client));
  else if(clause&&!clause.isTypeOnly){
   const names=[];if(clause.name)names.push(clause.name.text);
   if(clause.namedBindings&&ts.isNamedImports(clause.namedBindings))for(const entry of clause.namedBindings.elements)if(!entry.isTypeOnly)names.push(entry.name.text);
   if(names.length)replacement=`const {${names.join(',')}}=globalThis.__chapterRemovalEnvironment;`;
  }
  script=script.slice(0,item.getStart(ast))+replacement+script.slice(item.end);
 }
 return '<script lang="ts">'+script+'</script>';
});
const generation=fs.mkdtempSync(path.join(root,'.build/chapter-removal-dom-'));
fs.symlinkSync(path.join(dependencies,'node_modules'),path.join(generation,'node_modules'));
const compiled=compile(source,{filename:sourceFile,generate:'client',runes:true});
assert.deepEqual(compiled.warnings,[]);
const moduleFile=path.join(generation,'PublicChapterEditor.mjs');fs.writeFileSync(moduleFile,compiled.js.code);
const Component=(await import(pathToFileURL(moduleFile))).default;
// Retain the tiny owned compiled generation as test evidence; do not delete files.
after(()=>{window.close();delete globalThis.__chapterRemovalEnvironment;});
const CHAPTER='11111111-1111-8111-8111-111111111111',MANGA='22222222-2222-8222-8222-222222222222',ACCOUNT='33333333-3333-8333-8333-333333333333';
const deferred=()=>{let resolve;const promise=new Promise(yes=>resolve=yes);return{promise,resolve};};
const queued=()=>({status:202,body:{result:'ok',requestId:'44444444-4444-8444-8444-444444444444',sourceDeletionStatus:'instance-local-only'}});
const hidden=()=>({status:204,body:undefined});
const refusal=()=>({status:403,body:{errors:[{detail:'Current moderator grant revoked'}]}});
async function settle(){await new Promise(resolve=>setTimeout(resolve,0));flushSync();}
function button(target,label){const rows=[...target.querySelectorAll('button')].filter(row=>row.textContent.trim()===label);assert.equal(rows.length,1,label);return rows[0];}
async function click(target,label){button(target,label).click();await settle();}
async function render({moderator=false,response,accountError}={}){
 const initial={id:CHAPTER,version:1,title:'Owned chapter',volume:'',chapter:'1',language:'en',relationships:{groups:[],manga:{id:MANGA}},data:[]};
 const user={id:ACCOUNT,name:'Owned account',roles:moderator?['moderator']:['user'],permissions:['self.chapter.update','self.chapter.remove',...(moderator?['chapter.remove']:[])]};
 const state={calls:[],reads:0,disposed:false,serverModerator:moderator};
 const owner={assert(){if(state.disposed)throw Error('Disposed chapter owner');},dispose(){state.disposed=true;},async request(route,method,payload){this.assert();state.calls.push({route,method,payload:structuredClone(payload)});return response?response(state):queued();}};
 globalThis.__chapterRemovalEnvironment={
  loadUploadAccount:async current=>{current();if(accountError)throw accountError;return user;},
  publicUploadOwner:(target,account,kind)=>{assert.equal(target(),CHAPTER);assert.equal(account.id,ACCOUNT);assert.equal(kind,'chapter');return owner;},
  normalizeChapter:value=>value,
  publicCatalogueRead:async(operation,input)=>{assert.equal(operation,'chapter.get');assert.equal(input.id,CHAPTER);state.reads++;return{status:200,body:structuredClone(initial)};},
  record:value=>{assert(value&&typeof value==='object');return value;},resource:value=>value,
 };
 const target=document.createElement('div');document.querySelector('main').append(target);
 const component=mount(Component,{target,props:{initial}});flushSync();await settle();
 return{target,state,async close(){await unmount(component);target.remove();}};
}
test('actual ordinary dialog Cancel closes without any command or catalogue read',async()=>{
 const f=await render();try{await click(f.target,'Request chapter removal');assert.match(f.target.querySelector('[role=alertdialog]').textContent,/remains visible until approval/);await click(f.target,'Cancel');assert.equal(f.target.querySelector('[role=alertdialog]'),null);assert.equal(f.state.calls.length,0);assert.equal(f.state.reads,0);}finally{await f.close();}
});
test('actual ordinary confirmation submits captured review after same-account server grant gain and blocks duplicate clicks',async()=>{
 const gate=deferred(),f=await render({response:()=>gate.promise});try{
  await click(f.target,'Request chapter removal');const confirm=button(f.target,'Submit removal request');f.state.serverModerator=true;confirm.click();confirm.click();await settle();assert.equal(f.state.calls.length,1);assert.deepEqual(f.state.calls[0],{route:'/chapter/'+CHAPTER,method:'DELETE',payload:{removalMode:'request-review'}});assert.equal(f.target.querySelector('[role=alertdialog]'),null);
  gate.resolve(queued());await settle();assert.match(f.target.textContent,/awaiting instance moderator review/);assert.match(f.target.textContent,/remains visible until approval/);assert.doesNotMatch(f.target.textContent,/approved hiding/);
 }finally{await f.close();}
});
test('actual moderator confirmation submits explicit approval and confirms only204 hide',async()=>{
 const f=await render({moderator:true,response:hidden});try{await click(f.target,'Approve instance hide');assert.match(f.target.querySelector('[role=alertdialog]').textContent,/immediately/);await click(f.target,'Approve and hide on this instance');assert.equal(f.state.calls[0].payload.removalMode,'approve-hide');assert.match(f.target.textContent,/approved hiding this chapter/);assert.equal(button(f.target,'Approve instance hide').disabled,true);}finally{await f.close();}
});
test('actual mismatched removal effects stay unconfirmed and reload never replays them',async()=>{
 for(const moderator of [false,true]){const f=await render({moderator,response:moderator?queued:hidden});try{
  await click(f.target,moderator?'Approve instance hide':'Request chapter removal');await click(f.target,moderator?'Approve and hide on this instance':'Submit removal request');assert.match(f.target.textContent,/removal outcome is unconfirmed/);assert.doesNotMatch(f.target.textContent,/is awaiting instance moderator review|A moderator approved hiding/);assert.equal(f.state.calls.length,1);assert.equal(button(f.target,moderator?'Approve instance hide':'Request chapter removal').disabled,true);
  await click(f.target,'Reload current chapter');assert.equal(f.state.reads,1);assert.equal(f.state.calls.length,1);assert.match(f.target.textContent,/No mutation was replayed/);
 }finally{await f.close();}}
});
test('actual revoked approval exposes its refusal without silently falling back to review',async()=>{
 const f=await render({moderator:true,response:refusal});try{await click(f.target,'Approve instance hide');f.state.serverModerator=false;await click(f.target,'Approve and hide on this instance');assert.equal(f.state.calls.length,1);assert.equal(f.state.calls[0].payload.removalMode,'approve-hide');assert.match(f.target.textContent,/Current moderator grant revoked/);assert.doesNotMatch(f.target.textContent,/is awaiting instance moderator review|approved hiding/);await click(f.target,'Reload current chapter');assert.equal(f.state.calls.length,1);}finally{await f.close();}
});
test('actual unmount disposes an in-flight chapter owner and cannot update a newer mount',async()=>{
 const gate=deferred(),old=await render({response:()=>gate.promise});await click(old.target,'Request chapter removal');await click(old.target,'Submit removal request');assert.equal(old.state.calls.length,1);await old.close();assert.equal(old.state.disposed,true);
 const next=await render();try{gate.resolve(hidden());await settle();assert.equal(next.state.calls.length,0);assert.doesNotMatch(next.target.textContent,/approved hiding|unconfirmed|failed/);assert.equal(button(next.target,'Request chapter removal').disabled,false);}finally{await next.close();}
});
