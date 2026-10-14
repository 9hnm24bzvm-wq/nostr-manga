#!/usr/bin/env node
// Actual Svelte client proxy/rendering + composed createPage/uploadChunk and real
// upload-batch.ts. Owner and HTTP services are explicit finite stand-ins. JSDOM
// supplies DOM only, not layout/image/network/browser acceptance.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {createRequire} from 'node:module';
import {fileURLToPath,pathToFileURL} from 'node:url';
import test,{after} from 'node:test';
import {createFrontendOverlays} from '../../platform-integrations/frontend-overlays.mjs';
const root=path.resolve(path.dirname(fileURLToPath(import.meta.url)),'../..'), frontend=path.join(root,'frontend');
const dependencyRoot=process.env.WEEBDEX_FRONTEND_ROOT;
assert(dependencyRoot,'Use the existing verified exact-lock dependency root');
const require=createRequire(path.join(dependencyRoot,'package.json')), {compile}=require('svelte/compiler'), ts=require('typescript');
assert.equal(require('svelte/package.json').version,'5.55.1');
assert.equal(require('typescript/package.json').version,'5.9.3');
assert.match(fs.readFileSync(path.join(frontend,'yarn.lock'),'utf8'),/svelte@\^5\.45\.6:\s+version "5\.55\.1"/);
assert(process.env.WEEBDEX_BROWSER_DOM_NODE_MODULES,'Use the existing JSDOM dependency root');
assert.equal(require(path.join(process.env.WEEBDEX_BROWSER_DOM_NODE_MODULES,'jsdom/package.json')).version,'27.0.1');
const {JSDOM}=require(path.join(process.env.WEEBDEX_BROWSER_DOM_NODE_MODULES,'jsdom'));
const {window}=new JSDOM('<main></main>',{pretendToBeVisual:true,url:'http://127.0.0.1:1'});
for(const key of ['window','document','Node','Element','HTMLElement','HTMLInputElement','Text','Comment','Event','CustomEvent','MutationObserver','requestAnimationFrame','cancelAnimationFrame','getComputedStyle']) globalThis[key]=window[key];
const {mount,unmount,flushSync}=await import(pathToFileURL(path.join(dependencyRoot,'node_modules/svelte/src/index-client.js')));
const file=path.join(frontend,'src/lib/components/upload/UploadChapterPages.svelte');
const composed=createFrontendOverlays(frontend).transform.handler(fs.readFileSync(file,'utf8'),file).code;
const instance=composed.match(/<script lang="ts">([\s\S]*?)<\/script>/)[1];
const ast=ts.createSourceFile('upload.ts',instance,ts.ScriptTarget.Latest,true,ts.ScriptKind.TS);
const extract=names=>ast.statements.filter(row=>ts.isVariableStatement(row)&&row.declarationList.declarations.some(item=>names.includes(item.name.getText(ast)))).map(row=>row.getText(ast)).join('\n');
const factory=extract(['createPage']), chunks=extract(['uploadChunk','assertFileSession']);
const originalFactory=factory.replace(/\s*\/\/[^\n]*/g,'').replace('const candidate = $state(<UploadSessionFile>{','return <UploadSessionFile>{').replace(/\}\);\s*return candidate;/,'};');
assert.notEqual(factory,originalFactory);
const batch=fs.readFileSync(path.join(root,'platform-integrations/frontend-behavior/upload-batch.ts'),'utf8').replace(/^import[^\n]*\n/gm,'').replace(/^export const \[getUploadOwner[^\n]*\n/m,'').replace(/\bexport /g,'');
const batchJS=ts.transpileModule(batch,{compilerOptions:{target:ts.ScriptTarget.ES2022,module:ts.ModuleKind.None}}).outputText;
const entry=fs.readFileSync(path.join(frontend,'src/lib/components/upload/UploadChapterPagesEntry.svelte'),'utf8');
assert.match(entry,/\{#if !isDragging && \(!page.file \|\| page.error\)\}/);
fs.mkdirSync(path.join(root,'.build'),{recursive:true});
const generation=fs.mkdtempSync(path.join(root,'.build/reactive-candidate-'));fs.symlinkSync(path.join(dependencyRoot,'node_modules'),path.join(generation,'node_modules'));
after(()=>{window.close();fs.rmSync(generation,{recursive:true,force:true});});
async function componentFor(legacy) {
 const code=`<script lang="ts">let {environment}=$props();const {owner,HTTP,currentUpload,VisibleError}=environment;const PUBLIC_API_URL="/api",PUBLIC_UPLOAD_URL="/uploads";let uploadSession=$state({id:"OwnedStage",can_mutate:true});let pages=$state<UploadSessionFile[]>([]);let queue:UploadSessionFile[]=[];const Constants={Error:{Error:VisibleError}};${batchJS}\n${legacy?originalFactory:factory}\n${chunks}\nexport function seed(file:File){const newFiles=[createPage(file)];pages.push(...newFiles);queue=newFiles;}export async function submit(){try{await uploadChunk(queue);}catch(error){environment.failure(error);}}export function sameRow(){return pages[0]===queue[0];}export function snapshot(){return pages.map(row=>({id:row.id,error:row.error,recovery:row.upload_recovery?.state,file:!!row.file,thumbnail:row.thumbnail}));}</script>{#each pages as page}<h3>{page.original_file_name}</h3>{#if !page.file || page.error}<button>Replace</button><button>Remove</button>{/if}<span>{page.error??'pending'}</span>{/each}`;
 const target=path.join(generation,legacy?'Legacy.mjs':'Fixed.mjs');fs.writeFileSync(target,compile(code,{filename:legacy?'Legacy.svelte':'Fixed.svelte',generate:'client'}).js.code);return(await import(pathToFileURL(target))).default;
}
class VisibleError extends Error{constructor(title,body){super(title);this.title=title;this.body=body;}}
function environment(kind) {
 let release,started;const gate=new Promise(resolve=>{release=resolve;}),sent=new Promise(resolve=>{started=resolve;});const counts={posts:0,reads:0,failures:[]};
 const owner={actor:'SyntheticOwner',assert(){},alive:()=>true};
 return{counts,release,sent,owner,VisibleError,failure:error=>counts.failures.push(error),currentUpload:async()=>{counts.reads++;return{id:'OwnedStage',can_mutate:true,relationships:{files:[]}};},HTTP:class{constructor(_url,form){this.form=form;}setWithCredentials(){}setRequestHeader(){}addUploadListener(){}async POST(){counts.posts++;started();await gate;if(kind==='transport')throw Error('synthetic disconnected mutation');if(kind==='item')return{status:200,body:[{error:'synthetic item rejected'}]};return{status:500,body:{error:'synthetic rejection'}};}}};
}
async function run(legacy,kind){const env=environment(kind),target=document.createElement('div');document.querySelector('main').append(target);const Component=await componentFor(legacy),component=mount(Component,{target,props:{environment:env}});flushSync();component.seed(new File(['synthetic bytes'],'failed.png',{type:'image/png'}));flushSync();assert.equal(target.querySelectorAll('button').length,0);assert.equal(component.sameRow(),!legacy,'Only the reactive factory shares exact row identity');const pending=component.submit();await Promise.race([env.sent,pending.then(()=>{throw Error('Upload completed without reaching the owned POST');})]);env.release();await pending;flushSync();const result={buttons:target.querySelectorAll('button').length,text:target.textContent,rows:component.snapshot(),counts:env.counts};for(const row of result.rows)URL.revokeObjectURL(row.thumbnail);await unmount(component);target.remove();return result;}
test('real runtime reproduces the former raw-candidate failure after an awaited batch response',{timeout:5000},async()=>{const legacy=await run(true,'http');assert.equal(legacy.buttons,0);assert.match(legacy.text,/pending/);assert.equal(legacy.rows[0].error,undefined);assert.equal(legacy.counts.posts,1);});
for(const kind of ['http','transport','item'])test('reactive candidate renders truthful per-item recovery controls after '+kind,{timeout:5000},async()=>{const result=await run(false,kind);assert.equal(result.buttons,2);assert.equal(result.counts.posts,1);assert.equal(result.rows[0].recovery,kind==='item'?'rejected':'unknown');assert.equal(result.rows[0].file,true);assert.match(result.text,kind==='item'?/synthetic item rejected/:/Outcome unknown/);assert.equal(result.counts.failures.length,kind==='item'?0:1);});
