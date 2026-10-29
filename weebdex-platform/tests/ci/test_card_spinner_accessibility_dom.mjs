// Actual final-composed card, actual Spinner and actual Mutex. Only catalogue,
// library persistence/authority seams and the icon SVG are finite doubles. This
// checks DOM status exposure, not a screen-reader announcement or live save.
import fs from 'node:fs';
import path from 'node:path';
import {createHash} from 'node:crypto';
import test from 'node:test';
import assert from 'node:assert/strict';
import {component,module,root,writable,get,mounted,settle,window} from './account-fixpass-dom-fixture.mjs';
import {composed} from './composed-account-library-fixture.mjs';
import {cardSpinnerManifest,restoreCardSpinnerOutput} from '../../platform-integrations/local-library/card-spinner-overlays.mjs';
const Empty=await component('CardSpinnerEmpty','<span/>');
const Generic=await component('CardSpinnerGeneric','<script>let{children}=$props();</script><div>{@render children?.()}</div>');
const Image=await component('CardSpinnerImage','<script>let{src,alt}=$props();</script><img {src} {alt}/>');
const Aspect=await component('CardSpinnerAspect','<script>let{children,child}=$props();</script><div>{@render children?.()}{@render child?.({props:{}})}</div>');
const IconSvg=await component('CardSpinnerIconSvg','<script>let{class:cls,...rest}=$props();</script><svg class={cls} {...rest}/>');
const actualSpinnerSource=fs.readFileSync(path.join(root,'frontend/src/lib/components/ui/spinner/spinner.svelte'),'utf8');
const ActualSpinner=await component('CardSpinnerActualSpinner',actualSpinnerSource);
const {default:ActualMutex}=await module('CardSpinnerActualMutex',await composed('src/lib/Mutex.ts'));
class HTTPError extends Error {constructor(status,body,title){super(body);this.status=status;this.body=body;this.title=title;}}
globalThis.__accountFixpass={modules:{'$lib/HTTP':{HTTPError},'svelte-sonner':{toast:{error:(title,options)=>globalThis.__accountFixpass.calls.errors.push({title,detail:options?.description})}}}};
const feedback=await module('CardSpinnerActualMutationFeedback',fs.readFileSync(path.join(root,'platform-integrations/frontend-behavior/mutation-feedback.ts'),'utf8'));
const relative='src/lib/components/MangaSearchEntryCard.svelte',source=await composed(relative),hash=value=>createHash('sha256').update(value).digest('hex');
assert.equal(hash(source),cardSpinnerManifest[0].output_sha256,'exercise final composed production card');
const predecessor=restoreCardSpinnerOutput(source,relative),Card=await component('CardSpinnerCurrentCard',source),PreviousCard=await component('CardSpinnerPreviousCard',predecessor);
const id='11111111-1111-8111-8111-111111111111';
const manga={id,title:'Synthetic card title',language:'en',status:'ongoing',content_rating:'safe',relationships:{tags:[]}};
const deferred=()=>{let resolve,reject;const promise=new Promise((yes,no)=>{resolve=yes;reject=no;});return{promise,resolve,reject};};
const loading=host=>[...host.querySelectorAll('[role="status"][aria-label="Loading"]')];
function setup({mode='device',followed=false,known=true,owner='A'}={}){
 const initialFollows=followed?{[id]:['reading',1]}:{};
 const local=writable({ready:true,mode,scopeStatus:'verified'}),identity=writable({canUse:true,accountId:mode==='account'?'A':null,follows:initialFollows});
 const account=writable({owner,epoch:1,known:known?{[id]:true}:{},follows:initialFollows,pending:false,error:''});
 const pending=deferred(),calls={writes:[],applied:[],observe:[],cleanup:[],errors:[],refresh:0};
 const capture=()=>JSON.stringify([get(local).mode,get(identity).accountId]);
 const check=context=>assert.equal(capture(),context,'current library destination');
 const modules={
  '$localLibrary/library':{libraryIdentity:identity,localLibraryState:local,openLibrary:async()=>{},message:e=>e.message,captureLibraryContext:capture,assertLibraryContext:check,applyAccountFollow:(context,target,value)=>{check(context);calls.applied.push({target,value});identity.update(state=>{const follows={...state.follows};if(value)follows[target]=value;else delete follows[target];return{...state,follows};});}},
  '$localLibrary/native-account-library':{nativeAccountLibraryState:account,observeAccountLibraryTitle:target=>{calls.observe.push(target);return()=>calls.cleanup.push(target);},refreshAccountLibraryTitles:()=>calls.refresh++},
  '$localLibrary/action-owner':{createLibraryActionOwner:()=>({capture:()=>{const context=capture();return{assert:()=>check(context),alive:()=>capture()===context};}})},
  '$platform/mutation-feedback':feedback,
  '$publicApi/media':{retainedCoverUrl:()=>'',retainedPageUrl:()=>''},
  '$platform/discovery-reading':{discoveryReadingQuery:()=>''},
  '$app/state':{page:{url:new URL('https://catalog.example/'),params:{}}},
  '$env/static/public':{PUBLIC_COVER_URL:''},
  '$lib':{Mutex:ActualMutex,Constants:{Manga:{ReadingStatus:{Reading:'reading'},PublicationStatus:{Ongoing:'ongoing'},PublicationStatusText:{ongoing:'Ongoing'},ContentRating:{Safe:'safe'},ContentRatingText:{},DemographicText:{},RelationTypeText:{}},Tag:{Group:{}}},Locale:{Languages:{},to3166:()=> 'gb',toLanguage:()=> 'English'},Typography:{exec:()=>{}},Utils:{makeSlug:()=>'',parseDescriptionPlain:()=>''},API:{MangaFollow:{Update:(target,body,admitted)=>{admitted.assert();calls.writes.push({operation:'add',target,body,context:admitted.context});return pending.promise;},Unfollow:(target,admitted)=>{admitted.assert();calls.writes.push({operation:'remove',target,context:admitted.context});return pending.promise;}}}},
  '$lib/stores':{Session:writable(mode==='account'?{identity:{id:'A'}}:undefined)},
  '$lib/Utils':{cn:(...values)=>values.filter(value=>typeof value==='string'&&value).join(' ')},
  '@lucide/svelte':{Check:Empty,Plus:Empty,Loader2Icon:IconSvg},
  'http-status-codes':{StatusCodes:{NO_CONTENT:204}},'is-mobile':{default:()=>false},
  './ui/aspect-ratio':{AspectRatio:Aspect},'./ui/badge':{badgeVariants:()=>''},'./ui/ImageLoader.svelte':{default:Image},'./ui/scroll-area':{ScrollArea:Generic},'./ui/spinner':{Spinner:ActualSpinner}
 };
 globalThis.__accountFixpass={modules,calls};return{modules,local,identity,account,pending,calls};
}
for(const isGrid of [true,false])for(const mode of ['device','account'])for(const followed of [false,true])for(const outcome of ['acknowledged','refused'])test(`actual ${isGrid?'grid':'list'} ${mode} ${followed?'remove':'add'} has Loading only while ${outcome} mutation is pending`,async()=>{
 const env=setup({mode,followed}),f=await mounted(Card,{manga,isGrid});
 try{
  const button=f.host.querySelector('button');assert(button);assert.equal(button.getAttribute('aria-label'),followed?'Remove from library':'Add to library');assert.equal(button.disabled,false);assert.equal(loading(f.host).length,0);assert.equal(env.calls.observe.length,mode==='account'?1:0);
  button.click();
  // Same-tick activation before Svelte updates disabled still uses the real mutex.
  button.dispatchEvent(new window.MouseEvent('click',{bubbles:true,cancelable:true}));
  await settle();assert.equal(env.calls.writes.length,1);assert.equal(button.disabled,true);assert.equal(loading(f.host).length,1);assert.match(loading(f.host)[0].getAttribute('class'),/scale-100/);assert.equal(loading(f.host)[0].getAttribute('aria-hidden'),null);
  button.click();button.dispatchEvent(new window.MouseEvent('click',{bubbles:true,cancelable:true}));await settle();assert.equal(env.calls.writes.length,1,'pending repeated clicks cannot write twice');
  const request=env.calls.writes[0];assert.equal(request.operation,followed?'remove':'add');assert.equal(request.target,id);if(!followed)assert.deepEqual(request.body,{status:'reading',notification:true});
  env.pending.resolve({status:outcome==='acknowledged'?204:403});await settle();
  assert.equal(loading(f.host).length,0);assert.equal(button.disabled,false);assert.equal(env.calls.writes.length,1);assert.equal(env.calls.applied.length,outcome==='acknowledged'?1:0);assert.equal(env.calls.errors.length,outcome==='refused'?1:0);
  const saved=outcome==='acknowledged'?!followed:followed;assert.equal(id in get(env.identity).follows,saved);assert.equal(button.getAttribute('aria-label'),saved?'Remove from library':'Add to library');
 }finally{env.pending.resolve({status:403});await f.dispose();}
 assert.deepEqual(env.calls.cleanup,env.calls.observe,'every account-title observation is released, including acknowledgement refresh');
});
for(const options of [{known:false},{owner:'another-account'}])test('unknown current account membership stays disabled without a mutation Loading status: '+JSON.stringify(options),async()=>{
 const env=setup({mode:'account',...options}),f=await mounted(Card,{manga,isGrid:true});
 try{const button=f.host.querySelector('button');assert.equal(button.disabled,true);assert.equal(button.getAttribute('aria-label'),'Checking saved-title status');assert.equal(loading(f.host).length,0);assert.match(f.host.textContent,/Checking library/);button.click();button.dispatchEvent(new window.MouseEvent('click',{bubbles:true,cancelable:true}));await settle();assert.equal(env.calls.writes.length,0);assert.equal(button.disabled,true);assert.equal(loading(f.host).length,0);}finally{await f.dispose();}
});
for(const followed of [false,true])test('unconfirmed account '+(followed?'removal':'addition')+' drops only spinner and retains unknown-state disabled guard',async()=>{
 const env=setup({mode:'account',followed}),f=await mounted(Card,{manga,isGrid:true});
 try{const button=f.host.querySelector('button');button.click();await settle();assert.equal(loading(f.host).length,1);env.account.update(state=>({...state,known:{},error:'The previous update is unconfirmed. Check library status.'}));env.pending.reject(Error('Synthetic response lost'));await settle();assert.equal(loading(f.host).length,0);assert.equal(button.disabled,true);assert.equal(button.getAttribute('aria-label'),'Checking saved-title status');assert.equal(env.calls.applied.length,0);assert.equal(id in get(env.identity).follows,followed);assert.equal(env.calls.errors.length,1);button.click();button.dispatchEvent(new window.MouseEvent('click',{bubbles:true,cancelable:true}));await settle();assert.equal(env.calls.writes.length,1);assert.equal(env.calls.refresh,0,'no automatic uncertainty retry');}finally{env.pending.resolve({status:403});await f.dispose();}
});
test('six idle guest homepage cards expose no inactive Loading statuses',async()=>{
 const env=setup(),cards=[];try{for(let n=0;n<6;n++)cards.push(await mounted(Card,{manga:{...manga,id:id.slice(0,-1)+n,title:'Synthetic homepage title '+n},isGrid:true}));assert.equal(cards.reduce((count,f)=>count+loading(f.host).length,0),0);assert.equal(cards.reduce((count,f)=>count+f.host.querySelectorAll('button[aria-label="Add to library"]').length,0),6);assert.equal(env.calls.writes.length,0);}finally{for(const f of cards)await f.dispose();}
});
test('counterfactual predecessor contains an inactive DOM Loading status, without asserting it was announced',async()=>{setup();const f=await mounted(PreviousCard,{manga,isGrid:true});try{assert.equal(loading(f.host).length,1);assert.match(loading(f.host)[0].getAttribute('class'),/scale-0/);assert.equal(f.host.querySelector('button').disabled,false);}finally{await f.dispose();}});
test('presentation successor keeps all business script bytes and real Spinner leaf unchanged',()=>{assert.equal(hash(source),cardSpinnerManifest[0].output_sha256);assert.equal(source.match(/<script[^>]*>[\s\S]*?<\/script>/)[0],predecessor.match(/<script[^>]*>[\s\S]*?<\/script>/)[0]);assert.equal(fs.readFileSync(path.join(root,'frontend/src/lib/components/ui/spinner/spinner.svelte'),'utf8'),actualSpinnerSource);assert.match(actualSpinnerSource,/role="status" aria-label="Loading"/);});
