#!/usr/bin/env node
// Real original account-action/signing/IDB/transport modules and signatures.
// The lifetime callback models dialog ownership; the compiled parent connection
// is tested separately. No browser, extension, service or real network starts.
import assert from 'node:assert/strict';
import test from 'node:test';
import { fixture } from './library-management-fixture.mjs';

async function prepared(options) {
  const f = await fixture(options), identity = await f.load('protocol-identity.ts');
  const selected = f.publication.selectedSigner();
  const event = await f.nostr.signExact(selected.signer, f.owner, { kind: 0, created_at: Math.floor(Date.now()/1000), tags: [], content: '{}' });
  const review = identity.reviewWithdrawal(f.review.recipients[0], event);
  f.calls.kinds.length = 0;
  return { f, identity, review };
}

test('closing at deferred actual action invocation prevents a new signature', {timeout:5000}, async()=>{
  const {f,identity,review}=await prepared();let alive=true,checks=0;
  const current=()=>{if(!alive)throw Error('Dialog closed');if(++checks===4)queueMicrotask(()=>{alive=false})};
  try{await assert.rejects(identity.sendReviewed(review,true,current),/Dialog closed/);assert.equal(checks,4);assert.deepEqual(f.calls.kinds,[]);assert.equal((await identity.publicActionRecords()).length,0);assert.equal(f.calls.posts,0);}finally{await f.dispose()}
});

test('closing during action signature retains the exact signed intent before stopping delivery', {timeout:5000}, async()=>{
  const {f,identity,review}=await prepared({gateAt:'action'});
  try{const sending=identity.sendReviewed(review,true,f.assertCurrent);await f.entered;f.close();f.release();await assert.rejects(sending,/Management closed/);const records=await identity.publicActionRecords();assert.equal(records.length,1);assert(f.nostr.verifyEvent(records[0].event));assert.equal(records[0].event.kind,5);assert.deepEqual(JSON.parse(JSON.stringify(records[0].event.tags)),JSON.parse(JSON.stringify(review.tags)));assert.equal(records[0].relay.origin,review.relay.origin);assert.equal(records[0].state,'prepared');assert.equal(f.calls.posts,0);assert.deepEqual(f.calls.kinds,[5]);}finally{f.release();await f.dispose()}
});

test('closing before fresh HTTP proof invocation does not ask for another signature', {timeout:5000},async()=>{
  const {f,identity,review}=await prepared();
  try{
    const selected=f.publication.selectedSigner(),event=await f.nostr.signExact(selected.signer,f.owner,{kind:5,created_at:Math.floor(Date.now()/1000),tags:review.tags,content:''});
    const idb=await f.load('protocol/indexeddb.js');await idb.transaction(f.protocol.db,'meta','readwrite',async tx=>{tx.objectStore('meta').put([{event,relay:review.relay,path:'/account-deletions',purpose:review.purpose,state:'prepared',message:'Saved',cancelled:false,createdAt:Date.now()}],'weebdexSignedPublicActions-v1')});
    f.calls.kinds.length=0;let alive=true,checks=0;const current=()=>{if(!alive)throw Error('Dialog closed');if(++checks===4)queueMicrotask(()=>{alive=false})};
    await assert.rejects(identity.deliverPublicAction(event.id,current),/Dialog closed/);assert.equal(checks,4);assert.deepEqual(f.calls.kinds,[]);assert.equal(f.calls.posts,0);assert.equal((await identity.publicActionRecords())[0].event.id,event.id);
  }finally{await f.dispose()}
});

test('closing during a fresh action HTTP proof retains its original event and starts no POST', {timeout:5000},async()=>{
  const {f,identity,review}=await prepared({gateAt:'proof'});
  try{const sending=identity.sendReviewed(review,true,f.assertCurrent);await f.entered;const before=(await identity.publicActionRecords())[0];f.close();f.release();await assert.rejects(sending,/Management closed/);const after=(await identity.publicActionRecords())[0];assert.deepEqual(JSON.parse(JSON.stringify(after.event)),JSON.parse(JSON.stringify(before.event)));assert.equal(after.state,'outcome_unknown');assert.equal(f.calls.posts,0);assert.deepEqual(f.calls.kinds,[5,27235]);}finally{f.release();await f.dispose()}
});
