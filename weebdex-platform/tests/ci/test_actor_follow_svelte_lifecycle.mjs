#!/usr/bin/env node
// Reconstructed real pinned Svelte mount/key/effect-flush regression. Original
// controller/owner/authority execute; finite read/auth stand-ins are explicit.
// This is a lifecycle seam, not browser/native acceptance.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import os from 'node:os';
import {createRequire} from 'node:module';
import {createHash} from 'node:crypto';
import test from 'node:test';
import {createFrontendOverlays} from '../../platform-integrations/frontend-overlays.mjs';
const root=path.resolve(new URL('../../',import.meta.url).pathname),frontend=process.env.WEEBDEX_FRONTEND_ROOT,modules=process.env.WEEBDEX_BROWSER_DOM_NODE_MODULES;
assert(frontend&&modules,'Use restored reviewed exact-lock Svelte and DOM dependencies');
const require=createRequire(path.join(frontend,'package.json')),svelte=path.dirname(require.resolve('svelte/package.json'));
assert.equal(JSON.parse(fs.readFileSync(path.join(svelte,'package.json'))).version,'5.55.1');
const domRequire=createRequire(path.join(modules,'package.json'));
assert.equal(JSON.parse(fs.readFileSync(path.join(modules,'jsdom/package.json'))).version,'27.0.1');
assert.equal(createHash('sha256').update(fs.readFileSync(path.join(modules,'jsdom/lib/api.js'))).digest('hex'),'b4feb8ae099e1683da79137cf484a0952915e54d239140f124ecd7df5439b033');
const {JSDOM}=domRequire('jsdom'),{compile}=require('svelte/compiler'),esbuild=require('esbuild');
const provider=path.join(root,'frontend/src/lib/components/comments/comments-provider.svelte');
const composed=createFrontendOverlays(path.join(root,'frontend')).transform.handler(fs.readFileSync(provider,'utf8'),provider).code;
assert(composed.includes('{#key JSON.stringify([thread.id, $nativeAuthority.actor, $nativeAuthority.epoch])}'),'Model the actual authority-keyed boundary');
const behavior=path.join(root,'platform-integrations/frontend-behavior');
const files={
 'entry':`import Parent from 'fixture:Parent'; import Session from 'fixture:Session'; import {mount,unmount,tick} from 'svelte'; export {Parent,Session,mount,unmount,tick};`,
 'Session':`import {writable} from 'svelte/store'; import {bindNativeMutationAuthority} from ${JSON.stringify(path.join(behavior,'mutation-authority.ts'))}; const Session=writable(undefined); bindNativeMutationAuthority(Session); export default Session;`,
 'HTTP':`export class HTTPError extends Error {constructor(status,message){super(message);this.status=status;}} export default class HTTP {}`,
 'Env':`export const PUBLIC_API_URL='/api';`,
 'Errors':`export const Unknown=Error('Unknown'); export const TooManyRequests=Error('TooManyRequests'); export const UpdateConflict=Error('UpdateConflict');`,
 'Parent':`<script>import Child from 'fixture:Child';import {nativeAuthority} from ${JSON.stringify(path.join(behavior,'mutation-authority.ts'))};</script>{#key JSON.stringify(['owned-thread',$nativeAuthority.actor,$nativeAuthority.epoch])}<Child />{/key}`,
 'Child':`<script>import {onDestroy} from 'svelte'; import Session from 'fixture:Session'; import {createMutationOwner} from ${JSON.stringify(path.join(behavior,'authenticated-mutation.ts'))}; import {createActorFollowState} from ${JSON.stringify(path.join(behavior,'actor-follow-state.ts'))}; const h=globalThis.__actorLifecycle;h.mounts++;onDestroy(()=>h.destroys++);const owner=createMutationOwner(()=>'/group/target');const follow=createActorFollowState({owner,mutex:{isLocked:()=>false,exec:async operation=>operation()},target:()=> 'target',lifetime:()=> '/group/target',canMutate:()=>true,read:target=>h.read(target,$Session?.identity?.id),write:()=>{throw Error('No writes belong to this seam');},onError:error=>h.errors.push(error)});const off=follow.subscribe(state=>h.latest=state);onDestroy(off);$effect.pre(()=>follow.selectTarget());</script><button disabled={!$follow.ready} onclick={()=>follow.activate()}>{$follow.followed?'Unfollow':'Follow'}</button>`,
};

test('actual keyed Svelte flush destroys old Session subscriber before admitting exactly one current read',async()=>{
 const temporary=fs.mkdtempSync(path.join(os.tmpdir(),'actor-svelte-lifecycle-')),dom=new JSDOM('<div id="target"></div>',{url:'http://127.0.0.1:1/group/target'}),saved=new Map();
 const names=['window','document','navigator','HTMLElement','Element','Text','Comment','Document','Node','Event','MutationObserver','getComputedStyle'];
 for(const key of names){saved.set(key,Object.getOwnPropertyDescriptor(globalThis,key));Object.defineProperty(globalThis,key,{configurable:true,writable:true,value:key==='getComputedStyle'?dom.window.getComputedStyle.bind(dom.window):dom.window[key]});}
 const reads=[],h={mounts:0,destroys:0,errors:[],read(target,actor){let resolve;const promise=new Promise(yes=>resolve=yes);reads.push({target,actor,resolve});return promise;}};globalThis.__actorLifecycle=h;
 let mounted,runtime;
 try{
  const build=await esbuild.build({stdin:{contents:files.entry,resolveDir:frontend,sourcefile:'fixture-entry.js'},bundle:true,write:false,platform:'node',format:'cjs',conditions:['browser','svelte'],nodePaths:[path.join(frontend,'node_modules')],plugins:[{name:'real-svelte-owned-lifecycle',setup(build){
   build.onResolve({filter:/^fixture:/},args=>({path:args.path.slice(8),namespace:'fixture'}));
   build.onResolve({filter:/^\$lib\/stores\/Session$/},()=>({path:'Session',namespace:'fixture'}));build.onResolve({filter:/^\$lib\/HTTP$/},()=>({path:'HTTP',namespace:'fixture'}));build.onResolve({filter:/^\$env\/static\/public$/},()=>({path:'Env',namespace:'fixture'}));build.onResolve({filter:/^\$lib\/constants\/Error$/},()=>({path:'Errors',namespace:'fixture'}));
   build.onResolve({filter:/^svelte$/},()=>({path:path.join(svelte,'src/index-client.js')}));build.onResolve({filter:/^svelte\/store$/},()=>({path:path.join(svelte,'src/store/index-client.js')}));
   build.onLoad({filter:/.*/,namespace:'fixture'},args=>({contents:['Parent','Child'].includes(args.path)?compile(files[args.path],{filename:args.path+'.svelte',generate:'client',dev:false,runes:true}).js.code:files[args.path],loader:'js',resolveDir:frontend}));
  }}]});
  const bundle=path.join(temporary,'runtime.cjs');fs.writeFileSync(bundle,build.outputFiles[0].contents,{flag:'wx'});runtime=createRequire(import.meta.url)(bundle);
  const target=dom.window.document.querySelector('#target');mounted=runtime.mount(runtime.Parent,{target});await runtime.tick();assert.equal(h.mounts,1);assert.equal(reads.length,0);
  const account=id=>({identity:{id,is_verified:true},roles:[],permissions:[]});
  runtime.Session.set(account('A'));assert.equal(target.querySelector('button').disabled,true);assert.equal(reads.length,0,'Session reset happens before queued admission');await runtime.tick();await Promise.resolve();
  assert.equal(h.mounts,2);assert.equal(h.destroys,1);assert.equal(reads.length,1);assert.equal(reads[0].actor,'A');assert.equal(target.querySelector('button').textContent,'Follow');assert.equal(target.querySelector('button').disabled,true);
  reads[0].resolve({status:204});await runtime.tick();await runtime.tick();assert.equal(target.querySelector('button').textContent,'Unfollow');assert.equal(target.querySelector('button').disabled,false);
  runtime.Session.set(account('A'));runtime.Session.set(account('A'));assert.equal(h.latest.ready,false,'Session synchronously resets the live control state');await runtime.tick();await Promise.resolve();assert.equal(h.mounts,2);assert.equal(reads.length,2,'same-actor Session work coalesces without remount');
  reads[1].resolve({status:404});await runtime.tick();await runtime.tick();
  runtime.Session.set(account('B'));runtime.Session.set(account('A'));await runtime.tick();await Promise.resolve();assert.equal(h.mounts,3);assert.equal(h.destroys,2);assert.equal(reads.length,3);assert.equal(reads[2].actor,'A');
  await runtime.unmount(mounted);mounted=undefined;const removed=target.innerHTML;assert.equal(target.querySelector('button'),null);reads[2].resolve({status:204});await runtime.tick();assert.equal(target.innerHTML,removed);assert.deepEqual(h.errors,[]);
 }finally{if(mounted&&runtime)await runtime.unmount(mounted);delete globalThis.__actorLifecycle;for(const[key,descriptor]of saved){if(descriptor)Object.defineProperty(globalThis,key,descriptor);else delete globalThis[key];}dom.window.close();fs.rmSync(temporary,{recursive:true,force:true});}
});
