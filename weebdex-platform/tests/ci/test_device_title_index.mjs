import { retainedTitleHistorySource } from './retained_title_history_source.mjs';
// Finite real module + IndexedDB API tests. No browser, sockets or live service.
import test, { after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import vm from 'node:vm';
import { createRequire, stripTypeScriptTypes } from 'node:module';
import { fileURLToPath } from 'node:url';
import { webcrypto } from 'node:crypto';
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const deps = process.env.WEEBDEX_BROWSER_DOM_NODE_MODULES;
assert(deps, 'Use the existing exact-lock dependency root');
const require = createRequire(path.join(deps, '../package.json'));
const { indexedDB, IDBKeyRange, IDBObjectStore, IDBDatabase } = require(path.join(deps, 'fake-indexeddb'));
let fastStorageClock = false;
const storageTimer = (fn, ms) => setTimeout(fn, fastStorageClock && ms === 5000 ? 100 : fastStorageClock && ms === 250 ? 10 : ms);
const context = vm.createContext({ indexedDB, IDBKeyRange, crypto: webcrypto, URLSearchParams, URL, Date, AbortController, setTimeout: storageTimer, clearTimeout, console, TextEncoder, TextDecoder });
const modules = new Map();
async function load(file) {
  const full = path.resolve(root, file); if (modules.has(full)) return modules.get(full);
  let source = fs.readFileSync(full, 'utf8');
  if (full.endsWith('.ts')) source = stripTypeScriptTypes(source, { mode: 'transform' });
  const module = new vm.SourceTextModule(source, { context, identifier: full }); modules.set(full, module);
  await module.link(specifier => specifier === './site-client' ? new vm.SyntheticModule(['publicCatalogueRead'], function () { this.setExport('publicCatalogueRead', () => { throw Error('Unexpected global catalogue transport'); }); }, { context }) : load(path.relative(root, path.resolve(path.dirname(full), specifier + (path.extname(specifier) ? '' : '.ts'))))); return module;
}
const order = await load('platform-integrations/local-library/title-order.ts'); await order.evaluate();
const storage = await load('platform-integrations/local-library/title-index.ts'); await storage.evaluate();
const pagination = await load('platform-integrations/local-library/title-pagination.ts'); await pagination.evaluate();
const exact = await load('platform-integrations/public-api/exact-order.ts'); if (exact.status !== 'evaluated') await exact.evaluate();
const { titleOrderKey } = order.namespace, { TitleIndex, TITLE_INDEX_LIMITS } = storage.namespace, { TitlePager } = pagination.namespace;
const { compareDecimal } = exact.namespace;
const plain = value => JSON.parse(JSON.stringify(value));
const uuid = n => 'aaaaaaaa-aaaa-8aaa-aaaa-' + String(n).padStart(12, '0');
const compareValue = (a, b) => typeof a === 'number' && typeof b === 'number' ? a - b : String(a ?? '') < String(b ?? '') ? -1 : String(a ?? '') > String(b ?? '') ? 1 : 0;
// Execute the ACTUAL existing comparator, not a reimplementation oracle.
const compareSource = fs.readFileSync(path.join(root, 'platform-integrations/local-library/catalog.ts'), 'utf8');
const start = compareSource.indexOf('export function compareCatalogueRows');
vm.runInContext(stripTypeScriptTypes(compareSource.slice(start).replace('export function', 'function'), { mode: 'transform' }), context);
context.compareDecimal = compareDecimal; context.compareValue = compareValue;
const comparator = vm.runInContext('compareCatalogueRows', context);
const opened = new Set();
const index = async () => { const value = await TitleIndex.create(() => {}); opened.add(value); return value; };
const close = async value => { await value.dispose(); opened.delete(value); };
after(async () => { for (const value of opened) await value.dispose(); });
for (const direction of [1, -1]) {
  for (const [kind, values] of [
    ['decimal', ['0', '-0', '0.00', '1', '-1', '1.2', '1.20', '1.234', '-1.2', '-1.234', '0.001', '-0.001', '9007199254740993', '9007199254740992', '-99999999999999999999999999999999999999999', '00012.0300', null, '0.000000000000000000000000001']],
    ['title', ['61', '6161', '62', 'c3a9', 'e697a5', 'f09f9880', 'c385', '41', null]],
    ['timestamp', ['2026-01-01T00:00:00.000001Z', '2026-01-01T00:00:00.000002Z', '2026-01-01T00:00:00.000001Z', null]]
  ]) test(`actual IDB cursor parity with compareCatalogueRows: ${kind} ${direction}`, async () => {
    const rows = values.map((value, i) => ({ id: uuid(i), _sort_value: value, _sort_decimal: kind === 'decimal', _sort_text: '' })).reverse();
    const expected = [...rows].sort((a, b) => comparator(a, b, direction)).map(row => row.id), store = await index();
    try { await store.append(rows.map(row => ({ id: row.id, order: titleOrderKey(row, direction) }))); assert.deepEqual(plain((await store.page(0, 100)).map(row => row.id)), expected); assert.deepEqual(plain((await store.page(2, 3)).map(row => row.id)), expected.slice(2, 5)); } finally { await close(store); }
  });
}
test('IDB duplicate batch rolls back count, keys and bytes; exact-owner disposal leaves another owner intact', async () => {
  const a = await index(), b = await index(), row = { id: uuid(1), order: [0, 'a', '', uuid(1)] };
  try { await a.append([row]); await b.append([row]); await assert.rejects(a.append([row])); assert.equal((await a.page(0, 100)).length, 1); await close(a); assert.equal((await b.page(0, 100)).length, 1); } finally { await close(a); await close(b); }
});
test('owner count is aggregate in actual IDB, not per tab memory', async () => {
  const stores = [];
  try { for (let i = 0; i < TITLE_INDEX_LIMITS.owners; i++) stores.push(await index()); await assert.rejects(TitleIndex.create(() => {}), /Too many temporary/); } finally { for (const store of stores) await close(store); }
});
const candidates = n => Array.from({ length: n }, (_, i) => ({ id: uuid(i + 1), followedAt: '2026-01-01T00:00:00Z', status: 'reading' }));
const row = candidate => ({ id: candidate.id, _sort_value: String(1000000 - Number(candidate.id.slice(-12))), _sort_decimal: true, title: 'Title ' + candidate.id });
test('301 candidates need full consideration before any global sort/total; page hydration stays <=100', async () => {
  const store = await index(), calls = [], pager = new TitlePager(candidates(301), store, async batch => { calls.push(batch.length); return batch.map(row); }, () => {}, 20, 'rating', 1);
  try {
    for (let i = 0; i < 3; i++) { const result = await pager.load(); assert.equal(result.total, null); assert.equal(result.data.length, 0); assert.equal(result.ready, false); }
    const indexed = await pager.load(); assert.equal(indexed.total, 301); assert.equal(indexed.data.length, 0); assert.equal(indexed.ready, false);
    const first = await pager.load(); assert.equal(first.data[0].id, uuid(301)); assert.equal(first.data.at(-1).id, uuid(282));
    const last = await pager.load(16); assert.equal(last.data[0].id, uuid(1)); assert.deepEqual(calls, [100,100,100,1,20,1]);
  } finally { await close(store); }
});
test('sparse matching and missing identities are distinct from failed batch; failure does not advance', async () => {
  const store = await index(); let fail = true; const seen = [];
  const pager = new TitlePager(candidates(201), store, async batch => { seen.push(batch[0].id); if (batch[0].id === uuid(101) && fail) throw Error('503'); return batch.filter(key => Number(key.id.slice(-12)) > 199).map(row); }, () => {}, 20, 'rating', 1);
  try { assert.equal((await pager.load()).total, null); await assert.rejects(pager.load(), /503/); fail = false; await pager.load(); await pager.load(); const result = await pager.load(); assert.equal(result.total, 2); assert.deepEqual(plain(result.data.map(row => row.id)), [uuid(201), uuid(200)]); assert.deepEqual(seen.slice(0, 4), [uuid(1),uuid(101),uuid(101),uuid(201)]); } finally { await close(store); }
});
for (const change of ['key', 'visibility']) test(`lazy page ${change} drift requires rebuild, never silently skips`, async () => {
  const store = await index(); let changed = false;
  const pager = new TitlePager(candidates(101), store, async batch => batch.flatMap(key => changed && change === 'visibility' ? [] : [{ ...row(key), ...(changed ? { _sort_value: '0' } : {}) }]), () => {}, 20, 'rating', 1);
  try { await pager.load(); await pager.load(); changed = true; await assert.rejects(pager.load(), /Rebuild/); await assert.rejects(pager.load(), /Rebuild/); } finally { await close(store); }
});
test('in-flight owner change cannot commit a stale index batch', async () => {
  const store = await index(); let resolve, current = true;
  const pager = new TitlePager(candidates(101), store, batch => new Promise(done => { resolve = () => done(batch.map(row)); }), () => { if (!current) throw Error('account/revision/query changed'); }, 20, 'rating', 1);
  try { const pending = pager.load(); current = false; resolve(); await assert.rejects(pending, /changed/); assert.equal((await store.page(0,100)).length, 0); } finally { await close(store); }
});
test('executed original Search10001 control rejects before network; new pager admits only100 candidates at a time', async () => {
  const baseline = retainedTitleHistorySource('a8d428b-MangaFollow');
  const oldFunction = baseline.slice(baseline.indexOf('export async function Search'), baseline.indexOf('export async function LatestFeed')).replace('export async function', 'async function');
  const catalogue = await load('platform-integrations/public-api/library-catalogue.ts'); await catalogue.evaluate();
  Object.assign(context, { assertCataloguePreviewLibraryReady() {}, openLibrary: async () => {}, params: value => new URLSearchParams(value), captureLibraryContext: () => ({ mode: 'device' }), captureLibraryView: () => 'device', currentDeviceSnapshot: async () => ({ records: candidates(10001).map(row => ({ ...row, family: 'series' })) }), assertLibraryContext() {}, isCurrentCatalogueId: () => true, filteredCatalog: (...args) => catalogue.namespace.publicLibraryCandidates(...args), assertLibraryView() {}, HTTPError: Error });
  vm.runInContext(stripTypeScriptTypes(oldFunction, { mode: 'transform' }), context);
  await assert.rejects(vm.runInContext('Search(new URLSearchParams())', context), /bounded catalogue budget/);
  const store = await index(); let count = 0;
  const pager = new TitlePager(candidates(10001), store, async batch => { count += batch.length; return batch.map(row); }, () => {}, 20, 'rating', 1);
  try { const result = await pager.load(); assert.equal(count, 100); assert.equal(result.scanned, 100); assert.equal(result.candidates, 10001); assert.equal(result.total, null); assert.equal(result.data.length, 0); } finally { await close(store); }
});
for (const count of [30000, 500000]) test(`${count} local candidate keys admit one100-ID action without claiming a complete index or capacity acceptance`, async () => {
  const store = await index(); let hydrated = 0;
  const pager = new TitlePager(candidates(count), store, async batch => { hydrated += batch.length; return batch.map(row); }, () => {}, 20, 'rating', -1);
  try { const result = await pager.load(); assert.equal(hydrated, 100); assert.equal(result.scanned, 100); assert.equal(result.candidates, count); assert.equal(result.total, null); assert.equal(result.indexed, false); assert.equal(result.data.length, 0); } finally { await close(store); }
});
// Metadata fault injection below is test-only, and uses the same real IDB API
// transaction path as other callers. It never touches the saved library DB.
const scratchDatabase = 'weebdex-temporary-title-order-v1';
async function openScratch(version = 2) {
  return new Promise((resolve, reject) => {
    const q = indexedDB.open(scratchDatabase, version);
    q.onsuccess = () => { q.result.onversionchange = () => q.result.close(); resolve(q.result); };
    q.onerror = () => reject(q.error);
  });
}
async function metadata(operation) {
  // Production upgrades this scratch DB to schema 2. A version-1 open fails
  // before fault injection, so it cannot test rollback or lifecycle ownership.
  const db = await openScratch();
  assert.equal(db.version, 2);
  assert.deepEqual([...db.objectStoreNames], ['keys', 'observations', 'owners']);
  try { return await new Promise((resolve, reject) => { const tx = db.transaction('owners', 'readwrite'), store = tx.objectStore('owners'), q = store.getAll(); let value; q.onsuccess = () => { value = operation(q.result, store); }; tx.oncomplete = () => resolve(value); tx.onabort = () => reject(tx.error); }); } finally { db.close(); }
}
async function scratchRows() {
  const db = await openScratch();
  try {
    return await new Promise((resolve, reject) => {
      const tx = db.transaction(['owners', 'keys', 'observations'], 'readonly'), rows = {};
      tx.oncomplete = () => resolve(rows);
      tx.onabort = () => reject(tx.error ?? Error('Scratch inspection aborted'));
      for (const name of ['owners', 'keys', 'observations']) {
        const q = tx.objectStore(name).getAll(); q.onsuccess = () => { rows[name] = q.result; };
      }
    });
  } finally { db.close(); }
}
test('prior version-1 metadata fixture rejects schema 2 before its lifecycle assertion can run', async () => {
  const store = await index();
  try {
    await store.append([{ id: uuid(1), order: [0, 'a', '', uuid(1)] }], [uuid(2)]);
    const before = await scratchRows();
    await assert.rejects(openScratch(1), error => error.name === 'VersionError');
    assert.deepEqual(await scratchRows(), before);
    assert.equal((await metadata(rows => rows))[0].count, 2);
  } finally { await close(store); }
});
test('schema-2 observations and order keys share atomic rollback, identity uniqueness and exact-owner cleanup', async () => {
  const a = await index(), b = await index(), entry = { id: uuid(1), order: [0, 'a', '', uuid(1)] };
  try {
    await a.append([entry], [uuid(2)]); await b.append([entry], [uuid(2)]);
    const before = await scratchRows();
    const db = await openScratch();
    try {
      const tx = db.transaction(['keys', 'observations'], 'readonly'), keys = tx.objectStore('keys');
      assert.deepEqual(plain(keys.keyPath), ['owner', 'order']);
      assert.deepEqual(plain(keys.index('identity').keyPath), ['owner', 'id']); assert.equal(keys.index('identity').unique, true);
      assert.deepEqual(plain(tx.objectStore('observations').keyPath), ['owner', 'id']);
      await new Promise((resolve, reject) => { tx.oncomplete = resolve; tx.onabort = () => reject(tx.error); });
    } finally { db.close(); }
    // A new observation is queued first, then a moved key violates the real
    // unique [owner,id] index. Neither it nor owner accounting may commit.
    await assert.rejects(a.append([{ ...entry, order: [0, 'b', '', uuid(1)] }], [uuid(3)]), /saved library is unchanged/);
    assert.deepEqual(await scratchRows(), before);
    await assert.rejects(a.append([{ id: uuid(4), order: [0, 'c', '', uuid(4)] }], [uuid(2)]), /saved library is unchanged/);
    assert.deepEqual(await scratchRows(), before);
    assert.equal(before.owners.length, 2); assert.equal(before.keys.length, 2); assert.equal(before.observations.length, 2);
    for (const owner of before.owners) { assert.equal(owner.count, 2); assert.ok(owner.bytes > 0); }
    await close(a);
    const remaining = await scratchRows();
    assert.equal(remaining.owners.length, 1); assert.equal(remaining.keys.length, 1); assert.equal(remaining.observations.length, 1);
    assert.equal(remaining.keys[0].owner, remaining.owners[0].id); assert.equal(remaining.observations[0].owner, remaining.owners[0].id);
    assert.equal((await b.page(0, 10))[0].id, entry.id);
    await close(b); assert.deepEqual(await scratchRows(), { owners: [], keys: [], observations: [] });
  } finally { await close(a); await close(b); }
});
test('per-index and aggregate storage admission rolls back without dropping an owner or accepting partial keys', async () => {
  const a = await index(), b = await index(), c = await index(), d = await index(), entry = { id: uuid(1), order: [0, 'a', '', uuid(1)] };
  try {
    await metadata((rows, store) => { for (const value of rows) store.put({ ...value, bytes: TITLE_INDEX_LIMITS.bytes }); });
    await assert.rejects(a.append([entry]), /storage budget/); assert.equal((await a.page(0, 10)).length, 0);
    await metadata((rows, store) => { for (const value of rows) store.put({ ...value, bytes: TITLE_INDEX_LIMITS.totalBytes / 4 }); });
    await assert.rejects(a.append([entry]), /storage budget/); assert.equal((await a.page(0, 10)).length, 0);
    await metadata((rows, store) => { for (const value of rows) store.put({ ...value, bytes: 0 }); });
    await a.append([entry]); assert.equal((await a.page(0, 10)).length, 1);
  } finally { await close(a); await close(b); await close(c); await close(d); }
});
test('idle cleanup cannot delete a renewed owner, but old exact owner cannot erase its replacement', async () => {
  const a = await index(); let b;
  try {
    await a.append([{ id: uuid(1), order: [0,'a','',uuid(1)] }]);
    await metadata((rows, store) => rows.forEach(value => store.put({ ...value, touched: Date.now() - TITLE_INDEX_LIMITS.idleMs - 1 })));
    await a.page(0, 1); // Actual renewed lease, observed by the next transaction.
    b = await index(); assert.equal((await a.page(0, 1)).length, 1);
    await metadata((rows, store) => { const value = rows.find(value => value.count === 1); store.put({ ...value, token: 'replacement-token', lease: value.lease + 1 }); });
    await close(a); const rows = await metadata(values => values); assert.equal(rows.length, 2); assert.ok(rows.some(value => value.token === 'replacement-token'));
    await metadata((rows, store) => { for (const value of rows) if (value.token === 'replacement-token') store.put({ ...value, touched: 0 }); });
    const c = await index(); await close(c);
  } finally { await close(a); if (b) await close(b); }
});

test('actual storage quota exception aborts the whole key batch and reports a rebuild without advancing', async () => {
  const store = await index(), add = IDBObjectStore.prototype.add, before = await scratchRows();
  try {
    IDBObjectStore.prototype.add = function (...args) { if (this.name === 'keys') throw new DOMException('Injected quota failure', 'QuotaExceededError'); return add.apply(this,args); };
    await assert.rejects(store.append([{id:uuid(1),order:[0,'a','',uuid(1)]}], [uuid(2)]), /saved library is unchanged/);
    assert.deepEqual(await scratchRows(), before);
    IDBObjectStore.prototype.add = add;
    assert.equal((await store.page(0,100)).length,0);
    const rows=await metadata(values=>values);assert.equal(rows[0].count,0);assert.equal(rows[0].bytes,0);
  } finally { IDBObjectStore.prototype.add=add;await close(store); }
});

for (const direction of [1,-1]) test(`followedAt first page is locally ordered before whole-catalogue indexing: direction ${direction}`, async () => {
 const keys=candidates(301);keys[148]={...keys[148],followedAt:'2027-01-01T00:00:00Z'};keys[149]={...keys[149],followedAt:'2027-01-01T00:00:00Z'};
 const store=await index(),calls=[];const pager=new TitlePager(keys.reverse(),store,async batch=>{calls.push(batch.length);return batch.map(row).reverse();},()=>{},20,'followedAt',direction);
 try{const first=await pager.load();assert.equal(first.ready,true);assert.equal(first.total,null);assert.equal(first.scanned,100);assert.equal(first.localOrder,true);assert.equal(first.data[0].id,uuid(direction===1?1:149));assert.equal(first.data[1].id,uuid(direction===1?2:150));pager.releaseDisplayedPage();const second=await pager.load(2);assert.equal(second.ready,true);assert.deepEqual(calls,[100,20]);}finally{await close(store);}
});
test('followedAt sparse scans never truncate/filter before global local ordering or invent a total',async()=>{
 const store=await index(),pager=new TitlePager(candidates(301).reverse(),store,async batch=>batch.filter(key=>Number(key.id.slice(-12))>299).map(row),()=>{},20,'followedAt',-1);
 try{for(let i=0;i<3;i++){const result=await pager.load();assert.equal(result.ready,false);assert.equal(result.total,null);assert.equal(result.data.length,0);}const indexed=await pager.load();assert.equal(indexed.total,2);const final=indexed.ready?indexed:await pager.load();assert.deepEqual(plain(final.data.map(row=>row.id)),[uuid(300),uuid(301)]);}finally{await close(store);}
});
test('executed3df baseline needs full indexing for followedAt; revised pager provides the first bounded page',async()=>{
 const source=retainedTitleHistorySource('3df4223-title-pagination');
 const identifier=path.join(root,'platform-integrations/local-library/title-pagination-before.ts');const module=new vm.SourceTextModule(stripTypeScriptTypes(source,{mode:'transform'}),{context,identifier});
 await module.link(specifier=>load(path.relative(root,path.resolve(path.dirname(identifier),specifier+'.ts'))));await module.evaluate();
 for(const[Class,expected]of [[module.namespace.TitlePager,false],[TitlePager,true]]){const store=await index(),pager=new Class(candidates(30000),store,async batch=>batch.map(row),()=>{},20);try{const result=await pager.load();assert.equal(result.ready,expected);assert.equal(result.total,null);assert.equal(result.data.length,expected?20:0);assert.equal(result.scanned,100);}finally{await close(store);}}
});
test('old displayed DTO plus new pending DTO share one retention budget; explicit release admits replacement',async()=>{
 for(const release of [false,true]){const store=await index(),pager=new TitlePager(candidates(201),store,async batch=>batch.filter(key=>[uuid(1),uuid(101)].includes(key.id)).map(key=>({...row(key),description:'x'.repeat(750000)})),()=>{},1);
  try{assert.equal((await pager.load()).data.length,1);if(release)pager.releaseDisplayedPage();if(release)assert.equal((await pager.load(2)).data[0].id,uuid(101));else await assert.rejects(pager.load(2),/budget/);}finally{await close(store);}}
});

function holdTerminals(kind) {
 const original=IDBDatabase.prototype.transaction, callbacks=[];
 IDBDatabase.prototype.transaction=function(...args){const tx=original.apply(this,args);return new Proxy(tx,{get(target,key){const value=Reflect.get(target,key,target);return typeof value==='function'?value.bind(target):value;},set(target,key,value){if(key===kind&&typeof value==='function')return Reflect.set(target,key,event=>callbacks.push(()=>value(event)),target);return Reflect.set(target,key,value,target);}});};
 return {callbacks,restore(){IDBDatabase.prototype.transaction=original;},release(){for(const done of callbacks.splice(0))done();}};
}
const pause=ms=>new Promise(resolve=>setTimeout(resolve,ms));
test('index cursor cannot advance before confirmed commit even when all requests already succeeded',async()=>{
 const store=await index(),pager=new TitlePager(candidates(101),store,async batch=>batch.map(row),()=>{},20,'rating',1),held=holdTerminals('oncomplete');let done=false;
 try{const pending=pager.load().then(value=>{done=true;return value;});await pause(30);assert.equal(done,false);assert.equal(pager.offset,0);assert.equal(held.callbacks.length,1);held.restore();held.release();const result=await pending;assert.equal(result.scanned,100);assert.equal(pager.offset,100);}finally{held.restore();held.release();await close(store);}
});
for(const terminal of ['oncomplete','onabort'])test(`unconfirmed ${terminal} poisons further work without advancing; late terminal remains observed`,async()=>{
 const store=await index(),pager=new TitlePager(candidates(101),store,async batch=>batch.map(row),()=>{},20,'rating',1),held=holdTerminals(terminal),add=IDBObjectStore.prototype.add;fastStorageClock=true;
 if(terminal==='onabort')IDBObjectStore.prototype.add=function(...args){if(this.name==='keys')throw new DOMException('quota','QuotaExceededError');return add.apply(this,args);};
 try{await assert.rejects(pager.load(),/cleanup is unconfirmed/);assert.equal(pager.offset,0);await assert.rejects(store.page(0,1),/cleanup is unconfirmed/);assert.ok(held.callbacks.length);held.restore();IDBObjectStore.prototype.add=add;held.release();await pause(1);fastStorageClock=false;assert.equal((await store.page(0,100)).length,terminal==='oncomplete'?100:0);await assert.rejects(pager.load(),/cleanup is unconfirmed/);}finally{fastStorageClock=false;held.restore();IDBObjectStore.prototype.add=add;held.release();await close(store);}
});
test('create and dispose own finite terminal waits; repeated disposal never invents confirmed cleanup',async()=>{
 let held=holdTerminals('oncomplete');fastStorageClock=true;
 try{await assert.rejects(TitleIndex.create(()=>{}),/cleanup is unconfirmed/);held.restore();held.release();await pause(1);await metadata((rows,store)=>rows.forEach(row=>store.put({...row,touched:0})));}finally{held.restore();held.release();fastStorageClock=false;}
 const store=await index();held=holdTerminals('oncomplete');fastStorageClock=true;
 try{const pending=store.dispose();await assert.rejects(pending,/cleanup is unconfirmed/);assert.equal(store.dispose(),pending);await assert.rejects(TitleIndex.create(()=>{}),/cleanup is unconfirmed/);held.restore();held.release();await pause(1);const next=await index();await close(next);}finally{fastStorageClock=false;held.restore();held.release();opened.delete(store);}
});

test('incomplete stored page selection is rejected rather than claiming an exact partial page',async()=>{const store=await index(),pager=new TitlePager(candidates(101),store,async batch=>batch.map(row),()=>{},20,'rating',1);try{await pager.load();await pager.load();const page=store.page.bind(store);store.page=async(...args)=>(await page(...args)).slice(1);await assert.rejects(pager.load(),/Rebuild/);}finally{await close(store);}});

async function historicalPager(revision) {
 const source=retainedTitleHistorySource(revision+'-title-pagination');
 const identifier=path.join(root,'platform-integrations/local-library/title-pagination-'+revision+'.ts'),module=new vm.SourceTextModule(stripTypeScriptTypes(source,{mode:'transform'}),{context,identifier});
 await module.link(specifier=>load(path.relative(root,path.resolve(path.dirname(identifier),specifier+'.ts'))));await module.evaluate();return module.namespace.TitlePager;
}
for(const direction of [1,-1])test(`followedAt compares offset instants, equivalent fractions and sub-ms digits with ID ties: ${direction}`,async()=>{
 const dates=['2026-01-01T01:00:00+02:00','2026-01-01T00:00:00Z','2026-01-01T00:00:00.100Z','2026-01-01T01:00:00.1+01:00','2026-01-01T00:00:00.100000002Z','2026-01-01T00:00:00.100000001Z'];
 const keys=dates.map((followedAt,i)=>({id:uuid(i+1),followedAt,status:'reading'})),expected=(direction===1?[1,2,3,4,6,5]:[5,6,3,4,2,1]).map(uuid),Old=await historicalPager('08c3bb9');
 for(const[Class,correct]of [[Old,false],[TitlePager,true]]){const store=await index(),pager=new Class([...keys].reverse(),store,async batch=>batch.map(row).reverse(),()=>{},20,'followedAt',direction);try{const first=await pager.load();const ids=plain(first.data.map(row=>row.id));if(correct){assert.deepEqual(ids,expected);for(const data of first.data)assert.equal(data.followed_at,keys.find(key=>key.id===data.id).followedAt);pager.releaseDisplayedPage();const next=await pager.load();assert.deepEqual(plain(next.data.map(row=>row.id)),expected);}else assert.notDeepEqual(ids,expected);}finally{await close(store);}}
});
test('followedAt instant key preserves valid legacy dates, negative epochs, long ISO fractions and rejects invalid input',()=>{
 const {followedAtInstant,followedAtOrderKey}=order.namespace;
 const ingestion=fs.readFileSync(path.join(root,'platform-integrations/public-api/device-id-state.ts'),'utf8').split('\n').find(line=>line.startsWith('function date('));
 const acceptedDate=vm.runInNewContext(stripTypeScriptTypes(ingestion)+';date');assert.equal(acceptedDate('2026-01-01T01:00:00.1+01:00'),'2026-01-01T01:00:00.1+01:00');
 assert.deepEqual(plain(followedAtInstant('Thu, 01 Jan 1970 00:00:00 GMT')),[0,'0'.repeat(40)]);
 assert.deepEqual(plain(followedAtInstant('1969-12-31T23:59:59.9999Z')),[-1,'9999'.padEnd(40,'0')]);
 assert(indexedDB.cmp(followedAtOrderKey(uuid(1),'2026-01-01T00:00:00.0000000000000000001Z',1),followedAtOrderKey(uuid(2),'2026-01-01T00:00:00.0000000000000000002Z',1))<0);
 for(const value of ['not a date','2026-01-01T00:00:00.000000000000000000001Z'])assert.throws(()=>followedAtInstant(value),/Invalid/);
});
test('scope change after request success but before observed commit prevents cursor advance and publishing',async()=>{
 let current=true;const check=()=>{if(!current)throw Error('scope changed after requests');},store=await TitleIndex.create(check);opened.add(store);const pager=new TitlePager(candidates(101),store,async batch=>batch.map(row),check,20,'rating',1),held=holdTerminals('oncomplete');
 try{const pending=pager.load();await pause(40);assert.equal(held.callbacks.length,1);current=false;held.restore();held.release();await assert.rejects(pending,/scope changed/);assert.equal(pager.offset,0);assert.equal(pager.matches,0);assert.equal((await metadata(rows=>rows))[0].count,100);await close(store);assert.equal((await metadata(rows=>rows)).length,0);}finally{held.restore();held.release();await close(store);}
});
test('scope change during delayed creation is cleaned by its exact owner before rejection',async()=>{
 let current=true;const held=holdTerminals('oncomplete');try{const pending=TitleIndex.create(()=>{if(!current)throw Error('scope changed during open');});await pause(30);assert.equal(held.callbacks.length,1);current=false;held.restore();held.release();await assert.rejects(pending,/saved library is unchanged/);assert.equal((await metadata(rows=>rows)).length,0);}finally{held.restore();held.release();}
});
test('empty candidate set is exact without hydration; missing stored selection never becomes an empty successful page',async()=>{
 const empty=await index();try{const pager=new TitlePager([],empty,async()=>{throw Error('unscoped hydration');},()=>{}),result=await pager.load();assert.equal(result.ready,true);assert.equal(result.total,0);assert.equal(result.data.length,0);}finally{await close(empty);}
 const store=await index(),pager=new TitlePager(candidates(101),store,async batch=>batch.map(row),()=>{},20,'rating',1);try{await pager.load();await pager.load();store.page=async()=>[];await assert.rejects(pager.load(),/Rebuild/);}finally{await close(store);}
});

for(const direction of [1,-1])test(`bounded decimal grid cursor parity covers signed magnitude, scale and equivalent encodings: ${direction}`,async()=>{const values=[];for(const magnitude of [0,1,5,32,120])for(const fraction of ['', '1', '001', '9'.repeat(120)])for(const sign of ['', '-']){const whole='9'.repeat(magnitude)||'0',value=sign+whole+(fraction?'.'+fraction:'');values.push(value,sign+'00'+whole+(fraction?'.'+fraction+'00':'.00'));}values.push(null,'1.20','1.200');const rows=values.map((value,i)=>({id:uuid(i),_sort_value:value,_sort_decimal:true})),expected=[...rows].sort((a,b)=>comparator(a,b,direction)).map(row=>row.id),store=await index();try{await store.append(rows.map(row=>({id:row.id,order:titleOrderKey(row,direction)})));assert.deepEqual(plain((await store.page(0,100)).map(row=>row.id)),expected);}finally{await close(store);}});

test('real versionchange closes the title-index connection instead of blocking scratch database deletion', async () => {
 const store = await index();
 try {
  await store.append([{ id: uuid(1), order: [0, 'a', '', uuid(1)] }], [uuid(2)]);
  await new Promise((resolve, reject) => {
   const query = indexedDB.deleteDatabase(scratchDatabase);
   const timer = setTimeout(() => reject(Error('Title index did not close on versionchange')), 1000);
   query.onsuccess = () => { clearTimeout(timer); resolve(); };
   query.onerror = () => { clearTimeout(timer); reject(query.error); };
  });
  await assert.rejects(store.page(0, 1), /saved library is unchanged/);
  await assert.rejects(store.dispose(), error => error.name === 'InvalidStateError');
 } finally {
  // A failed/blocked deletion must still close the live connection. The known
  // closed-connection rejection is asserted above, then safely observed again.
  await store.dispose().catch(error => { if (error.name !== 'InvalidStateError') throw error; });
  opened.delete(store);
 }
 // A new actual schema-2 scratch DB remains usable after connection invalidation.
 const next = await index(); try { assert.equal((await next.page(0, 1)).length, 0); } finally { await close(next); }
});
