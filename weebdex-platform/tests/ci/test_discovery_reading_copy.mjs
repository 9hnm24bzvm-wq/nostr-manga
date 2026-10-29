// Bounded helper + mounted copy/link regressions; no server, browser, or writes.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { createHash } from 'node:crypto';
import { createRequire, stripTypeScriptTypes } from 'node:module';
import { fileURLToPath, pathToFileURL } from 'node:url';
import test, { after } from 'node:test';
import { discoveryCopyPatches } from '../../platform-integrations/frontend-discovery-copy-overlays.mjs';
import { createFrontendOverlays } from '../../platform-integrations/frontend-overlays.mjs';
import { createLocalLibraryOverlays } from '../../platform-integrations/local-library/overlays.mjs';
const root=path.resolve(path.dirname(fileURLToPath(import.meta.url)),'../..');
const load=async relative=>import('data:text/javascript;base64,'+Buffer.from(stripTypeScriptTypes(fs.readFileSync(path.join(root,relative),'utf8'),{mode:'transform'})).toString('base64'));
const {discoveryReadingQuery}=await load('platform-integrations/frontend-behavior/discovery-reading.ts');
const {titleReaderFilters}=await load('platform-integrations/bounded-reader/title-query.ts');
const {feedCommentsDestination}=await load('platform-integrations/public-api/feed-comments.ts');
const {libraryDiscovery,unfilteredLibraryHref}=await load('platform-integrations/local-library/library-discovery.ts');
const languages={en:'English',ja:'Japanese',fi:'Finnish',uk:'Ukrainian'};
const url=text=>new URL(text,'https://fixture.invalid');
const uuid='10a3e495-d581-47d9-a5eb-d162f35833e2';

test('only explicit translated search languages travel into title reader filters, never preferences or unrelated filters',()=>{
 const search=url('/search?availableTranslatedLang=en&availableTranslatedLang=fi&lang=ja&title=Private+query&group=unrelated&page=4&hasChapters=true');
 const original=search.href,query=discoveryReadingQuery(search,languages);
 assert.equal(query,'?tlang=en&tlang=fi');assert.equal(search.href,original);
 const preferences={tlang:['ja'],group:['preferred-group'],contentRating:['safe']};
 const selected=titleReaderFilters(q=>{for(const [key,values] of Object.entries(preferences))for(const value of values)q.append(key,value);},new URLSearchParams(query));
 assert.deepEqual(selected.getAll('tlang'),['en','fi']);assert.deepEqual(selected.getAll('group'),['preferred-group']);assert.deepEqual(selected.getAll('contentRating'),['safe']);assert.deepEqual(preferences.tlang,['ja']);
});
test('language carry is tab-local URL state, validates languages, deduplicates aliases, and never leaks into unrelated navigation',()=>{
 assert.equal(discoveryReadingQuery(url('/search?availableTranslatedLang[]=en&availableTranslatedLang=en'),languages),'?tlang=en');
 for(const route of ['/library?availableTranslatedLang=en','/title/'+uuid+'?availableTranslatedLang=en','/search?lang=ja','/search?availableTranslatedLang=xx','/search?availableTranslatedLang=en&hasChapters=false','/search?availableTranslatedLang=en&hasChapters=true&hasChapters=true','/search?'+Array(101).fill('availableTranslatedLang=en').join('&')])assert.equal(discoveryReadingQuery(url(route),languages),'',route);
 assert.equal(discoveryReadingQuery(url('/search?availableTranslatedLang=uk'),languages),'?tlang=uk');
 assert.equal(discoveryReadingQuery(url('/search'),languages),'');
});
test('comments prefer only a real thread, tolerate absent stats, and otherwise use a known parent without invented counts',()=>{
 const parent={id:uuid};
 assert.deepEqual(feedCommentsDestination({relationships:{thread:{id:'actual-thread'}}},parent),{href:'/forums/threads/actual-thread',label:'Comments',title:'Go to comments',replies:undefined});
 assert.equal(feedCommentsDestination({relationships:{thread:{id:'actual-thread',relationships:{stats:{replies:12}}}}},parent).replies,12);
 assert.deepEqual(feedCommentsDestination({relationships:{}},parent),{href:'/title/'+uuid+'?tab=comments',label:'Title comments',title:'Open comments on this title',replies:undefined});
 assert.equal(feedCommentsDestination(undefined,undefined),undefined);
 for(const id of ['../../account','https://evil.invalid','<script>','unknown'])assert.equal(feedCommentsDestination({relationships:{thread:{id:'../bad'}}},{id}),undefined);
});
test('library summary counts only compatible saved titles and does not confuse empty filtered candidates with an empty library',()=>{
 const list='a'.repeat(32),records=[{family:'lists',id:list,name:'Weekend'},{family:'series',id:uuid},{family:'series',id:'old'},{family:'members',list,series:uuid}];
 assert.deepEqual(libraryDiscovery(records,list,id=>id===uuid),{saved:1,listSaved:1,listName:'Weekend'});
 assert.equal(unfilteredLibraryHref(list),'/library?list='+list);assert.equal(unfilteredLibraryHref('" onclick="x'),'/library');
});
test('copy overlays pin immutable original hashes and keep intentional tag include/exclude totals',()=>{
 for(const patch of discoveryCopyPatches){const bytes=fs.readFileSync(path.join(root,'frontend',patch.path)),source=bytes.toString();assert.equal(createHash('sha256').update(bytes).digest('hex'),patch.sha256);assert.equal(source.split(patch.before).length-1,patch.count,patch.label);}
 const badges=discoveryCopyPatches.filter(patch=>patch.approved_discovery_copy_exception==='accurate_selected_total');assert.equal(badges.length,6);for(const patch of badges)assert.match(patch.after,/selected$/);
 assert.equal(discoveryCopyPatches.some(patch=>patch.before.includes('+{search.tag.length}')),false);
});

const dependencies=process.env.WEEBDEX_FRONTEND_ROOT,domRoot=process.env.WEEBDEX_BROWSER_DOM_NODE_MODULES;
assert(dependencies&&domRoot,'Use existing exact-lock dependency roots');
const require=createRequire(path.join(dependencies,'package.json')),{compile}=require('svelte/compiler'),ts=require('typescript'),{JSDOM}=require(path.join(domRoot,'jsdom'));
assert.equal(require('svelte/package.json').version,'5.55.1');assert.equal(require(path.join(domRoot,'jsdom/package.json')).version,'27.0.1');
const {window}=new JSDOM('<main></main>',{url:'https://fixture.invalid/library',pretendToBeVisual:true});
for(const key of ['window','document','Node','Element','HTMLElement','HTMLInputElement','HTMLTextAreaElement','HTMLSelectElement','HTMLMediaElement','HTMLFormElement','Text','Comment','Event','CustomEvent','MutationObserver','requestAnimationFrame','cancelAnimationFrame','getComputedStyle'])globalThis[key]=window[key];
const client=pathToFileURL(path.join(dependencies,'node_modules/svelte/src/index-client.js')).href;
const {mount,unmount,flushSync}=await import(client),{writable}=await import(pathToFileURL(path.join(dependencies,'node_modules/svelte/src/store/index-client.js')));
const temp=fs.mkdtempSync(path.join(os.tmpdir(),'discovery-copy-dom-'));fs.symlinkSync(path.join(dependencies,'node_modules'),path.join(temp,'node_modules'));
after(()=>{window.close();fs.rmSync(temp,{recursive:true,force:true});delete globalThis.__discoveryFixture;});
async function compiled(name,source,replace=false){
 if(replace)source=source.replace(/<script\b([^>]*)>([\s\S]*?)<\/script>/g,(_,attributes,script)=>{
  const ast=ts.createSourceFile(name+'.ts',script,ts.ScriptTarget.Latest,true,ts.ScriptKind.TS);
  for(const item of [...ast.statements].reverse())if(ts.isImportDeclaration(item)){
   const clause=item.importClause;let replacement='';
   if(item.moduleSpecifier.text==='svelte')replacement=item.getText(ast).replace(/(['"])svelte\1/,JSON.stringify(client));
   else if(clause&&!clause.isTypeOnly){const names=[];if(clause.name)names.push(clause.name.text);if(clause.namedBindings&&ts.isNamespaceImport(clause.namedBindings))names.push(clause.namedBindings.name.text);else if(clause.namedBindings)for(const entry of clause.namedBindings.elements)if(!entry.isTypeOnly)names.push(entry.name.text);if(names.length)replacement=`const {${names.join(',')}}=globalThis.__discoveryFixture;`;}
   script=script.slice(0,item.getStart(ast))+replacement+script.slice(item.end);
  }
  return '<script'+attributes+'>'+script+'</script>';
 });
 fs.writeFileSync(path.join(temp,name+'.mjs'),compile(source,{filename:name+'.svelte',generate:'client',runes:true}).js.code);return(await import(pathToFileURL(path.join(temp,name+'.mjs')))).default;
}
const Generic=await compiled('Generic','<script>let {children,feed=$bindable(),isShowFilter=$bindable()}=$props();</script><div>{@render children?.()}</div>');
const FeedCommentsLink=await compiled('FeedCommentsLink',fs.readFileSync(path.join(root,'platform-integrations/public-api/FeedCommentsLink.svelte'),'utf8'),true);
const settle=async()=>{for(let i=0;i<8;i++){await Promise.resolve();flushSync();}};
test('mounted missing chapter relationships offer title comments and never crash or claim absence',async()=>{
 globalThis.__discoveryFixture={MessagesSquareIcon:Generic,feedCommentsDestination};const component=mount(FeedCommentsLink,{target:document.querySelector('main'),props:{chapter:{},manga:{id:uuid}}});
 try{await settle();assert.equal(document.querySelector('a').getAttribute('href'),'/title/'+uuid+'?tab=comments');assert.match(document.body.textContent,/Title comments/);assert.doesNotMatch(document.body.textContent,/unavailable|\(0\)/);}finally{await unmount(component);}
});
test('mounted real thread without stats keeps direct discussion route and no fabricated zero count',async()=>{
 globalThis.__discoveryFixture={MessagesSquareIcon:Generic,feedCommentsDestination};const component=mount(FeedCommentsLink,{target:document.querySelector('main'),props:{chapter:{relationships:{thread:{id:'actual-thread'}}},manga:{id:uuid}}});
 try{await settle();assert.equal(document.querySelector('a').getAttribute('href'),'/forums/threads/actual-thread');assert.equal(document.querySelector('a').textContent.trim(),'Comments');}finally{await unmount(component);}
});
test('composed library heading follows selected list rename and clears on account switch without rendering user markup',async()=>{
 const list='c'.repeat(32),localLibraryState=writable({mode:'device',ready:true,records:[{family:'lists',id:list,name:'Weekend <reads>'}]}),libraryViewKey=writable('device:0:verified');
 globalThis.__discoveryFixture={localLibraryState,libraryViewKey,page:{url:url('/library?list='+list),state:{}},PUBLIC_SITE_NAME:'WeebDex',prefs:writable({items_per_page:'20'}),MangaSearch:Generic,MangaSearchFilter:Generic,MangaSearchPagination:Generic,DeviceTitleLibrary:Generic,replaceState(){},API:{},Constants:{},StatusCodes:{OK:200}};
 const frontend=path.join(root,'frontend'),file=path.join(frontend,'src/routes/(app)/library/+page.svelte'),original=fs.readFileSync(file,'utf8'),first=createFrontendOverlays(frontend).transform.handler(original,file)?.code??original,source=createLocalLibraryOverlays(frontend).transform.handler(first,file)?.code??first;
 const Route=await compiled('LibraryRoute',source,true),component=mount(Route,{target:document.querySelector('main')});
 try{await settle();assert.equal(document.querySelector('h2').textContent,'Weekend <reads> — My Library');assert.equal(document.querySelector('reads'),null);localLibraryState.set({mode:'device',ready:true,records:[{family:'lists',id:list,name:'Renamed'}]});await settle();assert.equal(document.querySelector('h2').textContent,'Renamed — My Library');localLibraryState.set({mode:'account',ready:true,records:[{family:'lists',id:list,name:'Private device list'}]});await settle();assert.equal(document.querySelector('h2').textContent,'My Library');}finally{await unmount(component);}
});


test('translated language link survives the retained URLSearchParams question-mark prefix', () => {
 const original=URLSearchParams.prototype.toString;
 try { URLSearchParams.prototype.toString=function(){const query=original.call(this);return query?'?'+query:'';}; assert.equal(discoveryReadingQuery(url('/search?availableTranslatedLang=en'),languages),'?tlang=en'); }
 finally { URLSearchParams.prototype.toString=original; }
});
