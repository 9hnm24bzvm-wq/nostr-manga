// Exact immutable counterfactual sources. No repository history or Git executable
// is needed in a clean connector-published clone. Provenance is not a fetch URL.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import { createHash } from 'node:crypto';
const base=new URL('../fixtures/title-history-baselines/',import.meta.url);
const manifestSha256='9068d664c7d7d4fbeed59b9b27cb3af029d1c6a0b64072728f2fd4e3f8142fde';
const names=['08c3bb9-history-feed','08c3bb9-title-pagination','3df4223-title-pagination','9893eea-title-feed','a8d428b-MangaFollow','bf36e22-MangaFollow','d645756-history-pagination','d645756-title-order'];
const sha256=bytes=>createHash('sha256').update(bytes).digest('hex');
function manifestFrom(bytes){
 assert.equal(sha256(bytes),manifestSha256,'Retained manifest SHA-256 changed');
 const manifest=JSON.parse(bytes.toString('utf8'));
 assert.deepEqual(Object.keys(manifest).sort(),['entries','schema']);
 assert.equal(manifest.schema,'retained-title-history-counterfactuals-v1');
 assert.deepEqual(Object.keys(manifest.entries).sort(),names,'Retained baseline entry set changed');
 for(const name of names){
  const entry=manifest.entries[name],dash=name.indexOf('-');
  assert.deepEqual(Object.keys(entry).sort(),['bytes','commit','file','git_blob','path','sha256']);
  assert.equal(entry.file,name+'.ts.txt');
  assert.equal(entry.path,'weebdex-platform/platform-integrations/local-library/'+name.slice(dash+1)+'.ts');
  assert.match(entry.commit,/^[a-f0-9]{40}$/);assert(entry.commit.startsWith(name.slice(0,dash)));
  assert.match(entry.git_blob,/^[a-f0-9]{40}$/);assert.match(entry.sha256,/^[a-f0-9]{64}$/);
  assert(Number.isSafeInteger(entry.bytes)&&entry.bytes>0&&entry.bytes<=32768,'Invalid retained source size');
 }
 return manifest;
}
// The optional synchronous read boundary permits corruption controls without
// rewriting tracked fixtures. Both manifest and source still require fixed pins.
export function retainedTitleHistorySource(name,read=fs.readFileSync){
 const manifest=manifestFrom(read(new URL('manifest.json',base)));
 assert(Object.hasOwn(manifest.entries,name),'Unknown retained source: '+name);
 const entry=manifest.entries[name],bytes=read(new URL(entry.file,base));
 assert.equal(bytes.length,entry.bytes,'Retained source byte count changed: '+name);
 assert.equal(sha256(bytes),entry.sha256,'Retained source SHA-256 changed: '+name);
 return bytes.toString('utf8');
}
