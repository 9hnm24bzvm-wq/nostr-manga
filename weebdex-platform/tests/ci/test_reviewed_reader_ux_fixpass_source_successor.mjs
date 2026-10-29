import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {createHash} from 'node:crypto';
import {execFileSync} from 'node:child_process';
import assert from 'node:assert/strict';
import test from 'node:test';
import {readReaderUxFixpassAssociation,restoreReaderUxFixpassPredecessor,restoreReaderUxPredecessor,readReaderUxAssociation} from './reviewed-card-spinner-source-successor.mjs';
const root=fileURLToPath(new URL('../../../',import.meta.url)),association=readReaderUxFixpassAssociation();
const hash=value=>createHash('sha256').update(value).digest('hex');
const git=file=>execFileSync('git',['show',association.predecessor_commit+':'+file],{cwd:root,maxBuffer:8*1024*1024,stdio:['ignore','pipe','pipe']});
test('second reader fixpass restores every exact physical source to the published commit without inheriting qualification',()=>{
 assert.equal(association.predecessor_commit,'e3a8ca84eef38fee4168cbde690696c4174e08e5');
 assert.equal(association.predecessor_tree,'f134c161efd97a1cab513844f7954057abc171ce');
 assert.equal(association.browser_executed,false);assert.equal(association.http_executed,false);
 assert.ok(association.rows.length>0);
 assert.equal(new Set(association.rows.map(row=>row.path)).size,association.rows.length);
 for(const row of association.rows){
  const current=fs.readFileSync(path.join(root,row.path));
  const restored=restoreReaderUxFixpassPredecessor(row.path,current);
  if(row.added){assert.equal(restored,null);assert.throws(()=>git(row.path));continue;}
  const original=git(row.path);
  assert.deepEqual(restored,original,row.path);assert.equal(hash(restored),row.previous_sha256);
  assert.notEqual(hash(current),hash(original),row.path);
 }
});
test('published reader metadata and its restoration implementation remain byte-identical, and the two exact source layers compose',()=>{
 const publishedMetadata=fs.readFileSync(path.join(root,association.published_association_path));
 assert.equal(hash(publishedMetadata),association.published_association_sha256);
 assert.deepEqual(publishedMetadata,git(association.published_association_path));
 const helper='weebdex-platform/tests/ci/reviewed-reader-ux-source-successor.mjs';
 assert.deepEqual(fs.readFileSync(path.join(root,helper)),git(helper));
 const published=readReaderUxAssociation();
 for(const row of published.rows){
  const current=fs.readFileSync(path.join(root,row.path));
  const restored=restoreReaderUxPredecessor(row.path,current);
  if(row.added){assert.equal(restored,null);continue;}
  const original=execFileSync('git',['show',published.predecessor_commit+':'+row.path],{cwd:root,maxBuffer:8*1024*1024,stdio:['ignore','pipe','pipe']});
  assert.deepEqual(restored,original,row.path);
 }
});
test('second reader fixpass refuses edited, partial, unknown, published, mistyped sources and all altered metadata',()=>{
 for(const row of association.rows){
  const current=fs.readFileSync(path.join(root,row.path)),changed=Buffer.from(current);changed[0]^=1;
  for(const invalid of [changed,current.subarray(1),Buffer.concat([current,Buffer.from(' ')]),null,{},new Uint8Array(current)])assert.throws(()=>restoreReaderUxFixpassPredecessor(row.path,invalid));
  assert.throws(()=>restoreReaderUxFixpassPredecessor('../'+row.path,current));
  if(!row.added)assert.throws(()=>restoreReaderUxFixpassPredecessor(row.path,git(row.path)));
 }
 for(const key of Object.keys(association)){const changed=structuredClone(association);delete changed[key];assert.throws(()=>readReaderUxFixpassAssociation(Buffer.from(JSON.stringify(changed))));}
 const duplicate=structuredClone(association);duplicate.rows.push(duplicate.rows[0]);assert.throws(()=>readReaderUxFixpassAssociation(Buffer.from(JSON.stringify(duplicate))));
 for(const bytes of [null,{},Buffer.from('{}'),new Uint8Array([1])])assert.throws(()=>readReaderUxFixpassAssociation(bytes));
});
