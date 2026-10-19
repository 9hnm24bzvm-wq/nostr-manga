// Exact passive collector with synthetic DOM-attribute stand-ins.
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';
import { captureUploadPreviewRefusals, projectUploadPreviewRefusals, uploadPreviewRefusalCodes } from '../../scripts/public-upload-recovery-browser.mjs';
import { previewPage, previewRefusalFixtures } from './public-upload-preview-refusal-fixture.mjs';
const invalid = { record_valid: false, truncated: false, codes: [] };
test('all fixed codes and the empty UI snapshot have closed current facts', async () => {
  const rows = await previewRefusalFixtures();
  assert.equal(uploadPreviewRefusalCodes.length, 10);
  assert.ok(Object.isFrozen(uploadPreviewRefusalCodes));
  for (const code of uploadPreviewRefusalCodes) assert.deepEqual(rows[code], {record_valid:true,truncated:false,codes:[code]});
  assert.deepEqual(rows.empty,{record_valid:true,truncated:false,codes:[]});
  for(const name of ['private','missing','read_error'])assert.deepEqual(rows[name],invalid);
});
test('retained current DOM order is bounded and never claims failure chronology', async () => {
  for (const count of [0,1,16,17,500]) {
    const codes=Array(count).fill('fetch'); const value=await captureUploadPreviewRefusals(previewPage(codes));
    assert.deepEqual(value,{record_valid:true,truncated:count>16,codes:codes.slice(0,16)});
  }
  assert.deepEqual(await captureUploadPreviewRefusals(previewPage(Array(501).fill('fetch'))),invalid);
});
test('malformed raw counts, codes, private values and extra fields are omitted',()=>{
  for(const raw of [null,[],{}, {count:true,codes:[]},{count:-1,codes:[]},{count:1.5,codes:[]},
    {count:1,codes:[]},{count:17,codes:Array(16).fill('fetch')},{count:0,codes:[],private:'reserved-private'},
    {count:1,codes:[{toString(){throw Error('reserved-private')}}]},{count:1,codes:['reserved-private']}])
    assert.deepEqual(projectUploadPreviewRefusals(raw),invalid);
});
test('throwing or changing raw getters never echo private values or poison output',()=>{
  const raw={count:1,get codes(){throw Error('reserved-private')}};
  assert.deepEqual(projectUploadPreviewRefusals(raw),invalid);
  let reads=0;const codes=[];Object.defineProperty(codes,0,{get(){reads++;return reads===1?'fetch':'reserved-private'}});codes.length=1;
  assert.deepEqual(projectUploadPreviewRefusals({count:1,codes}),{record_valid:true,truncated:false,codes:['fetch']});
  assert.equal(reads,1);
});
test('passive snapshot deadline refuses an unresponsive page without raw messages',async()=>{
  const start=performance.now(); assert.deepEqual(await captureUploadPreviewRefusals(previewPage([],{pending:true})),invalid);
  assert.ok(performance.now()-start<1800);
});
test('actual runner snapshots before observers close without weakening existing final oracles',()=>{
  const source=readFileSync(new URL('../../scripts/public-upload-recovery-browser.mjs',import.meta.url),'utf8');
  const call=source.indexOf('result.preview_transport_refusals = await captureUploadPreviewRefusals(page);');
  assert.ok(call>0&&call<source.indexOf('try { observer.close(); reader.close(); }',call));
  assert.ok(source.includes("assert.equal(observer.mutationCount(), 21, 'Exact natural-success journey mutation count')"));
  assert.ok(source.includes("row.native, 'finished'"));
});
test('composed producer stage vocabulary and existing error attribute bind to the collector',()=>{
  const source=readFileSync(new URL('../../platform-integrations/public-api/nostr-upload-request.ts',import.meta.url),'utf8');
  const match=source.match(/export const uploadRequestRefusalStages = Object\.freeze\(\[([\s\S]*?)\] as const\)/);
  assert.ok(match); const stages=[...match[1].matchAll(/'([^']+)'/g)].map(row=>row[1]);
  assert.deepEqual([...uploadPreviewRefusalCodes],[...stages,'unavailable']);
  const anchors=JSON.parse(readFileSync(new URL('../../platform-integrations/public-api/upload-anchors.json',import.meta.url),'utf8'));
  const text=JSON.stringify(anchors);
  assert.ok(text.includes('data-public-upload-preview-refusal={previewRefusal}'));
  assert.ok(text.includes('previewRefusal = uploadRequestRefusalStage(state.error)'));
});
