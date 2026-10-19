"""Owned, retaining adapter for exactly17 existing PostgreSQL18 diagnostics."""
import ctypes,hashlib,importlib.util,json,os,shutil,signal,socket,stat,subprocess,sys,tempfile,time
from pathlib import Path
D=Path(__file__).resolve().parent;R=D.parent;G=R/'pg18-diagnostics-worktree';P=G/'manga-protocol-lab';H=G/'weebdex-platform/tests/runtime/auth_fixture_guard.py';E=D/'runtime-evidence-03';ROOT=D/'root'
HELPER_SHA='17ff9c2eb04a4bdefaa3b61e975e77567f873208a83454afcdd6ca19c5cfc33e';BASE='5cbd05d91c38e22eb6a7509aa6e9f8c21b12e3b4';EXPECTED_INVENTORY_SHA='e4cbc464100b9ad3fa27fba5a2ee391bace8dc391f085b87955e90756c804774'
assert hashlib.sha256(H.read_bytes()).hexdigest()==HELPER_SHA
spec=importlib.util.spec_from_file_location('pg18_owner',H);h=importlib.util.module_from_spec(spec);spec.loader.exec_module(h)
CLEANUP=D/'cleanup-owned.py';assert hashlib.sha256(CLEANUP.read_bytes()).hexdigest()=='9d8aaf970c724c29a02b7867e00a8a0f524a9021569e7c3b64306b14d610de1a'
spec=importlib.util.spec_from_file_location('exact_cleanup',CLEANUP);c=importlib.util.module_from_spec(spec);spec.loader.exec_module(c)
assert ctypes.CDLL(None,use_errno=True).prctl(36,1,0,0,0)==0
os.umask(0o077)
CAP=1024**3;MEMORY_RESERVE=2*1024**3;FIXTURE_CAP=256*1024**2;OUTPUT_CAP=16*1024**2;FLOOR=4*1024**3;FORECAST=768*1024**2
known={};unknown={};children=[];started=time.monotonic();cancelled=None;peakrss=0;peakfixture=0;peakoutput=0;minimum=shutil.disk_usage(R).free;minmemory=2**63;mintmp=shutil.disk_usage('/tmp').free
paths=sorted({str(q.relative_to(G)) for q in (P/'bench/library-node').glob('*.py')}|{str(q.relative_to(G)) for q in (P/'server/migrations').glob('*.sql')}|{'manga-protocol-lab/bench/library-node/test-pg-diagnostics.sh','weebdex-platform/tests/runtime/auth_fixture_guard.py'})
pins={p:hashlib.sha256((G/p).read_bytes()).hexdigest() for p in paths}
report={'schema':'retained-pg18-diagnostics-owner-v1','sourceBase':BASE,'sourcePins':pins,'scope':'17 unchanged controlled CLI/snapshot diagnostics; synthetic loopback PG18 cluster; retained database and wrapper scratch','limits':{'seconds':178,'workSeconds':148,'cleanupSeconds':30,'ownedRssBytes':CAP,'hostAvailableBytes':MEMORY_RESERVE,'fixtureBytes':FIXTURE_CAP,'outputBytes':OUTPUT_CAP,'floorFreeBytes':FLOOR},'priorRuntimeSeconds':1.893,'cumulativeRuntimeSecondsCeiling':180,'driverSha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),'launcherSha256':hashlib.sha256((D/'launch-diagnostics-03.py').read_bytes()).hexdigest(),'postmasterGateSha256':hashlib.sha256((D/'exec-after-admission.py').read_bytes()).hexdigest()}
def memory():return int(next(v.split()[1] for v in Path('/proc/meminfo').read_text().splitlines() if v.startswith('MemAvailable:')))*1024
def allocation(root):
 seen=set();total=0
 if root is None:return 0
 for p in [root,*root.rglob('*')]:
  try:st=p.lstat()
  except FileNotFoundError:continue
  key=(st.st_dev,st.st_ino)
  if key not in seen:seen.add(key);total+=st.st_blocks*512
 return total

def installed():
 path=D/'installed-inventory.json';assert hashlib.sha256(path.read_bytes()).hexdigest()==EXPECTED_INVENTORY_SHA,'reviewed installed inventory changed'
 inventory=json.loads(path.read_text());expected={v['path']:v for v in inventory};actual={}
 for p in sorted(ROOT.rglob('*')):
  st=p.lstat();key=str(p.relative_to(ROOT))
  if stat.S_ISLNK(st.st_mode):assert p.resolve().is_relative_to(ROOT);actual[key]={'path':key,'symlink':os.readlink(p)}
  elif stat.S_ISREG(st.st_mode):actual[key]={'path':key,'bytes':st.st_size,'sha256':hashlib.sha256(p.read_bytes()).hexdigest()}
  else:assert stat.S_ISDIR(st.st_mode),'unexpected installed file kind'
 assert actual==expected,'installed exact membership/content changed'
 return len(actual)
def approved_source():
 actual=subprocess.run(['git','-C',str(G),'rev-parse','HEAD','HEAD^{tree}',BASE+'^{tree}'],check=True,capture_output=True,text=True).stdout.splitlines();assert actual[0]==BASE and actual[1]==actual[2]
 report.update(actualSourceHead=actual[0],actualSourceTree=actual[1],approvedSourceTree=actual[2])
 raw=subprocess.run(['git','-C',str(G),'cat-file','--batch'],input=''.join(BASE+':'+p+'\n' for p in paths).encode(),check=True,capture_output=True).stdout
 for path in paths:
  header,separator,raw=raw.partition(b'\n');fields=header.split();assert separator and len(fields)==3 and fields[1]==b'blob'
  size=int(fields[2]);body,raw=raw[:size],raw[size:];assert raw[:1]==b'\n';raw=raw[1:];assert hashlib.sha256(body).hexdigest()==pins[path]
 assert not raw;report['approvedSourceVerified']=True
runtime=None;pg_ident=None;listener_write=None;listener_released=False;baseline_listeners=set();admitted_listener=None
PRIOR_RUNTIMES=[Path('/tmp/mixed-diagnostic-p6lyp0d9'),Path('/tmp/mixed-diagnostic-29kvaini')]
def listeners():
 values=set()
 for table in ['tcp','tcp6']:
  for line in (Path('/proc/net')/table).read_text().splitlines()[1:]:
   fields=line.split()
   if fields[3]=='0A':values.add((table,fields[1],fields[9]))
 return values

def check():
 global peakrss,peakfixture,peakoutput,minimum,minmemory,mintmp,pg_ident,listener_released,listener_write,admitted_listener
 if cancelled:raise RuntimeError('owned diagnostics interrupted')
 live=h.members(known,unknown=unknown);rss=0
 for value in [*live,{'pid':os.getpid()}]:
  try:rss+=int((Path('/proc')/str(value['pid'])/'stat').read_text().rsplit(')',1)[1].split()[21])*os.sysconf('SC_PAGE_SIZE')
  except FileNotFoundError:pass
 peakrss=max(peakrss,rss);available=memory();minmemory=min(minmemory,available);free=shutil.disk_usage(R).free;minimum=min(minimum,free);tmpfree=shutil.disk_usage('/tmp').free;mintmp=min(mintmp,tmpfree)
 fixture_bytes=sum(allocation(p) for p in [*PRIOR_RUNTIMES,runtime] if p is not None);output_bytes=sum(allocation(p) for p in [D/'runtime-evidence',D/'runtime-evidence-02',E] if p.exists());peakfixture=max(peakfixture,fixture_bytes);peakoutput=max(peakoutput,output_bytes)
 if rss>CAP or available<MEMORY_RESERVE:raise RuntimeError('memory cap/reserve reached')
 if free<FLOOR+128*1024**2 or tmpfree<512*1024**2:raise RuntimeError('filesystem reserve reached')
 if fixture_bytes>FIXTURE_CAP or output_bytes>OUTPUT_CAP:raise RuntimeError('fixture/output allocation cap reached')
 if time.monotonic()-started>148:raise RuntimeError('diagnostics deadline reached')
 value=None
 if (E/'fixture-result.json').is_file():
  try:value=json.loads((E/'fixture-result.json').read_text())
  except json.JSONDecodeError:pass
 if value and value.get('postmasterBirth'):
  ident=value['postmasterBirth'];key=(ident['pid'],ident['start_ticks'])
  assert key in known and known[key]==ident,'postmaster birth not in owned lineage'
  if not listener_released:assert h.same(ident),'postmaster exited before listener admission'
  if pg_ident is None:pg_ident=ident
  else:assert pg_ident==ident,'postmaster identity changed'
 fresh=listeners()-baseline_listeners
 for current in fresh:assert current[:2]==('tcp','0100007F:D88D'),'unexpected new listener'
 if admitted_listener is not None:
  assert not fresh or fresh=={admitted_listener},'admitted listener generation changed'
 if not listener_released and pg_ident and h.owns_listener(pg_ident,55437):
  assert len(fresh)==1;admitted_listener=next(iter(fresh))
  report['listenerAdmission']={'postmasterBirth':pg_ident,'address':'127.0.0.1','port':55437,'ownedBeforeFirstSql':True,'listenerInode':admitted_listener[2]}
  os.write(listener_write,b'L');os.close(listener_write);listener_write=None;listener_released=True

def interrupted(sig,_):
 global cancelled
 cancelled=sig
signal.signal(signal.SIGINT,interrupted);signal.signal(signal.SIGTERM,interrupted)
out=err=None;gate_read=gate_write=listener_read=None;released=False
try:
 approved_source();report['installedEntriesBefore']=installed()
 assert FORECAST<CAP and memory()>=MEMORY_RESERVE+FORECAST,'memory forecast does not fit'
 assert minimum>=FLOOR+256*1024**2+OUTPUT_CAP and mintmp>=512*1024**2+FIXTURE_CAP,'filesystem projection does not fit'
 assert h.closed(55437),'fixed diagnostic port is not closed'
 for table in ['tcp','tcp6']:
  assert not any(int(line.split()[1].split(':')[1],16)==55437 for line in (Path('/proc/net')/table).read_text().splitlines()[1:]),'fixed diagnostic port is in use'
 with socket.socket() as vacancy:vacancy.bind(('127.0.0.1',55437))
 baseline_listeners=listeners();report['baselineListenerCount']=len(baseline_listeners)
 report['listenerGuard']='pre-launch baseline plus fixed new endpoint/inode and exact postmaster FD proof before SQL; no descendant FD sweep'
 E.mkdir(mode=0o700,exist_ok=False);runtime=Path(tempfile.mkdtemp(prefix='mixed-diagnostic-'));runtime.chmod(0o700);report['retainedRuntime']=str(runtime)
 env={k:v for k,v in os.environ.items() if not k.startswith('PG') and not k.startswith('MIXED_') and k not in {'PYTHONOPTIMIZE','PYTHONPATH','LD_PRELOAD','LD_LIBRARY_PATH'}};env.update(PYTHONDONTWRITEBYTECODE='1',PYTHONPATH=str(P/'bench/library-node'),DIAGNOSTIC_RUNTIME=str(runtime))
 out=(E/'stdout.log').open('w');err=(E/'stderr.log').open('w');gate_read,gate_write=os.pipe();listener_read,listener_write=os.pipe();env.update(OWNER_GATE_FD=str(gate_read),LISTENER_GATE_FD=str(listener_read))
 child=subprocess.Popen([sys.executable,'-B',str(D/'launch-diagnostics-03.py')],stdout=out,stderr=err,env=env,cwd=P,stdin=subprocess.DEVNULL,start_new_session=True,pass_fds=(gate_read,listener_read));children.append((child,None));os.close(gate_read);gate_read=None;os.close(listener_read);listener_read=None
 birth=h.row(child.pid);assert birth and birth['ppid']==os.getpid() and birth['session']==birth['group']==child.pid
 ident=h.identity(birth);children[-1]=(child,ident);known[(birth['pid'],birth['start_ticks'])]=ident;check();os.write(gate_write,b'G');released=True;os.close(gate_write);gate_write=None
 while child.poll() is None:check();time.sleep(.05)
 # A normally exited postmaster no longer has a live identity. Resource sampling
 # still occurs before terminal verification, with listener admission complete.
 report['returnCode']=child.returncode;out.flush();err.flush();assert child.returncode==0,'fixture process failed'
 fixture=json.loads((E/'fixture-result.json').read_text());assert fixture['status']=='passed' and fixture['tests']=={'testsRun':17,'failures':0,'errors':0,'skipped':0,'successful':True}
 assert listener_released and fixture['postmasterForced'] is False and fixture['postmasterExit']==0 and fixture['postmasterPidFileRemoved']
 assert all(hashlib.sha256((G/p).read_bytes()).hexdigest()==v for p,v in pins.items());report['installedEntriesAfter']=installed();report.update(success=True,tests=fixture['tests'],fixtureResultSha256=hashlib.sha256((E/'fixture-result.json').read_bytes()).hexdigest())
except BaseException as error:report.update(success=False,errorClass=type(error).__name__,error=str(error))
finally:
 for fd in [gate_read,gate_write,listener_read,listener_write]:
  if fd is not None:os.close(fd)
 cleanup_error=None;remaining=[];state={'forced':False};forced=None
 if children and not released:
  try:children[-1][0].wait(timeout=5)
  except Exception as error:cleanup_error='unadmitted child did not abort: '+type(error).__name__
 # If the launcher was interrupted, request fast shutdown from the exact
 # already-admitted postmaster before the general descendant cleanup phases.
 if pg_ident and h.same(pg_ident):
  try:h.send(pg_ident,signal.SIGINT)
  except Exception as error:cleanup_error='postmaster fast-stop request: '+type(error).__name__
 forced,remaining,stop_error=c.cleanup(h,children,known,unknown,min(started+178,time.monotonic()+30))
 if stop_error:cleanup_error=stop_error
 if out:out.close()
 if err:err.close()
 fixture={}
 if (E/'fixture-result.json').is_file():
  try:fixture=json.loads((E/'fixture-result.json').read_text())
  except json.JSONDecodeError:pass
 closed=h.closed(55437);pid_removed=runtime is None or not(runtime/'data/postmaster.pid').exists()
 clean=not remaining and not unknown and cleanup_error is None and forced is False and closed and pid_removed and fixture.get('postmasterForced',False) is False
 cleanup={'verifiedStopped':clean,'closedPort':closed,'port':55437,'postmasterPidFileRemoved':pid_removed,'forced':forced,'postmasterForced':fixture.get('postmasterForced'),'remaining':remaining,'unconfirmedBirths':list(unknown.values()),'ownedBirths':len(known),'error':cleanup_error,'retainedRuntime':str(runtime) if runtime else None,'ownerGateReleased':released,'listenerGateReleased':listener_released}
 if not E.exists():E.mkdir(mode=0o700)
 (E/'cleanup.json').write_text(json.dumps(cleanup,sort_keys=True,indent=2)+'\n');report.update(success=bool(report.get('success') and clean),seconds=round(time.monotonic()-started,3),maximumOwnedRssBytes=peakrss,maximumFixtureAllocationBytes=peakfixture,maximumEvidenceAllocationBytes=peakoutput,minimumAvailableMemoryBytes=minmemory,minimumWorkspaceFreeBytes=minimum,minimumTmpFreeBytes=mintmp,cleanup=cleanup,cleanupSha256=hashlib.sha256((E/'cleanup.json').read_bytes()).hexdigest())
 for name in ['stdout.log','stderr.log','fixture-result.json']:
  if(E/name).is_file():report[name+'Sha256']=hashlib.sha256((E/name).read_bytes()).hexdigest()
 (D/'runtime-owner-result-03.json').write_text(json.dumps(report,sort_keys=True,indent=2)+'\n');print(json.dumps({k:report.get(k) for k in ['success','error','seconds','maximumOwnedRssBytes','maximumFixtureAllocationBytes','cleanup','tests']}))
sys.exit(0 if report.get('success') else 1)
