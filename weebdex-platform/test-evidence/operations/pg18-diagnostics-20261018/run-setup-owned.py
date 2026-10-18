"""Bounded official PG18 setup owner; retain outputs and signal exact births."""
import ctypes,hashlib,importlib.util,json,os,shutil,signal,subprocess,sys,time,urllib.parse,urllib.request
from pathlib import Path
D=Path(__file__).resolve().parent;R=D.parent;G=R/'pg18-diagnostics-worktree';PROJECT=G/'manga-protocol-lab';H=G/'weebdex-platform/tests/runtime/auth_fixture_guard.py'
assert hashlib.sha256(H.read_bytes()).hexdigest()=='17ff9c2eb04a4bdefaa3b61e975e77567f873208a83454afcdd6ca19c5cfc33e'
spec=importlib.util.spec_from_file_location('owned_guard',H);h=importlib.util.module_from_spec(spec);spec.loader.exec_module(h)
CLEANUP=D/'cleanup-owned.py';assert hashlib.sha256(CLEANUP.read_bytes()).hexdigest()=='9d8aaf970c724c29a02b7867e00a8a0f524a9021569e7c3b64306b14d610de1a'
spec=importlib.util.spec_from_file_location('exact_cleanup',CLEANUP);c=importlib.util.module_from_spec(spec);spec.loader.exec_module(c)
assert ctypes.CDLL(None,use_errno=True).prctl(36,1,0,0,0)==0
os.umask(0o077);(D/'output').mkdir(mode=0o700,exist_ok=True)
CAP=1024**3;MEMORY_RESERVE=2*1024**3;OUTPUT_CAP=128*1024**2;FLOOR=4*1024**3
# Conservative bound for package validation/decompression, not observed RSS.
FORECAST=768*1024**2
known={};unknown={};children=[];exited_fd_races=0;started=time.monotonic();cancelled=None;peakrss=0;peakallocation=0;minimum=shutil.disk_usage(R).free;minmemory=2**63
input_paths=sorted({str(q.relative_to(G)) for q in (PROJECT/'bench/library-node').glob('*.py')}|{str(q.relative_to(G)) for q in (PROJECT/'server/migrations').glob('*.sql')}|{'weebdex-platform/tests/runtime/auth_fixture_guard.py'})
pins={p:hashlib.sha256((G/p).read_bytes()).hexdigest() for p in input_paths}
report={'schema':'isolated-pg18-setup-owner-v1','sourceBase':'5cbd05d91c38e22eb6a7509aa6e9f8c21b12e3b4','sourcePins':pins,'driverSha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),'launcherSha256':hashlib.sha256((D/'setup.py').read_bytes()).hexdigest(),'scope':'four approved official packages extracted into a new private prefix; no server/global installation','limits':{'seconds':180,'workSeconds':150,'cleanupPhaseBudgetSeconds':30,'rssBytes':CAP,'hostAvailableBytes':MEMORY_RESERVE,'outputsBytes':OUTPUT_CAP,'floorFreeBytes':FLOOR},'forecastBytes':FORECAST}
def memory():return int(next(v.split()[1] for v in Path('/proc/meminfo').read_text().splitlines() if v.startswith('MemAvailable:')))*1024
def interrupt(sig,_):
 global cancelled
 cancelled=sig
signal.signal(signal.SIGINT,interrupt);signal.signal(signal.SIGTERM,interrupt)
def check():
 global peakrss,peakallocation,minimum,minmemory,exited_fd_races
 if cancelled:raise RuntimeError('owned run interrupted')
 live=h.members(known,unknown=unknown);rss=0
 for row in [*live,{'pid':os.getpid()}]:
  try:rss+=int((Path('/proc')/str(row['pid'])/'stat').read_text().rsplit(')',1)[1].split()[21])*os.sysconf('SC_PAGE_SIZE')
  except FileNotFoundError:pass
 sockets=set()
 for member in live:
  if member['state']=='Z':continue
  try:
   for entry in (Path('/proc')/str(member['pid'])/'fd').iterdir():
    try:
     value=os.readlink(entry)
     if value.startswith('socket:['):sockets.add(value[8:-1])
    except FileNotFoundError:pass
  except FileNotFoundError:pass
  except PermissionError:
   current=h.same(h.identity(member))
   if current is not None and current['state']!='Z':raise
   exited_fd_races+=1
 for table in ['tcp','tcp6']:
  for line in (Path('/proc/net')/table).read_text().splitlines()[1:]:
   values=line.split();assert not(values[3]=='0A' and values[9] in sockets),'setup child started a listener'
 peakrss=max(peakrss,rss);available=memory();minmemory=min(minmemory,available);free=shutil.disk_usage(R).free;minimum=min(minimum,free)
 allocation=sum(p.lstat().st_blocks*512 for p in [D,*D.rglob('*')]);peakallocation=max(peakallocation,allocation)
 if rss>CAP or available<MEMORY_RESERVE:raise RuntimeError('memory cap/reserve reached')
 if free<FLOOR+128*1024**2 or allocation>OUTPUT_CAP:raise RuntimeError('filesystem cap/reserve reached')
 if time.monotonic()-started>150:raise RuntimeError('setup deadline reached')
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
 assert hashlib.sha256((D/'proposal.json').read_bytes()).hexdigest()=='eeaa90c2d4c6987c32ac40d39ace8246ed84e14d38d7f1358e1a9495e9580353';check()
 env={k:v for k,v in os.environ.items() if not k.startswith('PG') and k not in ['PYTHONOPTIMIZE','PYTHONPATH','LD_PRELOAD','LD_LIBRARY_PATH']};env.update(PYTHONDONTWRITEBYTECODE='1')
 normal_proxy=urllib.request.getproxies().get('https');assert normal_proxy
 proxy=urllib.parse.urlsplit(normal_proxy);assert proxy.scheme=='http' and proxy.hostname=='127.0.0.1' and 1024<proxy.port<65536 and proxy.username is None and proxy.password is None and not proxy.query and not proxy.fragment and proxy.path in ('','/')
 env['PG18_EXPECTED_HTTPS_PROXY_SHA256']=hashlib.sha256(normal_proxy.encode()).hexdigest()
 report['normalTransport']={'configuredLoopbackHttpsProxy':True,'proxyBindingSha256':env['PG18_EXPECTED_HTTPS_PROXY_SHA256'],'tlsVerification':True,'redirects':False}
 out=(D/'stdout.log').open('w');err=(D/'stderr.log').open('w')
 gate_read,gate_write=os.pipe();env['OWNER_GATE_FD']=str(gate_read)
 child=subprocess.Popen([sys.executable,'-B',str(D/'setup.py')],stdout=out,stderr=err,env=env,cwd=PROJECT,stdin=subprocess.DEVNULL,start_new_session=True,pass_fds=(gate_read,))
 children.append((child,None));os.close(gate_read);gate_read=None
 row=h.row(child.pid);assert row and row['ppid']==os.getpid() and row['group']==child.pid and row['session']==child.pid
 ident=h.identity(row);children[-1]=(child,ident);known[(child.pid,row['start_ticks'])]=ident
 check();os.write(gate_write,b'G');released=True;os.close(gate_write);gate_write=None
 while child.poll() is None:check();time.sleep(.05)
 check();out.flush();err.flush();report['returnCode']=child.returncode;assert child.returncode==0,'setup process failed'
 data=json.loads((D/'setup-result.json').read_text());assert data['status']=='passed' and len(data['downloaded'])==4 and len(data['tools'])==7,'setup preflight failed'
 assert all(hashlib.sha256((G/p).read_bytes()).hexdigest()==digest for p,digest in pins.items()),'source changed'
 report.update(success=True,setupResultSha256=hashlib.sha256((D/'setup-result.json').read_bytes()).hexdigest(),installedInventorySha256=hashlib.sha256((D/'installed-inventory.json').read_bytes()).hexdigest(),downloadedPackages=4,noServerStarted=True)
except Exception as e:report.update(success=False,errorClass=type(e).__name__,error=str(e))
finally:
 forced=None;remaining=[];cleanup_error=None;stop_state={'forced':False}
 if gate_read is not None:os.close(gate_read)
 if gate_write is not None:os.close(gate_write)
 # A child which was never admitted sees EOF on its private pipe and exits
 # before loading the test; this does not signal an unverified PID.
 if children and not released:
  try:children[-1][0].wait(timeout=5)
  except Exception as e:cleanup_error='unreleased child did not abort: '+type(e).__name__
 forced,remaining,stop_error=c.cleanup(h,children,known,unknown,min(started+180,time.monotonic()+30))
 if stop_error:cleanup_error=stop_error
 if out:out.close()
 if err:err.close()
 cleanup={'verifiedStopped':not remaining and not unknown and cleanup_error is None and forced is False,'forced':forced,'remaining':remaining,'unconfirmedBirths':list(unknown.values()),'error':cleanup_error,'ownedBirths':len(known),'retainedOutputs':True,'correctnessGateReleased':released}
 (D/'cleanup.json').write_text(json.dumps(cleanup,sort_keys=True,indent=2)+'\n');report.update(exitedFdInspectionRaces=exited_fd_races,seconds=round(time.monotonic()-started,3),maximumOwnedRssBytes=peakrss,maximumNamedAllocationBytes=peakallocation,minimumAvailableMemoryBytes=minmemory,minimumFreeBytes=minimum,cleanup=cleanup,cleanupSha256=hashlib.sha256((D/'cleanup.json').read_bytes()).hexdigest())
 for name in ['stdout.log','stderr.log','setup-result.json']:
  if (D/name).is_file():report[name+'Sha256']=hashlib.sha256((D/name).read_bytes()).hexdigest()
 report['success']=bool(report.get('success') and cleanup['verifiedStopped'])
 (D/'owner-result.json').write_text(json.dumps(report,sort_keys=True,indent=2)+'\n');print(json.dumps({'success':report.get('success'),'error':report.get('error'),'seconds':report['seconds'],'maximumOwnedRssBytes':peakrss,'cleanup':cleanup,'downloadedPackages':report.get('downloadedPackages')}))
sys.exit(0 if report.get('success') and cleanup['verifiedStopped'] else 1)
