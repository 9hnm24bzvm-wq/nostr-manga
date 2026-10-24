// Source-only module loader used by finite synthetic tests. Never starts HTTP,
// accesses a production service, creates credentials, or opens a database.
import fs from 'node:fs';
import path from 'node:path';
import vm from 'node:vm';
import {stripTypeScriptTypes} from 'node:module';
import {fileURLToPath} from 'node:url';
export const root=path.resolve(path.dirname(fileURLToPath(import.meta.url)),'../..');
export const id='11111111-1111-8111-8111-111111111111',other='22222222-2222-8222-8222-222222222222';
export const threadId='actual-native-catalogue-thread';
export const plain=value=>JSON.parse(JSON.stringify(value));
const context=vm.createContext({URL,URLSearchParams,TextEncoder,TextDecoder,Uint8Array,AbortController,setTimeout,clearTimeout,console});
const cache=new Map(),directory=path.join(root,'platform-integrations/public-api');
async function load(name){
 if(cache.has(name))return cache.get(name);
 const filename=path.join(directory,name+'.ts');
 const module=new vm.SourceTextModule(stripTypeScriptTypes(fs.readFileSync(filename,'utf8'),{mode:'transform',sourceUrl:filename}),{context});
 cache.set(name,module);
 await module.link(specifier=>load(path.posix.normalize(path.posix.join(path.posix.dirname(name),specifier))));
 return module;
}
const reader=await load('native-forum/reader');await reader.evaluate();
export const readerModule=reader.namespace;
export const contextModule=cache.get('native-forum/context').namespace;
export const targetBody=(kind='manga')=>({result:'ok',data:{kind,id,threadId},nostr:{source:'persisted-native-discussion-target'}});
export function contextFixture(options={}){
 const calls=[],timers=new Map();let origin='https://catalog.example',alive=true;
 const reader=readerModule.createNativeDiscussionTargetReader({
  origin:()=>origin,
  setTimer(callback,ms){if(![10000,250].includes(ms))throw Error('Unexpected timer');const key={};timers.set(key,{callback,ms});return key;},
  clearTimer:key=>timers.delete(key),
  fetch:async(url,request)=>{calls.push({url,request});await options.gate?.promise;return (typeof options.response==='function'?options.response(calls.length):options.response)??new Response(JSON.stringify(options.body??targetBody(options.kind)),{status:options.status??200,headers:{'content-type':'application/json'}});}
 });
 return{calls,timers,read:(signal,requestedThread=options.threadId??threadId)=>reader(requestedThread,signal,()=>{if(!alive)throw Error('Target changed');}),expire(ms=10000){for(const timer of [...timers.values()])if(timer.ms===ms)timer.callback();},invalidate(){alive=false;},changeOrigin(){origin='https://different.example';}};
}
