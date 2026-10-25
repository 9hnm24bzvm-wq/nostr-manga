import pathlib,subprocess,json,hashlib,sys,os
root=pathlib.Path.cwd();out=pathlib.Path(__file__).resolve().parent;phase=sys.argv[1];target=pathlib.Path('/workspace/shared/nostr-manga-recovery-20261018/finite-fixture-worktree/.build/fixture-python-recovery/owned/site-packages')
files=subprocess.check_output(['git','ls-files','-z','--','manga-protocol-lab/bench/library-node','manga-protocol-lab/src','manga-protocol-lab/server/tests/fixtures/real-images','manga-protocol-lab/results/rebuild-mixed-prep/readiness-20261018','weebdex-platform/scripts/run-bounded.py'],text=True).split('\0');sources={r:hashlib.sha256((root/r).read_bytes()).hexdigest() for r in files if r and (root/r).is_file()};deps={}
for f in sorted(target.rglob('*')):
 s=f.lstat();row={'mode':s.st_mode&0o7777,'mtimeNs':s.st_mtime_ns}
 if f.is_symlink():row.update(kind='symlink',target=os.readlink(f))
 elif f.is_dir():row.update(kind='directory')
 elif f.is_file():raw=f.read_bytes();row.update(kind='file',bytes=len(raw),sha256=hashlib.sha256(raw).hexdigest())
 deps[str(f.relative_to(target))]=row
for name,data in [('source-pins',sources),('dependency-inputs',deps)]:
 (out/(name+'-'+phase+'.json')).write_text(json.dumps(data,indent=2)+'\n')
 if phase=='after':assert data==json.loads((out/(name+'-before.json')).read_text()),name+' changed'
print('phase',phase,'sources',len(sources),'dependencyentries',len(deps))
