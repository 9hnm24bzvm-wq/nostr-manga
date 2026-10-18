import pathlib,json,hashlib,os,sys,gzip
out=pathlib.Path('weebdex-platform/test-evidence/frontend/final-discovered-ci-20261018');phase=sys.argv[1]
roots=json.loads((out/'physical-dependency-root-discovery.json').read_text())['roots'];snapshots={};summary={}
for value in roots:
 root=pathlib.Path(value);manifest={}
 for parent,dirs,files in os.walk(root,followlinks=False):
  for name in sorted(list(dirs)+files):
   p=pathlib.Path(parent)/name;s=p.lstat();record={'mode':s.st_mode&0o7777,'mtimeNs':s.st_mtime_ns}
   if p.is_symlink():record.update(kind='symlink',target=os.readlink(p))
   elif p.is_file():record.update(kind='file',bytes=s.st_size,sha256=hashlib.sha256(p.read_bytes()).hexdigest())
   elif p.is_dir():record.update(kind='directory')
   else:raise RuntimeError('Unexpected dependency entry '+str(p))
   manifest[str(p.relative_to(root))]=record
 snapshots[value]=manifest;summary[value]={'entries':len(manifest),'regularFiles':sum(x['kind']=='file'for x in manifest.values()),'canonicalManifestSha256':hashlib.sha256(json.dumps(manifest,sort_keys=True,separators=(',',':')).encode()).hexdigest()}
encoded=json.dumps(snapshots,sort_keys=True,separators=(',',':')).encode();(out/f'physical-dependencies-{phase}.json.gz').write_bytes(gzip.compress(encoded,mtime=0));(out/f'physical-dependencies-{phase}-summary.json').write_text(json.dumps(summary,indent=2)+'\n')
if phase=='after':assert gzip.decompress((out/'physical-dependencies-before.json.gz').read_bytes())==encoded,'Physical dependency bytes, metadata or symlinks changed'
print('physical roots',len(roots),'regular files',sum(x['regularFiles']for x in summary.values()),'entries',sum(x['entries']for x in summary.values()))
