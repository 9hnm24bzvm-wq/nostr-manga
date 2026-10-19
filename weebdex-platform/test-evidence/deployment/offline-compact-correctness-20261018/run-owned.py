"""One bounded offline correctness process; retain outputs and signal exact births."""
import ctypes,hashlib,importlib.util,json,os,shutil,signal,subprocess,sys,time
from pathlib import Path
D=Path(__file__).resolve().parent;R=D.parent;G=R/'repository';PROJECT=G/'manga-protocol-lab';DEPS=R/'finite-fixture-worktree/.build/fixture-python-recovery/owned/site-packages';INVENTORY=R/'finite-fixture-worktree/weebdex-platform/test-evidence/frontend/finite-fixture-portability-20261018/installed-inventory.json';H=G/'weebdex-platform/tests/runtime/auth_fixture_guard.py'
spec=importlib.util.spec_from_file_location('owned_guard',H);h=importlib.util.module_from_spec(spec);spec.loader.exec_module(h)
assert ctypes.CDLL(None,use_errno=True).prctl(36,1,0,0,0)==0
os.umask(0o077);(D/'output').mkdir(mode=0o700,exist_ok=True)
CAP=1536*1024**2;MEMORY_RESERVE=2*1024**3;OUTPUT_CAP=64*1024**2;FLOOR=4*1024**3
# Conservative planning allowance, not an observed peak: two 511,202-record
# libraries, 512B/row, doubled for encoding/allocator overhead, plus128MiB runtime.
FORECAST=2*511202*512*2+128*1024**2
known={};unknown={};children=[];started=time.monotonic();cancelled=None;peakrss=0;peakallocation=0;minimum=shutil.disk_usage(R).free;minmemory=2**63
input_paths=sorted({str(q.relative_to(G)) for q in (PROJECT/'src').rglob('*.py')}|{'manga-protocol-lab/bench/library-node/test_compact.py','manga-protocol-lab/bench/library-node/run.py','manga-protocol-lab/requirements.txt','weebdex-platform/tests/runtime/auth_fixture_guard.py'})
pins={p:hashlib.sha256((G/p).read_bytes()).hexdigest() for p in input_paths}
report={'schema':'offline-compact-correctness-v1','sourceBase':'be283b664f699e8c7175f6eac0200cd22e212c8a','sourcePins':pins,'dependencyInventorySha256':hashlib.sha256(INVENTORY.read_bytes()).hexdigest(),'driverSha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),'launcherSha256':hashlib.sha256((D/'launch-correctness.py').read_bytes()).hexdigest(),'scope':'unchanged30k/500k in-memory correctness file, not benchmark/performance qualification','limits':{'seconds':120,'workSeconds':90,'cleanupPhaseBudgetSeconds':25,'rssBytes':CAP,'hostAvailableBytes':MEMORY_RESERVE,'outputsBytes':OUTPUT_CAP,'floorFreeBytes':FLOOR},'forecastBytes':FORECAST}
def installed():
 assert hashlib.sha256(INVENTORY.read_bytes()).hexdigest()=='7e7c5c88bc831111cc7583dbc2d6706ca133a9cff2d427dfb8b54f1085c2dcca','reviewed dependency inventory changed'
 rows=json.loads(INVENTORY.read_text());assert len(rows)==41
 paths=list(DEPS.rglob('*'));assert not DEPS.is_symlink() and all(not p.is_symlink() and (p.is_dir() or p.is_file()) for p in paths),'unexpected dependency link or special file'
 assert {str(p.relative_to(DEPS)) for p in paths if p.is_file()}=={row['path'] for row in rows},'dependency file membership changed'
 for row in rows:
  p=DEPS/row['path'];assert p.resolve().is_relative_to(DEPS) and p.is_file() and p.stat().st_size==row['bytes'] and hashlib.sha256(p.read_bytes()).hexdigest()==row['sha256']
 return len(rows)
def memory():return int(next(v.split()[1] for v in Path('/proc/meminfo').read_text().splitlines() if v.startswith('MemAvailable:')))*1024
def interrupt(sig,_):
 global cancelled
 cancelled=sig
signal.signal(signal.SIGINT,interrupt);signal.signal(signal.SIGTERM,interrupt)
def check():
 global peakrss,peakallocation,minimum,minmemory
 if cancelled:raise RuntimeError('owned run interrupted')
 live=h.members(known,unknown=unknown);rss=0
 for row in [*live,{'pid':os.getpid()}]:
  try:rss+=int((Path('/proc')/str(row['pid'])/'stat').read_text().rsplit(')',1)[1].split()[21])*os.sysconf('SC_PAGE_SIZE')
  except FileNotFoundError:pass
 peakrss=max(peakrss,rss);available=memory();minmemory=min(minmemory,available);free=shutil.disk_usage(R).free;minimum=min(minimum,free)
 allocation=sum(p.lstat().st_blocks*512 for p in [D,*D.rglob('*')]);peakallocation=max(peakallocation,allocation)
 if rss>CAP or available<MEMORY_RESERVE:raise RuntimeError('memory cap/reserve reached')
 if free<FLOOR+128*1024**2 or allocation>OUTPUT_CAP:raise RuntimeError('filesystem cap/reserve reached')
 if time.monotonic()-started>90:raise RuntimeError('correctness deadline reached')
def approved_source():
 base=report['sourceBase']
 actual=subprocess.run(['git','-C',str(G),'rev-parse','HEAD','HEAD^{tree}',base+'^{tree}'],check=True,capture_output=True,text=True).stdout.splitlines()
 report.update(actualSourceHead=actual[0],actualSourceTree=actual[1],approvedSourceTree=actual[2])
 assert actual[0]==base and actual[1]==actual[2],'checkout is not the approved immutable base'
 raw=subprocess.run(['git','-C',str(G),'cat-file','--batch'],input=''.join(base+':'+p+'\n' for p in input_paths).encode(),check=True,capture_output=True).stdout
 for name in input_paths:
  header,separator,raw=raw.partition(b'\n');assert separator
  fields=header.split();assert len(fields)==3 and fields[1]==b'blob','approved source file missing'
  size=int(fields[2]);body,raw=raw[:size],raw[size:];assert raw[:1]==b'\n';raw=raw[1:]
  assert hashlib.sha256(body).hexdigest()==pins[name],'working source differs from approved commit: '+name
 assert not raw
 report['approvedSourceVerified']=True
out=err=None;gate_read=gate_write=None;released=False
try:
 approved_source()
 assert FORECAST<CAP and memory()>=MEMORY_RESERVE+FORECAST,'memory forecast/reserve does not fit'
 assert minimum>=FLOOR+256*1024**2 and minimum-OUTPUT_CAP>=FLOOR+128*1024**2,'filesystem projection does not fit'
 report['installedFilesVerifiedBefore']=installed();check()
 env={k:v for k,v in os.environ.items() if not k.startswith('PG') and k not in ['PYTHONOPTIMIZE','PYTHONPATH']};env.update(PYTHONDONTWRITEBYTECODE='1',NODE_RESULTS=str(D/'output'),PYTHONPATH=os.pathsep.join([str(DEPS),str(PROJECT/'bench/library-node'),str(PROJECT)]))
 out=(D/'stdout.log').open('w');err=(D/'stderr.log').open('w')
 gate_read,gate_write=os.pipe();env['COMPACT_OWNED_GATE_FD']=str(gate_read)
 child=subprocess.Popen([sys.executable,'-B',str(D/'launch-correctness.py')],stdout=out,stderr=err,env=env,cwd=PROJECT,stdin=subprocess.DEVNULL,start_new_session=True,pass_fds=(gate_read,))
 children.append((child,None));os.close(gate_read);gate_read=None
 row=h.row(child.pid);assert row and row['ppid']==os.getpid() and row['group']==child.pid and row['session']==child.pid
 ident=h.identity(row);children[-1]=(child,ident);known[(child.pid,row['start_ticks'])]=ident
 check();os.write(gate_write,b'G');released=True;os.close(gate_write);gate_write=None
 while child.poll() is None:check();time.sleep(.05)
 check();out.flush();err.flush();report['returnCode']=child.returncode;assert child.returncode==0,'offline correctness process failed'
 data=json.loads((D/'stdout.log').read_text());assert [v['completed_chapters'] for v in data]==[30000,500000]
 assert all(v['identical_five_updates'] is True and 0<v['decoded_records_after']<v['decoded_records_before'] for v in data)
 report['results']=data;report['installedFilesVerifiedAfter']=installed()
 assert all(hashlib.sha256((G/p).read_bytes()).hexdigest()==digest for p,digest in pins.items()),'source changed'
 closure=json.loads((D/'import-closure.json').read_text());assert all(pins['manga-protocol-lab/'+p]==digest and hashlib.sha256((PROJECT/p).read_bytes()).hexdigest()==digest for p,digest in closure.items()),'imported source changed or was not pre-pinned'
 report.update(success=True,sourceImportClosureSha256=hashlib.sha256((D/'import-closure.json').read_bytes()).hexdigest(),dependencyInventoryUnchanged=hashlib.sha256(INVENTORY.read_bytes()).hexdigest()==report['dependencyInventorySha256'])
except Exception as e:report.update(success=False,errorClass=type(e).__name__,error=str(e))
finally:
 forced=None;remaining=[];cleanup_error=None
 if gate_read is not None:os.close(gate_read)
 if gate_write is not None:os.close(gate_write)
 # A child which was never admitted sees EOF on its private pipe and exits
 # before loading the test; this does not signal an unverified PID.
 if children and not released:
  try:children[-1][0].wait(timeout=5)
  except Exception as e:cleanup_error='unreleased child did not abort: '+type(e).__name__
 try:forced,remaining=h.stop(children,known=known,unknown=unknown,state={})
 except Exception as e:cleanup_error=type(e).__name__+': '+str(e)
 if out:out.close()
 if err:err.close()
 cleanup={'verifiedStopped':not remaining and not unknown and cleanup_error is None,'forced':forced,'remaining':remaining,'unconfirmedBirths':list(unknown.values()),'error':cleanup_error,'ownedBirths':len(known),'retainedOutputs':True,'correctnessGateReleased':released}
 (D/'cleanup.json').write_text(json.dumps(cleanup,sort_keys=True,indent=2)+'\n');report.update(seconds=round(time.monotonic()-started,3),maximumOwnedRssBytes=peakrss,maximumNamedAllocationBytes=peakallocation,minimumAvailableMemoryBytes=minmemory,minimumFreeBytes=minimum,cleanup=cleanup,cleanupSha256=hashlib.sha256((D/'cleanup.json').read_bytes()).hexdigest())
 for name in ['stdout.log','stderr.log','runtime.json']:
  if (D/name).is_file():report[name+'Sha256']=hashlib.sha256((D/name).read_bytes()).hexdigest()
 (D/'result.json').write_text(json.dumps(report,sort_keys=True,indent=2)+'\n');print(json.dumps({'success':report.get('success'),'error':report.get('error'),'seconds':report['seconds'],'maximumOwnedRssBytes':peakrss,'cleanup':cleanup,'results':report.get('results')}))
sys.exit(0 if report.get('success') and cleanup['verifiedStopped'] else 1)
