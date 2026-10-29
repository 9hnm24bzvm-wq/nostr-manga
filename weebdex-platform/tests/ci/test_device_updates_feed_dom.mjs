// Finite mounted component test. Existing locked Svelte/JSDOM only; no browser,
// socket, app server, production build, native API or real user data.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { createRequire, stripTypeScriptTypes } from 'node:module';
import vm from 'node:vm';
import { webcrypto } from 'node:crypto';
import { fileURLToPath, pathToFileURL } from 'node:url';
import test, { after } from 'node:test';
import { createPublicApiOverlays } from '../../platform-integrations/public-api/overlays.mjs';
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const { libraryDiscovery } = await import('data:text/javascript;base64,' + Buffer.from(stripTypeScriptTypes(fs.readFileSync(path.join(root, 'platform-integrations/local-library/library-discovery.ts'), 'utf8'), { mode: 'transform' })).toString('base64'));
const dependencies = process.env.WEEBDEX_FRONTEND_ROOT, domRoot = process.env.WEEBDEX_BROWSER_DOM_NODE_MODULES;
assert(dependencies && domRoot, 'Use existing exact-lock dependency roots');
const require = createRequire(path.join(dependencies, 'package.json')), { compile } = require('svelte/compiler'), ts = require('typescript');
assert.equal(require('svelte/package.json').version, '5.55.1'); assert.equal(require(path.join(domRoot, 'jsdom/package.json')).version, '27.0.1');
const { JSDOM } = require(path.join(domRoot, 'jsdom'));
const { window } = new JSDOM('<main></main>', { url: 'https://title.fixture/library', pretendToBeVisual: true });
for (const key of ['window', 'document', 'Node', 'Element', 'HTMLElement', 'HTMLInputElement', 'HTMLTextAreaElement', 'HTMLSelectElement', 'HTMLMediaElement', 'HTMLFormElement', 'Text', 'Comment', 'Event', 'CustomEvent', 'MutationObserver', 'requestAnimationFrame', 'cancelAnimationFrame', 'getComputedStyle']) globalThis[key] = window[key];
const client = pathToFileURL(path.join(dependencies, 'node_modules/svelte/src/index-client.js')).href;
const { mount, unmount, flushSync } = await import(client), { writable } = await import(pathToFileURL(path.join(dependencies, 'node_modules/svelte/src/store/index-client.js')));
const temp = fs.mkdtempSync(path.join(os.tmpdir(), 'device-updates-dom-')); fs.symlinkSync(path.join(dependencies, 'node_modules'), path.join(temp, 'node_modules'));
after(() => { window.close(); fs.rmSync(temp, { recursive: true, force: true }); delete globalThis.__updatesFixture; });
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
        if (names.length) replacement = `const { ${names.join(', ')} } = globalThis.__updatesFixture;`;
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
// Use the actual latest-entry renderer and actual public overlay, not a stub:
// normalized public chapters legitimately have no native forum relationship.
String.prototype.ago = function () { return 'just now'; };
const entryFile = path.join(dependencies, 'src/lib/components/LatestUpdatesGridEntry.svelte');
const entrySource = fs.readFileSync(entryFile, 'utf8');
const publicOverlay = createPublicApiOverlays(dependencies, {enabled:true});
const ActualLatestEntry = await compiled('ActualLatestEntry', process.env.WEEBDEX_UPDATES_RENDERER_BASELINE === '1' ? entrySource : publicOverlay.transform.handler(entrySource, entryFile).code, true);
const Feed = await compiled('DeviceUpdatesFeed', fs.readFileSync(path.join(root, 'platform-integrations/local-library/DeviceUpdatesFeed.svelte'), 'utf8'), true);
const deferred = () => { let resolve, reject; const promise = new Promise((yes,no) => { resolve=yes;reject=no; }); return { promise,resolve,reject }; };
const data = id => ({ id, title: 'Chapter-'+id, language:'en', published_at:'2026-10-22T00:00:00Z', relationships:{manga:{id:'m'+id,title:'Title-'+id,relationships:{}}} });
const result = (extra = {}) => ({ page:1,limit:100,data:[],total:null,scanned:1,parents:1005,chapters:2,indexed:false,ready:false,started:'2026-10-22T00:00:00Z', ...extra });
const settle = async () => { for(let i=0;i<24;i++){await Promise.resolve();flushSync();} };
async function fixture(load, {opening, disposal, cancellation, query = '', latestOnly = false, records = []} = {}) {
  const prefs=writable({items_per_page:'42',version:0}), views=writable('device:0:verified'), identity=writable({accountId:null}), authority=writable({epoch:0}), mangaView=writable('grid');
  prefs.applyMangaPreferences = q => q.set('contentRating','safe'); prefs.applyChapterPreferences = q => q.set('tlang','en');
  const localLibraryState = writable({records,ready:true,mode:'device'});
  const calls=[], opened=[], releases=[];let disposed=0,cancelled=0,navigation=0;
  globalThis.__updatesFixture={libraryDiscovery,isCurrentCatalogueId:id=>typeof id==='string',prefs,mangaView,localLibraryState,setReadHistory:()=>{},writable,libraryViewKey:views,libraryIdentity:identity,nativeAuthority:authority,captureNavigationEpoch:()=>navigation,page:{url:new URL('https://title.fixture/library'+query)},Constants:{MangaGridBreakpoints:{},MangaSliderBreakpoints:{}},Grid:Generic,Carousel:Generic,ChapterFeedMangaEntry:Entry,LatestUpdatesGridEntry:ActualLatestEntry,Locale:{to3166:()=>'gb',toLanguage:()=>'English'},Utils:{chapterHeadline:chapter=>chapter.title},MessagesSquareIcon:Generic,UsersIcon:Generic,ImageLoader:Generic,Button,Spinner:Generic,MangaSearchEntryCard:Entry,message:error=>error.message,UpdatesCancelled:class extends Error {constructor(){super('cancelled');}},TitleIndexStopped:class extends Error{},HistoryTransportUnsettled:class extends Error{},async openDeviceUpdatesFeed(query,latestOnly,assertOwner){opened.push({query,latestOnly,assertOwner});if(opening)await opening.promise;return{async load(page){calls.push(page);return load(page,opened.length);},async dispose(){disposed++;if(disposal)await disposal.promise;},releaseDisplayedPage(){releases.push(document.body.textContent);},cancel(){cancelled++;cancellation?.();}};}};
  const component=mount(Feed,{target:document.querySelector('main'),props:{latestOnly}});await settle();
  return{prefs,views,identity,authority,calls,opened,releases,disposed:()=>disposed,cancelled:()=>cancelled,navigate(){navigation++;},async close(){await unmount(component);await settle();},async click(text){const button=[...document.querySelectorAll('button')].find(row=>row.textContent===text);assert(button,`Missing button ${text}: ${document.body.textContent}`);button.click();await settle();}};
}
test('bounded progress automatically shows completed latest updates through actual renderer without a forum context',async()=>{
 let count=0;const f=await fixture(async()=>++count===1?result():count===2?result({scanned:1005,total:1,indexed:true}):result({scanned:1005,total:1,indexed:true,ready:true,data:[data('last')]}),{latestOnly:true});
 try{assert.match(document.body.textContent,/Checking your followed titles/);assert.doesNotMatch(document.body.textContent,/No updates matched/);assert.equal(f.opened[0].latestOnly,true);assert.equal(f.opened[0].query.get('limit'),'10');await f.click('Continue loading');assert.match(document.body.textContent,/Title-last/);assert.match(document.body.textContent,/Chapter-last/);assert.doesNotMatch(document.body.textContent,/Show sorted|Loading updates|DTO|atomic|10,000/);assert.equal(document.querySelector('a[title="Go to comments"]'),null);}finally{await f.close();}
});
test('next-page failure removes previous displayed data and retries the exact intended page',async()=>{
 let count=0;const f=await fixture(async n=>{if(++count===2)throw Error('503 without skip');return result({page:n,total:205,indexed:true,ready:true,data:[data(String(n))]});});
 try{assert.equal(f.opened[0].query.get('limit'),'100');await f.click('Next page');assert.doesNotMatch(document.body.textContent,/Title-1/);assert.match(document.body.textContent,/503 without skip/);await f.click('Retry updates');assert.match(document.body.textContent,/Title-2/);assert.deepEqual(f.calls,[1,2,2]);}finally{await f.close();}
});
for(const changed of ['prefs','views','identity','authority'])test(`${changed} invalidates delayed update ownership`,async()=>{
 const pending=deferred();let count=0;const f=await fixture(async()=>++count===1?pending.promise:result({total:0,indexed:true,ready:true}));
 try{f[changed].set(changed==='prefs'?{items_per_page:20,version:1}:changed==='identity'?{accountId:'different'}:changed==='authority'?{epoch:1}:'device:changed');await settle();pending.resolve(result({data:[data('stale')],ready:true,indexed:true,total:1}));await settle();assert.doesNotMatch(document.body.textContent,/Title-stale/);assert(f.disposed()>=1);}finally{await f.close();}
});
test('unmount during opening disposes the late owner without showing data',async()=>{const pending=deferred(),f=await fixture(async()=>result(),{opening:pending});await f.close();pending.resolve();await settle();assert.equal(f.disposed(),1);assert.deepEqual(f.calls,[]);});
test('rebuild waits for storage disposal before opening another scan',async()=>{const disposal=deferred(),f=await fixture(async()=>result({total:0,indexed:true,ready:true}),{disposal});try{await f.click('Refresh updates');assert.equal(f.opened.length,1);disposal.resolve();await settle();assert.equal(f.opened.length,2);}finally{disposal.resolve();await f.close();}});

test('actual mounted pager traverses 1,005 sparse parents in 51 bounded indexing actions, not 1,005 clicks',async()=>{
 const {IDBFactory,IDBKeyRange}=require(path.join(domRoot,'fake-indexeddb'));
 const context=vm.createContext({indexedDB:new IDBFactory(),IDBKeyRange,crypto:webcrypto,Date,URLSearchParams,AbortController,setTimeout,clearTimeout,TextEncoder,console}),modules=new Map();
 async function load(relative){const full=path.resolve(root,relative);if(modules.has(full))return modules.get(full);let source=fs.readFileSync(full,'utf8');if(full.endsWith('.ts'))source=stripTypeScriptTypes(source,{mode:'transform'});const module=new vm.SourceTextModule(source,{context,identifier:full});modules.set(full,module);await module.link(specifier=>load(path.relative(root,path.resolve(path.dirname(full),specifier+(path.extname(specifier)?'':'.ts')))));return module;}
 const storage=await load('platform-integrations/local-library/title-index.ts');await storage.evaluate();const pagination=await load('platform-integrations/local-library/updates-pagination.ts');await pagination.evaluate();
 const store=await storage.namespace.TitleIndex.create(()=>{}),parents=Array.from({length:1005},(_,n)=>'p'+n),last={...data('last'),published_at:'2026-10-22T00:00:00Z',_sort_value:1,relationships:{manga:{id:'p1004',title:'Title-last',relationships:{}}}};let scans=0;
 const pager=new pagination.namespace.UpdatesPager(parents,store,{async scan(parent){scans++;return parent==='p1004'?{total:1,data:[last]}:{total:0,data:[]};},async hydrate(){return[last];}},()=>{},100,1,false);
 let finished=Promise.resolve();const f=await fixture(page=>finished=pager.load(page));
 try{await finished;await settle();assert.equal(scans,20);for(let action=1;action<51;action++){assert.doesNotMatch(document.body.textContent,/Title-last/);await f.click('Continue loading');await finished;await settle();}assert.equal(scans,1005);await finished;await settle();assert.equal(f.calls.length,52);assert.match(document.body.textContent,/Title-last/);}finally{await f.close();await pager.dispose();}
});

test('completed scan budget notice pauses rather than silently restarting work',async()=>{let calls=0;const f=await fixture(async()=>{calls++;return result({indexed:true,total:1,notice:'budget'});});try{assert.equal(calls,1);assert.match(document.body.textContent,/Continue loading/);assert.doesNotMatch(document.body.textContent,/Loading updates/);}finally{await f.close();}});
test('empty completed scan is displayed without another user action',async()=>{let calls=0;const f=await fixture(async()=>++calls===1?result({indexed:true,total:0}):result({indexed:true,ready:true,total:0}));try{assert.equal(calls,2);assert.match(document.body.textContent,/No chapters found/);assert.doesNotMatch(document.body.textContent,/Show sorted|Continue loading/);}finally{await f.close();}});

test('cancel forwards to the active load and allows retry without a stuck spinner',async()=>{const pending=deferred();let calls=0;const f=await fixture(async()=>++calls===1?pending.promise:result({indexed:true,ready:true,total:0}),{cancellation:()=>pending.reject(Error('Update loading was cancelled.'))});try{assert.match(document.body.textContent,/Loading updates/);await f.click('Cancel loading');assert.equal(f.cancelled(),1);assert.doesNotMatch(document.body.textContent,/Loading updates/);assert.match(document.body.textContent,/cancelled/);await f.click('Retry updates');assert.match(document.body.textContent,/No chapters found/);}finally{await f.close();}});
test('actual latest entry preserves a real native discussion link',async()=>{const chapter=data('forum');chapter.relationships.thread={id:'actual-thread',relationships:{stats:{replies:0}}};const f=await fixture(async()=>result({indexed:true,ready:true,total:1,data:[chapter]}),{latestOnly:true});try{assert.equal(document.querySelector('a[title="Go to comments"]').getAttribute('href'),'/forums/threads/actual-thread');}finally{await f.close();}});


test('virgin updates provide a first-title discovery action only after a complete empty scan', async () => {
 const f=await fixture(async()=>result({total:0,parents:0,scanned:0,indexed:true,ready:true}));
 try { assert.match(document.body.textContent,/Save your first title/); assert.equal(document.querySelector('a').getAttribute('href'),'/search'); assert.doesNotMatch(document.body.textContent,/No chapters found/); } finally { await f.close(); }
});
test('saved titles with updates off lead to library instead of claiming no saved titles', async () => {
 const f=await fixture(async()=>result({total:0,parents:0,scanned:0,indexed:true,ready:true}),{records:[{family:'series',id:'title',notification:false}]});
 try { assert.match(document.body.textContent,/Chapter updates are off/); assert.equal(document.querySelector('a').getAttribute('href'),'/library'); assert.doesNotMatch(document.body.textContent,/Save your first title/); } finally { await f.close(); }
});
test('followed titles without matching chapters offer preferences rather than discovery', async () => {
 const f=await fixture(async()=>result({total:0,parents:1,scanned:1,indexed:true,ready:true}),{records:[{family:'series',id:'title',notification:true}]});
 try { assert.match(document.body.textContent,/current preferences/); assert.equal(document.querySelector('a').getAttribute('href'),'/settings/preferences'); assert.doesNotMatch(document.body.textContent,/Save your first title|updates are off/); } finally { await f.close(); }
});
