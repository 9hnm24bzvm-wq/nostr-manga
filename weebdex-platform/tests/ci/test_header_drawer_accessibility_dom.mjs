// Fully composed header/drawer plus real Button, Dialog.Trigger wrapper and
// Sidebar action leaves. Dialog/sidebar state and outer primitives are finite
// adapters; this proves rendered name/tooltip forwarding and unchanged callbacks,
// not native-browser focus, layout or authorization acceptance.
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import {createRequire} from 'node:module';
import {fileURLToPath,pathToFileURL} from 'node:url';
import {createHash} from 'node:crypto';
import test,{after} from 'node:test';
import assert from 'node:assert/strict';
import {composed} from './composed-account-library-fixture.mjs';
import {integrationPatches} from '../../platform-integrations/six-persona/integration-patches.mjs';
import {sixPersonaOverlayManifest,restoreSixPersonaOutput} from '../../platform-integrations/six-persona/overlays.mjs';

const root=fileURLToPath(new URL('../../',import.meta.url));
const deps=process.env.WEEBDEX_FRONTEND_ROOT,dom=process.env.WEEBDEX_BROWSER_DOM_NODE_MODULES;
assert(deps&&dom,'Use the qualified dependency roots');
const require=createRequire(path.join(deps,'package.json')),{compile}=require('svelte/compiler'),ts=require('typescript');
assert.equal(require('svelte/package.json').version,'5.55.1');
assert.equal(require(path.join(dom,'jsdom/package.json')).version,'27.0.1');
const {window}=new(require(path.join(dom,'jsdom')).JSDOM)('<main></main>',{url:'https://header.fixture/search',pretendToBeVisual:true});
for(const key of ['window','document','Node','Element','HTMLElement','HTMLInputElement','HTMLButtonElement','HTMLMediaElement','Text','Comment','Event','KeyboardEvent','MouseEvent','CustomEvent','MutationObserver','requestAnimationFrame','cancelAnimationFrame','getComputedStyle'])globalThis[key]=window[key];
Object.defineProperty(globalThis,'navigator',{configurable:true,value:window.navigator});
window.matchMedia=()=>({matches:false,addEventListener(){},removeEventListener(){}});
const svelte=pathToFileURL(path.join(deps,'node_modules/svelte/src/index-client.js')).href;
const store=pathToFileURL(path.join(deps,'node_modules/svelte/src/store/index-client.js')).href;
const {mount,unmount,flushSync}=await import(svelte),{writable,get}=await import(store);
const directory=fs.mkdtempSync(path.join(os.tmpdir(),'header-drawer-accessibility-'));
fs.symlinkSync(path.join(deps,'node_modules'),path.join(directory,'node_modules'));
const env={modules:{'$lib/Utils':{cn:(...values)=>values.filter(Boolean).join(' ')},'tailwind-variants':require('tailwind-variants'),'$platform/install-prompt':{isInstallPromptEvent:()=>false,isStandaloneNavigator:()=>false}}};
globalThis.__headerDrawerAccessibility=env;
after(()=>{window.close();fs.rmSync(directory,{recursive:true,force:true});delete globalThis.__headerDrawerAccessibility;});
function rewrite(script,name){
 const ast=ts.createSourceFile(name+'.ts',script,ts.ScriptTarget.Latest,true,ts.ScriptKind.TS);
 for(const row of [...ast.statements].reverse()){
  if(!ts.isImportDeclaration(row))continue;
  const spec=row.moduleSpecifier.text,clause=row.importClause;let replacement='';
  if(spec==='svelte'||spec==='svelte/store')replacement=row.getText(ast).replace(/(['"])svelte(?:\/store)?\1/,JSON.stringify(spec==='svelte'?svelte:store));
  else if(clause&&!clause.isTypeOnly){
   const ref=`globalThis.__headerDrawerAccessibility.modules[${JSON.stringify(spec)}]`;
   if(clause.name)replacement+=`const ${clause.name.text}=${ref}.default;`;
   if(clause.namedBindings&&ts.isNamespaceImport(clause.namedBindings))replacement+=`const ${clause.namedBindings.name.text}=${ref};`;
   else if(clause.namedBindings)for(const item of clause.namedBindings.elements)if(!item.isTypeOnly)replacement+=`const ${item.name.text}=${ref}[${JSON.stringify(item.propertyName?.text??item.name.text)}];`;
  }
  script=script.slice(0,row.getStart(ast))+replacement+script.slice(row.end);
 }
 return script;
}
async function component(name,source){
 source=source.replace(/(<script[^>]*>)([\s\S]*?)(<\/script>)/g,(_,start,script,end)=>start+rewrite(script,name)+end);
 const result=compile(source,{filename:name+'.svelte',generate:'client',runes:true});
 assert.deepEqual(result.warnings,[],name+' compiler warnings');
 const file=path.join(directory,name+'.mjs');fs.writeFileSync(file,result.js.code);return import(pathToFileURL(file));
}
const leaf=async(name,relative)=>(await component(name,await composed(relative))).default;
const Icon=(await component('Icon','<script>let{...rest}=$props();</script><svg aria-hidden="true" {...rest}></svg>')).default;
const Generic=(await component('Generic','<script>let{children,child,...rest}=$props();</script>{#if child}{@render child({props:rest})}{:else}<div>{@render children?.()}</div>{/if}')).default;
const Hidden=(await component('Hidden','<script>let{children,...rest}=$props();</script><div hidden {...rest}>{@render children?.()}</div>')).default;
const DialogRoot=(await component('DialogRoot','<script>import{setContext}from"svelte";let{open=$bindable(false),children}=$props();setContext("header-dialog",{get open(){return open},set open(value){open=value}});</script>{@render children?.()}')).default;
const DialogTriggerPrimitive=(await component('DialogTriggerPrimitive','<script>import{getContext}from"svelte";let{ref=$bindable(null),children,...rest}=$props();const context=getContext("header-dialog");</script><button bind:this={ref} type="button" {...rest} onclick={()=>context.open=true}>{@render children?.()}</button>')).default;
const DialogContent=(await component('DialogContent','<script>import{getContext}from"svelte";let{children}=$props();const context=getContext("header-dialog");</script>{#if context.open}<div role="dialog" aria-label="Search">{@render children?.()}</div>{/if}')).default;
const SearchDialog=(await component('SearchDialog','<label>Find titles, authors, and groups<input aria-label="Search query"/></label>')).default;
env.modules['bits-ui']={Dialog:{Trigger:DialogTriggerPrimitive}};
const buttonModule=await component('RealButton',await composed('src/lib/components/ui/button/button.svelte'));
const MenuAction=await leaf('RealMenuAction','src/lib/components/ui/sidebar/sidebar-menu-action.svelte');
const GroupAction=await leaf('RealGroupAction','src/lib/components/ui/sidebar/sidebar-group-action.svelte');
const DialogTrigger=await leaf('RealDialogTrigger','src/lib/components/ui/dialog/dialog-trigger.svelte');
const headerPath='src/lib/components/AppHeader.svelte',sidebarPath='src/lib/components/AppSidebar.svelte';
const current=new Map(await Promise.all([headerPath,sidebarPath].map(async relative=>[relative,await composed(relative)])));
const labels=new Set(['compact-and-wide-navigation-toggle-accessible-name','compact-search-trigger-accessible-name','drawer-action-title-and-name-match-current-destination']);
const patches=integrationPatches.filter(row=>labels.has(row.label));assert.equal(patches.length,3);
function predecessor(relative){let value=current.get(relative);for(const row of patches.filter(row=>row.path===relative).reverse()){assert.equal(value.split(row.after).length-1,row.count);value=value.split(row.after).join(row.before);}return value;}
const Header=(await component('Header',current.get(headerPath))).default,PreviousHeader=(await component('PreviousHeader',predecessor(headerPath))).default;
const Drawer=(await component('Drawer',current.get(sidebarPath))).default,PreviousDrawer=(await component('PreviousDrawer',predecessor(sidebarPath))).default;
function setup({mobile=true,signedIn=true,library=true}={}){
 const calls={toggle:0,close:0,open:0};
 const sidebar={isMobile:mobile,open:false,openMobile:false,toggle(){calls.toggle++;sidebar.openMobile=!sidebar.openMobile;},setOpenMobile(value){calls.close++;sidebar.openMobile=value;},setOpen(value){calls.open++;sidebar.open=value;}};
 const Session=writable(signedIn?{identity:{id:'SyntheticReader'},roles:['user'],permissions:[]}:undefined),libraryIdentity=writable({canUse:library}),isShowSearchDialog=writable(false),isShowSidebar=writable(true);
 Object.assign(env.modules,{'$app/state':{page:{url:new URL('https://header.fixture/search')}},'$lib/components/SearchDialog.svelte':{default:SearchDialog},'$lib/components/ui/button':{Button:buttonModule.default,buttonVariants:buttonModule.buttonVariants},'$lib/components/ui/dialog':{Root:DialogRoot,Trigger:DialogTrigger,Content:DialogContent},'$lib/components/ui/sidebar':{useSidebar:()=>sidebar,Root:Generic,Content:Generic,Group:Generic,GroupLabel:Generic,GroupAction,GroupContent:Generic,Menu:Generic,MenuItem:Generic,MenuButton:Generic,MenuAction,Separator:Generic,Footer:Generic},'$lib/components/ui/tooltip':{Provider:Generic,Root:Generic,Trigger:Generic,Content:Hidden},'$lib/stores':{isMobile:{current:mobile},isShowSearchDialog,isShowSidebar,Session},'$localLibrary/library':{libraryIdentity},'@lucide/svelte':new Proxy({},{get:()=>Icon})});
 return{calls,sidebar,Session,libraryIdentity,isShowSearchDialog,isShowSidebar};
}
async function settle(){for(let i=0;i<8;i++){await new Promise(resolve=>setImmediate(resolve));flushSync();}}
async function render(Component,options){const state=setup(options),host=document.createElement('section');document.querySelector('main').append(host);const instance=mount(Component,{target:host});await settle();return{...state,host,async close(){await unmount(instance);host.remove();await settle();}};}
const name=node=>node.getAttribute('aria-label')??node.textContent.trim();
const hash=value=>createHash('sha256').update(value).digest('hex');

test('exact final composition contains only three reversible markup changes with identical scripts',()=>{
 for(const relative of [headerPath,sidebarPath]){
  const next=current.get(relative),before=predecessor(relative),row=sixPersonaOverlayManifest.find(row=>row.path===relative);
  assert.equal(hash(next),row.output_sha256);
  assert.deepEqual(next.match(/<script[^>]*>[\s\S]*?<\/script>/g),before.match(/<script[^>]*>[\s\S]*?<\/script>/g));
  assert.equal(hash(fs.readFileSync(path.join(root,'frontend',relative))),row.original_sha256);
  assert.equal(typeof restoreSixPersonaOutput(next,relative),'string');
  for(const patch of patches.filter(row=>row.path===relative)){assert.equal(before.split(patch.before).length-1,patch.count);assert(!patch.before.includes('<script'));}
 }
});
test('counterfactual compact header renders the two unnamed icon-only buttons',async()=>{const f=await render(PreviousHeader);try{const buttons=[...f.host.querySelectorAll('button')];assert.equal(buttons.length,2);assert.deepEqual(buttons.map(name),['','']);}finally{await f.close();}});
for(const mobile of [true,false])test((mobile?'compact':'wide')+' navigation button forwards its name and keeps the original toggle callback',async()=>{const f=await render(Header,{mobile});try{const button=f.host.querySelector('button[aria-label="Toggle navigation"]');assert(button);assert.equal(name(button),'Toggle navigation');button.click();button.click();await settle();assert.equal(f.calls.toggle,2);assert.equal(f.sidebar.openMobile,false);assert.equal(f.host.querySelector('a[href="/"]').textContent,'WeebDex');}finally{await f.close();}});
test('compact named search trigger forwards through real wrapper and opens the unchanged bound search state',async()=>{const f=await render(Header);try{const trigger=f.host.querySelector('button[aria-label="Search titles, authors, and groups"]');assert(trigger);assert.equal(trigger.getAttribute('data-slot'),'dialog-trigger');assert.equal(get(f.isShowSearchDialog),false);trigger.click();await settle();assert.equal(get(f.isShowSearchDialog),true);assert.equal(f.host.querySelectorAll('[role=dialog]').length,1);assert(f.host.querySelector('input[aria-label="Search query"]'));}finally{await f.close();}});
test('desktop search keeps its visible search and keyboard hints rather than gaining a mismatched label',async()=>{const f=await render(Header,{mobile:false});try{const trigger=f.host.querySelector('[data-slot="dialog-trigger"]');assert.equal(trigger.getAttribute('aria-label'),null);assert.match(name(trigger),/Search\.\.\./);assert.match(name(trigger),/CTRL K/);trigger.click();await settle();assert.equal(get(f.isShowSearchDialog),true);}finally{await f.close();}});
test('existing Ctrl-K behavior and input exclusion survive; unmount removes the keyboard listener',async()=>{const f=await render(Header);try{const input=document.createElement('input');f.host.append(input);input.dispatchEvent(new KeyboardEvent('keydown',{bubbles:true,ctrlKey:true,code:'KeyK',cancelable:true}));await settle();assert.equal(get(f.isShowSearchDialog),false);window.dispatchEvent(new KeyboardEvent('keydown',{ctrlKey:true,code:'KeyK',cancelable:true}));await settle();assert.equal(get(f.isShowSearchDialog),true);f.isShowSearchDialog.set(false);}finally{await f.close();}window.dispatchEvent(new KeyboardEvent('keydown',{ctrlKey:true,code:'KeyK',cancelable:true}));await settle();assert.equal(get(f.isShowSearchDialog),false);});
test('counterfactual drawer demonstrates Settings destination with wrong Create Group tooltip',async()=>{const f=await render(PreviousDrawer);try{const settings=f.host.querySelector('a[href="/settings/library"]');assert(settings);assert.equal(settings.textContent.trim(),'Settings');assert.equal(settings.getAttribute('title'),'Create Group');assert.equal(settings.getAttribute('aria-label'),null);}finally{await f.close();}});
for(const signedIn of [true,false])test((signedIn?'account':'device')+' drawer uses each action name for both accessible label and tooltip without changing destinations',async()=>{const f=await render(Drawer,{signedIn});try{const actions=[...f.host.querySelectorAll('[data-sidebar="menu-action"]')];assert.deepEqual(actions.map(a=>[a.getAttribute('href'),a.getAttribute('title'),a.getAttribute('aria-label'),a.textContent.trim()]),[['/settings/library','Settings','Settings','Settings'],['/catalogue-tools?kind=group','Create Group','Create Group','Create Group']]);const title=f.host.querySelector('[data-sidebar="group-action"]');assert.equal(title.getAttribute('href'),'/drafts/title');assert.equal(title.getAttribute('title'),'Add Title');assert.equal(title.textContent.trim(),'Add Title');assert(f.host.querySelector('a[href="/upload"]'));}finally{await f.close();}});
test('unavailable library scope does not create a phantom settings action',async()=>{const f=await render(Drawer,{signedIn:false,library:false});try{assert.equal(f.host.querySelector('a[href="/settings/library"]'),null);const actions=[...f.host.querySelectorAll('[data-sidebar="menu-action"]')];assert.equal(actions.length,1);assert.equal(name(actions[0]),'Create Group');assert.equal(actions[0].getAttribute('title'),'Create Group');}finally{await f.close();}});
test('account/library boundary removes the old action and retains the correctly labelled group action',async()=>{const f=await render(Drawer);try{assert(f.host.querySelector('a[href="/settings/library"]'));f.libraryIdentity.set({canUse:false});f.Session.set(undefined);await settle();assert.equal(f.host.querySelector('a[href="/settings/library"]'),null);assert.equal(f.host.querySelector('a[href="/user/SyntheticReader"]'),null);assert.equal(name(f.host.querySelector('[data-sidebar="menu-action"]')),'Create Group');assert(f.host.querySelector('a[href^="/auth/login"]'));}finally{await f.close();}});
