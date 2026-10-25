// Finite source-comparison controls only. No current execution is replaced by history.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {createHash} from 'node:crypto';
import test from 'node:test';
import {readScheduledAssociation,restoreReviewedScheduledSource,scheduledAssociationSha256} from './reviewed-scheduled-source-successors.mjs';
import {restoreLocalPageSource} from './reviewed-local-page-source-successors.mjs';
const digest=value=>createHash('sha256').update(value).digest('hex');
const metadata=fs.readFileSync(new URL('../../docs/fixtures/reviewed-scheduled-source-associations.json',import.meta.url));
const data=readScheduledAssociation();
const encoded=value=>Buffer.from(JSON.stringify(value,null,2)+'\n');
// This input is a historical scheduling-stage comparison, never execution.
function comparison(row){let value=fs.readFileSync(new URL('../../../'+row.path,import.meta.url),'utf8');if(row.kind==='adapterFixturePrefix')value=value.slice(value.indexOf('const context ='),value.indexOf('async function ready('));return restoreLocalPageSource(value,row.kind);}
test('exact historical scheduling-stage successors retain language-era source hashes and no execution claims',()=>{
 assert.equal(digest(metadata),scheduledAssociationSha256);
 assert.equal(data.product_freeze,'11ea3ea9f593c134f2ba478bb84fa683f4b14149');
 assert.equal(data.browser_executed,false);assert.equal(data.http_executed,false);
 assert.match(data.scope,/never runtime\/build substitution or historical execution relabelling/);
 assert.deepEqual(data.rows.map(row=>[row.kind,row.previous_sha256]),[
  ['uploadClient','2ef51d1d4590a0154e79ba38a7af55566ca42f8c3e6a007064abc58b8c6a88aa'],
  ['adapterFixturePrefix','eaaf343f1c09c19a609f172b3bb70273040a13e0a67e1fddba23373e86d83a2a'],
 ]);
 for(const row of data.rows){const source=comparison(row),previous=restoreReviewedScheduledSource(source,row.kind);assert.equal(digest(source),row.current_sha256);assert.equal(Buffer.byteLength(source),row.current_bytes);assert.equal(digest(previous),row.previous_sha256);assert.equal(Buffer.byteLength(previous),row.previous_bytes);assert.equal(comparison(row),source);}
});
for(const row of data.rows)test(row.kind+': every unique seam and old/partial/duplicate source fails closed',()=>{
 const source=comparison(row),previous=restoreReviewedScheduledSource(source,row.kind);
 for(const value of [previous,source+' ',source.slice(1),null,Buffer.from(source),{},1])assert.throws(()=>restoreReviewedScheduledSource(value,row.kind));
 for(const edit of row.literal_reversals){assert.equal(source.split(edit.current).length,2);assert.throws(()=>restoreReviewedScheduledSource(source.replace(edit.current,()=>edit.previous),row.kind));assert.throws(()=>restoreReviewedScheduledSource(source+edit.current,row.kind));}
 for(const kind of ['unknown','constructor','__proto__',''])assert.throws(()=>restoreReviewedScheduledSource(source,kind));
});
test('all scheduling metadata fields, seams, order and encoding are fixed',()=>{
 const candidates=[metadata.toString(),Buffer.concat([metadata,Buffer.from(' ')]),encoded(Object.fromEntries(Object.entries(data).reverse()))];
 for(const key of Object.keys(data))for(const op of ['delete','change']){const value=structuredClone(data);if(op==='delete')delete value[key];else value[key]='unknown';candidates.push(encoded(value));}
 for(let i=0;i<data.rows.length;i++){
  for(const key of Object.keys(data.rows[i])){const value=structuredClone(data);delete value.rows[i][key];candidates.push(encoded(value));}
  for(let j=0;j<data.rows[i].literal_reversals.length;j++)for(const key of ['current','previous']){const value=structuredClone(data);value.rows[i].literal_reversals[j][key]+=' ';candidates.push(encoded(value));}
 }
 for(const mutate of [value=>value.rows.reverse(),value=>value.rows.push(value.rows[0]),value=>value.rows.pop(),value=>value.unknown=true]){const value=structuredClone(data);mutate(value);candidates.push(encoded(value));}
 for(const value of candidates){assert.throws(()=>readScheduledAssociation(value));assert.throws(()=>restoreReviewedScheduledSource(comparison(data.rows[0]),data.rows[0].kind,value));}
});
