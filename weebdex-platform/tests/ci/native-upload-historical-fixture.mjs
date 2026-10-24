// Authentic historical inputs for legacy controls, not current-product coverage.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import os from 'node:os';
import {createHash} from 'node:crypto';
import {browserModules} from '../../scripts/ci/run-native-upload-diagnostic.mjs';
const hash=value=>createHash('sha256').update(value).digest('hex');
const fixtureUrl=new URL('./fixtures/native-upload-legacy-1b35d7a6.json',import.meta.url);
const expectedBundle="e6140bb5051a2b74e9e3aed3e7de1b3d44cfaa17db5792c95894a785ff99f8e5";
const expectedPaths=["frontend/src/lib/constants/Error.ts", "platform-integrations/public-api/nostr-upload-request.ts", "platform-integrations/public-api/upload-client.ts", "platform-integrations/public-api/upload-multipart.ts", "platform-integrations/public-api/validation.ts", "scripts/vite.platform.config.mjs"];
function fixture(){
 const bytes=fs.readFileSync(fixtureUrl);assert.equal(hash(bytes),expectedBundle,'Historical fixture bytes changed');
 const value=JSON.parse(bytes);assert.equal(value.schema,1);assert.equal(value.source_commit,'1b35d7a6a1e5247931b23183f9c8368483d9d511');
 assert.deepEqual(value.files.map(row=>row.path),expectedPaths);
 for(const row of value.files){const data=Buffer.from(row.content);assert.equal(data.length,row.bytes);assert.equal(hash(data),row.sha256);assert.equal(row.mode,'100644');const oid=createHash('sha1').update(Buffer.from('blob '+data.length+'\0')).update(data).digest('hex');assert.equal(oid,row.git_blob);}
 return value.files;
}
export function historicalTransport(){
 const row=fixture().find(row=>row.path==='platform-integrations/public-api/nostr-upload-request.ts');
 assert.equal(row.sha256,'7093cdee6a1eff833447f0f4c488e52569a74e8df8e083e46f516368c03788de');return row.content;
}
export function historicalBrowserModules(){
 const files=fixture();
 // These are the retained public harness, entry and QA plugin, not an invented
 // historical execution tree. Their exact association is intentional.
 for(const [name,expected]of [
  ['run-native-upload-diagnostic.mjs','aa1ec8f2c3186c4a134a1c604c2fbe3a2e6013470f648a971525b0f71c9c2ec0'],
  ['native-upload-diagnostic-browser.mjs','8fbb21b61c34219b367c66cf25ef7bc1d0277a3836f2fdeca6ba40bc86c05359'],
 ])assert.equal(hash(fs.readFileSync(new URL('../../scripts/ci/'+name,import.meta.url))),expected);
 assert.equal(hash(fs.readFileSync(new URL('../../scripts/public-upload-refusal-qa.mjs',import.meta.url))),'19b6bbb04add4276c24e9918c52e29f951e1450e0f2bb0b6db0683e9be15896f');
 const root=fs.mkdtempSync(path.join(os.tmpdir(),'historical-upload-source-'));
 try{
  for(const row of files){const destination=path.join(root,row.path);fs.mkdirSync(path.dirname(destination),{recursive:true});fs.writeFileSync(destination,row.content,{flag:'wx',mode:0o600});}
  const entry=path.join(root,'scripts/ci/native-upload-diagnostic-browser.mjs');fs.mkdirSync(path.dirname(entry),{recursive:true});fs.copyFileSync(new URL('../../scripts/ci/native-upload-diagnostic-browser.mjs',import.meta.url),entry);
  return browserModules(root);
 }finally{fs.rmSync(root,{recursive:true,force:true});}
}
