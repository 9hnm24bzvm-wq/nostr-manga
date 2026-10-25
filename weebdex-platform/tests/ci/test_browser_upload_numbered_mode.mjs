#!/usr/bin/env node
// Runs actual composed restoreUpload + real checked/stage callbacks. Native DOM
// inputs and checkbox button model retained bindings; Bits UI/layout, images,
// network and browser acceptance are outside this seam.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import vm from 'node:vm';
import {createRequire} from 'node:module';
import {fileURLToPath} from 'node:url';
import test from 'node:test';
import {createFrontendOverlays} from '../../platform-integrations/frontend-overlays.mjs';
const root=path.resolve(path.dirname(fileURLToPath(import.meta.url)),'../..'), dependency=process.env.WEEBDEX_FRONTEND_ROOT;
assert(dependency,'Use the verified exact-lock dependency root');
const require=createRequire(path.join(dependency,'package.json')),ts=require('typescript');
const {JSDOM}=require(path.join(process.env.WEEBDEX_BROWSER_DOM_NODE_MODULES,'jsdom'));
const journey=fs.readFileSync(path.join(root,'scripts/browser-journeys.mjs'),'utf8');
const parse=(text,kind=ts.ScriptKind.JS)=>ts.createSourceFile('source',text,ts.ScriptTarget.Latest,true,kind);
function find(ast,predicate){let match;function walk(node){if(predicate(node))match=node;ts.forEachChild(node,walk);}walk(ast);assert(match);return match;}
const journeyAST=parse(journey),checked=find(journeyAST,node=>ts.isVariableDeclaration(node)&&node.name.getText(journeyAST)==='checked').initializer.getText(journeyAST);
const operation=id=>find(journeyAST,node=>ts.isCallExpression(node)&&node.expression.getText(journeyAST)==='uploadCheck'&&node.arguments[0]?.text===id).arguments[1].getText(journeyAST);
const file=path.join(root,'frontend/src/lib/components/upload/UploadChapter.svelte'),composed=createFrontendOverlays(path.join(root,'frontend')).transform.handler(fs.readFileSync(file,'utf8'),file).code;
const instance=composed.match(/<script lang="ts">([\s\S]*?)<\/script>/)[1],parentAST=parse(instance,ts.ScriptKind.TS);
const restore=find(parentAST,node=>ts.isVariableDeclaration(node)&&node.name.getText(parentAST)==='restoreUpload').initializer.getText(parentAST);
const metadata=fs.readFileSync(path.join(root,'frontend/src/lib/components/upload/UploadChapterMetadata.svelte'),'utf8');
assert.equal((metadata.match(/disabled=\{disabled \|\| isOneshot\}/g)??[]).length,2);
assert.match(metadata,/id="oneshot"\s+bind:checked=\{isOneshot\}[\s\S]*?draft.volume = "";\s+draft.chapter = "";/);
const emit=text=>ts.transpileModule(text,{compilerOptions:{target:ts.ScriptTarget.ES2022,module:ts.ModuleKind.None}}).outputText;
function fixture(draft={},blocked=false){
 const {window}=new JSDOM('<button id="oneshot" role="checkbox"></button><input placeholder="Chapter Name"><input placeholder="Volume No."><input placeholder="Chapter No.">');
 const button=window.document.querySelector('button'),vol=window.document.querySelector('[placeholder="Volume No."]'),chapter=window.document.querySelector('[placeholder="Chapter No."]');let clicks=0;
 const context=vm.createContext({structuredClone,toast:{info(){}},resumeNotice:()=>'',owner:{assert(){}},prepareResume:value=>value,snapshot:()=>({}),signature:()=>'',HTMLInputElement:window.HTMLInputElement,marker:'Synthetic',draft:undefined,isOneshot:false});
 const reflect=()=>{button.setAttribute('aria-checked',String(context.isOneshot));button.disabled=blocked;vol.disabled=chapter.disabled=blocked||context.isOneshot;};
 button.addEventListener('click',()=>{context.isOneshot=!context.isOneshot;context.draft.volume='';context.draft.chapter='';reflect();});
 const locator=node=>({async waitFor({state}){assert.equal(state,'visible');assert(node.isConnected);},async evaluate(callback){return vm.runInContext('('+callback.toString()+')',context)(node);},async click(){assert(!node.disabled,'Native checkbox is disabled');clicks++;node.click();},async fill(value){assert(!node.disabled,'Native numbered input is disabled');node.value=value;}});
 context.page={locator(selector){assert.equal(selector,'#oneshot');return locator(button);},getByPlaceholder(name,{exact}){assert.equal(exact,true);return locator(window.document.querySelector('[placeholder="'+name+'"]'));}};
 vm.runInContext('globalThis.restore='+emit('('+restore+')').trim().replace(/;$/,''),context);
 context.restore({id:'OwnedStage',draft,relationships:{files:[]}});reflect();vm.runInContext('globalThis.checked='+checked,context);
 return{context,button,vol,chapter,get clicks(){return clicks;},run:id=>vm.runInContext('('+operation(id)+')()',context)};
}
test('empty restored staging really selects oneshot and the old direct numbered fill is rejected',async()=>{const value=fixture();assert.equal(value.context.isOneshot,true);await assert.rejects(value.run('stage-volume'),/Native numbered input is disabled/);});
for(const draft of [{},{volume:'1',chapter:'7'}])test('stage intent chooses numbered mode before filling volume/chapter from '+JSON.stringify(draft),async()=>{const value=fixture(draft);await value.run('stage-metadata');await value.run('stage-volume');await value.run('stage-chapter');assert.equal(value.context.isOneshot,false);assert.equal(value.vol.value,'1');assert.equal(value.chapter.value,'7');assert.equal(value.clicks,draft.chapter?0:1);});
test('disabled checkbox fails closed and never forces numbered fields enabled',async()=>{const value=fixture({},true);await assert.rejects(value.run('stage-metadata'),/Native checkbox is disabled/);assert.equal(value.context.isOneshot,true);assert(value.vol.disabled&&value.chapter.disabled);assert.equal(value.clicks,0);});
