import pathlib,json,hashlib,os,sys
out=pathlib.Path(__file__).resolve().parent;root=pathlib.Path.cwd();phase=sys.argv[1];target=root/'weebdex-platform/.build/python-migration-approved-20261018-0556/site-packages';manifest={}
for f in sorted(target.rglob('*')):
 s=f.lstat();row={'mode':s.st_mode&0o7777,'mtimeNs':s.st_mtime_ns}
 if f.is_symlink():row.update(kind='symlink',target=os.readlink(f))
 elif f.is_dir():row.update(kind='directory')
 elif f.is_file():raw=f.read_bytes();row.update(kind='file',bytes=len(raw),sha256=hashlib.sha256(raw).hexdigest())
 manifest[str(f.relative_to(target))]=row
(out/('installed-'+phase+'.json')).write_text(json.dumps(manifest,indent=2)+'\n')
if phase=='after':assert manifest==json.loads((out/'installed-before.json').read_text()),'Installed package input changed during smoke/migration tests'
a=json.loads((out/'consumer-alias.json').read_text());p=pathlib.Path(a['alias']);assert p.is_symlink()and str(p.resolve())==a['physicalTarget']and p.lstat().st_mtime_ns==a['mtimeNs']and p.lstat().st_mode&0o7777==a['mode']
print('installed entries',len(manifest),'consumer alias unchanged')
