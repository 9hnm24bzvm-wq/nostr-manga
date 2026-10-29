// Finite mounted component test. Existing locked Svelte/JSDOM only; no browser,
// socket, app server, production build, native API or real user data.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { createRequire, stripTypeScriptTypes } from 'node:module';
import { fileURLToPath, pathToFileURL } from 'node:url';
import test, { after } from 'node:test';
import { createLocalLibraryOverlays } from '../../platform-integrations/local-library/overlays.mjs';
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const { libraryDiscovery, unfilteredLibraryHref } = await import('data:text/javascript;base64,' + Buffer.from(stripTypeScriptTypes(fs.readFileSync(path.join(root, 'platform-integrations/local-library/library-discovery.ts'), 'utf8'), { mode: 'transform' })).toString('base64'));
const dependencies = process.env.WEEBDEX_FRONTEND_ROOT, domRoot = process.env.WEEBDEX_BROWSER_DOM_NODE_MODULES;
assert(dependencies && domRoot, 'Use existing exact-lock dependency roots');
const require = createRequire(path.join(dependencies, 'package.json')), { compile } = require('svelte/compiler'), ts = require('typescript');
assert.equal(require('svelte/package.json').version, '5.55.1'); assert.equal(require(path.join(domRoot, 'jsdom/package.json')).version, '27.0.1');
const { JSDOM } = require(path.join(domRoot, 'jsdom'));
const { window } = new JSDOM('<main></main>', { url: 'https://title.fixture/library', pretendToBeVisual: true });
for (const key of ['window', 'document', 'Node', 'Element', 'HTMLElement', 'HTMLInputElement', 'HTMLTextAreaElement', 'HTMLSelectElement', 'HTMLMediaElement', 'HTMLFormElement', 'Text', 'Comment', 'Event', 'CustomEvent', 'MutationObserver', 'requestAnimationFrame', 'cancelAnimationFrame', 'getComputedStyle']) globalThis[key] = window[key];
const client = pathToFileURL(path.join(dependencies, 'node_modules/svelte/src/index-client.js')).href;
const { mount, unmount, flushSync } = await import(client), { writable } = await import(pathToFileURL(path.join(dependencies, 'node_modules/svelte/src/store/index-client.js')));
const temp = fs.mkdtempSync(path.join(os.tmpdir(), 'device-title-dom-')); fs.symlinkSync(path.join(dependencies, 'node_modules'), path.join(temp, 'node_modules'));
after(() => { window.close(); fs.rmSync(temp, { recursive: true, force: true }); delete globalThis.__titleFixture; });
async function compiled(name, source, replace = false) {
  if (replace) source = source.replace(/<script lang="ts">([\s\S]*?)<\/script>/, (_, script) => {
    const ast = ts.createSourceFile(name + '.ts', script, ts.ScriptTarget.Latest, true, ts.ScriptKind.TS);
    for (const item of [...ast.statements].reverse()) if (ts.isImportDeclaration(item)) {
      const clause = item.importClause; let replacement = '';
      if (item.moduleSpecifier.text === 'svelte') replacement = item.getText(ast).replace(/(['"])svelte\1/, JSON.stringify(client));
      else if (clause && !clause.isTypeOnly) {
        const names = []; if (clause.name) names.push(clause.name.text);
        if (clause.namedBindings && ts.isNamespaceImport(clause.namedBindings)) names.push(clause.namedBindings.name.text);
        else if (clause.namedBindings) for (const entry of clause.namedBindings.elements) if (!entry.isTypeOnly) names.push(entry.name.text);
        if (names.length) replacement = `const { ${names.join(', ')} } = globalThis.__titleFixture;`;
      }
      script = script.slice(0, item.getStart(ast)) + replacement + script.slice(item.end);
    }
    return '<script lang="ts">' + script + '</script>';
  });
  const result = compile(source, { filename: name + '.svelte', generate: 'client', runes: true });
  fs.writeFileSync(path.join(temp, name + '.mjs'), result.js.code); return (await import(pathToFileURL(path.join(temp, name + '.mjs')))).default;
}
const Generic = await compiled('Generic', '<script>let {children}=$props();</script><div>{@render children?.()}</div>');
const Button = await compiled('Button', '<script>let {children,variant,...rest}=$props();</script><button {...rest}>{@render children?.()}</button>');
const Entry = await compiled('Entry', '<script>let {manga}=$props();</script><article>{manga.title}</article>');
const Feed = await compiled('DeviceTitleLibrary', fs.readFileSync(path.join(root, 'platform-integrations/local-library/DeviceTitleLibrary.svelte'), 'utf8'), true);
const deferred = () => { let resolve, reject; const promise = new Promise((yes,no) => { resolve=yes;reject=no; }); return { promise,resolve,reject }; };
const data = id => ({ id, title: 'Title-'+id });
const result = (extra = {}) => ({ page:1,limit:20,data:[],total:null,scanned:100,candidates:301,indexed:false,ready:false,started:'2026-10-22T00:00:00Z', ...extra });
const settle = async () => { for(let i=0;i<8;i++){await Promise.resolve();flushSync();} };
async function fixture(load, {opening, disposal, query = '', records = []} = {}) {
  const prefs=writable({items_per_page:'42',version:0}), views=writable('device:0:verified'), identity=writable({accountId:null}), authority=writable({epoch:0}), mangaView=writable('grid');
  const calls=[], opened=[], releases=[];let disposed=0,cancelled=0,navigation=0;
  globalThis.__titleFixture={libraryDiscovery,unfilteredLibraryHref,localLibraryState:writable({records,ready:true,mode:'device'}),isCurrentCatalogueId:id=>typeof id==='string',prefs,mangaView,libraryViewKey:views,libraryIdentity:identity,nativeAuthority:authority,captureNavigationEpoch:()=>navigation,page:{url:new URL('https://title.fixture/library'+query)},Constants:{MangaGridBreakpoints:{}},Grid:Generic,Button,Spinner:Generic,MangaSearchEntryCard:Entry,message:error=>error.message,TitleCancelled:class extends Error {constructor(){super('cancelled');}},TitleIndexStopped:class extends Error{},HistoryTransportUnsettled:class extends Error{},async openDeviceTitleFeed(query,assertOwner){opened.push({query,assertOwner});if(opening)await opening.promise;return{async load(page){calls.push(page);return load(page,opened.length);},async dispose(){disposed++;if(disposal)await disposal.promise;},releaseDisplayedPage(){releases.push(document.body.textContent);},cancel(){cancelled++;}};}};
  const component=mount(Feed,{target:document.querySelector('main')});await settle();
  return{prefs,views,identity,authority,calls,opened,releases,disposed:()=>disposed,cancelled:()=>cancelled,navigate(){navigation++;},async close(){await unmount(component);await settle();},async click(text){const button=[...document.querySelectorAll('button')].find(row=>row.textContent===text);assert(button,`Missing button ${text}: ${document.body.textContent}`);button.click();await settle();}};
}
test('index progress never looks like an empty or globally sorted early result; completed results appear automatically',async()=>{
 let count=0;const f=await fixture(async()=>++count===1?result():count===2?result({scanned:301,total:2,indexed:true}):result({scanned:301,total:2,indexed:true,ready:true,data:[data('last-id-first')]}));
 try{assert.match(document.body.textContent,/Checked 100 of 301 saved titles/);assert.doesNotMatch(document.body.textContent,/No saved titles matched/);await f.click('Continue loading');assert.match(document.body.textContent,/2 matching titles/);assert.match(document.body.textContent,/Title-last-id-first/);assert.match(document.body.textContent,/Refresh to check for changes/);assert.deepEqual(f.calls,[1,1,1]);}finally{await f.close();}
});
test('next-page failure removes old DTO page and retry keeps intended page',async()=>{
 let count=0;const f=await fixture(async n=>{if(++count===2)throw Error('503 batch not skipped');return result({page:n,indexed:true,ready:true,total:100,data:[data(String(n))]});});
 try{await f.click('Next page');assert.ok(f.releases.length);assert(f.releases.every(text=>!text.includes('Title-1')));assert.doesNotMatch(document.body.textContent,/Title-1/);assert.match(document.body.textContent,/503 batch not skipped/);await f.click('Retry titles');assert.match(document.body.textContent,/Title-2/);assert.deepEqual(f.calls,[1,2,2]);}finally{await f.close();}
});
for(const changed of ['prefs','views','identity','authority'])test(`${changed} invalidates old owner and cannot admit delayed rows`,async()=>{
 const pending=deferred(),f=await fixture(async(n,open)=>open===1?pending.promise:result({indexed:true,ready:true,total:1,data:[data('new')]}));
 try{f[changed].set(changed==='prefs'?{items_per_page:'20',version:1}:changed==='views'?'device:1:verified':changed==='identity'?{accountId:'b'}:{epoch:2});await settle();pending.resolve(result({indexed:true,ready:true,total:1,data:[data('old')]}));await settle();assert.match(document.body.textContent,/Title-new/);assert.doesNotMatch(document.body.textContent,/Title-old/);assert.equal(f.disposed(),1);assert.throws(f.opened[0].assertOwner,/cancelled/);}finally{await f.close();}
});
test('navigation start invalidates pending success before URL commit',async()=>{
 const pending=deferred(),f=await fixture(()=>pending.promise);try{f.navigate();pending.resolve(result({indexed:true,ready:true,total:1,data:[data('old')]}));await settle();assert.doesNotMatch(document.body.textContent,/Title-old/);}finally{await f.close();}
});
test('cancel during opening disposes late index, then explicit retry is possible',async()=>{
 const opening=deferred(),f=await fixture(async()=>result(),{opening});try{await f.click('Cancel loading');assert.match(document.body.textContent,/cancelled/);opening.resolve();await settle();assert.equal(f.disposed(),1);assert.deepEqual(f.calls,[]);await f.click('Retry titles');assert.deepEqual(f.calls,[1]);}finally{await f.close();}
});
test('unmount ignores late reads; no duplicate-click continuation is admitted',async()=>{
 const pending=deferred(),f=await fixture(()=>pending.promise);assert.deepEqual(f.calls,[1]);await f.close();assert.equal(f.disposed(),1);pending.resolve(result({indexed:true,ready:true,total:1,data:[data('late')]}));await settle();assert.doesNotMatch(document.body.textContent,/Title-late/);
});
for(const name of ['TitleIndexStopped','HistoryTransportUnsettled'])test(`${name} visibly stops until explicit rebuild`,async()=>{
 const f=await fixture(async()=>{throw new globalThis.__titleFixture[name]('Storage or transport requires recovery');});try{assert.match(document.body.textContent,/Storage or transport requires recovery/);assert.match(document.body.textContent,/Refresh library/);assert.doesNotMatch(document.body.textContent,/Retry titles|Continue loading/);await settle();assert.deepEqual(f.calls,[1]);await f.click('Refresh library');assert.deepEqual(f.calls,[1,1]);}finally{await f.close();}
});
test('custom list/URL filters and items-per-page retained without injecting unrelated discovery defaults',async()=>{
 const f=await fixture(async()=>result(),{query:'?list='+ 'a'.repeat(32)+'&reading_status=reading&contentRating=safe&sort=title'});try{const q=f.opened[0].query;assert.equal(q.get('list'),'a'.repeat(32));assert.equal(q.get('sort'),'title');assert.equal(q.get('limit'),'42');assert.deepEqual(q.getAll('contentRating'),['safe']);}finally{await f.close();}
});
test('exact original route remains account-capable; only two device title markup exceptions',async()=>{
 const {localLibraryApprovedTitleContinuationExceptions}=await import('../../platform-integrations/local-library/overlays.mjs');
 const frontend=path.join(root,'frontend'),file=path.join(frontend,'src/routes/(app)/library/+page.svelte'),original=fs.readFileSync(file,'utf8');let composed=createLocalLibraryOverlays(frontend).transform.handler(original,file).code;
 assert.match(composed,/\{#if \$localLibraryState.mode === "device"\}[\s\S]*<DeviceTitleLibrary \/>[\s\S]*\{:else\}[\s\S]*<MangaSearch/);assert.equal(composed.match(/API.MangaFollow.Search\(q\)/g).length,1);assert.equal(localLibraryApprovedTitleContinuationExceptions.length,2);compile(composed,{filename:file,generate:'client'});
 for(const entry of [...localLibraryApprovedTitleContinuationExceptions].reverse()){assert.equal(entry.path,'src/routes/(app)/library/+page.svelte');assert.equal(entry.count,1);assert.equal(composed.split(entry.after).length-1,1);composed=composed.replace(entry.after,entry.before);}
 const markup=s=>s.replace(/<script\b[^>]*>[\s\S]*?<\/script>/g,'<script></script>').replaceAll('{#key page.url.search + ":" + $libraryViewKey}','{#key page.url.search}');assert.equal(markup(composed),markup(original));assert.equal(fs.readFileSync(file,'utf8'),original);
});

test('followedAt unknown-total page is usable without forcing complete indexing',async()=>{const f=await fixture(async n=>result({page:n,localOrder:true,ready:true,data:[data(String(n))]}));try{assert.match(document.body.textContent,/Title-1/);assert.match(document.body.textContent,/Checked 100 of 301 saved titles/);assert.match(document.body.textContent,/Refresh to check for changes/);assert.doesNotMatch(document.body.textContent,/Continue loading/);await f.click('Next page');assert.match(document.body.textContent,/Title-2/);}finally{await f.close();}});
test('rebuild waits for old index disposal and visibly stops on unconfirmed cleanup',async()=>{const disposal=deferred(),f=await fixture(async()=>result(),{disposal});try{await f.click('Refresh library');assert.equal(f.opened.length,1);disposal.reject(new globalThis.__titleFixture.TitleIndexStopped('Temporary title storage cleanup is unconfirmed'));await settle();assert.match(document.body.textContent,/cleanup is unconfirmed/);assert.equal(f.opened.length,1);}finally{await f.close();}});


test('virgin library offers discovery rather than claiming an intentional filter mismatch', async () => {
 const f=await fixture(async()=>result({total:0,candidates:0,scanned:0,indexed:true,ready:true}));
 try { assert.match(document.body.textContent,/Your library is empty/); assert.match(document.body.textContent,/Add to Library/); assert.equal(document.querySelector('a').getAttribute('href'),'/search'); assert.doesNotMatch(document.body.textContent,/match these filters/); } finally { await f.close(); }
});
test('filtered empty library preserves saved titles and selected list on Clear filters', async () => {
 const list='a'.repeat(32), records=[{family:'series',id:'title'},{family:'lists',id:list,name:'Weekend reads'},{family:'members',list,series:'title'}];
 const f=await fixture(async()=>result({total:0,candidates:0,scanned:0,indexed:true,ready:true}),{query:'?list='+list+'&reading_status=completed',records});
 try { assert.match(document.body.textContent,/No saved titles match these filters/); assert.match(document.body.textContent,/have not been removed/); assert.equal(document.querySelector('a').getAttribute('href'),'/library?list='+list); assert.doesNotMatch(document.body.textContent,/Your library is empty/); } finally { await f.close(); }
});
test('empty selected list uses its actual name and opens the rest of the library', async () => {
 const list='b'.repeat(32), records=[{family:'series',id:'title'},{family:'lists',id:list,name:'Weekend & <reads>'}];
 const f=await fixture(async()=>result({total:0,candidates:0,scanned:0,indexed:true,ready:true}),{query:'?list='+list,records});
 try { assert.match(document.body.textContent,/No saved titles in “Weekend & <reads>” yet/); assert.equal(document.querySelector('a').getAttribute('href'),'/library'); assert.equal(document.querySelector('reads'),null); assert.doesNotMatch(document.body.textContent,/match these filters|Your library is empty/); } finally { await f.close(); }
});
test('empty-state discovery is not shown for an unfinished or failed library scan', async () => {
 const pending=deferred(), f=await fixture(()=>pending.promise);
 try { assert.equal(document.querySelector('a[href="/search"]'),null); pending.reject(Error('Unavailable')); await settle(); assert.equal(document.querySelector('a[href="/search"]'),null); assert.match(document.body.textContent,/Unavailable/); } finally { await f.close(); }
});
