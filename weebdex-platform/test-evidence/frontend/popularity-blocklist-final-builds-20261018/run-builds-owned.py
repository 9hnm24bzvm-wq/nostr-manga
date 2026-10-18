import datetime,hashlib,importlib.util,json,os,pathlib,shutil,subprocess,sys,signal,time,re
R=pathlib.Path('/workspace/shared/nostr-manga-recovery-20261017');W=R/'popularity-final-build-worktree-20261018';P=W/'weebdex-platform';E=R/'popularity-final-builds-evidence-20261018';NODE=pathlib.Path('/opt/codex/runtimes/codex-primary-runtime/dependencies/node/bin/node');DEP=R/'replacement-frontend-validation-02/dependencies-before.json';COMMIT='351cf41bfa2955e37cc3ac90cf4b4f37b965bcc9'
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def now():return datetime.datetime.now(datetime.timezone.utc).isoformat()
def write(p,v):p.write_text(json.dumps(v,indent=2)+'\n')
def load(name,p):
 s=importlib.util.spec_from_file_location(name,p);m=importlib.util.module_from_spec(s);s.loader.exec_module(m);return m
assert sha(DEP)=='e51d6db7e15636d1a4bcaa913d5fcb0f0553fdff985a2f9ea716e3e9471cbbda';assert sha(NODE)=='bc17c508ffeed0ec622934f9b7fa72f8e78da65350e63c3eceb56fa688aa5e12'
deps=json.loads(DEP.read_text());shared=R/'frontend-build-worktree/weebdex-platform/.build';roots=[shared/n for n in ['frontend-restore3/node_modules','local-library-noble-2.0.1','local-signing-base-2.0.0']];tools=load('new_build_frozen_source',P/'scripts/ci/prepare-hosted-browser.py')
def verify_deps():
 actual={str(p)for root in roots for p in root.rglob('*')if p.is_symlink()or not p.is_dir()};assert actual=={x['path']for x in deps}
 for row in deps:
  p=pathlib.Path(row['path'])
  if row['type']=='symlink':assert p.is_symlink()and str(p.readlink())==row['target']and p.exists()
  else:assert p.is_file()and not p.is_symlink()and p.stat().st_size==row['bytes']and p.stat().st_mode&0o7777==row['mode']and sha(p)==row['sha256'],str(p)
 return True
label=sys.argv[1];assert label in ['setup','default','enabled']
if label=='setup':
 E.mkdir();tools.verify_frontend();assert subprocess.check_output(['git','-C',str(W),'rev-parse','HEAD'],text=True).strip()==COMMIT;assert verify_deps();assert shutil.disk_usage(R).free>=4*1024**3+256*1024**2
 before=tools.source_snapshot();write(E/'source-before.json',before)
 frontend_inputs={p:v for p,v in before.items() if p.startswith(('weebdex-platform/frontend/','weebdex-platform/platform-integrations/','weebdex-platform/scripts/'))};write(E/'frontend-inputs-before.json',frontend_inputs)
 write(E/'canonical-before.json',{p:v for p,v in before.items() if p.startswith('weebdex-platform/frontend/')})
 for profile in ['default','enabled']:
  stage=P/'.build'/('frontend-popularity-final-20261018-'+profile);shutil.copytree(P/'frontend',stage);container=stage/'node_modules';container.mkdir()
  for package in roots[0].iterdir():
   if not package.name.startswith('.'):(container/package.name).symlink_to(package,target_is_directory=package.is_dir())
 for name in ['local-library-noble-2.0.1','local-signing-base-2.0.0']:
  link=P/'.build'/name
  if link.is_symlink():assert link.resolve()==(shared/name).resolve()
  else:link.symlink_to(shared/name,target_is_directory=True)
 write(E/'setup.json',{'source_commit':COMMIT,'source_tree':subprocess.check_output(['git','-C',str(W),'rev-parse','HEAD^{tree}'],text=True).strip(),'source_map_sha256':sha(E/'source-before.json'),'dependencies_manifest_path':str(DEP),'dependencies_manifest_sha256':sha(DEP),'verified_dependency_entries':len(deps),'canonical_frontend_files':679,'all_inputs_unchanged':True,'frontend_inputs_sha256':sha(E/'frontend-inputs-before.json'),'frontend_inputs':len(frontend_inputs),'canonical_manifest_sha256':sha(E/'canonical-before.json'),'no_dependency_install_or_download':True,'at_utc':now()});print('Setup verified source and 39226 dependency entries',flush=True);raise SystemExit(0)
setup=json.loads((E/'setup.json').read_text());before=json.loads((E/'source-before.json').read_text());assert tools.source_snapshot()==before and verify_deps();tools.verify_frontend();assert shutil.disk_usage(R).free>=4*1024**3+256*1024**2
stage=P/'.build'/('frontend-popularity-final-20261018-'+label);assert(stage/'package.json').read_bytes()==(P/'frontend/package.json').read_bytes();assert(stage/'yarn.lock').read_bytes()==(P/'frontend/yarn.lock').read_bytes();D=E/label;D.mkdir();assert not(P/'artifacts/frontend').exists()
env={k:v for k,v in os.environ.items()if k in ['HOME','TMPDIR','TMP','TEMP','LANG','LC_ALL','LC_CTYPE','SSL_CERT_FILE','SSL_CERT_DIR']};env.update(PATH=str(NODE.parent)+':/usr/bin:/bin',NODE_OPTIONS='--max-old-space-size=2048',WEEBDEX_SEMANTIC_NODE_MODULES=str(stage/'node_modules'),WEEBDEX_QA_BUILD_RESOURCE_PROFILE='serial-terser',PUBLIC_MANGADEX_CATALOGUE='1'if label=='enabled'else'0',WEEBDEX_FRONTEND_ROOT=str(stage),PYTHONDONTWRITEBYTECODE='1')
argv=[str(NODE),str(P/'scripts/build-frontend.mjs')]
started=now();began=time.monotonic();minimum=shutil.disk_usage(R).free;min_mem=10**20;max_rss=0;stop=None;births={};signals=[]
def scan(group):
 current=[]
 for file in pathlib.Path('/proc').glob('[0-9]*/stat'):
  try:
   fields=file.read_text().rsplit(')',1)[1].split();pid=int(file.parent.name)
   if int(fields[2])!=group or int(fields[3])!=group or fields[0]=='Z':continue
   key=(pid,int(fields[19]));rss=int(fields[21])*os.sysconf('SC_PAGE_SIZE');current.append((key,rss))
   if key not in births:
    fd=os.pidfd_open(pid,0);again=file.read_text().rsplit(')',1)[1].split()
    if int(again[19])!=key[1] or int(again[2])!=group or int(again[3])!=group:os.close(fd);continue
    births[key]=fd
  except (OSError,ValueError,IndexError):continue
 return current
def send(sig,group):
 scan(group)
 for key,fd in births.items():
  try:signal.pidfd_send_signal(fd,sig,None,0);signals.append({'birth':list(key),'signal':sig.name})
  except ProcessLookupError:pass
with (D/'build.log').open('xb') as output:
 child=subprocess.Popen(argv,cwd=P,env=env,stdout=output,stderr=subprocess.STDOUT,start_new_session=True)
 try:
  while child.poll() is None:
   current=scan(child.pid);free=shutil.disk_usage(R).free;minimum=min(minimum,free);rss=sum(value for _,value in current);max_rss=max(max_rss,rss)
   available=int(re.search(r'^MemAvailable:\s+(\d+)',pathlib.Path('/proc/meminfo').read_text(),re.M)[1])*1024;min_mem=min(min_mem,available)
   if free<4*1024**3+128*1024**2:stop='cleanup disk reserve reached'
   elif rss>4*1024**3:stop='owned build RSS4GiB cap'
   elif available<1024**3:stop='shared available memory1GiB reserve'
   elif time.monotonic()-began>900:stop='900s build deadline'
   if stop:send(signal.SIGTERM,child.pid);break
   time.sleep(.1)
  try:child.wait(timeout=10)
  except subprocess.TimeoutExpired:send(signal.SIGKILL,child.pid);child.wait(timeout=10)
  survivors=scan(child.pid)
  if survivors:send(signal.SIGTERM,child.pid);time.sleep(.5);survivors=scan(child.pid)
  if survivors:send(signal.SIGKILL,child.pid);time.sleep(.5);survivors=scan(child.pid)
 finally:
  for fd in births.values():os.close(fd)
owned={'command':argv,'cwd':str(P),'deadline_seconds':900,'elapsed_seconds':round(time.monotonic()-began,3),'process_exit_code':child.returncode,'deadline_exceeded':stop=='900s build deadline','cleanup_verified':not survivors,'surviving_owned_process_ids':[list(key) for key,_ in survivors],'owned_process_births':[list(key) for key in births],'cleanup_signals':signals,'stop_reason':stop,'minimum_free_bytes':minimum,'cleanup_reserve_bytes':128*1024**2,'floor_bytes':4*1024**3,'maximum_owned_rss_bytes':max_rss,'minimum_available_memory_bytes':min_mem,'terminal_receipt':True,'identity':'verified process births and pidfd-owned descendants'};write(D/'owned.json',owned)
r={'schema':'integrated-frontend-production-build-v2','label':label,'source_commit':COMMIT,'started_at_utc':started,'ended_at_utc':now(),'source_map_sha256':sha(E/'source-before.json'),'frontend_inputs_sha256':sha(E/'frontend-inputs-before.json'),'canonical_manifest_sha256':sha(E/'canonical-before.json'),'dependencies_map_sha256':sha(DEP),'command':argv,'environment':{key:env[key]for key in ['NODE_OPTIONS','WEEBDEX_QA_BUILD_RESOURCE_PROFILE','PUBLIC_MANGADEX_CATALOGUE','WEEBDEX_FRONTEND_ROOT']},'exit_code':child.returncode,'owned':owned,'source_unchanged':tools.source_snapshot()==before,'dependencies_unchanged':verify_deps(),'free_after':shutil.disk_usage(R).free,'build_log_sha256':sha(D/'build.log'),'browser_executed':False,'dependency_downloaded':False,'pass':False}
tools.verify_frontend();r['canonical_frontend_679_files_preserved']=True
if child.returncode==0:
 integration=P/'test-evidence/frontend/build-integration.json';shutil.copyfile(integration,D/'build-integration.json');shutil.copyfile(P/'test-evidence/frontend/integration-source-manifest.json',D/'integration-source-manifest.json');value=json.loads(integration.read_text());assert value['public_signed_upload_enabled']==(label=='enabled')and value['public_api_catalogue_enabled']==(label=='enabled');root=P/'artifacts/frontend';assert not any(p.is_symlink()for p in root.rglob('*'));rows=[{'path':str(p.relative_to(root)),'bytes':p.stat().st_size,'mode':p.stat().st_mode&0o7777,'sha256':sha(p)}for p in sorted(root.rglob('*'))if p.is_file()];assert len(rows)>200;assert sha(root/'index.html')==value['spa_fallback_sha256'];kept=E/('artifact-'+label);root.rename(kept);write(D/'artifact-files.json',{'files':rows,'file_count':len(rows),'total_bytes':sum(row['bytes']for row in rows),'index_sha256':value['spa_fallback_sha256']});r.update(artifact=str(kept),artifact_files=len(rows),artifact_bytes=sum(row['bytes']for row in rows),artifact_manifest_sha256=sha(D/'artifact-files.json'),index_sha256=value['spa_fallback_sha256'],build_integration_sha256=sha(D/'build-integration.json'))
r['pass']=child.returncode==0 and stop is None and owned['process_exit_code']==0 and owned['deadline_exceeded']is False and owned['cleanup_verified']is True and owned['surviving_owned_process_ids']==[]and r['source_unchanged']and r['dependencies_unchanged']and r['free_after']>=4*1024**3
write(D/'result.json',r);print(json.dumps({k:v for k,v in r.items()if k!='owned'},indent=2),flush=True);raise SystemExit(0 if r['pass']else 1)
