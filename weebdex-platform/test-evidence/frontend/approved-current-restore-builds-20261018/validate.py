import datetime,gzip,hashlib,importlib.util,json,os,pathlib,shutil,subprocess,sys
sys.dont_write_bytecode=True
W=pathlib.Path(__file__).resolve().parents[2];P=W/'weebdex-platform';E=W/'.build/frontend-current-restore';D=P/'.build/approved-dependency-restore-20261018';NODE=pathlib.Path('/opt/codex/runtimes/codex-primary-runtime/dependencies/node/bin/node')
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
def write(p,v):p.parent.mkdir(parents=True,exist_ok=True);p.write_text(json.dumps(v,indent=2)+'\n')
def load_module(name,path):
 spec=importlib.util.spec_from_file_location(name,path);m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m);return m
owner=load_module('owner',E/'owned-guard.py');tools=load_module('tools',P/'scripts/ci/prepare-hosted-browser.py')
def tracked():
 return {p:sha(W/p)for p in subprocess.check_output(['git','ls-files','-z'],cwd=W,text=True).split('\0')if p and not p.startswith('weebdex-platform/test-evidence/')}
def assert_source():
 assert tracked()==json.loads((E/'tracked-source-before.json').read_text()),'Tracked source inputs changed';tools.verify_frontend()
def roots():return [pathlib.Path(x)for x in json.loads((E/'physical-root-discovery.json').read_text())['roots']]
def snapshot(label):
 value={}
 for root in roots():
  st=root.lstat();assert root.resolve(strict=True)==root and root.is_dir()and not root.is_symlink();manifest={'.':{'kind':'directory','mode':st.st_mode&0o7777,'mtimeNs':st.st_mtime_ns}}
  for parent,dirs,files in os.walk(root,followlinks=False):
   for name in sorted(dirs+files):
    p=pathlib.Path(parent)/name;s=p.lstat();row={'mode':s.st_mode&0o7777,'mtimeNs':s.st_mtime_ns}
    if p.is_symlink():
     target=p.resolve(strict=True);assert any(target==r or r in target.parents for r in roots()),'Dependency target outside discovery';row.update(kind='symlink',target=os.readlink(p))
    elif p.is_file():row.update(kind='file',bytes=s.st_size,sha256=sha(p))
    elif p.is_dir():row.update(kind='directory')
    else:raise RuntimeError('Unexpected dependency kind')
    manifest[str(p.relative_to(root))]=row
  value[str(root)]=manifest
 data=json.dumps(value,sort_keys=True,separators=(',',':')).encode();dest=E/('physical-'+label+'.json.gz');dest.write_bytes(gzip.compress(data,mtime=0))
 if label!='before':assert data==gzip.decompress((E/'physical-before.json.gz').read_bytes()),'Dependency bytes/modes/times/targets changed'
 return {'roots':len(value),'entries':sum(map(len,value.values())),'sha256':sha(dest)}
def environment(label):
 keep={k:v for k,v in os.environ.items()if k in ['LANG','LC_ALL','LC_CTYPE','SSL_CERT_FILE','SSL_CERT_DIR','NODE_EXTRA_CA_CERTS']}
 audit=E/label/'cache-audit';audit.mkdir(parents=True)
 keep.update(PATH=str(NODE.parent)+':/usr/bin:/bin',HOME=str(D/'home'),TMPDIR=str(D/'tmp'),PYTHONDONTWRITEBYTECODE='1',NODE_OPTIONS='--max-old-space-size=2048 --require='+str(P/'scripts/semantic-readonly-cache-guard.cjs'),WEEBDEX_READONLY_CACHE_ROOTS=json.dumps([str(x)for x in roots()]),WEEBDEX_READONLY_AUDIT_DIR=str(audit),WEEBDEX_SEMANTIC_NODE_MODULES=str(D/'frontend/node_modules'),PUBLIC_MANGADEX_CATALOGUE='1'if label.endswith('1')else'0',WEEBDEX_QA_BUILD_RESOURCE_PROFILE='serial-terser')
 return keep
label=sys.argv[1];sys.dont_write_bytecode=True
if label=='setup':
 assert_source();primary=(D/'frontend/node_modules').resolve(strict=True)
 nobleroot=D/'signer-host/weebdex-platform/.build/local-library-noble-2.0.1';baseroot=D/'signer-host/weebdex-platform/.build/local-signing-base-2.0.0'
 rs=[primary,nobleroot.resolve(strict=True),baseroot.resolve(strict=True)]
 for name,target in [('local-library-noble-2.0.1',rs[1]),('local-signing-base-2.0.0',rs[2])]:
  link=P/'.build'/name;assert not link.exists()and not link.is_symlink();link.symlink_to(target,target_is_directory=True)
 write(E/'physical-root-discovery.json',{'roots':[str(x)for x in rs],'primary_root_included':True,'dependency_package_alias_roots':['@lucide/svelte','svelte-sonner','axios','http-status-codes','@noble/curves','@noble/hashes','@scure/base'],'excluded_unexecuted_package_cache':[str(D/'yarn-bootstrap'),str(D/'yarn-cache'),str(D/'npm-cache')],'source_config_sha256':sha(P/'scripts/vite.platform.config.mjs')})
 bindings=[]
 for name,base in [('@lucide/svelte',primary),('svelte-sonner',primary),('axios',primary),('http-status-codes',primary),('@noble/curves',rs[1]/'node_modules'),('@noble/hashes',rs[1]/'node_modules'),('@scure/base',rs[2]/'node_modules')]:
  target=(base/name).resolve(strict=True);assert any(target==r or r in target.parents for r in rs);bindings.append({'package':name,'physical_target':str(target),'package_sha256':sha(target/'package.json')})
 write(E/'alias-physical-bindings.json',bindings);write(E/'installed-toolchain.json',[{'package':name,'version':json.loads((primary/name/'package.json').read_text())['version'],'package_sha256':sha(primary/name/'package.json')}for name in ['typescript','svelte','svelte-check','vite','@sveltejs/kit','@lucide/svelte','magic-string','@jridgewell/trace-mapping']]);print(snapshot('before'));raise SystemExit(0)
assert_source();snapshot('before-'+label)
if label=='finish':write(E/'tracked-source-after.json',tracked());print(snapshot('after'));raise SystemExit(0)
assert label in ['strict0','strict1','vite0','vite1']
env=environment(label);os.environ.clear();os.environ.update(env)
if label.startswith('strict'):
 output=P/'.build'/('current-restore-'+label);os.environ['WEEBDEX_SEMANTIC_OUTPUT']=str(output)
 result=owner.run([str(NODE),str(P/'scripts/check-composed-frontend.mjs')],P,E/label/'strict-owned.json',600)
 if result:raise SystemExit(result)
 summary=json.loads((output/'evidence/summary.json').read_text());assert summary['status']=='passed'and summary['errors']==0
 shutil.copytree(output/'evidence',E/label/'semantic-evidence');write(E/label/'result.json',{'profile':label,'summary':summary,'source_unchanged':True,'browser_executed':False});print(label,summary['errors'],summary['warnings'],flush=True)
else:
 stage=P/'.build'/('current-restore-'+label);shutil.copytree(P/'frontend',stage);container=stage/'node_modules';container.mkdir()
 for entry in (D/'frontend/node_modules').iterdir():
  if not entry.name.startswith('.'):(container/entry.name).symlink_to(entry,target_is_directory=entry.is_dir())
 os.environ['WEEBDEX_FRONTEND_ROOT']=str(stage)
 assert not(P/'artifacts/frontend').exists()
 result=owner.run([str(NODE),str(P/'scripts/build-frontend.mjs')],P,E/label/'build-owned.json',900)
 if result:raise SystemExit(result)
 for name in ['build-integration.json','integration-source-manifest.json']:shutil.copyfile(P/'test-evidence/frontend'/name,E/label/name)
 artifact=P/'artifacts/frontend';assert not any(p.is_symlink()for p in artifact.rglob('*'));rows=[{'path':str(p.relative_to(artifact)),'bytes':p.stat().st_size,'mode':p.stat().st_mode&0o7777,'sha256':sha(p)}for p in sorted(artifact.rglob('*'))if p.is_file()];assert len(rows)>200
 kept=E/('artifact-'+label);artifact.rename(kept);manifest={'files':rows,'file_count':len(rows),'total_bytes':sum(x['bytes']for x in rows)};write(E/label/'artifact-files.json',manifest)
 os.environ.update(WEEBDEX_FRONTEND_ARTIFACT_ROOT=str(kept),WEEBDEX_FRONTEND_ARTIFACT_MANIFEST=str(E/label/'artifact-files.json'))
 result=owner.run([str(NODE),str(P/'tests/ci/test_frontend_generated_css.mjs')],P,E/label/'css-owned.json',30)
 if result:raise SystemExit(result)
 write(E/label/'result.json',{'profile':label,'source_unchanged':True,'browser_executed':False,'artifact_files':len(rows),'artifact_bytes':manifest['total_bytes'],'artifact_manifest_sha256':sha(E/label/'artifact-files.json')});print(label,len(rows),manifest['total_bytes'],flush=True)
audits=[]
for p in sorted((E/label/'cache-audit').glob('*.json')):
 v=json.loads(p.read_text());assert v['protected_roots']==[str(r)for r in roots()]and v['protected_write_attempts']==0 and v['denied']==[];audits.append({'path':str(p.relative_to(E)),'sha256':sha(p),'pid':v['pid'],'checked_node_mutations':v['checked_node_mutations']})
assert audits and sum(x['checked_node_mutations']for x in audits)>0
write(E/label/'cache-audit-summary.json',{'records':audits,'protected_write_attempts':0});assert_source();print(snapshot('after-'+label))
