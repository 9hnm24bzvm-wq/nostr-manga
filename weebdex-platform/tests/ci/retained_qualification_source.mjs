// Historical originals are fixtures, never current-product substitutes. Git is
// not required. The fixed manifest witness prevents coordinated substitutions.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {createHash} from 'node:crypto';
const base=new URL('../fixtures/qualification-baselines/',import.meta.url);
const manifestSha256='7e9d0a0407aa1438ed21f359b9e53e198135ba4979efe7939fe247f1a3f75a89';
const digest=(bytes,kind='sha256')=>createHash(kind).update(bytes).digest('hex');
export function retainedQualificationSource(name,read=fs.readFileSync){
 const encoded=read(new URL('manifest.json',base));
 assert.equal(digest(encoded),manifestSha256,'Retained baseline manifest SHA-256 changed');
 const manifest=JSON.parse(encoded);
 assert.equal(manifest.schema,'retained-qualification-counterfactuals-v1');
 assert(Object.hasOwn(manifest.entries,name),'Unknown retained qualification source: '+name);
 const entry=manifest.entries[name];
 assert.deepEqual(Object.keys(entry).sort(),['bytes','commit','file','git_blob','path','sha256']);
 assert.match(entry.file,/^(sources|device-restore)\/[A-Za-z0-9_./-]+\.txt$/);
 assert(!entry.file.split('/').includes('..'));
 const source=read(new URL(entry.file,base));
 assert.equal(source.length,entry.bytes,'Retained source byte count changed: '+name);
 assert.equal(digest(source),entry.sha256,'Retained source SHA-256 changed: '+name);
 assert.equal(digest(Buffer.concat([Buffer.from('blob '+source.length+'\0'),source]),'sha1'),entry.git_blob,'Retained source Git blob changed: '+name);
 return source.toString('utf8');
}
