import assert from 'node:assert/strict';
import fs from 'node:fs';
import {createHash} from 'node:crypto';
import test from 'node:test';
import {retainedQualificationSource} from './retained_qualification_source.mjs';
const base=new URL('../fixtures/qualification-baselines/',import.meta.url),encoded=fs.readFileSync(new URL('manifest.json',base)),manifest=JSON.parse(encoded),name='native-user-site-client',file=manifest.entries[name].file;
const source=fs.readFileSync(new URL(file,base)),changed=Buffer.from(source);changed[0]^=1;
const isManifest=url=>url.pathname.endsWith('/manifest.json');
test('all nineteen retained originals match immutable manifest and Git blob witnesses',()=>{
 assert.equal(Object.keys(manifest.entries).length,19);
 for(const [key,entry] of Object.entries(manifest.entries))assert.equal(retainedQualificationSource(key),fs.readFileSync(new URL(entry.file,base),'utf8'));
});
test('native old source retains its exact independent SHA-256',()=>{
 assert.equal(createHash('sha256').update(retainedQualificationSource(name)).digest('hex'),'cd6379824dfa6218c5b5b373a36be80129f611bb235f81225bc62aab6f67ed00');
});
test('altered manifest and coordinated original substitution are refused',()=>{
 for(const replacement of [Buffer.concat([encoded,Buffer.from(' ')]),Buffer.from(JSON.stringify({...manifest,entries:{...manifest.entries,[name]:{...manifest.entries[name],sha256:createHash('sha256').update(changed).digest('hex')}}}))]){
  assert.throws(()=>retainedQualificationSource(name,url=>isManifest(url)?replacement:changed),/manifest SHA-256 changed/);
 }
});
test('same-size corruption and truncated source are refused',()=>{
 for(const bytes of [changed,source.subarray(1)])assert.throws(()=>retainedQualificationSource(name,url=>isManifest(url)?encoded:bytes),/Retained source (SHA-256|byte count) changed/);
});
test('missing fixture is an error, not a skipped old-source control',()=>{
 assert.throws(()=>retainedQualificationSource(name,url=>{if(isManifest(url))return encoded;throw Error('Exact original absent');}),/Exact original absent/);
});
test('unknown and inherited names cannot select current or substitute source',()=>{
 for(const key of ['current-site-client','../native-user-site-client','toString','__proto__'])assert.throws(()=>retainedQualificationSource(key),/Unknown retained qualification source/);
});
