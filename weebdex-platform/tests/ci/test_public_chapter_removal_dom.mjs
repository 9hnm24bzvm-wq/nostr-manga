// Actual shared pinned Svelte parent, page child, controller and signed adapter.
// Transport, signer/proof and native image-grid rendering remain finite stand-ins.
// This is not browser, HTTP, cryptography, PostgreSQL or image-grid acceptance.
import assert from 'node:assert/strict';
import test from 'node:test';
import {render,button,settle,deferred,CHAPTER,ACCOUNT} from './signed-chapter-route-fixture.mjs';
const hidden=()=>({status:204,body:undefined,bodyAvailable:true,headers:{'x-weebdex-instance-removal':'soft-hidden','x-weebdex-source-deletion-status':'instance-local-only'}});
const refusal=()=>({status:403,body:{result:'error',errors:[{status:403,title:'Forbidden',detail:'Current moderator grant revoked'}]},bodyAvailable:true,headers:{'content-type':'application/json'}});
const moderator=()=>({roles:['moderator'],permissions:['self.chapter.update','self.chapter.remove','chapter.remove']});
async function click(f,label){button(f.host,label).click();await settle();}
function removal(f,mode){assert.equal(f.writes.length,1);const call=f.writes[0],url=new URL(call.url);assert.equal(call.method,'DELETE');assert.equal(url.pathname,'/api/v1/chapter/'+CHAPTER);assert.equal(url.searchParams.get('expectedAccountId'),ACCOUNT);assert.equal(url.searchParams.get('removalMode'),mode);assert.equal(call.body,undefined);}

test('actual ordinary dialog Cancel closes without any command or catalogue read',async()=>{
 const f=await render();try{
  const reads=f.requests.length;await click(f,'Request chapter removal');
  assert.match(f.host.querySelector('[role=alertdialog]').textContent,/remains visible until approval/);
  await click(f,'Cancel');assert.equal(f.host.querySelector('[role=alertdialog]'),null);
  assert.equal(f.writes.length,0);assert.equal(f.requests.length,reads);
 }finally{await f.close();}
});
test('actual ordinary confirmation submits captured review after same-account server grant gain and blocks duplicate clicks',async()=>{
 const gate=deferred(),f=await render();try{
  f.state.gate=gate;await click(f,'Request chapter removal');const confirm=button(f.host,'Submit removal request');
  f.state.roles=['moderator'];f.state.permissions.push('chapter.remove');confirm.click();confirm.click();await settle();
  removal(f,'request-review');assert.equal(f.host.querySelector('[role=alertdialog]'),null);
  gate.release();await settle();assert.match(f.host.textContent,/awaiting instance moderator review/);
  assert.match(f.host.textContent,/remains visible until approval/);assert.doesNotMatch(f.host.textContent,/approved hiding/);
 }finally{gate.release();await f.close();}
});
test('actual moderator confirmation submits explicit approval and confirms only204 hide',async()=>{
 const f=await render(moderator());try{
  f.state.customResponse=hidden();await click(f,'Approve instance hide');
  assert.match(f.host.querySelector('[role=alertdialog]').textContent,/immediately/);
  await click(f,'Approve and hide on this instance');removal(f,'approve-hide');
  assert.match(f.host.textContent,/approved hiding this chapter/);assert.equal(button(f.host,'Approve instance hide').disabled,true);
 }finally{await f.close();}
});
test('actual mismatched removal effects stay unconfirmed and reload never replays them',async()=>{
 for(const staff of [false,true]){const f=await render(staff?moderator():{});try{
  // The shared transport's default is a fully bound202 review receipt. A204
  // receipt is equally explicit, but cannot satisfy an ordinary review intent.
  if(!staff)f.state.customResponse=hidden();
  await click(f,staff?'Approve instance hide':'Request chapter removal');
  await click(f,staff?'Approve and hide on this instance':'Submit removal request');
  removal(f,staff?'approve-hide':'request-review');assert.match(f.host.textContent,/removal (?:effect )?was not confirmed/);
  assert.doesNotMatch(f.host.textContent,/is awaiting instance moderator review|A moderator approved hiding/);
  assert.equal(button(f.host,staff?'Approve instance hide':'Request chapter removal').disabled,true);
  const reads=f.state.accessReads;await click(f,'Reload current chapter');assert.equal(f.state.accessReads,reads+1);
  assert.equal(f.writes.length,1);assert.match(f.host.textContent,/No mutation was replayed/);
  assert.equal((await f.openOwner()).chapterPending(),true);
 }finally{await f.close();}}
});
test('actual revoked approval exposes its refusal without silently falling back to review',async()=>{
 const f=await render(moderator());try{
  await click(f,'Approve instance hide');f.state.customResponse=refusal();
  await click(f,'Approve and hide on this instance');removal(f,'approve-hide');
  assert.match(f.host.textContent,/Current moderator grant revoked/);
  assert.doesNotMatch(f.host.textContent,/is awaiting instance moderator review|approved hiding/);
  await click(f,'Reload current chapter');assert.equal(f.writes.length,1);
 }finally{await f.close();}
});
test('actual unmount disposes an in-flight chapter owner and cannot update a newer mount',async()=>{
 const gate=deferred(),old=await render();old.state.gate=gate;
 old.state.replyTransform=result=>Object.assign(result,hidden());
 await click(old,'Request chapter removal');await click(old,'Submit removal request');assert.equal(old.writes.length,1);
 const owner=old.state.owners[0];assert.equal(owner.alive(),true);await old.close();assert.equal(owner.alive(),false);
 const next=await render();try{
  gate.release();await settle();assert.equal(next.writes.length,0);
  assert.doesNotMatch(next.host.textContent,/approved hiding|unconfirmed|failed/);
  assert.equal(button(next.host,'Request chapter removal').disabled,false);
 }finally{gate.release();await next.close();}
});
test('removal-only actor without upload grants remains usable after confirmed empty staging',async()=>{
 const f=await render({permissions:['self.chapter.remove']});try{
  assert.equal(f.state.session,undefined);assert(f.requests.some(call=>call.method==='GET'&&new URL(call.url).pathname==='/api/v1/upload'));
  assert.equal(button(f.host,'Edit Pages').disabled,true);assert.equal(button(f.host,'Replace Pages').disabled,true);
  assert.equal(button(f.host,'Request chapter removal').disabled,false);
  await click(f,'Request chapter removal');await click(f,'Submit removal request');removal(f,'request-review');
  assert.match(f.host.textContent,/awaiting instance moderator review/);
 }finally{await f.close();}
});
