import {restoreModeratorTransportSource} from './reviewed-moderator-transport-source-successors.mjs';
// One current transport, two comparison views. No service, HTTP or pipe timing claims.
import assert from 'node:assert/strict';import fs from 'node:fs';import test from 'node:test';import {createHash} from 'node:crypto';
import {readReportTransportAssociation,restoreReportTransportSource,reportTransportAssociationSha256} from './reviewed-report-transport-source-successors.mjs';
import {readNativeSourceAssociation,restoreNativeTransportSource} from './reviewed-native-source-successors.mjs';
import {stripUploadRefusalQaSource} from '../../scripts/public-upload-refusal-qa.mjs';
const metadata=fs.readFileSync(new URL('../../docs/fixtures/native-report-transport-source-association.json',import.meta.url)),data=readReportTransportAssociation();
const physical=restoreModeratorTransportSource(data.path,fs.readFileSync(new URL('../../../'+data.path,import.meta.url),'utf8'),'raw'); // Explicit predecessor comparison, not current execution.
const hash=value=>createHash('sha256').update(value).digest('hex'),encoded=value=>Buffer.from(JSON.stringify(value,null,2)+'\n');
const current=mode=>mode==='raw'?physical:stripUploadRefusalQaSource(physical);
test('raw and QA source witnesses preserve unchanged native association and do not qualify old stream/timing receipts',()=>{
 assert.equal(hash(metadata),reportTransportAssociationSha256);assert.equal(data.path,'weebdex-platform/platform-integrations/public-api/nostr-upload-request.ts');
 assert.equal(data.current_product_source,'b922c9deb677be3c8d2e6c2c78249d79a4ee16f5');
 assert.equal(data.browser_executed,false);assert.equal(data.http_executed,false);assert.equal(data.pipe_timing_qualified,false);
 assert.match(data.scope,/Never runtime\/build substitution or historical pipe\/timing execution relabelling/);
 const historical=readNativeSourceAssociation();
 for(const mode of ['raw','qa_stripped']){
  const row=data[mode],value=current(mode),previous=restoreReportTransportSource(data.path,value,mode);
  assert.equal(Buffer.byteLength(value),row.current_bytes);assert.equal(hash(value),row.current_sha256);
  assert.equal(Buffer.byteLength(previous),row.previous_bytes);assert.equal(hash(previous),row.previous_sha256);
  assert.equal(hash(previous),historical.transport[mode].current_sha256);
  assert.equal(hash(restoreNativeTransportSource(previous,mode)),historical.transport[mode].previous_sha256);
  assert(Buffer.isBuffer(restoreReportTransportSource(data.path,Buffer.from(value),mode)));
 }
 assert.equal(stripUploadRefusalQaSource(restoreReportTransportSource(data.path,physical,'raw')),restoreReportTransportSource(data.path,stripUploadRefusalQaSource(physical),'qa_stripped'));
});
for(const mode of ['raw','qa_stripped'])test('exact '+mode+' bytes, paths, modes and every literal reject unknown input',()=>{
 const value=current(mode),previous=restoreReportTransportSource(data.path,value,mode),row=data[mode];
 for(const candidate of [previous,value+' ',value.slice(1),String.fromCharCode(value.charCodeAt(0)^1)+value.slice(1),null,{},new Uint8Array(Buffer.from(value))])assert.throws(()=>restoreReportTransportSource(data.path,candidate,mode));
 for(const edit of row.literal_reversals){assert.equal(value.split(edit.current).length,2);assert.throws(()=>restoreReportTransportSource(data.path,value.replace(edit.current,()=>edit.previous),mode));assert.throws(()=>restoreReportTransportSource(data.path,value+edit.current,mode));}
 for(const path of ['',null,data.path+'unknown'])assert.throws(()=>restoreReportTransportSource(path,value,mode));
 for(const wrong of ['',null,'unknown',mode==='raw'?'qa_stripped':'raw'])assert.throws(()=>restoreReportTransportSource(data.path,value,wrong));
});
test('complete metadata digest rejects altered fields, source claims, row/literal ordering and encodings',()=>{
 const candidates=[Buffer.concat([metadata,Buffer.from(' ')]),metadata.subarray(1),metadata.toString('utf8'),encoded(Object.fromEntries(Object.entries(data).reverse()))];
 for(const key of Object.keys(data))for(const action of ['delete','change']){const value=structuredClone(data);if(action==='delete')delete value[key];else value[key]='unknown';candidates.push(encoded(value));}
 for(const mode of ['raw','qa_stripped']){
  for(const key of Object.keys(data[mode])){const value=structuredClone(data);delete value[mode][key];candidates.push(encoded(value));}
  for(let i=0;i<data[mode].literal_reversals.length;i++)for(const key of ['current','previous']){const value=structuredClone(data);value[mode].literal_reversals[i][key]+=' ';candidates.push(encoded(value));}
  const value=structuredClone(data);value[mode].literal_reversals.reverse();candidates.push(encoded(value));
 }
 const unknown=structuredClone(data);unknown.unknown=true;candidates.push(encoded(unknown));
 for(const candidate of candidates){assert.throws(()=>readReportTransportAssociation(candidate));assert.throws(()=>restoreReportTransportSource(data.path,physical,'raw',candidate));}
});
test('actual transport and preparation helpers never import comparison restoration',()=>{
 for(const file of ['platform-integrations/public-api/nostr-upload-request.ts','scripts/public-upload-refusal-qa.mjs','scripts/prepare-frontend-integration.mjs'])assert.doesNotMatch(fs.readFileSync(new URL('../../'+file,import.meta.url),'utf8'),/reviewed-report-transport-source-successors|restoreReportTransportSource/);
});
