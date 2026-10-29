// Actual ChapterFeed composition with finite read-only feed/account adapters.
// This qualifies the empty-state explanation only: reader completion persistence
// and real account HTTP/browser flows remain separately qualified.
import fs from 'node:fs';
import path from 'node:path';
import {createHash} from 'node:crypto';
import test from 'node:test';
import assert from 'node:assert/strict';
import {component,root,window,writable,get,mounted,settle} from './account-fixpass-dom-fixture.mjs';
import {composedBaseline,composed} from './composed-account-library-fixture.mjs';
import {uploadCompletionPatches} from '../../platform-integrations/six-persona-upload-overlays.mjs';
import {contributorRoutePatches} from '../../platform-integrations/public-api/contributor-route-patches.mjs';
import {staffWorkspacePatches} from '../../platform-integrations/staff-workspace/patches.mjs';
import {integrationPatches} from '../../platform-integrations/six-persona/integration-patches.mjs';

const relative='src/lib/components/ChapterFeed.svelte';
const oldCopy=["The database may still be updating, or you haven't read anything yet.",'Any chapters you read will appear here shortly.'];
const newCopy=['History shows completed chapters, not every page you have opened.','Finish a chapter to add it to History. If you have already finished chapters, check your filters.'];
const baseline=await composedBaseline(relative),original=fs.readFileSync(path.join(root,'frontend',relative),'utf8');
const patches=[...uploadCompletionPatches,...contributorRoutePatches,...staffWorkspacePatches,...integrationPatches].filter(row=>row.path===relative);
const isCopy=row=>oldCopy.some(text=>row.before.includes(text));
assert(patches.some(isCopy),'Expected the reviewed history-copy successor');
function applyExact(source,rows){for(const row of rows){assert.equal(source.split(row.before).length-1,row.count,'exact source anchor: '+row.label);source=source.split(row.before).join(row.after);}return source;}
const previous=applyExact(baseline,patches.filter(row=>!isCopy(row))),current=applyExact(baseline,patches);
let expected=previous;for(let index=0;index<oldCopy.length;index++){assert.equal(expected.split(oldCopy[index]).length-1,1);expected=expected.replace(oldCopy[index],newCopy[index]);}
assert.equal(current,expected,'The reviewed delta may change only the two history paragraph text values');
const Feed=await component('HistoryCompletionCurrent',current),PreviousFeed=await component('HistoryCompletionPrevious',previous);
const Generic=await component('HistoryCopyGeneric','<script>let{children}=$props();</script><div>{@render children?.()}</div>');
const Button=await component('HistoryCopyButton','<script>let{children,variant,...rest}=$props();</script><button {...rest}>{@render children?.()}</button>');
const Spinner=await component('HistoryCopySpinner','<span role="status">Loading…</span>');
const Entry=await component('HistoryCopyEntry','<script>let{manga}=$props();</script><article data-existing-chapter>{manga.title}</article>');
const ErrorComponent=await component('HistoryCopyError','<script>let{error}=$props();</script><p role="alert">{error.body}</p>');
const mangaId='11111111-1111-4111-8111-111111111111',chapterId='22222222-2222-4222-8222-222222222222';
const empty=()=>({status:200,body:{page:1,limit:100,total:0,data:[],map:{manga:{}}}});
class FeedError extends Error{constructor(title,body){super(body);this.title=title;this.body=body;}}
function environment(route,{response=empty(),hold}={}){
 window.history.replaceState({},'',route);
 const page={url:new URL(window.location.href),state:{}};
 const Session=writable({identity:{id:'SyntheticHistoryReader'},roles:['user'],permissions:[]}),views=writable('account:SyntheticHistoryReader');
 const calls={requests:[],historyReads:[],mutations:[],navigation:[],errors:[]};
 const context={generation:1,key:'synthetic-account-context'};
 const forbidden=method=>(...args)=>{calls.mutations.push({method,args});assert.fail('History explanation must not mutate: '+method);};
 const modules={
  '$localLibrary/library':{libraryIdentity:writable({canUse:true,accountId:'SyntheticHistoryReader'}),libraryViewKey:views,localLibraryState:writable({ready:true,mode:'account'}),captureLibraryView:()=>get(views),captureLibraryContext:()=>context,assertLibraryContext:value=>assert.equal(value,context),saveProgress:forbidden('saveProgress')},
  '$localLibrary/native-account-library':{readNativeVisibleChapterHistory:async(chapters,assertOwner)=>{assertOwner();calls.historyReads.push(chapters.map(row=>row.id));return{status:200,body:{[mangaId]:{[chapterId]:null}}};},saveNativeAccountProgress:forbidden('saveNativeAccountProgress')},
  '$platform/mutation-authority':{nativeAuthority:writable({actor:'SyntheticHistoryReader',epoch:1})},
  '$platform/feed-continuation':{createFeedContinuation:()=>{assert.fail('Complete/empty finite feed needs no pagination observer');}},
  '$platform/navigation-authority':{captureNavigationEpoch:()=>1,sameNavigationEpoch:epoch=>epoch===1},
  '$app/navigation':{replaceState:(href,state)=>calls.navigation.push({href,state})},
  '$app/state':{page},
  '$lib':{API:{ChapterHistory:{Search:()=>assert.fail('Do not load broad signed history'),Update:forbidden('ChapterHistory.Update'),UpdateBatch:forbidden('ChapterHistory.UpdateBatch')}},Constants:{Error:{Error:FeedError,Unknown:new FeedError('Read unavailable','History read was not confirmed.'),TooManyRequests:new FeedError('Too many requests','Try again later.')}}},
  '$lib/Context':{setReadHistory:value=>calls.readHistory=value},'$lib/stores':{Session},
  '$lib/components/ui/button':{Button},'$lib/components/ui/empty':{Root:Generic,Header:Generic,Title:Generic,Description:Generic},'$lib/components/ui/Grid.svelte':{default:Generic},
  './ChapterFeedMangaEntry.svelte':{default:Entry},'./ErrorComponent.svelte':{default:ErrorComponent},'./GenericPagination.svelte':{default:Generic},'./ui/spinner':{Spinner},
  'http-status-codes':{StatusCodes:{OK:200,TOO_MANY_REQUESTS:429}},'svelte-sonner':{toast:{error:(title,options)=>calls.errors.push({title,options})}}
 };
 globalThis.__accountFixpass={modules};
 return{calls,Session,views,onSearch:async pageNumber=>{calls.requests.push({method:'GET',route,page:pageNumber});if(hold)await hold;return structuredClone(response);}};
}
async function render(route,{old=false,...options}={}){const env=environment(route,options),view=await mounted(old?PreviousFeed:Feed,{onSearch:env.onSearch});return{...env,...view};}
const paragraphs=host=>[...host.querySelectorAll('p')].map(node=>node.textContent.trim());
const hash=value=>createHash('sha256').update(value).digest('hex');

test('reviewed history successor changes exactly two paragraphs; scripts, request code and originals remain identical',()=>{
 assert.equal(current,expected);
 assert.deepEqual(current.match(/<script[^>]*>[\s\S]*?<\/script>/g),previous.match(/<script[^>]*>[\s\S]*?<\/script>/g));
 let reversed=current;for(const row of [...patches].reverse()){assert.equal(reversed.split(row.after).length-1,row.count);reversed=reversed.split(row.after).join(row.before);}assert.equal(reversed,baseline);
 assert.equal(hash(fs.readFileSync(path.join(root,'frontend',relative))),hash(original));
});
test('final registered composition equals the exact candidate exercised by the DOM tests',async()=>assert.equal(await composed(relative),current));
test('counterfactual authenticated history suggests all opened chapters will appear',async()=>{const f=await render('/history',{old:true});try{assert.deepEqual(paragraphs(f.host),oldCopy);assert.equal(f.calls.requests.length,1);assert.deepEqual(f.calls.historyReads,[]);assert.deepEqual(f.calls.mutations,[]);}finally{await f.dispose();}});
test('authenticated empty history explains completion rather than an imaginary background delay',async()=>{const f=await render('/history');try{assert.deepEqual(paragraphs(f.host),newCopy);assert.match(f.host.textContent,/No results found/);assert.doesNotMatch(f.host.textContent,/database may still be updating|appear here shortly|haven.t read anything yet/);assert.deepEqual(f.calls.requests,[{method:'GET',route:'/history',page:undefined}]);assert.deepEqual(f.calls.historyReads,[]);assert.deepEqual(f.calls.mutations,[]);assert.deepEqual(f.calls.navigation,[]);}finally{await f.dispose();}});
for(const route of ['/library','/library/updates','/search','/history?translatedLanguage=en','/search?q=example'])test('unrelated or explicitly filtered empty state remains unchanged: '+route,async()=>{
 const prior=await render(route,{old:true});let before,requests;try{before=prior.host.textContent;requests=prior.calls.requests;assert.deepEqual(prior.calls.mutations,[]);}finally{await prior.dispose();}
 const f=await render(route);try{assert.equal(f.host.textContent,before);assert.deepEqual(f.calls.requests,requests);assert.deepEqual(f.calls.mutations,[]);assert(!f.host.textContent.includes(newCopy[0]));if(route.includes('?'))assert.match(f.host.textContent,/Try adjusting your search or using different keywords/);else if(route.startsWith('/library'))assert.match(f.host.textContent,/haven't added anything to your library yet/);else assert.match(f.host.textContent,/no entries matching your filters were found/);}finally{await f.dispose();}
});
test('pending history read shows loading, not a premature completion explanation',async()=>{let release;const hold=new Promise(resolve=>release=resolve);const f=await render('/history',{hold});try{assert(f.host.querySelector('[role=status]'));assert(!f.host.textContent.includes(newCopy[0]));assert.deepEqual(f.calls.mutations,[]);release();await settle();assert.deepEqual(paragraphs(f.host),newCopy);assert.equal(f.calls.requests.length,1);assert.deepEqual(f.calls.mutations,[]);}finally{release();await f.dispose();}});
test('refused history read remains an error, not a false empty completed-history result',async()=>{const f=await render('/history',{response:{status:503}});try{assert.match(f.host.querySelector('[role=alert]').textContent,/not confirmed/);assert(!f.host.textContent.includes(newCopy[0]));assert.equal(f.calls.requests.length,1);assert.deepEqual(f.calls.mutations,[]);}finally{await f.dispose();}});
test('confirmed completed history entries remain visible without empty copy or any progress write',async()=>{const response={status:200,body:{page:1,limit:100,total:1,data:[{id:chapterId,relationships:{manga:{id:mangaId}}}],map:{manga:{[mangaId]:{id:mangaId,title:'Completed synthetic chapter',relationships:{}}}}}};const f=await render('/history',{response});try{assert.equal(f.host.querySelector('[data-existing-chapter]').textContent,'Completed synthetic chapter');assert(!f.host.textContent.includes(newCopy[0]));assert.deepEqual(f.calls.historyReads,[[chapterId]]);assert.deepEqual(f.calls.mutations,[]);assert.equal(f.calls.requests.length,1);}finally{await f.dispose();}});
