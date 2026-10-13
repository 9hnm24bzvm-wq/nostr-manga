#!/usr/bin/env python3
"""Admit exactly two official lock-integrity archives; never execute package hooks."""
from pathlib import Path, PurePosixPath
import base64,hashlib,io,json,tarfile,urllib.request
ROOT=Path(__file__).resolve().parents[1]
LOCK=ROOT.parent/'manga-protocol-lab/browser-client/package-lock.json'
STAGE=ROOT/'.build/local-library-noble-2.0.1'
if STAGE.exists():raise SystemExit('Refusing to reuse an uncertain dependency stage')
STAGE.mkdir(); lock=json.loads(LOCK.read_text()); manifest={'source_lock':str(LOCK.relative_to(ROOT.parent)),'source_lock_sha256':hashlib.sha256(LOCK.read_bytes()).hexdigest(),'package_hooks_executed':False,'original_node_modules_used':False,'packages':[]}
for name in ['@noble/curves','@noble/hashes']:
 row=lock['packages']['node_modules/'+name]; short=name.split('/')[1]; url=f'https://registry.npmjs.org/@noble/{short}/-/{short}-2.0.1.tgz'
 if row['version']!='2.0.1' or row['resolved']!=url or row.get('hasInstallScript'):raise SystemExit('Exact official package contract differs')
 with urllib.request.urlopen(urllib.request.Request(url,headers={'User-Agent':'weebdex-owned-dependency-restore/1'}),timeout=6) as response:
  if response.status!=200 or response.geturl()!=url:raise SystemExit('Official download was not the exact successful endpoint')
  data=response.read(8*1024*1024+1)
 if len(data)>8*1024*1024:raise SystemExit('Package archive size limit exceeded')
 algorithm,encoded=row['integrity'].split('-',1)
 if algorithm!='sha512' or base64.b64encode(hashlib.sha512(data).digest()).decode()!=encoded:raise SystemExit('Lock integrity mismatch')
 archive=STAGE/f'{short}-2.0.1.tgz';archive.write_bytes(data)
 target=STAGE/'node_modules'/name; target.mkdir(parents=True)
 files=[];total=0
 with tarfile.open(fileobj=io.BytesIO(data),mode='r:gz') as tar:
  members=tar.getmembers()
  if len(members)>2048:raise SystemExit('Package member limit exceeded')
  for member in members:
   namepath=PurePosixPath(member.name)
   if namepath.is_absolute() or '..' in namepath.parts or not namepath.parts or namepath.parts[0]!='package' or not(member.isdir() or member.isfile()):raise SystemExit('Unsafe/nonregular package member')
   relative=Path(*namepath.parts[1:]); dest=target/relative
   if member.isdir():dest.mkdir(parents=True,exist_ok=True);continue
   total+=member.size
   if total>64*1024*1024:raise SystemExit('Package expanded byte budget exceeded')
   content=tar.extractfile(member).read();dest.parent.mkdir(parents=True,exist_ok=True);dest.write_bytes(content)
   files.append({'path':str(relative),'bytes':len(content),'sha256':hashlib.sha256(content).hexdigest()})
 package=json.loads((target/'package.json').read_text())
 if package['name']!=name or package['version']!='2.0.1':raise SystemExit('Admitted package identity mismatch')
 manifest['packages'].append({'name':name,'version':'2.0.1','url':url,'lock_integrity':row['integrity'],'archive_sha256':hashlib.sha256(data).hexdigest(),'archive_bytes':len(data),'expanded_bytes':total,'files':files})
 print(json.dumps({'package':name,'version':'2.0.1','official_get_status':200,'lock_integrity_verified':True,'files':len(files)}),flush=True)
(ROOT/'test-evidence/frontend/noble-restore/package-provenance.json').write_text(json.dumps(manifest,indent=2)+'\n')
