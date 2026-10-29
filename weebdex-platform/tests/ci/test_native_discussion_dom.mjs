// Explicit advanced-mode assembly mounts pinned Svelte route/provider components and real signed controller; ordinary-mode cases use the actual ordinary wrapper.
//
// resolver, sanitizer and typography. Native cookie UI and reader chrome are
// explicit stand-ins. No browser, real signer, HTTP, DB or deployment acceptance.
import {createHash} from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import {createRequire} from 'node:module';
import {pathToFileURL} from 'node:url';
import test from 'node:test';
import assert from 'node:assert/strict';
import {root,id,other,threadId,plain,readerModule,contextModule,targetBody} from './native-discussion-context-fixture.mjs';
import {fixture as signedFixture,actor,loadSignedFixtureModule,deferred,RequestFailure} from './report-draft-fixture.mjs';
import {createPublicApiOverlays} from '../../platform-integrations/public-api/overlays.mjs';
import {createFrontendOverlays} from '../../platform-integrations/frontend-overlays.mjs';
import {createReaderOverlays} from '../../platform-integrations/frontend-reader-overlays.mjs';
import {createLocalLibraryOverlays} from '../../platform-integrations/local-library/overlays.mjs';
const dependencies=process.env.WEEBDEX_FRONTEND_ROOT,domModules=process.env.WEEBDEX_BROWSER_DOM_NODE_MODULES;
assert(dependencies,'Use the existing pinned frontend dependency root');assert(domModules,'Use the existing pinned JSDOM dependency root');
const require=createRequire(path.join(dependencies,'package.json')),{compile}=require('svelte/compiler'),ts=require('typescript');
assert.equal(require('svelte/package.json').version,'5.55.1');assert.equal(require('typescript/package.json').version,'5.9.3');assert.equal(require(path.join(domModules,'jsdom/package.json')).version,'27.0.1');
const{JSDOM}=require(path.join(domModules,'jsdom')),{window}=new JSDOM('<main></main>',{url:'https://catalog.example/title/'+id,pretendToBeVisual:true});
for(const key of ['window','document','Node','Element','HTMLElement','HTMLInputElement','HTMLTextAreaElement','HTMLSelectElement','HTMLImageElement','HTMLMediaElement','HTMLFormElement','Text','Comment','Event','MouseEvent','CustomEvent','MutationObserver','requestAnimationFrame','cancelAnimationFrame','getComputedStyle'])globalThis[key]=window[key];
window.HTMLElement.prototype.scrollIntoView=()=>{};
const svelte=pathToFileURL(path.join(dependencies,'node_modules/svelte/src/index-client.js')).href;
const{mount,unmount,flushSync}=await import(svelte),{writable}=await import(pathToFileURL(path.join(dependencies,'node_modules/svelte/src/store/index-client.js')));
fs.mkdirSync(path.join(root,'.build'),{recursive:true});const generation=fs.mkdtempSync(path.join(root,'.build/native-discussion-dom-'));
fs.symlinkSync(path.join(dependencies,'node_modules'),path.join(generation,'node_modules'));
test.after(()=>{window.close();fs.rmSync(generation,{recursive:true,force:true});delete globalThis.__nativeDiscussionEnvironment;});
async function compiled(name,source,replace=true){
 if(replace)source=source.replace(/<script[^>]*>([\s\S]*?)<\/script>/,(_all,script)=>{
  const ast=ts.createSourceFile(name+'.ts',script,ts.ScriptTarget.Latest,true,ts.ScriptKind.TS);
  for(const item of [...ast.statements].reverse()){
   if(!ts.isImportDeclaration(item))continue;
   const specifier=item.moduleSpecifier.text,clause=item.importClause,reference=`globalThis.__nativeDiscussionEnvironment.modules[${JSON.stringify(specifier)}]`;let replacement='';
   if(specifier==='svelte')replacement=item.getText(ast).replace(/(['"])svelte\1/,JSON.stringify(svelte));
   else if(clause&&!clause.isTypeOnly){
    if(clause.name)replacement+=`const ${clause.name.text}=${reference}.default;`;
    if(clause.namedBindings&&ts.isNamespaceImport(clause.namedBindings))replacement+=`const ${clause.namedBindings.name.text}=${reference};`;
    else if(clause.namedBindings)for(const row of clause.namedBindings.elements)if(!row.isTypeOnly)replacement+=`const ${row.name.text}=${reference}[${JSON.stringify(row.propertyName?.text??row.name.text)}];`;
   }
   script=script.slice(0,item.getStart(ast))+replacement+script.slice(item.end);
  }
  return'<script lang="ts">'+script+'</script>';
 });
 const output=compile(source,{filename:name+'.svelte',generate:'client',runes:true}).js.code,file=path.join(generation,name+'.mjs');fs.writeFileSync(file,output);return(await import(pathToFileURL(file))).default;
}
const frontend=path.join(root,'frontend');
function composed(relative,enabled=true){const file=path.join(frontend,relative);let source=fs.readFileSync(file,'utf8');for(const plugin of [createFrontendOverlays(frontend),createReaderOverlays(frontend),createLocalLibraryOverlays(frontend),createPublicApiOverlays(frontend,{enabled})])source=plugin.transform.handler(source,file)?.code??source;return source;}
async function jsModule(name,source,runes=false){source=ts.transpile(source,{target:ts.ScriptTarget.ES2022,module:ts.ModuleKind.ESNext}).replace(/(['"])svelte\1/g,JSON.stringify(svelte));if(runes)source=require('svelte/compiler').compileModule(source,{filename:name+'.svelte.js',generate:'client'}).js.code;const file=path.join(generation,name+'.mjs');fs.writeFileSync(file,source);return import(pathToFileURL(file));}
const context=await jsModule('CommentsContext',composed('src/lib/components/comments/context.svelte.ts'),true);
const Utils=await jsModule('Utils',fs.readFileSync(path.join(frontend,'src/lib/Utils.ts'),'utf8')),Typography=await jsModule('Typography',fs.readFileSync(path.join(frontend,'src/lib/Typography.svelte.ts'),'utf8'));
await jsModule('MarkedExtensions',fs.readFileSync(path.join(frontend,'src/lib/MarkedExtensions.ts'),'utf8').replace('import { Utils } from "$lib";','import * as Utils from "./Utils.mjs";'));
// Install the real retained comment renderer and sanitization hooks without the
// unrelated root layout's global URLSearchParams/prototype patches.
const formatting=fs.readFileSync(path.join(frontend,'src/routes/+layout.ts'),'utf8');
await jsModule('CommentFormatting','import DOMPurify from "dompurify"; import {marked,type Tokens} from "marked"; import * as MarkedExtensions from "./MarkedExtensions.mjs"; const PUBLIC_PROXY_URL="/synthetic-image-proxy";\n'+formatting.slice(formatting.indexOf('const isSafeUrl')));
// Setup is a leaf stand-in; the shared gate lifecycle/focus is tested separately.
const IdentityGate=await compiled('IdentityGate','<script>let{show}=$props();</script>{#if show}<button data-identity-gate>Set up public identity</button>{/if}',false);
const Empty=await compiled('Empty','<span></span>',false),Generic=await compiled('Generic','<script>let{children}=$props();</script><div>{@render children?.()}</div>',false);
const BrowserComments=await compiled('BrowserComments','<script>import{onMount}from"svelte";import{Session}from"$lib/stores";const env=globalThis.__nativeDiscussionEnvironment;onMount(()=>{env.browserMounts++;});</script><div data-browser-comments>Browser account {$Session.identity.id}<button onclick={()=>env.nativeWrites.push({account:$Session.identity.id,operation:"create"})}>Browser comment action</button></div>');
const Button=await compiled('Button','<script>let{children,type="button",...rest}=$props();</script><button {type} {...rest}>{@render children?.()}</button>',false);
const NativeProvider=await compiled('NativeProvider',composed('src/lib/components/comments/comments-provider.svelte'));
const integration=name=>fs.readFileSync(path.join(root,'platform-integrations',name),'utf8');
const UnavailableContext=await compiled('UnavailableContext',integration('public-api/native-forum/UnavailableContext.svelte'));
const Provider=await compiled('Provider',integration('public-api/native-forum/Provider.svelte'));
const Discussion=await compiled('Discussion',integration('public-api/native-forum/AdvancedComments.svelte'));
const OrdinaryDiscussion=await compiled('OrdinaryDiscussion',integration('public-api/native-forum/Comments.svelte'));
// Exact pre-change wrapper, pinned here rather than rewriting a historical baseline.
const historicalWrapper=String.raw`<script lang="ts">
 import {Comments} from '$lib/components/comments';
 import {useComments} from '$lib/components/comments/context.svelte';
 let {class:className}:{class?:string}=$props();
 const comments=useComments();
</script>
{#if comments?.thread}<Comments class={className}/>{:else}<p class={className} role="status">Comments are unavailable for this resource.</p>{/if}
`;
assert.equal(createHash('sha256').update(historicalWrapper).digest('hex'),'4356d78b5642e2865908b006f8040d37ab5ad174e31e876714046448505ef908');
const HistoricalDiscussion=await compiled('HistoricalDiscussion',historicalWrapper);
const SignedDiscussion=await compiled('SignedDiscussion',integration('public-api/native-forum/SignedDiscussion.svelte'));
const MangaDiscussion=await compiled('MangaDiscussion',integration('local-library/MangaDiscussion.svelte'));
const Reader=await compiled('Reader',composed('src/lib/components/reader/reader.svelte'));
const GroupPage=await compiled('GroupPage',composed('src/routes/(app)/group/[id]/comments/+page.svelte'));
const ForumPage=await compiled('ForumPage',composed('src/routes/(app)/forums/threads/[id]/+page.svelte'));
const Harness=await compiled('Harness',`<script>
 import Provider from './Provider.mjs';import Discussion from './Discussion.mjs';import MangaDiscussion from './MangaDiscussion.mjs';import Reader from './Reader.mjs';import GroupPage from './GroupPage.mjs';import ForumPage from './ForumPage.mjs';
 const env=globalThis.__nativeDiscussionEnvironment;
 let resource=$state.raw(env.initialResource),url=$state(new URL(env.initialUrl));
 env.resource=()=>resource;env.currentUrl=()=>url;env.setLocation=value=>url=new URL(value);
 env.setResource=value=>{resource=value;env.group.set(value);};
</script>
{#if env.surface==='forum'}<ForumPage data={{thread:resource.relationships.thread}} />
{:else}<Provider thread={resource.relationships.thread}>
 {#if env.surface==='manga'}<MangaDiscussion />
 {:else if env.surface==='chapter'}<Reader />
 {:else if env.surface==='group'}<GroupPage />
 {:else}<Discussion target={{kind:env.kind,targetId:resource.id,threadId:resource.relationships.thread?.id??''}} />{/if}
</Provider>{/if}`,false);
const signedClient=await loadSignedFixtureModule('native-forum/signed-client');
const markdown='**Bold** and [safe](https://example.test/)\n\n<script>window.__unsafe=1</script>\n\n> [quote=Native author,user=NativeAuthor,post=7]\n> Quoted text\n\n||Secret||';
const nativeThread=(value=threadId)=>({id:value,subject:'Actual thread',body:'',locked:false,created_at:'2026-10-22T00:00:00Z',updated_at:'2026-10-22T00:00:00Z',relationships:{stats:{replies:21,up:2,down:1},author:{id:'NativeAuthor',name:'Native author'},manga:{id:'NOT-A-PUBLIC-TARGET'}}});
const resource=(target=id,thread=nativeThread())=>({id:target,title:'Synthetic title',name:'Synthetic group',chapter:'1',relationships:{thread,manga:{id:other,title:'Parent title',relationships:{}}}});
const reply=(page=1)=>({id:page===1?7:27,reply_number:page===1?1:21,version:3,body:markdown,state:'active',created_at:'2026-10-22T00:00:00Z',relationships:{author:{id:'NativeAuthor',name:'Native author'},stats:{up:2,down:1},vote_state:0}});
function metadata(request){const url=new URL(request.url),parts=url.pathname.split('/');return{scope:'instance-account',source:'persisted-native-discussion',identity:'native_catalogue_resource',accountId:actor,targetKind:parts[4],targetId:parts[5],threadId:request.body?.expectedThreadId??url.searchParams.get('expectedThreadId')};}
function receipt(request){const b=request.body,r=reply();if(b.operation==='create'){r.id=8;r.reply_number=2;r.version=1;r.body=b.body;}if(b.operation==='update'){r.version=b.version+1;r.body=b.body;}if(b.operation==='remove'){r.version=b.version+1;r.state='deleted_by_user';delete r.body;delete r.relationships.author;}if(b.operation==='reply_vote')r.relationships.vote_state=b.vote;return{status:200,body:{result:'ok',nostr:{...metadata(request),operation:b.operation,changed:true,requestMethod:request.method,requestPath:new URL(request.url).pathname,requestHash:createHash('sha256').update(JSON.stringify(b)).digest('hex')},data:['create','update','remove','reply_vote'].includes(b.operation)?{reply:r}:{}}};}
function reportResponse(request){
 const url=new URL(request.url),meta={...metadata(request),source:'persisted-native-discussion-report'};
 if(request.method==='POST')return{status:200,body:{result:'ok',nostr:{...meta,requestMethod:'POST',requestPath:url.pathname,requestHash:createHash('sha256').update(JSON.stringify(request.body)).digest('hex')},data:{report:{id:'ActualNativeReport',status:'waiting'}}}};
 const type=url.searchParams.get('targetType');return{status:200,body:{result:'ok',nostr:meta,data:{target:{type,threadId:meta.threadId,...(type==='reply'?{replyId:Number(url.searchParams.get('replyId')),version:Number(url.searchParams.get('expectedReplyVersion'))}:{})},reasons:[{id:type+'_actual_reason',version:4,reason:'Actual native reason',detailsRequired:true}]}}};
}
async function settle(){for(let i=0;i<12;i++){await new Promise(resolve=>setImmediate(resolve));flushSync();}}
function button(host,label){const node=[...host.querySelectorAll('button')].find(row=>row.textContent.trim()===label);assert.ok(node,'button '+label+'; '+host.textContent);return node;}
async function click(host,label){button(host,label).click();await settle();}
function edit(host,text){const node=host.querySelector('textarea');assert.ok(node);node.value=text;node.dispatchEvent(new Event('input',{bubbles:true}));flushSync();}
const posts=f=>f.signedCalls.filter(request=>request.method==='POST');
async function ui(options={}){
 const surface=options.surface??'manga',kind=options.kind??(surface==='forum'?'chapter':surface==='direct'?'manga':surface),signedCalls=[],contextCalls=[];
 const identity=writable({owner:options.connected===false?null:'a'.repeat(64),generation:1});
 const nativeAuthority=writable({actor:'BrowserB',epoch:0}),Session=writable({identity:{id:'BrowserB'}});
 const f=signedFixture({permissions:options.permissions??['reply.create','self.reply.update','self.reply.remove'],handler:async request=>{
  if(!/^\/api\/v1\/nostr\/(manga|chapter|group)\/[^/]+\/discussion(?:\/reports(?:\/context)?)?$/.test(new URL(request.url).pathname))return;
  signedCalls.push(request);const handled=await options.signedHandler?.(request);if(handled!==undefined)return handled;
  if(new URL(request.url).pathname.includes('/reports'))return reportResponse(request);
  if(request.method==='POST')return receipt(request);
  const page=Number(new URL(request.url).searchParams.get('page')),r=reply(page);if(options.locked)r.relationships.vote_state=1;
  return{status:200,body:{result:'ok',nostr:metadata(request),data:{thread:{id:metadata(request).threadId,locked:!!options.locked},threadVote:options.locked?1:0,feed:{total:21,limit:20,page,data:[r]},actions:{create:!options.locked,threadVote:true,replies:{[r.id]:{update:!options.locked,remove:true,delete:true,vote:true}}}}}};
 }});
 const initialResource=resource(id,options.missingThread?undefined:nativeThread());if(options.missingThread)initialResource.relationships.thread=undefined;
 const initialUrl=options.url??'https://catalog.example/'+(surface==='forum'?'forums/threads/'+threadId:surface==='group'?'group/'+id+'/comments':surface==='chapter'?'chapter/'+id:'title/'+id);
 const env={surface,kind,initialResource,initialUrl,group:writable(initialResource),modules:{},browserMounts:0,nativeWrites:[]};
 const page={get url(){return env.currentUrl?.()??new URL(initialUrl);},data:{},state:{}};
 const readTarget=readerModule.createNativeDiscussionTargetReader({origin:()=>page.url.origin,fetch:async(url,request)=>{contextCalls.push({url,request});await options.contextGate?.promise;return new Response(JSON.stringify(options.contextBody??targetBody(kind)),{status:options.contextStatus??200,headers:{'content-type':'application/json'}});}});
 const reader={get chapter(){return env.resource();},prefs:writable({layout:'Scroll',panel_position:'left',panel:false,gap:0}),pages:[],isViewingLastPage:true,isShowComments:true};
 Object.assign(env.modules,{
  '$app/state':{page},'$app/navigation':{goto:async value=>{window.history.pushState({},'',value);env.setLocation(new URL(value,initialUrl).href);},replaceState:()=>{}},
  '$lib':{Utils,Typography,Constants:{Reader:{Layout:{Scroll:'Scroll'},CommentsVisibility:{Always:'Always',Collapsed:'Collapsed'}}}},
  '$lib/stores':{Session,commentsVisibility:writable('Always'),isMobile:{current:false},skipEndOfChapterPage:writable(false)},
  '$lib/Context':{getManga:()=>()=>env.resource(),getGroup:()=>env.group},
  '$lib/components/comments':{Comments:BrowserComments,Provider:NativeProvider},
  '$lib/components/comments/context.svelte':context,'./context.svelte':context,
  '$platform/mutation-authority':{nativeAuthority},
  '../../local-library/IdentityGate.svelte':{default:IdentityGate},
  '../../local-library/publication':{signerIdentity:identity},
  './context':contextModule,'./reader':{readNativeDiscussionTarget:readTarget},
  './signed-client':{createSignedNativeDiscussion:(kind,target,thread,check,_deps,signal)=>signedClient.createSignedNativeDiscussion(kind,target,thread,check,f.deps,signal)},
  './SignedDiscussion.svelte':{default:SignedDiscussion},'./UnavailableContext.svelte':{default:UnavailableContext},
  '$publicApi/native-forum':{Provider,Comments:options.historical?HistoricalDiscussion:options.ordinary?OrdinaryDiscussion:Discussion},'../public-api/native-forum':{Provider,Comments:options.historical?HistoricalDiscussion:options.ordinary?OrdinaryDiscussion:Discussion},
  '../public-api/library-staging':{publicUuidLibraryEnabled:options.enabled!==false},
  './ProtocolIdentityPanel.svelte':{default:Empty},'$localLibrary/ProtocolIdentityPanel.svelte':{default:Empty},
  '$env/static/public':{PUBLIC_SITE_NAME:'Synthetic catalogue'},
  '$lib/components/reader/reader-panel.svelte':{default:Empty},'$lib/components/ui/sidebar':{Outer:Generic,useSidebar:()=>({open:true,openMobile:true})},
  '../ui/button/button.svelte':{default:Button},'./reader-indicator.svelte':{default:Empty},'./reader-last-page.svelte':{default:Empty},'./reader-pages.svelte':{default:Empty},'./reader-pagination.svelte':{default:Empty},
  './Thread.svelte':{default:Empty},'$lib/components/ui/NotFound.svelte':{default:Empty}
 });
 // Reader's relative context has a distinct import meaning from the Provider.
 // Supply both named APIs without replacing either production implementation.
 env.modules['./context.svelte']={...context,useReader:()=>reader};
 globalThis.__nativeDiscussionEnvironment=env;window.history.replaceState({},'',initialUrl);
 const host=document.createElement('main');document.body.append(host);const instance=mount(Harness,{target:host});await settle();
 return{...f,env,page,host,identity,nativeAuthority,Session,signedCalls,contextCalls,async signed(){await click(host,'Public identity');},async dispose(){await unmount(instance);host.remove();}};
}

test('cookie account B and signer account A require independent explicit selections',async()=>{
 const f=await ui();try{
  assert.equal(f.env.browserMounts,0);assert.equal(f.signedCalls.length,0);assert.equal(f.host.querySelector('textarea'),null);
  await click(f.host,'Email account');assert.match(f.host.textContent,/Browser account BrowserB/);await click(f.host,'Browser comment action');assert.deepEqual(f.env.nativeWrites,[{account:'BrowserB',operation:'create'}]);assert.equal(f.signedCalls.length,0);
  await f.signed();assert.equal(f.host.querySelector('[data-browser-comments]'),null);assert.match(f.host.textContent,/Signed discussion as Actual bound user/);assert.match(f.host.textContent,new RegExp(actor));assert.equal(f.env.nativeWrites.length,1);assert.equal(f.account.user.id,actor);
 }finally{await f.dispose();}
});
for(const surface of ['manga','chapter','group'])test('actual '+surface+' surface passes its public UUID and actual stored native thread',async()=>{
 const f=await ui({surface});try{await f.signed();const element=f.host.querySelector('[data-signed-discussion-kind]');assert.equal(element.dataset.signedDiscussionKind,surface);assert.equal(element.dataset.signedDiscussionTarget,id);assert.equal(element.dataset.threadId,threadId);assert.equal(f.contextCalls.length,0);assert.ok(f.signedCalls.every(request=>new URL(request.url).pathname==='/api/v1/nostr/'+surface+'/'+id+'/discussion'));assert.equal(f.env.nativeWrites.length,0);}finally{await f.dispose();}
});
test('actual forum route ignores native relation IDs and requires persisted target provenance',async()=>{
 const f=await ui({surface:'forum',kind:'chapter'});try{assert.equal(f.contextCalls.length,1);assert.equal(f.contextCalls[0].request.credentials,'omit');assert.equal(f.signedCalls.length,0);await f.signed();assert.equal(f.host.querySelector('[data-signed-discussion-kind]').dataset.signedDiscussionKind,'chapter');assert.equal(f.host.querySelector('[data-signed-discussion-target]').dataset.signedDiscussionTarget,id);assert.doesNotMatch(f.signedCalls[0].url,/NOT-A-PUBLIC-TARGET/);}finally{await f.dispose();}
});
for(const failure of ['missing','unavailable','wrong-kind','wrong-thread','wrong-source'])test('forum '+failure+' never guesses a signed target and retains explicit browser choice',async()=>{
 const body=targetBody('manga');if(failure==='wrong-kind')body.data.kind='user';if(failure==='wrong-thread')body.data.threadId='different';if(failure==='wrong-source')body.nostr.source='guessed';
 const f=await ui({surface:'forum',contextBody:body,contextStatus:failure==='missing'?404:failure==='unavailable'?503:200});try{assert.equal(button(f.host,'Public identity').disabled,true);assert.equal(f.signedCalls.length,0);await click(f.host,'Email account');assert.ok(f.host.querySelector('[data-browser-comments]'));if(failure==='missing')assert.match(f.host.textContent,/No verified manga/);else assert.ok(f.host.querySelector('[role=alert]'));}finally{await f.dispose();}
});
test('rich comments and exact normalized review render safely before one create dispatch',async()=>{
 const f=await ui();try{await f.signed();assert.ok(f.host.querySelector('article strong'));assert.ok(f.host.querySelector('article a[href="https://example.test/"]'));assert.equal(f.host.querySelector('article script'),null);assert.equal(window.__unsafe,undefined);const spoiler=f.host.querySelector('article [data-spoiler]');assert.ok(spoiler);spoiler.click();assert.ok(spoiler.hasAttribute('data-visible'));
  edit(f.host,'  Reviewed **body**  ');await click(f.host,'Review comment');const review=f.host.querySelector('[aria-label="Review signed discussion action"]');assert.ok(review.querySelector('strong'));assert.match(review.textContent,/Actual bound user/);assert.equal(posts(f).length,0);
  const confirm=button(f.host,'Confirm signed action');confirm.click();confirm.click();await settle();assert.equal(posts(f).length,1);assert.deepEqual(plain(posts(f)[0].body),{expectedAccountId:actor,expectedTargetId:id,expectedThreadId:threadId,operation:'create',body:'Reviewed **body**'});assert.equal(f.host.querySelector('textarea').value,'');assert.equal(f.env.nativeWrites.length,0);
 }finally{await f.dispose();}
});
for(const label of ['Reply to #1','Edit #1','Remove #1','Permanently delete #1','Upvote #1','Downvote #1','Upvote discussion','Downvote discussion'])test('reviewed native '+label+' preserves exact reply version, parent or expected vote',async()=>{
 const f=await ui({surface:'group'});try{await f.signed();await click(f.host,label);if(label.startsWith('Reply')||label.startsWith('Edit')){edit(f.host,'Reviewed change');await click(f.host,'Review comment');}const review=f.host.querySelector('[aria-label="Review signed discussion action"]');assert.ok(review);assert.equal(posts(f).length,0);await click(f.host,'Confirm signed action');assert.equal(posts(f).length,1);const body=posts(f)[0].body;if(label.startsWith('Reply'))assert.equal(body.parentId,7);else if(!label.includes('discussion')){assert.equal(body.replyId,7);assert.equal(body.version,3);}if(/vote/.test(label)){assert.equal(body.expectedVote,0);assert.equal(body.vote,label.startsWith('Downvote')?-1:1);}assert.equal(f.env.nativeWrites.length,0);}finally{await f.dispose();}
});
test('gate-off title retains the original native component without signed selection or reads',async()=>{const f=await ui({enabled:false});try{assert.ok(f.host.querySelector('[data-browser-comments]'));assert.equal(f.host.querySelector('[aria-label="Discussion account"]'),null);assert.equal(f.signedCalls.length,0);assert.equal(f.contextCalls.length,0);}finally{await f.dispose();}});
test('missing stored thread never mounts cookie or signed discussion',async()=>{const f=await ui({missingThread:true});try{assert.match(f.host.textContent,/Comments are unavailable/);assert.equal(f.env.browserMounts,0);assert.equal(f.signedCalls.length,0);assert.equal(f.contextCalls.length,0);}finally{await f.dispose();}});
test('unconnected signer cannot be silently replaced by the browser account',async()=>{const f=await ui({connected:false});try{assert.equal(button(f.host,'Public identity').disabled,true);assert.equal(f.signedCalls.length,0);assert.equal(f.env.browserMounts,0);assert.ok(f.host.querySelector('[data-identity-gate]'));}finally{await f.dispose();}});
test('URL pages and order are bounded and browser-style Back clears stale review',async()=>{
 const f=await ui({url:'https://catalog.example/title/'+id+'?page=2&sort=oldest'});try{await f.signed();assert.match(f.signedCalls.at(-1).url,/page=2&order=asc$/);await click(f.host,'Previous comments');assert.equal(f.page.url.search,'?sort=oldest');await click(f.host,'Latest');assert.equal(f.page.url.search,'');edit(f.host,'Must not survive navigation');await click(f.host,'Review comment');f.env.setLocation('https://catalog.example/title/'+id+'?page=2&sort=oldest');await settle();assert.equal(f.host.querySelector('[aria-label="Review signed discussion action"]'),null);assert.equal(posts(f).length,0);assert.match(f.signedCalls.at(-1).url,/page=2&order=asc$/);}finally{await f.dispose();}
});
for(const query of ['?page=0','?page=1000001','?page=2&page=3','?sort=unknown','?sort=latest&sort=oldest'])test('malformed page controls do not initialize signed requests '+query,async()=>{const f=await ui({url:'https://catalog.example/title/'+id+query});try{await f.signed();assert.match(f.host.textContent,/Invalid discussion page or sort/);assert.equal(f.signedCalls.length,0);assert.equal(f.calls.length,0);}finally{await f.dispose();}});
for(const change of ['signer','target','cookie-authority'])test(change+' change clears review without automatic signed or cookie re-selection',async()=>{
 const f=await ui();try{await f.signed();edit(f.host,'Do not move this draft');await click(f.host,'Review comment');const reads=f.signedCalls.length;if(change==='signer')f.identity.set({owner:'b'.repeat(64),generation:2});if(change==='target')f.env.setResource(resource(other));if(change==='cookie-authority')f.nativeAuthority.set({actor:'BrowserC',epoch:1});await settle();assert.equal(f.host.querySelector('textarea'),null);assert.equal(f.host.querySelector('[aria-label="Review signed discussion action"]'),null);assert.equal(posts(f).length,0);assert.equal(f.signedCalls.length,reads);assert.equal(f.env.browserMounts,0);assert.equal(f.host.querySelector('[data-native-discussion-mode]').dataset.nativeDiscussionMode,'unselected');}finally{await f.dispose();}
});
test('same signer rebind between review and confirmation cannot dispatch to a new account',async()=>{const f=await ui();try{await f.signed();edit(f.host,'Reviewed as account A');await click(f.host,'Review comment');f.account.user.id=other;await click(f.host,'Confirm signed action');assert.equal(posts(f).length,0);assert.equal(f.host.querySelector('textarea'),null);assert.match(f.host.textContent,/another account/);assert.equal(f.env.nativeWrites.length,0);}finally{await f.dispose();}});
test('unknown write outcome stays fenced across reload and explicit mode changes',async()=>{
 const f=await ui({signedHandler:request=>{if(request.method==='POST')throw new RequestFailure('Receipt lost','uncertain');}});try{await f.signed();edit(f.host,'May have committed');await click(f.host,'Review comment');await click(f.host,'Confirm signed action');assert.match(f.host.textContent,/unknown outcome/);await click(f.host,'Reload signed comments');assert.equal(f.host.querySelector('textarea'),null);await click(f.host,'Email account');await f.signed();assert.equal(f.host.querySelector('textarea'),null);assert.equal(posts(f).length,1);assert.equal(f.fences.size,1);assert.equal(f.env.nativeWrites.length,0);}finally{await f.dispose();}
});
test('Cancel while POST is pending preserves unknown-outcome fence without replay',async()=>{
 const gate=deferred(),f=await ui({signedHandler:async request=>{if(request.method==='POST'){await gate.promise;return receipt(request);}}});try{await f.signed();edit(f.host,'Sent once');await click(f.host,'Review comment');button(f.host,'Confirm signed action').click();await settle();assert.equal(posts(f).length,1);await click(f.host,'Cancel discussion request');assert.equal(posts(f)[0].signal.aborted,true);gate.release();await settle();assert.equal(f.fences.size,1);await click(f.host,'Reload signed comments');assert.equal(f.host.querySelector('textarea'),null);assert.equal(posts(f).length,1);assert.match(f.host.textContent,/unknown outcome/);}finally{gate.release();await f.dispose();}
});
test('navigation during delayed signed read discards old page and aborts its request',async()=>{
 const gate=deferred();let first=true;const f=await ui({signedHandler:async request=>{if(request.method==='GET'&&first){first=false;await gate.promise;}}});try{button(f.host,'Public identity').click();await settle();const old=f.signedCalls[0];f.env.setLocation('https://catalog.example/title/'+id+'?page=2');await settle();assert.equal(old.signal.aborted,true);gate.release();await settle();assert.match(f.host.textContent,/Page 2/);assert.equal(f.host.querySelector('article').id,'p27');assert.equal(posts(f).length,0);}finally{gate.release();await f.dispose();}
});
test('unmount cancels delayed forum context before any signer or cookie request',async()=>{const gate=deferred(),f=await ui({surface:'forum',contextGate:gate});assert.equal(f.contextCalls.length,1);await f.dispose();assert.equal(f.contextCalls[0].request.signal.aborted,true);gate.release();await settle();assert.equal(f.signedCalls.length,0);assert.equal(f.env.browserMounts,0);});
test('locked discussion exposes only existing-vote clear and no compose or new vote',async()=>{const f=await ui({locked:true});try{await f.signed();assert.equal(f.host.querySelector('textarea'),null);assert.ok(button(f.host,'Remove discussion upvote'));assert.ok(button(f.host,'Remove upvote #1'));assert.equal([...f.host.querySelectorAll('button')].some(row=>row.textContent.trim()==='Downvote #1'),false);await click(f.host,'Remove upvote #1');await click(f.host,'Confirm signed action');assert.equal(posts(f)[0].body.vote,0);assert.equal(posts(f)[0].body.expectedVote,1);}finally{await f.dispose();}});

for(const surface of ['manga','chapter','group','forum'])test('negative control: historical wrapper on '+surface+' cannot satisfy signed selection or review',async()=>{
 const f=await ui({surface,historical:true});try{
  assert.ok(f.host.querySelector('[data-browser-comments]'),'Historical widget renders only native cookie comments');
  const hasSignedButton=[...f.host.querySelectorAll('button')].some(row=>row.textContent.trim()==='Public identity');
  assert.equal(hasSignedButton,false);assert.throws(()=>assert.equal(hasSignedButton,true),{code:'ERR_ASSERTION'},'Current signed-mode acceptance must reject the historical wrapper');
  assert.equal(f.host.querySelector('[aria-label="Review signed discussion action"]'),null);assert.equal(f.host.querySelector('[data-signed-discussion-kind]'),null);assert.equal(f.signedCalls.length,0);assert.equal(f.contextCalls.length,0);
 }finally{await f.dispose();}
});

async function reportReview(f,reply=false){
 await click(f.host,reply?'Report comment #1':'Report discussion');
 const select=f.host.querySelector('[aria-label="Report reason"]');assert.ok(select);select.value=(reply?'reply':'thread')+'_actual_reason';select.dispatchEvent(new Event('change',{bubbles:true}));
 const details=f.host.querySelector('[aria-label="Report details"]');details.value='Reviewed private explanation';details.dispatchEvent(new Event('input',{bubbles:true}));flushSync();await click(f.host,'Review report');
}
for(const surface of ['manga','chapter','group','forum'])for(const replyTarget of [false,true])test('mounted '+surface+' native '+(replyTarget?'reply':'thread')+' report reviews selected actor, stored target and reason before exact submit',async()=>{
 const f=await ui({surface,permissions:['report.create']});try{
  await f.signed();await reportReview(f,replyTarget);
  assert.equal(posts(f).length,0);assert.match(f.host.querySelector('[aria-label="Native discussion report"]').textContent,/Actual bound user/);assert.match(f.host.textContent,/Actual native reason/);if(replyTarget)assert.match(f.host.textContent,/reporting this reply/);
  await click(f.host,'Confirm private report');assert.equal(posts(f).length,1);const request=posts(f)[0];assert.equal(request.body.expectedAccountId,actor);assert.equal(request.body.expectedTargetId,id);assert.equal(request.body.expectedUserId,undefined);assert.equal(request.body.expectedThreadId,threadId);assert.equal(request.body.expectedReasonVersion,4);assert.equal(request.body.targetType,replyTarget?'reply':'thread');assert.equal(request.body.details,'Reviewed private explanation');assert.match(request.url,/\/discussion\/reports$/);assert.equal(f.env.nativeWrites.length,0);assert.match(f.host.textContent,/sent privately/);assert.equal(f.host.querySelector('[aria-label="Native discussion report"]'),null);
 }finally{await f.dispose();}
});
test('mounted generic reporting has no permission inference and cancellation sends nothing',async()=>{for(const permissions of [[],['report.create']]){const f=await ui({permissions});try{await f.signed();if(!permissions.length)assert.equal([...f.host.querySelectorAll('button')].some(row=>row.textContent==='Report discussion'),false);else{await reportReview(f);await click(f.host,'Cancel report');assert.equal(f.host.querySelector('[aria-label="Native discussion report"]'),null);}assert.equal(posts(f).length,0);assert.equal(f.env.nativeWrites.length,0);}finally{await f.dispose();}}});
for(const change of ['page','signer','target','binding','browser-mode'])test('mounted native report review cannot survive '+change,async()=>{
 const f=await ui({permissions:['report.create']});try{await f.signed();await reportReview(f,true);
  if(change==='page'){f.env.setLocation('https://catalog.example/title/'+id+'?page=2');await settle();}
  if(change==='signer'){f.identity.set({owner:'b'.repeat(64),generation:2});await settle();}
  if(change==='target'){f.env.setResource(resource(other));await settle();}
  if(change==='browser-mode')await click(f.host,'Email account');
  if(change==='binding'){f.account.user.id=other;await click(f.host,'Confirm private report');}
  assert.equal(f.host.querySelector('[aria-label="Review native discussion report"]'),null);assert.equal(posts(f).length,0);assert.equal(f.env.nativeWrites.length,0);
 }finally{await f.dispose();}
});
for(const mode of ['uncertain','cancel'])test('mounted '+mode+' native report stays fenced across reload and mode changes',async()=>{
 const gate=deferred(),f=await ui({permissions:['report.create','reply.create'],signedHandler:async request=>{if(request.method==='POST'){if(mode==='uncertain')throw new RequestFailure('Report receipt lost','uncertain');await gate.promise;return reportResponse(request);}}});try{
  await f.signed();await reportReview(f);button(f.host,'Confirm private report').click();await settle();assert.equal(posts(f).length,1);
  if(mode==='cancel'){await click(f.host,'Cancel discussion request');assert.equal(posts(f)[0].signal.aborted,true);gate.release();await settle();}
  await click(f.host,'Reload signed comments');assert.equal(f.fences.size,1);assert.match(f.host.textContent,/report has an unknown result/);assert.equal(f.host.querySelector('textarea'),null);assert.equal(button(f.host,'Report discussion').disabled,true);assert.equal(f.host.querySelector('a[href="/reports/signed/native"]')?.textContent,'View your discussion reports');
  await click(f.host,'Email account');await f.signed();assert.equal(f.fences.size,1);assert.equal(button(f.host,'Report discussion').disabled,true);assert.equal(posts(f).length,1);assert.equal(f.env.nativeWrites.length,0);
 }finally{gate.release();await f.dispose();}
});
test('delayed native report reasons discard navigation and unmount without a write',async()=>{const gate=deferred(),f=await ui({permissions:['report.create'],signedHandler:async request=>{if(request.url.includes('/reports/context')){await gate.promise;return reportResponse(request);}}});try{await f.signed();button(f.host,'Report discussion').click();await settle();const request=f.signedCalls.at(-1);f.env.setLocation('https://catalog.example/title/'+id+'?page=2');await settle();assert.equal(request.signal.aborted,true);gate.release();await settle();assert.equal(f.host.querySelector('[aria-label="Native discussion report"]'),null);assert.equal(posts(f).length,0);}finally{gate.release();await f.dispose();}});

for(const surface of ['manga','chapter','group','forum'])test('ordinary '+surface+' comments open immediately with the regular account, without signer activity',async()=>{const f=await ui({surface,ordinary:true});try{assert.ok(f.host.querySelector('[data-browser-comments]'));assert.equal(f.env.browserMounts,1);assert.equal(f.host.querySelector('[aria-label="Discussion account"]'),null);assert.equal(f.host.querySelector('[data-identity-gate]'),null);assert.equal(f.signedCalls.length,0);assert.equal(f.contextCalls.length,0);assert.equal(f.env.nativeWrites.length,0);await click(f.host,'Browser comment action');assert.deepEqual(f.env.nativeWrites,[{account:'BrowserB',operation:'create'}]);assert.equal(f.signedCalls.length,0);}finally{await f.dispose();}});
