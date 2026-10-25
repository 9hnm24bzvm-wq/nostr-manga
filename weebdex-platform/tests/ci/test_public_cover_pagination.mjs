import test from 'node:test';
import assert from 'node:assert/strict';
import { paginationFixture, coverId, deferred, changed, actor, manga, row, json, bytes, plain } from './cover-pagination-fixture.mjs';

test('actual reader pages all 205 covers including an older main, one explicit bounded request per page',async()=>{
 const f=paginationFixture(),controller=await f.open(),first=await controller.read(),middle=await controller.read(100),last=await controller.read(200);
 assert.equal(f.reads().length,3);assert.deepEqual([first,middle,last].map(p=>[p.offset,p.limit,p.total,p.data.length]),[[0,100,205,100],[100,100,205,100],[200,100,205,5]]);
 assert.equal(new Set([...first.data,...middle.data,...last.data].map(row=>row.id)).size,205);
 assert.equal(last.data.at(-1).id,coverId(205));assert.equal(last.data.at(-1).main,true);
 for(const [index,request]of f.reads().entries()){
  assert.deepEqual([...new URL(request.url).searchParams],[['limit','100'],['offset',String(index*100)],['expectedAccountId',actor]]);
  assert.ok(f.proofs.some(proof=>proof.tags.some(tag=>tag[0]==='u'&&tag[1]===request.url)));
 }
 const saved=last.data.at(-1),next=await controller.update(saved,changed(saved));assert.equal(next.version,2);assert.equal(next.main,true);assert.deepEqual(plain(next.original),plain(saved.original));
 assert.deepEqual(new Uint8Array(await(await controller.preview(next)).arrayBuffer()),bytes);
 await controller.read(0);await assert.rejects(()=>controller.update(saved,changed(saved,'Off-page')),/Read the exact current cover/);
 assert.equal(f.writes().length,1);
});

test('offsets above 10,000 and 32-bit limits remain exact and bounded, never clamped to accessible history',async()=>{
 const f=paginationFixture(0),c=await f.open();
 for(const offset of[10100,4294967296,Number.MAX_SAFE_INTEGER]){const page=await c.read(offset);assert.equal(page.offset,offset);assert.equal(page.total,0);assert.equal(page.data.length,0);}
 const count=f.calls.length;for(const offset of[-1,1.5,NaN,Infinity,Number.MAX_SAFE_INTEGER+1])await assert.rejects(()=>c.read(offset),e=>e.status===422);assert.equal(f.calls.length,count);
});

for(const mode of['wrong-offset','wrong-limit','wrong-total','short-page','duplicate','foreign-parent','negative-total','unsafe-total'])test('actual reader rejects '+mode+' without retaining editable history',async()=>{
 const f=paginationFixture(),c=await f.open(),saved=(await c.read()).data[0];
 f.hook(request=>{if(!request.path.endsWith('/covers'))return;const data=plain(f.rows().slice(100,200)),body={result:'ok',response:'collection',data,limit:100,offset:100,total:205};
 if(mode==='wrong-offset')body.offset=0;if(mode==='wrong-limit')body.limit=99;if(mode==='wrong-total')body.total=199;if(mode==='short-page')data.pop();if(mode==='duplicate')data[1]=data[0];if(mode==='foreign-parent')data[0].relationships=[{type:'manga',id:actor}];if(mode==='negative-total')body.total=-1;if(mode==='unsafe-total')body.total=Number.MAX_SAFE_INTEGER+1;return json(body);});
 await assert.rejects(()=>c.read(100));await assert.rejects(()=>c.update(saved,changed(saved)),/Read the exact current cover/);assert.equal(f.writes().length,0);
});

test('read replacement holds mutation, overlapping page dispatch and title-version fences',async()=>{
 const f=paginationFixture(),c=await f.open(),saved=(await c.read()).data[0],gate=deferred();
 f.hook(async request=>{if(request.path.endsWith('/covers'))await gate.promise;});
 const loading=c.read(100);await assert.rejects(()=>c.read(200),/operation is in progress/);await assert.rejects(()=>c.update(saved,changed(saved)));
 f.title.version++;gate.release();await assert.rejects(()=>loading,/Title changed while reading covers/);assert.equal(f.writes().length,0);
});

test('same-title metadata changes require a fresh cover page before modifying a saved row',async()=>{
 const f=paginationFixture(),c=await f.open(),saved=(await c.read(200)).data[0];f.title.version++;
 await assert.rejects(()=>c.update(saved,changed(saved)),/Read the exact current cover/);assert.equal(f.writes().length,0);
 const fresh=(await c.read(200)).data[0];await c.update(fresh,changed(fresh));assert.equal(f.writes().length,1);
});

test('page reads preserve pending-command no-replay fence and changed account/service/navigation guards',async()=>{
 for(const mode of['account','service','title','route','disposed']){
  const f=paginationFixture(),c=await f.open(),saved=(await c.read(200)).data.at(-1);
  if(mode==='account')f.account.user.id=manga;
  if(mode==='service')f.policy.service_key='c'.repeat(64);
  if(mode==='title')f.title.id=actor;
  if(mode==='route')f.state.active=false;
  if(mode==='disposed')c.dispose();
  await assert.rejects(()=>c.update(saved,changed(saved)));assert.equal(f.writes().length,0);
 }
 const f=paginationFixture(),c=await f.open(),saved=(await c.read(200)).data[0];
 f.hook(request=>{if(request.init.method==='PUT')return json({},503);});
 await assert.rejects(()=>c.update(saved,changed(saved)));assert.equal(c.pending(),true);
 await c.read(0);await c.read(200);assert.equal(c.pending(),true);await assert.rejects(()=>c.update(saved,changed(saved)));assert.equal(f.writes().length,1);
});

test('removed last-page records disappear on explicit reload and old versions cannot be reused',async()=>{
 const f=paginationFixture(101),c=await f.open(),saved=(await c.read(100)).data[0];await c.remove(saved);
 const empty=await c.read(100);assert.equal(empty.total,100);assert.equal(empty.data.length,0);
 await assert.rejects(()=>c.update(saved,changed(saved)));const previous=await c.read(0);assert.equal(previous.data.length,100);assert.equal(f.writes().length,1);
});
