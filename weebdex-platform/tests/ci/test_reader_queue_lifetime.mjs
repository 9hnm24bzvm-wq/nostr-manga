// Actual composed queue script with controlled ticks. No DOM/browser or HTTP.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import vm from 'node:vm';
import { stripTypeScriptTypes } from 'node:module';
import test from 'node:test';
import { createFrontendOverlays } from '../../platform-integrations/frontend-overlays.mjs';
const root = path.resolve(new URL('../../', import.meta.url).pathname), frontend = path.join(root, 'frontend');
const file = path.join(frontend, 'src/lib/components/reader/reader-queue.svelte');
const source = createFrontendOverlays(frontend).transform.handler(fs.readFileSync(file, 'utf8'), file).code;
const script = source.match(/<script[^>]*>([\s\S]*?)<\/script>/)[1];
const executable = stripTypeScriptTypes(script.replace(/^\s*import[^;]+;\s*$/gm, ''), { mode: 'transform' });
function fixture() {
  const ticks = [], destroys = [], requests = [], effects = [], page = {index:0, data:{name:'0.png'}};
  const reader = {pages:[page], currentPage:page, spreads:[], prefs:{}, chapter:{node:'http://127.0.0.1:1',id:'synthetic'}};
  const env = { console:{info(){}}, URL:{revokeObjectURL(){}}, window:{setTimeout(){throw Error('Unexpected retry');}},
    $state:x=>x, $effect:fn=>effects.push(fn), $readerPrefs:{preload:0,parallel:2},
    onDestroy:fn=>destroys.push(fn), useReader:()=>reader, tick:()=>new Promise(resolve=>ticks.push(resolve)),
    Mutex:class {exec(fn){return fn();}wait(fn){return fn();}},
    XMLHttpRequest:class {constructor(){requests.push(this);}open(){}send(){}abort(){this.onabort?.();}}
  };
  vm.runInNewContext(executable + '\nthis.update=updateQueue;this.process=processQueue;this.setQueue=value=>queue=value;',env);
  return {env,page,reader,requests,effects,destroy:()=>destroys.forEach(fn=>fn()),async step(){assert(ticks.length,'Expected suspended production tick');ticks.shift()();for(let i=0;i<8;i++)await Promise.resolve();},ticks};
}
test('destroyed initial queue cannot enqueue after its first suspended tick',async()=>{const f=fixture(),work=f.env.update();f.destroy();await f.step();assert.equal(f.ticks.length,0);await work;assert.equal(f.page.isLoading,undefined);assert.equal(f.requests.length,0);});
test('destroyed queue cannot publish after its second suspended update tick',async()=>{const f=fixture(),work=f.env.update();await f.step();f.destroy();await f.step();assert.equal(f.ticks.length,0);await work;assert.equal(f.requests.length,0);});
test('disposal after process tick cannot claim a loading page',async()=>{const f=fixture();f.env.setQueue([0]);const work=f.env.process();f.destroy();await f.step();assert.equal(f.ticks.length,0);await work;assert.equal(f.requests.length,0);assert.equal(f.page.isLoading,undefined);});
test('disposal during final per-page tick cannot steal replacement queue claim',async()=>{const f=fixture();f.env.setQueue([0]);const work=f.env.process();await f.step();f.destroy();await f.step();await work;assert.equal(f.requests.length,0);assert.equal(f.page.isLoading,undefined);});
test('obsolete page identity is rechecked after a deferred admission',async()=>{const f=fixture();f.env.setQueue([0]);const work=f.env.process();await f.step();f.reader.pages=[{index:0,data:{name:'replacement.png'}}];await f.step();await work;assert.equal(f.requests.length,0);assert.equal(f.page.isLoading,undefined);});
test('active request is aborted and its claim released synchronously at replacement',async()=>{const f=fixture();f.env.setQueue([0]);const work=f.env.process();await f.step();await f.step();await work;assert.equal(f.requests.length,1);assert.equal(f.page.isLoading,true);f.destroy();assert.equal(f.page.isLoading,false);f.page.isLoading=true;/* Replacement queue owns the same page now. */for(let i=0;i<8;i++)await Promise.resolve();if(f.ticks.length)await f.step();assert.equal(f.page.isLoading,true,'Old abort finalizer must not clear the replacement claim');});
