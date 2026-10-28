import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {createHash} from 'node:crypto';
import {execFileSync} from 'node:child_process';
import assert from 'node:assert/strict';
import test from 'node:test';
import {readReaderUxAssociation,restoreReaderUxPredecessor} from './reviewed-reader-ux-source-successor.mjs';
const root=fileURLToPath(new URL('../../../',import.meta.url)),association=readReaderUxAssociation();
const hash=value=>createHash('sha256').update(value).digest('hex');
test('reader UX association binds exact reviewed current source and immutable predecessor without inherited execution claims',()=>{
 assert.equal(association.predecessor_commit,'1fbe4f4f9ca17463274023772a6c757427f0db5c');
 assert.equal(association.browser_executed,false);assert.equal(association.http_executed,false);
 assert.equal(new Set(association.rows.map(row=>row.path)).size,association.rows.length);
 for(const row of association.rows){
  const current=fs.readFileSync(path.join(root,row.path));
  const restored=restoreReaderUxPredecessor(row.path,current);
  if(row.added){assert.equal(restored,null);continue;}
  const original=execFileSync('git',['show',association.predecessor_commit+':'+row.path],{cwd:root,maxBuffer:8*1024*1024});
  assert.deepEqual(restored,original,row.path);assert.equal(hash(restored),row.previous_sha256);
 }
});
test('reader UX comparison refuses edited, truncated, substituted, duplicate or unknown source and modified metadata',()=>{
 for(const row of association.rows){
  const current=fs.readFileSync(path.join(root,row.path)),changed=Buffer.from(current);changed[0]^=1;
  for(const invalid of [changed,current.subarray(1),Buffer.concat([current,Buffer.from(' ')]),null,{},new Uint8Array(current)])assert.throws(()=>restoreReaderUxPredecessor(row.path,invalid));
  assert.throws(()=>restoreReaderUxPredecessor('../'+row.path,current));
  if(!row.added){const previous=restoreReaderUxPredecessor(row.path,current);assert.throws(()=>restoreReaderUxPredecessor(row.path,previous));}
 }
 for(const key of Object.keys(association)){const changed=structuredClone(association);delete changed[key];assert.throws(()=>readReaderUxAssociation(Buffer.from(JSON.stringify(changed))));}
 const duplicate=structuredClone(association);duplicate.rows.push(duplicate.rows[0]);assert.throws(()=>readReaderUxAssociation(Buffer.from(JSON.stringify(duplicate))));
});
