// Actual composed profile renderer; real loader/alias target helper. Finite
// catalogue transport and leaf/action components only; no live account writes.
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import {createHash} from 'node:crypto';
import {createRequire} from 'node:module';
import {pathToFileURL} from 'node:url';
import test,{after} from 'node:test';
import assert from 'node:assert/strict';
import {root,fixture as navigationFixture,current,id,other,legacy} from './native-user-navigation-fixture.mjs';
import {composed} from './composed-account-library-fixture.mjs';
import {profilePresentationManifest,applyProfilePresentationOutput,restoreProfilePresentationOutput} from '../../platform-integrations/local-library/profile-presentation-overlays.mjs';
const deps=process.env.WEEBDEX_FRONTEND_ROOT,dom=process.env.WEEBDEX_BROWSER_DOM_NODE_MODULES;
assert(deps&&dom,'Use existing locked dependency roots');
const require=createRequire(path.join(deps,'package.json')),{compile}=require('svelte/compiler'),ts=require('typescript');
assert.equal(require('svelte/package.json').version,'5.55.1');assert.equal(require(path.join(dom,'jsdom/package.json')).version,'27.0.1');
const{window}=new(require(path.join(dom,'jsdom')).JSDOM)('<main></main>',{url:'https://profile.fixture/user/'+legacy,pretendToBeVisual:true});
for(const key of ['window','document','Node','Element','HTMLElement','HTMLInputElement','HTMLButtonElement','HTMLMediaElement','Text','Comment','Event','MouseEvent','CustomEvent','MutationObserver','requestAnimationFrame','cancelAnimationFrame','getComputedStyle'])globalThis[key]=window[key];
const svelte=pathToFileURL(path.join(deps,'node_modules/svelte/src/index-client.js')).href,{mount,unmount,flushSync}=await import(svelte),{writable}=await import(pathToFileURL(path.join(deps,'node_modules/svelte/src/store/index-client.js')));
const directory=fs.mkdtempSync(path.join(os.tmpdir(),'profile-presentation-'));fs.symlinkSync(path.join(deps,'node_modules'),path.join(directory,'node_modules'));
after(()=>{window.close();fs.rmSync(directory,{recursive:true,force:true});delete globalThis.__profilePresentation;});
async function component(name,source,replace=true){
 if(replace)source=source.replace(/<script[^>]*>([\s\S]*?)<\/script>/,(_,script)=>{
  const ast=ts.createSourceFile(name+'.ts',script,ts.ScriptTarget.Latest,true,ts.ScriptKind.TS);
  for(const row of [...ast.statements].reverse()){
   if(!ts.isImportDeclaration(row))continue;const clause=row.importClause;let replacement='';
   if(row.moduleSpecifier.text==='svelte')replacement=row.getText(ast).replace(/(['"])svelte\1/,JSON.stringify(svelte));
   else if(clause&&!clause.isTypeOnly){const names=[];if(clause.name)names.push(clause.name.text);if(clause.namedBindings&&ts.isNamespaceImport(clause.namedBindings))names.push(clause.namedBindings.name.text);else if(clause.namedBindings)for(const item of clause.namedBindings.elements)if(!item.isTypeOnly)names.push(item.name.text);if(names.length)replacement=`const {${names.join(',')}}=globalThis.__profilePresentation;`;}
   script=script.slice(0,row.getStart(ast))+replacement+script.slice(row.end);
  }
  return '<script lang="ts">'+script+'</script>';
 });
 const file=path.join(directory,name+'.mjs');fs.writeFileSync(file,compile(source,{filename:name+'.svelte',generate:'client',runes:true}).js.code);return(await import(pathToFileURL(file))).default;
}
const Empty=await component('Empty','<span></span>',false),Generic=await component('Generic','<script>let{children}=$props();</script><div>{@render children?.()}</div>',false),Button=await component('Button','<script>let{children,type="button",variant,...rest}=$props();</script><button {type} {...rest}>{@render children?.()}</button>',false);
const Follow=await component('Follow','<script>let{kind,targetId,requestedId}=$props();$effect(()=>globalThis.__profilePresentation.followProps.push({kind,targetId,requestedId}));</script><button type="button">Follow</button>',false),ProfileActions=await component('ProfileActions','<script>let{targetId,requestedId}=$props();$effect(()=>globalThis.__profilePresentation.profileProps.push({targetId,requestedId}));</script>',false);
const relative='src/routes/(app)/user/[id]/User.svelte',hash=x=>createHash('sha256').update(x).digest('hex'),original=fs.readFileSync(path.join(root,'frontend',relative),'utf8'),latest=await composed(relative),manifest=profilePresentationManifest[0];
const predecessor=hash(latest)===manifest.output_sha256?restoreProfilePresentationOutput(latest,relative):latest,corrected=applyProfilePresentationOutput(predecessor,relative);
const User=await component('User',corrected),PreviousUser=await component('PreviousUser',predecessor);
const settle=async()=>{for(let i=0;i<16;i++){await Promise.resolve();flushSync();}};
async function render({name='FixtureReader',requested=legacy,old=false}={}){
 const navigation=navigationFixture({transform:(response,request)=>request.path==='/user/'+id?{...response,body:{...response.body,data:{...response.body.data,attributes:{...response.body.data.attributes,username:name}}}}:response});
 const result=await navigation.client.read('user.get',{id:legacy}),followProps=[],profileProps=[],reads=[];
 const env={profileActionRequestedId:current.profileActionRequestedId,SessionProfileActions:ProfileActions,NativeProfileActions:Empty,AccountFollow:Follow,page:{url:new URL('https://profile.fixture/user/'+requested),params:{id:requested}},PUBLIC_SITE_NAME:'Fixture',writable,Button,AspectRatio:Generic,Badge:Generic,ImageLoader:Empty,Spinner:Empty,buttonVariants:()=>'',cn:()=>'',BanIcon:Empty,FilesIcon:Empty,FlagIcon:Empty,InfoIcon:Empty,MessagesSquare:Empty,SettingsIcon:Empty,UserMinusIcon:Empty,UserPlusIcon:Empty,useComments:()=>({stats:{replies:1}}),ReportDialog:{},setUser:value=>env.user=value,setChaptersCount:()=>{},Session:writable({identity:{id:'signed-in-reader'}}),prefs:Object.assign(writable({uploaders:[]}),{applyMangaPreferences(){},applyChapterPreferences(){}}),Permission:{User:{canUpdate:()=>false}},Constants:{User:{RoleOrder:['user'],RoleText:{user:'Reader'},Role:{}}},StatusCodes:{OK:200},Mutex:class{subscribe(fn){fn(false);return()=>{};}exec(action){return action();}isLocked(){return false;}},createMutationOwner:()=>({capture(){assert.fail('Presentation must not start a mutation');}}),createActorFollowState:options=>{assert.equal(options.canRead(),false);return{subscribe(fn){fn({followed:false,loading:false,updating:false});return()=>{};},selectTarget(){},activate(){assert.fail('Presentation must not follow');}};},API:{Chapter:{async Search(q){reads.push(q.get('uploader'));return{status:200,body:{total:0}};}},User:{GetFollowStatus(){assert.fail('No legacy follow lookup');}},Settings:{Update(){assert.fail('No settings mutation');}}},toast:{error(){assert.fail('No presentation error toast');}},followProps,profileProps};
 globalThis.__profilePresentation=env;const host=document.createElement('section');document.querySelector('main').append(host);const instance=mount(old?PreviousUser:User,{target:host,props:{data:result.body}});await settle();
 return{host,env,reads,navigation,followProps,profileProps,async close(){await unmount(instance);host.remove();await settle();}};
}
test('counterfactual composed predecessor exposes the resource UUID after @',async()=>{const f=await render({old:true});try{assert.equal(f.host.querySelector('h1').textContent,'FixtureReader');assert.equal(f.host.querySelector('strong').textContent,'@'+id);}finally{await f.close();}});
for(const name of ['FixtureReader','Reader-07','読者さん'])test('actual profile displays valid human username '+name+' without public UUID text',async()=>{const f=await render({name});try{assert.equal(f.host.querySelector('h1').textContent,name);assert.equal(f.host.querySelector('strong').textContent,'@'+name);assert(!f.host.textContent.includes(id));assert.equal(f.host.querySelector('details').open,false);}finally{await f.close();}});
for(const name of ['Readable Display Name','reader@example.invalid','FixtureReader\n','<img src=x onerror=alert(1)>'])test('unavailable/unsafe handle omits the secondary line, never falls back to UUID: '+JSON.stringify(name),async()=>{const f=await render({name});try{assert.equal(f.host.querySelector('h1').textContent,name);assert.equal(f.host.querySelector('strong'),null);assert(!f.host.textContent.includes(id));assert.equal(f.host.querySelector('h1 img'),null);}finally{await f.close();}});
test('canonical navigation and verified legacy action targets remain unchanged',async()=>{for(const requested of [legacy,id]){const f=await render({requested});try{assert.equal(f.navigation.calls[0].path,'/legacy/resolve');assert.deepEqual(f.reads,[id]);const links=[...f.host.querySelectorAll('nav a')].map(x=>x.getAttribute('href'));assert(links.includes('/user/'+id));assert(links.includes('/user/'+id+'/comments'));assert.deepEqual(f.followProps,[{kind:'user',targetId:id,requestedId:id}]);assert.deepEqual(f.profileProps,[{targetId:id,requestedId:id}]);assert(f.navigation.calls.every(row=>row.method!=='PUT'&&row.method!=='DELETE'));}finally{await f.close();}}});
test('unknown or different requested route is never normalized by a cosmetic change',async()=>{for(const requested of ['UnresolvedAlias',other]){const f=await render({requested});try{assert.equal(f.followProps.at(-1).requestedId,requested);assert.equal(f.profileProps.at(-1).requestedId,requested);assert(!f.host.textContent.includes(id));}finally{await f.close();}}});
test('reactive profile rename drops an obsolete handle rather than keeping an ID fallback',async()=>{const f=await render();try{f.env.user.update(user=>({...user,name:'Readable Display Name'}));await settle();assert.equal(f.host.querySelector('h1').textContent,'Readable Display Name');assert.equal(f.host.querySelector('strong'),null);assert(!f.host.textContent.includes(id));assert.equal(f.followProps.at(-1).requestedId,id);}finally{await f.close();}});
test('exact presentation successor is reversible, script-identical, and refuses modified or duplicate source',()=>{assert.equal(restoreProfilePresentationOutput(corrected,relative),predecessor);assert.equal(corrected.match(/<script[^>]*>[\s\S]*?<\/script>/)[0],predecessor.match(/<script[^>]*>[\s\S]*?<\/script>/)[0]);assert.equal(hash(original),manifest.original_sha256);assert.equal(fs.readFileSync(path.join(root,'frontend',relative),'utf8'),original);assert.throws(()=>applyProfilePresentationOutput(predecessor+' ',relative));assert.throws(()=>applyProfilePresentationOutput(corrected,relative));assert.throws(()=>restoreProfilePresentationOutput(corrected+' ',relative));assert.equal(applyProfilePresentationOutput('untouched','another-route'),'untouched');});

test('final production composition includes the exact profile successor',()=>assert.equal(hash(latest),manifest.output_sha256));
