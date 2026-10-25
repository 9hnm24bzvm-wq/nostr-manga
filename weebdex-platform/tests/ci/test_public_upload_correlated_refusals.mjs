import assert from 'node:assert/strict';
import test from 'node:test';
import fs from 'node:fs';
import { captureUploadRequestRefusals, validateUploadRefusalRead, uploadRefusalReaderReady } from '../../scripts/public-upload-transport-refusals.mjs';
import { pageWithReader,snapshot,correlatedFixture } from './public-upload-correlated-refusal-fixture.mjs';
const id='a'.repeat(64), record={event_id:id,stage:'body-read',transport_status:200};
test('actual callback imports only the same-origin QA getter and keeps exact private ID',async()=>{
  const page=pageWithReader(()=>snapshot([record]));const value=await captureUploadRequestRefusals(page,[id]);
  assert.equal(value.capture_valid,true);assert.deepEqual(JSON.parse(JSON.stringify(value.snapshot.records)),[record]);assert.equal(page.imports,1);
});
test('fresh document handshake requires an available enabled empty valid ring',async()=>{
  assert.equal(await uploadRefusalReaderReady(pageWithReader(()=>snapshot())),true);
  for(const raw of [snapshot([record]),snapshot([],{record_valid:false}),snapshot([],{enabled:false})])
    assert.equal(await uploadRefusalReaderReady(pageWithReader(()=>raw)),false);
});
test('record arriving after early reads is retained without any UI state',async()=>{
  let calls=0;const page=pageWithReader(()=>snapshot(++calls<3?[]:[record]));
  const value=await captureUploadRequestRefusals(page,[id]);assert.equal(value.capture_valid,true);assert.equal(calls,3);
});
test('bounded reads end without fabricating a missing terminal record',async()=>{
  let calls=0;const value=await captureUploadRequestRefusals(pageWithReader(()=>{calls++;return snapshot();}),[id]);
  assert.equal(value.capture_valid,true);assert.equal(value.snapshot.records.length,0);assert.equal(calls,17);
});
test('disabled QA build and thrown getter are unavailable',async()=>{
  for(const page of [pageWithReader(()=>snapshot([],{enabled:false})),pageWithReader(()=>{throw Error('PRIVATE getter');})]){
    assert.deepEqual(await captureUploadRequestRefusals(page),{capture_valid:false,snapshot:null});
  }
});
test('missing or never-settling module evaluation cannot strand cleanup',async()=>{
  for(const page of [{evaluate:async()=>{throw Error('PRIVATE import');}},{evaluate:()=>new Promise(()=>{})}]){
    assert.deepEqual(await captureUploadRequestRefusals(page),{capture_valid:false,snapshot:null});
  }
});
test('malformed or private values are rejected without echo',async()=>{
  for(const raw of [null,snapshot([{...record,stage:'PRIVATE SENTINEL'}]),snapshot([record,record]),snapshot([{...record,transport_status:true}]),{...snapshot(),extra:'PRIVATE'}]){
    assert.equal(validateUploadRefusalRead(raw),false);
    const value=await captureUploadRequestRefusals({evaluate:async()=>raw});
    assert.deepEqual(value,{capture_valid:false,snapshot:null});assert.ok(!JSON.stringify(value).includes('PRIVATE'));
  }
});
test('exact cap and sticky producer invalidity retain their distinction',()=>{
  const records=Array.from({length:16},(_,i)=>({...record,event_id:i.toString(16).padStart(64,'0')}));
  assert.equal(validateUploadRefusalRead(snapshot(records,{truncated:true})),true);
  assert.equal(validateUploadRefusalRead(snapshot([...records,record])),false);
  assert.equal(validateUploadRefusalRead(snapshot([],{record_valid:false})),true);
});
test('invalid expected IDs stop before reader import',async()=>{
  for(const ids of [[id,id],['PRIVATE'],Array.from({length:17},(_,i)=>i.toString(16).padStart(64,'0'))]){
    const page=pageWithReader(()=>snapshot());assert.deepEqual(await captureUploadRequestRefusals(page,ids),{capture_valid:false,snapshot:null});assert.equal(page.imports,0);
  }
});
test('actual passive observer failure ID joins the callback ring fixture',async()=>{
  const f=await correlatedFixture();assert.equal(f.browser.rows.length,1);assert.equal(f.browser.rows[0].native,'failed');
  assert.equal(f.captures[0].snapshot.records[0].event_id,f.browser.rows[0].event_id);assert.deepEqual(f.browser.failures,['signed_native_failed']);
});
test('runner captures before its explicit reload and before observer teardown',()=>{
  const source=fs.readFileSync(new URL('../../scripts/public-upload-recovery-browser.mjs',import.meta.url),'utf8');
  const capture=source.indexOf('result.transport_refusal_captures.push(await captureUploadRequestRefusals(page));');
  assert.ok(capture>0&&capture<source.indexOf('await page.reload('));
  const final=source.indexOf('result.transport_refusal_captures.push(await captureUploadRequestRefusals(page,');
  assert.ok(final>capture&&final<source.indexOf('try { observer.close(); reader.close(); }'));
});

