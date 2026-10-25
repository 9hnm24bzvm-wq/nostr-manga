// Production typed client, with only its transport replaced. No network or browser.
import fs from 'node:fs';
import path from 'node:path';
import vm from 'node:vm';
import {stripTypeScriptTypes} from 'node:module';
import {fileURLToPath} from 'node:url';
import assert from 'node:assert/strict';
import test from 'node:test';

const root=path.resolve(path.dirname(fileURLToPath(import.meta.url)),'../..');
const directory=path.join(root,'platform-integrations/public-api');
const context=vm.createContext({URLSearchParams,Set,console,fetch:()=>{throw Error('Unexpected real network');}}),modules=new Map();
async function load(name){
 if(modules.has(name))return modules.get(name);
 const file=path.join(directory,name+'.ts');
 const module=new vm.SourceTextModule(stripTypeScriptTypes(fs.readFileSync(file,'utf8'),{mode:'transform',sourceUrl:file}),{context,identifier:file});
 modules.set(name,module);await module.link(specifier=>load(specifier.replace(/^\.\//,'')));return module;
}
const module=await load('client');await module.evaluate();const {PublicApiClient}=module.namespace;
const first='abcdefab-cdef-8abc-8def-abcdefabcdef',second='22222222-2222-8222-8222-222222222222';
const row=(id=first,type='manga')=>({id,type,attributes:{},relationships:[]});
const response=(data,limit=data.length,total=data.length,offset=0)=>({status:200,body:{result:'ok',response:'collection',data,limit,total,offset}});
const read=value=>new PublicApiClient(async()=>value).collection('/manga','manga');
const refused=async value=>assert.rejects(()=>read(value),error=>error.name==='PublicApiContractError'&&error.status===502);

test('collection rejects duplicate normalized UUID resources without trimming or retry',async()=>{
 for(const ids of [[first,first],[first,first.toUpperCase()]]){
  let calls=0;const value=response(ids.map(id=>row(id))),before=JSON.stringify(value);
  await assert.rejects(()=>new PublicApiClient(async()=>{calls++;return value;}).collection('/manga','manga'),/duplicate resource UUID/);
  assert.equal(calls,1);assert.equal(JSON.stringify(value),before);
 }
});
test('collection rejects rows beyond its declared limit including count-only limit zero',async()=>{
 await refused(response([row(),row(second)],1,2));await refused(response([row()],0,1));
});
test('collection rejects rows beyond remaining total or beyond an exhausted offset',async()=>{
 await refused(response([row(),row(second)],10,41,40));await refused(response([row()],10,0));await refused(response([row()],10,40,40));await refused(response([row()],10,40,41));
});
test('count-only and beyond-end empty pages retain actual metadata and response identity',async()=>{
 for(const value of [response([],0,42),response([],10,42,42),response([],10,42,43)])assert.equal(await read(value),value);
});
test('valid short relation prefixes preserve the actual continuation metadata',async()=>{
 const value=response([row(first,'manga_relation')],32,65,32);value.body.nextOffset=33;value.body.nostr={snapshot:'a'.repeat(64)};
 const before=JSON.stringify(value);assert.equal(await new PublicApiClient(async()=>value).collection('/manga/'+second+'/relation','manga_relation'),value);assert.equal(JSON.stringify(value),before);
});
test('valid pages preserve server order, resource identities and signal',async()=>{
 const value=response([row(second),row(first)],2,5,2),signal=new AbortController().signal;let sent;
 const result=await new PublicApiClient(async request=>{sent=request;return value;}).collection('/manga','manga',new URLSearchParams({limit:'2',offset:'2'}),signal);
 assert.equal(result,value);assert.deepEqual(Array.from(result.body.data,item=>item.id),[second,first]);assert.equal(sent.signal,signal);assert.match(sent.path,/limit=2&offset=2/);
});
test('malformed metadata and wrong-kind resources remain fail-closed',async()=>{
 for(const key of ['total','limit','offset'])for(const bad of [-1,1.5,Number.MAX_SAFE_INTEGER+1,null,'2',true]){const value=response([]);value.body[key]=bad;await refused(value);}
 await refused(response([row(first,'chapter')]));
});
test('non-success transport responses are preserved without treating errors as pages',async()=>{
 for(const status of [401,403,404,409,429,503]){const value={status,body:{result:'error'}};assert.equal(await read(value),value);}
});
