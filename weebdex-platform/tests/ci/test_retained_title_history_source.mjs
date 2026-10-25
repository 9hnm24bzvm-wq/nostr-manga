import assert from 'node:assert/strict';
import fs from 'node:fs';
import { createHash } from 'node:crypto';
import test from 'node:test';
import { retainedTitleHistorySource } from './retained_title_history_source.mjs';
const base=new URL('../fixtures/title-history-baselines/',import.meta.url),manifestBytes=fs.readFileSync(new URL('manifest.json',base)),manifest=JSON.parse(manifestBytes),name='d645756-history-pagination',file=manifest.entries[name].file;
const changed=Buffer.from(fs.readFileSync(new URL(file,base)));changed[0]^=1;
const isManifest=url=>url.pathname.endsWith('/manifest.json');
test('fixed manifest witness admits exactly eight original baseline sources',()=>{
 assert.equal(Object.keys(manifest.entries).length,8);
 for(const [key,entry] of Object.entries(manifest.entries))assert.equal(retainedTitleHistorySource(key),fs.readFileSync(new URL(entry.file,base),'utf8'));
});
test('altered manifest bytes cannot substitute a baseline',()=>{
 const altered=Buffer.concat([manifestBytes,Buffer.from(' ')]);assert.throws(()=>retainedTitleHistorySource(name,url=>isManifest(url)?altered:fs.readFileSync(url)),/manifest SHA-256 changed/);
});
test('altered same-size source bytes fail the original SHA witness',()=>{
 assert.throws(()=>retainedTitleHistorySource(name,url=>url.pathname.endsWith('/'+file)?changed:fs.readFileSync(url)),/source SHA-256 changed/);
});
test('coordinated fixture and manifest rewriting cannot bypass the fixed manifest witness',()=>{
 const altered=JSON.parse(manifestBytes);altered.entries[name].sha256=createHash('sha256').update(changed).digest('hex');const encoded=Buffer.from(JSON.stringify(altered));
 assert.throws(()=>retainedTitleHistorySource(name,url=>isManifest(url)?encoded:url.pathname.endsWith('/'+file)?changed:fs.readFileSync(url)),/manifest SHA-256 changed/);
});
test('unknown entries and inherited object names are refused',()=>{
 for(const key of ['current-history-pagination','../history-pagination','toString','__proto__'])assert.throws(()=>retainedTitleHistorySource(key),/Unknown retained source/);
});
