// Finite actual pager/storage modules with synthetic catalogue rows. Not a browser/capacity result.
import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import vm from 'node:vm';
import { createRequire, stripTypeScriptTypes } from 'node:module';
import { fileURLToPath } from 'node:url';
import { webcrypto } from 'node:crypto';
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)),'../..');
const deps = process.env.WEEBDEX_BROWSER_DOM_NODE_MODULES; assert(deps,'An explicit existing fake-indexeddb dependency root is required');
const require = createRequire(path.join(deps,'../package.json'));
const { indexedDB, IDBKeyRange } = require(path.join(deps,'fake-indexeddb'));
const context = vm.createContext({ indexedDB, IDBKeyRange, crypto:webcrypto, URLSearchParams, Date, AbortController, setTimeout, clearTimeout, TextEncoder, console });
const modules = new Map();
async function load(relative) {
 const full=path.resolve(root,relative); if(modules.has(full))return modules.get(full);
 let source=fs.readFileSync(full,'utf8');if(full.endsWith('.ts'))source=stripTypeScriptTypes(source,{mode:'transform'});
 const module=new vm.SourceTextModule(source,{context,identifier:full});modules.set(full,module);
 await module.link(specifier=>load(path.relative(root,path.resolve(path.dirname(full),specifier+(path.extname(specifier)?'':'.ts')))));return module;
}
const storage=await load('platform-integrations/local-library/title-index.ts');await storage.evaluate();
const pagination=await load('platform-integrations/local-library/updates-pagination.ts');await pagination.evaluate();
const titles=await load('platform-integrations/local-library/title-pagination.ts');await titles.evaluate();
const {TitleIndex}=storage.namespace,{UpdatesPager}=pagination.namespace,{TitlePager}=titles.namespace;
const row=(n,parent='p',value=n,date='2026-10-22T12:00:00.000001Z')=>({id:'c'+String(n).padStart(6,'0'),published_at:date,_sort_value:value,_sort_text:'',relationships:{manga:{id:parent}}});
const plain=value=>JSON.parse(JSON.stringify(value));
async function fixture(parents,rows,{latest=false,limit=20,direction=1,source:custom}={}){
 const store=await TitleIndex.create(()=>{});let alive=true;const calls=[];
 const source=custom??{async scan(parent,offset,size){calls.push({parent,offset,size});const selected=rows.filter(row=>row.relationships.manga.id===parent).sort((a,b)=>direction*(a._sort_value-b._sort_value)||a.id.localeCompare(b.id));return{total:selected.length,data:selected.slice(offset,offset+size)};},async hydrate(keys){return keys.map(key=>rows.find(row=>row.id===key.id)).filter(Boolean);}};
 return{store,calls,pager:new UpdatesPager(parents,store,source,()=>{if(!alive)throw Error('owner changed');},limit,direction,latest),invalidate(){alive=false;}};
}
async function finish(pager){let result;do{result=await pager.load();}while(!result.indexed);return pager.load();}
for(const direction of [-1,1])test(`1,005 notified parents preserve global order after sparse filtering, ${direction}`,async()=>{
 const parents=Array.from({length:1005},(_,n)=>'p'+n),rows=parents.flatMap((parent,n)=>n%17===0?[row(n,parent,1005-n)]:[]),f=await fixture(parents,rows,{direction});
 try{for(let n=0;n<51;n++){const value=await f.pager.load();assert.equal(value.ready,false);assert.equal(value.data.length,0);assert.equal(value.total,n===50?rows.length:null);assert(value.scanned<=Math.min((n+1)*20,1005));}const page=await f.pager.load();assert.equal(page.ready,true);assert.equal(page.total,rows.length);assert.deepEqual(plain(page.data.map(r=>r.id)),rows.sort((a,b)=>direction*(a._sort_value-b._sort_value)).slice(0,20).map(r=>r.id));assert.equal(f.calls.length,1005);assert(f.calls.every(call=>call.size<=100));}finally{await f.pager.dispose();}
});
test('newest per parent uses exact normalized instants and ascending ID ties, independently of final order',async()=>{
 const rows=[row(1,'a',90,'2026-10-22T12:00:00.000001Z'),row(2,'a',80,'2026-10-22T13:00:00.000002+01:00'),row(3,'a',70,'2026-10-22T12:00:00.000002Z'),row(4,'b',5,'2026-10-22T12:00:00Z')],f=await fixture(['a','b'],rows,{latest:true});
 try{const page=await finish(f.pager);assert.deepEqual(plain(page.data.map(r=>r.id)),[row(4).id,row(2).id]);assert.equal(page.total,2);}finally{await f.pager.dispose();}
});
test('multi-page parent does not finalize or disclose total before its last page',async()=>{
 const rows=Array.from({length:205},(_,n)=>row(n)),f=await fixture(['p'],rows);try{const done=await f.pager.load();assert.equal(done.total,205);assert.equal(done.scanned,1);assert.deepEqual(f.calls.map(c=>c.offset),[0,100,200]);assert.equal((await f.pager.load(11)).data.length,5);}finally{await f.pager.dispose();}
});
test('changed total refuses without advancing continuation',async()=>{let calls=0;const rows=Array.from({length:100},(_,n)=>row(n)),f=await fixture(['p'],[],{source:{async scan(){return{total:++calls===1?200:201,data:rows};},async hydrate(){return[];}}});try{await assert.rejects(f.pager.load(),/changed/);await assert.rejects(f.pager.load(),/changed/);assert.equal(calls,2);}finally{await f.pager.dispose();}});
test('vanished selected row refuses rather than shrinking an apparently exact page',async()=>{const rows=[row(1)],f=await fixture(['p'],rows);try{await f.pager.load();rows.length=0;await assert.rejects(f.pager.load(),/changed/);}finally{await f.pager.dispose();}});
test('owner invalidation admits no delayed scan rows',async()=>{let release;const f=await fixture(['p'],[],{source:{scan:()=>new Promise(resolve=>release=resolve),async hydrate(){return[];}}});try{const pending=f.pager.load();f.invalidate();release({total:1,data:[row(1)]});await assert.rejects(pending,/owner changed/);assert.equal((await f.store.page(0,100)).length,0);}finally{await f.pager.dispose();}});
test('cancelled scan can retry the exact unacknowledged page',async()=>{let release,calls=0;const f=await fixture(['p'],[],{source:{scan:()=>++calls===1?new Promise(resolve=>release=resolve):Promise.resolve({total:1,data:[row(1)]}),async hydrate(){return[row(1)];}}});try{const pending=f.pager.load();f.pager.cancel();release({total:1,data:[row(1)]});await assert.rejects(pending,/cancelled/);assert.equal((await f.pager.load()).total,1);assert.equal((await f.pager.load()).data.length,1);}finally{await f.pager.dispose();}});
test('unique owner/id rejects changed order keys and optional parent remains absent for title rows',async()=>{const store=await TitleIndex.create(()=>{});try{await store.append([{id:'same',order:[1]}]);await assert.rejects(store.append([{id:'same',order:[2]}]));assert.deepEqual(plain(await store.page(0,10)),[{id:'same',order:[1]}]);}finally{await store.dispose();}});
test('latest nonwinning page duplicate rolls back observations and winner rows together',async()=>{const store=await TitleIndex.create(()=>{});try{await store.append([],['old']);await assert.rejects(store.append([{id:'winner',mangaId:'p',order:[1]}],['new','old']));assert.equal((await store.page(0,100)).length,0);await store.append([{id:'winner',mangaId:'p',order:[1]}],['new']);assert.equal((await store.page(0,100)).length,1);}finally{await store.dispose();}});
test('duplicate chapter moved on a nonwinning latest-only page poisons scan',async()=>{
 let calls=0;const first=Array.from({length:100},(_,n)=>row(n,'p',n,'2026-10-22T12:00:00Z'));const f=await fixture(['p'],[],{latest:true,source:{async scan(){return++calls===1?{total:101,data:first}:{total:101,data:[{...first[0],_sort_value:200,published_at:'2025-10-22T12:00:00Z'}]};},async hydrate(){return[];}}});try{await assert.rejects(f.pager.load(),/storage failed/);assert.equal((await f.store.page(0,100)).length,0);}finally{await f.pager.dispose();}
});
test('title-only pager remains compatible with shared scratch schema',async()=>{const store=await TitleIndex.create(()=>{}),candidates=[{id:'title',status:'reading',followedAt:'2026-10-22T00:00:00Z'}],pager=new TitlePager(candidates,store,async()=>[{id:'title',_sort_value:7}],()=>{},20,'rating',1);try{const result=await pager.load();assert.equal(result.ready,true);assert.equal(result.total,1);assert.equal(result.data[0].id,'title');}finally{await pager.dispose();}});
test('invalid nullable publication time cannot select a fabricated latest winner',async()=>{const f=await fixture(['p'],[row(1,'p',null,null)],{latest:true});try{await assert.rejects(f.pager.load(),/saved-device date/);assert.equal((await f.store.page(0,100)).length,0);}finally{await f.pager.dispose();}});
const { IDBFactory, IDBObjectStore } = require(path.join(deps,'fake-indexeddb'));
const dbName='weebdex-temporary-title-order-v1';
function openDb(factory,version,upgrade){return new Promise((resolve,reject)=>{const request=factory.open(dbName,version);request.onupgradeneeded=()=>upgrade?.(request.result,request.transaction);request.onsuccess=()=>resolve(request.result);request.onerror=()=>reject(request.error);});}
function oldSchema(db){db.createObjectStore('owners',{keyPath:'id'});db.createObjectStore('keys',{keyPath:['owner','order']});}
test('blocked v1 upgrade retains open ownership until actual settlement, then safely reopens',async()=>{
 const factory=new IDBFactory();context.indexedDB=factory;context.setTimeout=(fn,ms)=>setTimeout(fn,ms===5000?20:ms===250?10:ms);
 const old=await openDb(factory,1,oldSchema);
 try{await assert.rejects(TitleIndex.create(()=>{}),/did not open in time/);await assert.rejects(TitleIndex.create(()=>{}),/previous title index opening/);old.close();await new Promise(resolve=>setTimeout(resolve,30));const store=await TitleIndex.create(()=>{});await store.dispose();}finally{old.close();context.indexedDB=indexedDB;context.setTimeout=setTimeout;}
});
test('failed v2 uniqueness migration aborts atomically and retains old scratch data',async()=>{
 const factory=new IDBFactory();context.indexedDB=factory;const old=await openDb(factory,1,(db,tx)=>{oldSchema(db);tx.objectStore('keys').add({owner:'old',id:'same',order:[1]});tx.objectStore('keys').add({owner:'old',id:'same',order:[2]});});old.close();
 try{await assert.rejects(TitleIndex.create(()=>{}),/could not open/);const retained=await openDb(factory,1);assert.equal(retained.objectStoreNames.contains('observations'),false);const request=retained.transaction('keys').objectStore('keys').count();const count=await new Promise((resolve,reject)=>{request.onsuccess=()=>resolve(request.result);request.onerror=()=>reject(request.error);});assert.equal(count,2);retained.close();}finally{context.indexedDB=indexedDB;}
});
test('versionchange closes the old connection and refuses continuation rather than reopening under it',async()=>{
 const factory=new IDBFactory();context.indexedDB=factory;const store=await TitleIndex.create(()=>{});const next=await openDb(factory,3);try{await assert.rejects(store.page(0,1),/storage failed/);await assert.rejects(store.dispose());}finally{next.close();context.indexedDB=indexedDB;}
});
test('quota failure rolls back observed identities and emitted keys without touching another owner',async()=>{
 const a=await TitleIndex.create(()=>{}),b=await TitleIndex.create(()=>{}),original=IDBObjectStore.prototype.add;
 try{await b.append([{id:'b',order:[1]}]);IDBObjectStore.prototype.add=function(value,...rest){if(this.name==='keys'&&value.id==='quota')throw new DOMException('quota','QuotaExceededError');return original.call(this,value,...rest);};await assert.rejects(a.append([{id:'quota',mangaId:'p',order:[1]}],['observed']),/storage failed/);IDBObjectStore.prototype.add=original;await a.append([{id:'ok',mangaId:'p',order:[1]}],['observed']);assert.equal((await a.page(0,100)).length,1);assert.equal((await b.page(0,100)).length,1);}finally{IDBObjectStore.prototype.add=original;await a.dispose();await b.dispose();}
});

test('latest-only selected publication fingerprint changes even when global sort key is unchanged',async()=>{const rows=[row(1,'p',7)],f=await fixture(['p'],rows,{latest:true});try{await f.pager.load();rows[0]={...rows[0],published_at:'2025-10-22T12:00:00Z'};await assert.rejects(f.pager.load(),/changed/);}finally{await f.pager.dispose();}});

test('partial-parent tails cannot overrun the 1,000 observed-row action admission',async()=>{const parents=Array.from({length:11},(_,n)=>'p'+n),rows=parents.flatMap((parent,n)=>Array.from({length:n===10?100:99},(_,i)=>row(n*100+i,parent,i))),f=await fixture(parents,rows);try{const first=await f.pager.load();assert.equal(first.chapters,990);assert.equal(first.scanned,10);assert.equal(first.total,null);assert.equal(f.calls.length,10);const second=await f.pager.load();assert.equal(second.chapters,1090);assert.equal(second.total,1090);assert.equal(f.calls[10].offset,0);assert.equal(f.calls[10].size,100);}finally{await f.pager.dispose();}});

test('public traversal permits unexposed tie ordering but still enforces primary monotonicity',async()=>{const rows=[row(2,'p',1),row(1,'p',1)];const f=await fixture(['p'],rows,{source:{primaryOrderOnly:true,async scan(){return{total:2,data:rows};},async hydrate(keys){return keys.map(key=>rows.find(row=>row.id===key.id));}}});try{const result=await finish(f.pager);assert.deepEqual(plain(result.data.map(row=>row.id)),['c000001','c000002']);}finally{await f.pager.dispose();}const changed=await fixture(['p'],[],{source:{primaryOrderOnly:true,async scan(){return{total:2,data:[row(1,'p',2),row(2,'p',1)]};},async hydrate(){return[];}}});try{await assert.rejects(changed.pager.load(),/changed/);}finally{await changed.pager.dispose();}});
