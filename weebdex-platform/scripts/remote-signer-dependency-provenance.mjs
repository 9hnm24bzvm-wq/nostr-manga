// Exact official archives are restored separately; production checks never fetch or install.
import fs from 'node:fs';
import path from 'node:path';
import {createHash} from 'node:crypto';
import {fileURLToPath} from 'node:url';
const root=path.resolve(path.dirname(fileURLToPath(import.meta.url)),'..');
const sha=bytes=>createHash('sha256').update(bytes).digest('hex');
const LOCK_SHA='0f2ab48d9c6e4f486c0672f384ed6cb4736bf1f20cd876c83b46466075066160';
const PROVENANCE_SHA='4ac4adcb7f241d077786d1ea57ad7403a63db4e81663b16c3cb13dd6123f31dc';
const versions={'nostr-tools':'2.25.2','@noble/ciphers':'2.1.1','@noble/curves':'2.0.1','@noble/hashes':'2.0.1','@scure/base':'2.0.0'};
export function verifyRemoteSignerDependencies(project=root){
 const lockFile=path.join(project,'platform-integrations/local-library/remote-signing-dependencies/package-lock.json');
 const provenanceFile=path.join(project,'test-evidence/frontend/remote-signer-deps/package-provenance.json');
 const lockBytes=fs.readFileSync(lockFile),bytes=fs.readFileSync(provenanceFile);
 if(sha(lockBytes)!==LOCK_SHA||sha(bytes)!==PROVENANCE_SHA)throw Error('Unreviewed remote signer dependency provenance');
 const lock=JSON.parse(lockBytes),provenance=JSON.parse(bytes);
 if(provenance.source_lock_sha256!==LOCK_SHA||provenance.package_hooks_executed!==false||provenance.packages.length!==5)throw Error('Unexpected remote signer package scope');
 const stage=path.join(project,'.build/remote-signer-nostr-tools-2.25.2'),checked=[];
 for(const [name,version] of Object.entries(versions)){
  const rows=provenance.packages.filter(row=>row.name===name),pin=lock.packages['node_modules/'+name];
  if(rows.length!==1||rows[0].version!==version||pin.version!==version||rows[0].integrity!==pin.integrity||rows[0].url!==pin.resolved)throw Error('Remote signer package identity changed: '+name);
  const row=rows[0],archive=path.join(stage,name.split('/').at(-1)+'-'+version+'.tgz');
  const archiveBytes=fs.readFileSync(archive);
  if(sha(archiveBytes)!==row.archive_sha256||'sha512-'+createHash('sha512').update(archiveBytes).digest('base64')!==pin.integrity)throw Error('Remote signer official archive changed: '+name);
  const aliases=name==='@scure/base'?['local-signing-base-2.0.0']:name==='@noble/curves'||name==='@noble/hashes'?['local-library-noble-2.0.1']:[];
  for(const dependencyRoot of [stage,...aliases.map(value=>path.join(project,'.build',value))]){
   const packageRoot=path.join(dependencyRoot,'node_modules',name),seen=new Set();
   for(const file of row.files){
    if(typeof file.path!=='string'||file.path.split('/').some(part=>part==='..'||!part)||path.isAbsolute(file.path)||seen.has(file.path))throw Error('Invalid remote signer package file scope');
    seen.add(file.path);const target=path.join(packageRoot,file.path),stat=fs.lstatSync(target);
    if(!stat.isFile()||stat.size!==file.bytes||sha(fs.readFileSync(target))!==file.sha256)throw Error('Remote signer package file changed: '+name+'/'+file.path);
   }
  }
  checked.push({name,version,archive_sha256:row.archive_sha256,files_verified:row.files.length,production_aliases_verified:1+aliases.length});
 }
 return {source_lock_sha256:LOCK_SHA,package_provenance_sha256:PROVENANCE_SHA,package_hooks_executed:false,verification_network_used:false,packages:checked};
}
