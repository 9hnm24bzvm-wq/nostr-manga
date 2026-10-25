import datetime,gzip,hashlib,importlib.util,json,os,pathlib,re,shutil,signal,subprocess,sys,time
R=pathlib.Path('/workspace/shared/nostr-manga-recovery-20261017');W=R/'profile-title-final-build-worktree-20261018';P=W/'weebdex-platform';E=R/'profile-title-final-builds-evidence-20261018';NODE=pathlib.Path('/opt/codex/runtimes/codex-primary-runtime/dependencies/node/bin/node');COMMIT='34042537b23df594a0c7923773ea4f00c6e47b9c';A=R/'composed-frontend-validation-worktree-20261017/weebdex-platform/test-evidence/frontend/complete-discovered-physical-ci-20261018';DISCOVERY=A/'physical-dependency-root-discovery.json';DEP=R/'replacement-frontend-validation-02/dependencies-before.json';FLOOR=4*1024**3;RESERVE=128*1024**2
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
now=lambda:datetime.datetime.now(datetime.timezone.utc).isoformat()
def write(p,v):p.write_text(json.dumps(v,indent=2)+'\n')
assert sha(DISCOVERY)=='5aa2182074885fbe327f96144daa29086c1a41e5d565489dbb9b684250548690';assert sha(NODE)=='bc17c508ffeed0ec622934f9b7fa72f8e78da65350e63c3eceb56fa688aa5e12';assert sha(DEP)=='e51d6db7e15636d1a4bcaa913d5fcb0f0553fdff985a2f9ea716e3e9471cbbda'
roots=[pathlib.Path(x)for x in json.loads(DISCOVERY.read_text())['roots']];assert len(roots)==7 and len(set(x.resolve()for x in roots))==7 and all(x.resolve()==x for x in roots)
shared=R/'frontend-build-worktree/weebdex-platform/.build';old_roots=[shared/n for n in ['frontend-restore3/node_modules','local-library-noble-2.0.1','local-signing-base-2.0.0']];deps=json.loads(DEP.read_text())
def verify_deps():
 actual={str(p)for root in old_roots for p in root.rglob('*')if p.is_symlink()or not p.is_dir()};assert actual=={x['path']for x in deps}
 for x in deps:
  p=pathlib.Path(x['path'])
  if x['type']=='symlink':assert p.is_symlink()and str(p.readlink())==x['target']and p.exists()
  else:assert p.is_file()and not p.is_symlink()and p.stat().st_size==x['bytes']and p.stat().st_mode&0o7777==x['mode']and sha(p)==x['sha256'],str(p)
 return True
spec=importlib.util.spec_from_file_location('frozen_build_tools',P/'scripts/ci/prepare-hosted-browser.py');tools=importlib.util.module_from_spec(spec);spec.loader.exec_module(tools)
def product():
 files=subprocess.check_output(['git','ls-files','-z','--','frontend','platform-integrations','scripts','contracts'],cwd=P,text=True).split('\0');return{'weebdex-platform/'+f:sha(P/f)for f in files if f}
def physical(phase):
 snapshots={};summary={};symlinks=0
 for root in roots:
  manifest={}
  for parent,dirs,files in os.walk(root,followlinks=False):
   for name in sorted(dirs+files):
    p=pathlib.Path(parent)/name;s=p.lstat();row={'mode':s.st_mode&0o7777,'mtimeNs':s.st_mtime_ns}
    if p.is_symlink():
     row.update(kind='symlink',target=os.readlink(p));target=p.resolve(strict=True);assert any(target==r or r in target.parents for r in roots),str(p);symlinks+=1
    elif p.is_file():row.update(kind='file',bytes=s.st_size,sha256=sha(p))
    elif p.is_dir():row.update(kind='directory')
    else:raise RuntimeError('Unexpected dependency kind '+str(p))
    manifest[str(p.relative_to(root))]=row
  snapshots[str(root)]=manifest;summary[str(root)]={'entries':len(manifest),'regularFiles':sum(x['kind']=='file'for x in manifest.values()),'canonicalManifestSha256':hashlib.sha256(json.dumps(manifest,sort_keys=True,separators=(',',':')).encode()).hexdigest()}
 encoded=json.dumps(snapshots,sort_keys=True,separators=(',',':')).encode();(E/('physical-dependencies-'+phase+'.json.gz')).write_bytes(gzip.compress(encoded,mtime=0));write(E/('physical-dependencies-'+phase+'-summary.json'),summary)
 if phase=='before':assert encoded==gzip.decompress((A/'physical-dependencies-after-consumers.json.gz').read_bytes()),'Inputs changed since explicit account release'
 else:assert encoded==gzip.decompress((E/'physical-dependencies-before.json.gz').read_bytes()),'Physical cache bytes/modes/mtimeNs/symlinks changed'
 return{'roots':len(roots),'entries':sum(x['entries']for x in summary.values()),'regular_files':sum(x['regularFiles']for x in summary.values()),'symlinks':symlinks,'snapshot_sha256':sha(E/('physical-dependencies-'+phase+'.json.gz'))}
def audit_summary(D):
 rows=[]
 for p in sorted((D/'cache-audit').glob('*.json')):
  v=json.loads(p.read_text());assert v['protected_roots']==[str(r)for r in roots];assert v['protected_write_attempts']==0 and v['denied']==[];rows.append({'file':str(p.relative_to(E)),'sha256':sha(p),'pid':v['pid'],'checked_mutations':v['checked_node_mutations'],'roots':len(v['protected_roots'])})
 assert rows and sum(x['checked_mutations']for x in rows)>0;write(D/'cache-audit-summary.json',{'records':rows,'protected_write_attempts':0,'all_physical_roots_protected':True});return rows

def run_owned(argv,D,env,label,seconds=900):
 births={};signals=[];start=time.monotonic();minimum=shutil.disk_usage(R).free;min_mem=10**20;max_rss=0;stop=None
 assert minimum>=FLOOR+2*RESERVE,'Insufficient launch reserve'
 def scan(group):
  current=[]
  for file in pathlib.Path('/proc').glob('[0-9]*/stat'):
   try:
    f=file.read_text().rsplit(')',1)[1].split();pid=int(file.parent.name)
    if int(f[2])!=group or int(f[3])!=group or f[0]=='Z':continue
    key=(pid,int(f[19]));current.append((key,int(f[21])*os.sysconf('SC_PAGE_SIZE')))
    if key not in births:
     fd=os.pidfd_open(pid,0);again=file.read_text().rsplit(')',1)[1].split()
     if int(again[19])!=key[1]or int(again[2])!=group or int(again[3])!=group:os.close(fd);continue
     births[key]=fd
   except(OSError,ValueError,IndexError):continue
  return current
 def send(sig,group):
  scan(group)
  for key,fd in births.items():
   try:signal.pidfd_send_signal(fd,sig,None,0);signals.append({'birth':list(key),'signal':sig.name})
   except ProcessLookupError:pass
 with (D/(label+'.log')).open('xb') as output:
  child=subprocess.Popen(argv,cwd=P,env=env,stdout=output,stderr=subprocess.STDOUT,start_new_session=True);scan(child.pid)
  try:
   while child.poll()is None:
    current=scan(child.pid);free=shutil.disk_usage(R).free;minimum=min(minimum,free);max_rss=max(max_rss,sum(x[1]for x in current));available=int(re.search(r'^MemAvailable:\s+(\d+)',pathlib.Path('/proc/meminfo').read_text(),re.M)[1])*1024;min_mem=min(min_mem,available)
    if free<FLOOR+RESERVE:stop='cleanup disk reserve reached'
    elif max_rss>FLOOR:stop='owned RSS4GiB cap'
    elif available<1024**3:stop='available memory1GiB reserve'
    elif time.monotonic()-start>seconds:stop='deadline reached'
    if stop:send(signal.SIGTERM,child.pid);break
    time.sleep(.1)
   try:child.wait(timeout=10)
   except subprocess.TimeoutExpired:send(signal.SIGKILL,child.pid);child.wait(timeout=10)
   survivors=scan(child.pid)
   if survivors:send(signal.SIGTERM,child.pid);time.sleep(.5);survivors=scan(child.pid)
   if survivors:send(signal.SIGKILL,child.pid);time.sleep(.5);survivors=scan(child.pid)
  finally:
   for fd in births.values():os.close(fd)
 value={'command':argv,'cwd':str(P),'deadline_seconds':seconds,'elapsed_seconds':round(time.monotonic()-start,3),'exit_code':child.returncode,'stop_reason':stop,'cleanup_verified':not survivors,'surviving_owned_process_ids':[list(k)for k,_ in survivors],'owned_process_births':[list(k)for k in births],'cleanup_signals':signals,'minimum_free_bytes':minimum,'floor_bytes':FLOOR,'cleanup_reserve_bytes':RESERVE,'maximum_owned_rss_bytes':max_rss,'minimum_available_memory_bytes':min_mem,'identity':'verified process births and pidfd-owned descendants'};write(D/(label+'-owned.json'),value);assert value['exit_code']==0 and stop is None and not survivors,value;return value
label=sys.argv[1];assert label in['setup','default','enabled','finish']
if label=='setup':
 E.mkdir();assert subprocess.check_output(['git','rev-parse','HEAD'],cwd=W,text=True).strip()==COMMIT;assert verify_deps();tools.verify_frontend();assert shutil.disk_usage(R).free>=FLOOR+2*RESERVE
 before=tools.source_snapshot();write(E/'source-before.json',before);write(E/'product-before.json',product());write(E/'canonical-before.json',{p:v for p,v in before.items()if p.startswith('weebdex-platform/frontend/')});shutil.copyfile(DISCOVERY,E/'physical-dependency-root-discovery.json');v=physical('before')
 for profile in['default','enabled']:
  stage=P/'.build'/('frontend-profile-title-final-'+profile);shutil.copytree(P/'frontend',stage);container=stage/'node_modules';container.mkdir()
  for package in old_roots[0].iterdir():
   if not package.name.startswith('.'):(container/package.name).symlink_to(package,target_is_directory=package.is_dir())
 for name in['local-library-noble-2.0.1','local-signing-base-2.0.0']:(P/'.build'/name).symlink_to(shared/name,target_is_directory=True)
 aliases={'@lucide/svelte':'frontend-restore3/node_modules/@lucide/svelte/dist/lucide-svelte.js','svelte-sonner':'frontend-restore3/node_modules/svelte-sonner/dist/index.js','axios':'frontend-restore3/node_modules/axios/index.js','http-status-codes':'frontend-restore3/node_modules/http-status-codes/build/es/index.js','@noble/curves':'local-library-noble-2.0.1/node_modules/@noble/curves','@noble/hashes':'local-library-noble-2.0.1/node_modules/@noble/hashes','@scure/base':'local-signing-base-2.0.0/node_modules/@scure/base'}
 bindings=[]
 for name,target in aliases.items():
  real=(shared/target).resolve(strict=True);covered=[str(r)for r in roots if real==r or r in real.parents];assert covered;bindings.append({'alias':name,'physical_target':str(real),'protected_roots':covered})
 write(E/'alias-physical-bindings.json',{'configuration_sha256':sha(P/'scripts/vite.platform.config.mjs'),'bindings':bindings,'full_primary_root_explicit':str(old_roots[0])in[str(r)for r in roots]})
 write(E/'setup.json',{'source_commit':COMMIT,'source_tree':subprocess.check_output(['git','rev-parse','HEAD^{tree}'],cwd=W,text=True).strip(),'physical_dependency_inventory':v,'original_verified_dependency_entries':len(deps),'original_manifest_sha256':sha(DEP),'canonical_files':679,'at_utc':now(),'no_download_copy_or_install_of_dependencies':True});print('Setup',v,flush=True);raise SystemExit(0)
before=json.loads((E/'source-before.json').read_text());products=json.loads((E/'product-before.json').read_text());assert tools.source_snapshot()==before and product()==products;tools.verify_frontend();assert verify_deps()
if label=='finish':
 write(E/'source-after.json',tools.source_snapshot());write(E/'product-after.json',product());v=physical('after');print('Final snapshots unchanged',v,flush=True);raise SystemExit(0)
physical('before-'+label);D=E/label;D.mkdir();(D/'cache-audit').mkdir();stage=P/'.build'/('frontend-profile-title-final-'+label);assert not(P/'artifacts/frontend').exists()
env={k:v for k,v in os.environ.items()if k in['HOME','TMPDIR','TMP','TEMP','LANG','LC_ALL','LC_CTYPE','SSL_CERT_FILE','SSL_CERT_DIR']};env.update(PATH=str(NODE.parent)+':/usr/bin:/bin',NODE_OPTIONS='--max-old-space-size=2048 --require='+str(P/'scripts/semantic-readonly-cache-guard.cjs'),WEEBDEX_READONLY_CACHE_ROOTS=json.dumps([str(r)for r in roots]),WEEBDEX_READONLY_AUDIT_DIR=str(D/'cache-audit'),WEEBDEX_SEMANTIC_NODE_MODULES=str(stage/'node_modules'),WEEBDEX_QA_BUILD_RESOURCE_PROFILE='serial-terser',PUBLIC_MANGADEX_CATALOGUE='1'if label=='enabled'else'0',WEEBDEX_FRONTEND_ROOT=str(stage),PYTHONDONTWRITEBYTECODE='1')
started=now();owned=run_owned([str(NODE),str(P/'scripts/build-frontend.mjs')],D,env,'build');tools.verify_frontend();assert tools.source_snapshot()==before and product()==products and verify_deps();dep=physical('after-'+label)
for name in['build-integration.json','integration-source-manifest.json']:shutil.copyfile(P/'test-evidence/frontend'/name,D/name)
integration=json.loads((D/'build-integration.json').read_text());assert integration['public_signed_upload_enabled']==(label=='enabled')and integration['public_api_catalogue_enabled']==(label=='enabled');artifact=P/'artifacts/frontend';assert not any(p.is_symlink()for p in artifact.rglob('*'));rows=[{'path':str(p.relative_to(artifact)),'bytes':p.stat().st_size,'mode':p.stat().st_mode&0o7777,'sha256':sha(p)}for p in sorted(artifact.rglob('*'))if p.is_file()];assert len(rows)>200 and sha(artifact/'index.html')==integration['spa_fallback_sha256'];kept=E/('artifact-'+label);artifact.rename(kept);write(D/'artifact-files.json',{'files':rows,'file_count':len(rows),'total_bytes':sum(x['bytes']for x in rows),'index_sha256':integration['spa_fallback_sha256']})
env.update(WEEBDEX_FRONTEND_ARTIFACT_ROOT=str(kept),WEEBDEX_FRONTEND_ARTIFACT_MANIFEST=str(D/'artifact-files.json'));css=run_owned([str(NODE),str(P/'tests/ci/test_frontend_generated_css.mjs')],D,env,'css-consumer',30);audits=audit_summary(D);assert tools.source_snapshot()==before and product()==products;physical('after-css-'+label)
value={'schema':'physical-guarded-production-vite-v1','profile':label,'source_commit':COMMIT,'source_tree':json.loads((E/'setup.json').read_text())['source_tree'],'started_at_utc':started,'ended_at_utc':now(),'pass':True,'build':owned,'css_consumer':css,'source_unchanged':True,'canonical_frontend_files':679,'product_pins_sha256':sha(E/'product-before.json'),'physical_dependencies':dep,'cache_guard_audit_count':len(audits),'protected_write_attempts':0,'artifact_directory':str(kept),'artifact_files':len(rows),'artifact_bytes':sum(x['bytes']for x in rows),'artifact_manifest_sha256':sha(D/'artifact-files.json'),'index_sha256':integration['spa_fallback_sha256'],'browser_executed':False,'dependency_install_download_copy':False,'qa':{'node_heap_MiB':2048,'terser_workers':1,'deadline_seconds':900,'floor_bytes':FLOOR,'cleanup_reserve_bytes':RESERVE},'free_after_bytes':shutil.disk_usage(R).free};write(D/'result.json',value);print(json.dumps({k:v for k,v in value.items()if k not in['build','css_consumer']},indent=2),flush=True)
