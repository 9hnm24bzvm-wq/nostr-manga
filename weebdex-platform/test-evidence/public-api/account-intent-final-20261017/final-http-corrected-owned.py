import datetime,hashlib,json,os,pathlib,re,subprocess,sys
import signal,shutil,time
R=pathlib.Path('/workspace/shared/nostr-manga-recovery-20261017');W=R/'account-intent-boundaries-worktree-20261017';E=R/'account-intent-runtime-evidence-20261017';T=R/'toolchains';name,exe,*args=sys.argv[1:];log=E/(name+'.log');result=E/(name+'.json')
if name == 'frozen-chapter-http':
 exe = re.search(r'Executable tests/public_chapter_writes.rs \(([^)]+)\)', (R/'title-build-chapter.log').read_text())[1]
assert shutil.disk_usage(W).free>=4*1024**3+256*1024**2
assert not log.exists() and not result.exists();before=set((W/'weebdex-platform/test-runtime').glob('auth-*'))
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest();now=lambda:datetime.datetime.now(datetime.timezone.utc).isoformat()
env=os.environ.copy();env.update(PLATFORM_TEST_PG_BIN=str(T/'debian-pg/root/usr/lib/postgresql/17/bin'),LD_LIBRARY_PATH=str(T/'debian-pg/root/usr/lib/x86_64-linux-gnu'),STRFRY_BIN=str(R/'upload-replacement-worktree/manga-protocol-lab/vendor/strfry/strfry'),PYTHONDONTWRITEBYTECODE='1')
pins=json.loads((E/'final-source-before.json').read_text());assert all(sha(W/p)==v for p,v in pins.items())
compiled=json.loads((E/'final-compile-02.json').read_text());assert compiled['passed'] and compiled['executableFiles']['tests/public_chapter_writes.rs']['path']==exe and compiled['executableFiles']['tests/public_chapter_writes.rs']['sha256']==sha(pathlib.Path(exe))
cmd=['bash',str(W/'weebdex-platform/scripts/run-auth-tests.sh'),exe,*args];r={'schema':'owned-integrated-pipeline-http-run-v1','started_at_utc':now(),'source_commit':subprocess.check_output(['git','-C',str(W),'rev-parse','HEAD'],text=True).strip(),'command':cmd,'test_executable_sha256':sha(pathlib.Path(exe)),'pass':False};child=None
try:
 with log.open('xb') as output:
  child=subprocess.Popen(cmd,cwd=W,env=env,stdout=output,stderr=subprocess.STDOUT,start_new_session=True)
  fd=os.pidfd_open(child.pid,0)
  floor=4*1024**3;reserve=128*1024**2;deadline=time.monotonic()+220
  r['disk_guard']={'floor_bytes':floor,'cleanup_reserve_bytes':reserve,'min_observed_free_bytes':shutil.disk_usage(W).free,'signalled':False,'identity':'pidfd-owned guard child'}
  try:
   while child.poll() is None:
    free=shutil.disk_usage(W).free;r['disk_guard']['min_observed_free_bytes']=min(free,r['disk_guard']['min_observed_free_bytes'])
    if free<floor+reserve:
     signal.pidfd_send_signal(fd,signal.SIGTERM,None,0);r['disk_guard']['signalled']=True;r['disk_guard']['reason']='cleanup reserve reached';break
    if time.monotonic()>=deadline:raise subprocess.TimeoutExpired(cmd,220)
    time.sleep(.2)
   r['exit_code']=child.wait(timeout=35)
  finally:os.close(fd)
except BaseException as exc:
 r['error']={'class':type(exc).__name__,'message':str(exc)}
 if child is not None and child.poll() is None:
  child.terminate()
  try:r['exit_code']=child.wait(timeout=35)
  except subprocess.TimeoutExpired:r['cleanup_unconfirmed']=True
finally:
 r['source_file_count']=len(pins);r['source_changed']=[p for p,v in pins.items() if sha(W/p)!=v];r['test_executable_sha256_after']=sha(pathlib.Path(exe));r['executable_unchanged']=r['test_executable_sha256_after']==r['test_executable_sha256'];r['ended_at_utc']=now();r['log_sha256']=sha(log);r['child_reaped']=child is not None and child.poll() is not None
 try:
  new=set((W/'weebdex-platform/test-runtime').glob('auth-*'))-before;assert len(new)==1;cp=new.pop()/'cleanup.json';c=json.loads(cp.read_text());r['cleanup_receipt_sha256']=sha(cp);r['cleanup_path']=str(cp);r['cleanup']={key:c[key] for key in ['postgres_cleanup','exit','forced','closed_port','remaining_owned_processes','cleanup_errors','unconfirmed_births','unconfirmed_child_pids']};assert c['postgres_cleanup']=='verified_stopped' and not c['forced'] and c['closed_port'] and not c['remaining_owned_processes'] and not c['cleanup_errors'] and not c['unconfirmed_births'] and not c['unconfirmed_child_pids']
  m=re.search(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored; \d+ measured; (\d+) filtered out; finished in ([\d.]+)s',log.read_text());assert m;r['tests']={'passed':int(m[1]),'failed':int(m[2]),'ignored':int(m[3]),'filtered_out':int(m[4]),'seconds':float(m[5])};r['cases']=re.findall(r'^test (\w+) \.\.\. ok$',log.read_text(),re.M);assert r['tests']['passed']>0 and r['tests']['failed']==0
  r['pass']=not r['source_changed'] and not r.get('disk_guard',{}).get('signalled',False) and r['executable_unchanged'] and r['exit_code']==0 and r['child_reaped'] and not r.get('error') and not r.get('cleanup_unconfirmed')
 except BaseException as exc:r['receipt_error']={'class':type(exc).__name__,'message':str(exc)}
 result.write_text(json.dumps(r,indent=2)+'\n')
print(json.dumps({k:v for k,v in r.items() if k not in ['command','cleanup_path','cleanup']},indent=2),flush=True)
raise SystemExit(0 if r['pass'] else 1)
