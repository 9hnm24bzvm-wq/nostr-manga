// Actual adapter modules with finite Fetch streams; no live HTTP/database claim.
import fs from 'node:fs';
import path from 'node:path';
import vm from 'node:vm';
import {stripTypeScriptTypes} from 'node:module';
import {fileURLToPath} from 'node:url';
import test from 'node:test';
import assert from 'node:assert/strict';
const root=path.resolve(path.dirname(fileURLToPath(import.meta.url)),'../..'),dir=path.join(root,'platform-integrations/public-api');
const timers=new Map();let serial=0;
const context=vm.createContext({URLSearchParams,AbortController,TextEncoder,TextDecoder,Uint8Array,atob,console,setTimeout:(fn,ms)=>{assert.ok([15000,250].includes(ms));timers.set(++serial,fn);return serial;},clearTimeout:id=>timers.delete(id)}),modules=new Map();
async function load(name){const file=path.join(dir,name+'.ts');if(!modules.has(file)){const mod=new vm.SourceTextModule(stripTypeScriptTypes(fs.readFileSync(file,'utf8'),{mode:'transform'}),{context,identifier:file});modules.set(file,mod);await mod.link((spec,parent)=>load(path.relative(dir,path.resolve(path.dirname(parent.identifier),spec))));}return modules.get(file);}
const mod=await load('user-memberships');await mod.evaluate();const {normalizeMembershipPage,createMembershipReader,membershipCursor}=mod.namespace;
const user='11111111-1111-8111-8111-111111111111',other='22222222-2222-8222-8222-222222222222',id=n=>'aaaaaaaa-aaaa-8aaa-8aaa-'+String(n).padStart(12,'0');
const group=(n,name='Group '+n)=>({id:id(n),type:'scanlation_group',attributes:{name,version:1},relationships:[],membership:{isLeader:n===1,isOfficer:n===2}});
const cursor=(row,owner=user)=>Buffer.from(JSON.stringify({version:1,userId:owner,groupId:row.id,name:row.attributes.name})).toString('base64url');
const page=(data,after=null,next=null)=>({result:'ok',response:'membership_page',data,limit:32,after,nextCursor:next,nostr:{scope:'public-native-user',source:'persisted-visible-native-memberships',identity:'native_public_user',requestedId:user,userId:user,consistency:'live-keyset-v1'}});
const json=value=>new Response(JSON.stringify(value),{headers:{'content-type':'application/json'}});
test.afterEach(()=>assert.equal(timers.size,0,'page clears its deadline'));
test('target-only role projection preserves server DB order, names, ties and multibyte labels',()=>{const rows=[group(9,'界'),group(2,'Same'),group(1,'Same')],result=normalizeMembershipPage(page(rows),user);assert.deepEqual(Array.from(result.data,row=>row.id),rows.map(row=>row.id));assert.equal(result.data[1].isOfficer,true);assert.equal(result.data[2].isLeader,true);assert.equal(result.data[0].name,'界');});
test('exact public cursor follows a full page without exposing native identity',()=>{const rows=Array.from({length:32},(_,n)=>group(n+1,'同名')),token=cursor(rows.at(-1));const result=normalizeMembershipPage(page(rows,null,token),user);assert.equal(result.nextCursor,token);assert.equal(membershipCursor(token,user).groupId,id(32));assert.doesNotMatch(Buffer.from(token,'base64url').toString(),/native|member|account/i);});
for(const [name,change]of [
 ['publisher provenance',b=>b.nostr.identity='protocol_publisher'],['foreign target',b=>b.nostr.userId=other],['wrong request',b=>b.nostr.requestedId=other],['private count',b=>b.total=33],['snapshot claim',b=>b.nostr.consistency='atomic'],['members leak',b=>b.data[0].relationships=[{id:other,type:'member'}]],['private attribute',b=>b.data[0].attributes.email='private'],['foreign edge flag',b=>b.data[0].membership.permissions=[]],['duplicate group',b=>b.data.push(b.data[0])],['missing role',b=>delete b.data[0].membership.isOfficer],['fake version',b=>b.data[0].attributes.version=0],['long name',b=>b.data[0].attributes.name='界'.repeat(201)],['empty name',b=>b.data[0].attributes.name=''],['short page continuation',b=>b.nextCursor=cursor(b.data[0])],['shifted echo',b=>b.after=cursor(b.data[0])]
])test('strict projection rejects '+name,()=>{const body=page([group(1)]);change(body);assert.throws(()=>normalizeMembershipPage(body,user));});
for(const [name,change]of [['wrong last group',b=>b.nextCursor=cursor(group(31))],['wrong last name',b=>b.nextCursor=cursor(group(32,'wrong'))],['foreign user cursor',b=>b.nextCursor=cursor(group(32),other)],['cursor extension',b=>{const value={version:1,userId:user,groupId:id(32),name:'Group 32',private:'no'};b.nextCursor=Buffer.from(JSON.stringify(value)).toString('base64url');}]])test('continuation refuses '+name,()=>{const rows=Array.from({length:32},(_,n)=>group(n+1)),body=page(rows,null,cursor(rows.at(-1)));change(body);assert.throws(()=>normalizeMembershipPage(body,user));});
test('anonymous bounded fetch sends no cookies or signing proof',async()=>{const calls=[],read=createMembershipReader(async(url,init)=>{calls.push({url,init});return json(page([group(1)]));});const result=await read(user);assert.equal(result.data.length,1);assert.equal(calls[0].url,`/api/v1/nostr/user/${user}/memberships?limit=32`);assert.equal(calls[0].init.credentials,'omit');assert.deepEqual(Object.keys(calls[0].init.headers),['Accept']);assert.equal(calls[0].init.method,'GET');assert.equal(calls[0].init.signal.aborted,false);});
test('continuation carries exact opaque cursor and no implicit sort',async()=>{const token=cursor(group(1)),read=createMembershipReader(async(url)=>{assert.equal(new URL(url,'http://fixture').searchParams.get('after'),token);return json(page([group(2)],token));});assert.equal((await read(user,token)).data[0].id,id(2));});
for(const status of [404,409,503])test('HTTP '+status+' is truthful failure without fallback or replay',async()=>{let calls=0;const read=createMembershipReader(async()=>{calls++;return new Response('{}',{status,headers:{'content-type':'application/json'}});});await assert.rejects(read(user),error=>error.status===status);assert.equal(calls,1);});
test('wire overflow fails closed',async()=>{const read=createMembershipReader(async()=>new Response('x'.repeat(65537),{headers:{'content-type':'application/json'}}));await assert.rejects(read(user),error=>error.status===503);});
test('declared wire overflow fails before parsing',async()=>{const read=createMembershipReader(async()=>new Response('{}',{headers:{'content-type':'application/json','content-length':'65537'}}));await assert.rejects(read(user),error=>error.status===503);});
test('invalid UTF-8 and malformed JSON cannot become an empty list',async()=>{for(const bytes of [new Uint8Array([255]),new TextEncoder().encode('not json')]){const read=createMembershipReader(async()=>new Response(bytes,{headers:{'content-type':'application/json'}}));await assert.rejects(read(user));}});
test('caller abort suppresses a fetch that ignores cancellation',async()=>{const abort=new AbortController();let resolve,signal;const read=createMembershipReader(async(_,init)=>{signal=init.signal;return await new Promise(r=>resolve=r);});const pending=read(user,null,abort.signal);for(let i=0;i<10;i++)await Promise.resolve();abort.abort();resolve(json(page([group(1)])));await assert.rejects(pending,error=>error.status===499);assert.equal(signal.aborted,true);});
test('deadline suppresses pending stream and requests cancellation',async()=>{let cancelled=false;const read=createMembershipReader(async()=>new Response(new ReadableStream({start(){},cancel(){cancelled=true;}}),{headers:{'content-type':'application/json'}}));const pending=read(user);await Promise.resolve();await Promise.resolve();[...timers.values()][0]();await assert.rejects(pending,error=>error.status===503);assert.equal(cancelled,true);});
test('preaborted read and foreign cursor do not fetch',async()=>{let calls=0;const read=createMembershipReader(async()=>{calls++;return json(page([]));}),abort=new AbortController();abort.abort();await assert.rejects(read(user,null,abort.signal));await assert.rejects(read(user,cursor(group(1),other)));assert.equal(calls,0);});
test('empty public memberships remain an honest empty live page',async()=>{const result=await createMembershipReader(async()=>json(page([])))(user);assert.equal(result.data.length,0);assert.equal(result.nextCursor,null);});
test('maximum JSON-escaped and800UTF8-byte names have usable next-page cursor roundtrips',async()=>{for(const name of ['\u0001'.repeat(200),'🦀'.repeat(200)]){const rows=Array.from({length:32},(_,n)=>group(n+1,name)),token=cursor(rows.at(-1));assert.ok(token.length<=2048);let calls=0;const read=createMembershipReader(async url=>{calls++;const after=new URL(url,'http://fixture').searchParams.get('after');return json(after?page([group(33,name)],after):page(rows,null,token));});const first=await read(user),second=await read(user,first.nextCursor);assert.equal(second.data[0].name,name);assert.equal(calls,2);}});
for(const target of ['154965c1-6a5d-4e15-9a87-b783db928ba5',user])test('fresh reader and preferred public alias both retain a genuine empty group page: '+target,async()=>{
 const body=page([]);body.nostr.requestedId=target;body.nostr.userId=target;
 const read=createMembershipReader(async(url,init)=>{assert.equal(url,`/api/v1/nostr/user/${target}/memberships?limit=32`);assert.equal(init.credentials,'omit');return json(body);});
 const result=await read(target);assert.equal(result.userId,target);assert.equal(result.data.length,0);assert.equal(result.nextCursor,null);
});

test('retained frontend query-prefix patch cannot produce a double question mark',async()=>{
 const original=URLSearchParams.prototype.toString;
 URLSearchParams.prototype.toString=function(){const value=original.call(this);return value?'?'+value:'';};
 try {
  const token=cursor(group(1));
  for(const after of [null,token]){
   const read=createMembershipReader(async(url)=>{assert.equal(url,`/api/v1/nostr/user/${user}/memberships?limit=32${after?'&after='+encodeURIComponent(after):''}`);assert.equal(new URL(url,'https://fixture').searchParams.get('limit'),'32');return json(page([],after));});
   assert.equal((await read(user,after)).data.length,0);
  }
 }finally{URLSearchParams.prototype.toString=original;}
});
