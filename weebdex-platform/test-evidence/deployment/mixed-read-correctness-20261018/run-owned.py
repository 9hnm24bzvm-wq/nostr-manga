"""One bounded offline correctness process; retain outputs and signal exact births."""
import ctypes,hashlib,importlib.util,json,os,shutil,signal,subprocess,sys,time
from pathlib import Path
D=Path(__file__).resolve().parent;R=D.parent;G=R/'post-nginx-integration-worktree';PROJECT=G/'manga-protocol-lab';DEPS=R/'finite-fixture-worktree/.build/fixture-python-recovery/owned/site-packages';INVENTORY=R/'finite-fixture-worktree/weebdex-platform/test-evidence/frontend/finite-fixture-portability-20261018/installed-inventory.json';H=G/'weebdex-platform/tests/runtime/auth_fixture_guard.py'
assert hashlib.sha256(H.read_bytes()).hexdigest()=='17ff9c2eb04a4bdefaa3b61e975e77567f873208a83454afcdd6ca19c5cfc33e','reviewed ownership helper changed'
spec=importlib.util.spec_from_file_location('owned_guard',H);h=importlib.util.module_from_spec(spec);spec.loader.exec_module(h)
assert ctypes.CDLL(None,use_errno=True).prctl(36,1,0,0,0)==0
os.umask(0o077);(D/'output').mkdir(mode=0o700,exist_ok=True)
CAP=1536*1024**2;MEMORY_RESERVE=2*1024**3;OUTPUT_CAP=64*1024**2;FLOOR=4*1024**3
# Conservative planning allowance, not an observed peak: two 511,202-record
# libraries, 512B/row, doubled for encoding/allocator overhead, plus128MiB runtime.
# This test holds only one30k and one500k library; this larger two500k estimate
# also leaves headroom for selective HTTP/base64 response buffers.
FORECAST=2*511202*512*2+128*1024**2
ports=set();known={};unknown={};children=[];started=time.monotonic();cancelled=None;peakrss=0;peakallocation=0;minimum=shutil.disk_usage(R).free;minmemory=2**63
input_paths=sorted({str(q.relative_to(G)) for q in (PROJECT/'src').rglob('*.py')}|{'manga-protocol-lab/bench/library-node/test_mixed_reads.py','manga-protocol-lab/bench/library-node/mixed.py','manga-protocol-lab/bench/library-node/mixed_metrics.py','manga-protocol-lab/bench/library-node/process_metrics.py','manga-protocol-lab/bench/library-node/run.py','manga-protocol-lab/requirements.txt','weebdex-platform/tests/runtime/auth_fixture_guard.py'})
pins={p:hashlib.sha256((G/p).read_bytes()).hexdigest() for p in input_paths}
report={'schema':'bounded-mixed-read-correctness-v1','sourceBase':'6c5000081092134facb8713ccea202fc37edd7b5','sourcePins':pins,'dependencyInventorySha256':hashlib.sha256(INVENTORY.read_bytes()).hexdigest(),'driverSha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),'launcherSha256':hashlib.sha256((D/'launch-correctness.py').read_bytes()).hexdigest(),'scope':'one unchanged mixed read method over controlled loopback HTTP; no application capacity or recovery qualification','limits':{'seconds':120,'workSeconds':90,'cleanupPhaseBudgetSeconds':25,'rssBytes':CAP,'hostAvailableBytes':MEMORY_RESERVE,'outputsBytes':OUTPUT_CAP,'floorFreeBytes':FLOOR},'forecastBytes':FORECAST}
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
def observe_listeners(live):
 sockets=set()
 for member in live:
  try:
   for entry in (Path('/proc')/str(member['pid'])/'fd').iterdir():
    try:
     target=os.readlink(entry)
     if target.startswith('socket:['):sockets.add(target[8:-1])
    except FileNotFoundError:pass
  except FileNotFoundError:pass
 for table in ['tcp','tcp6']:
  for line in (Path('/proc/net')/table).read_text().splitlines()[1:]:
   fields=line.split()
   if fields[3]!='0A' or fields[9] not in sockets:continue
   address,port=fields[1].split(':');port=int(port,16)
   assert table=='tcp' and address=='0100007F' and 1024<port<65536,'non-private owned listener'
   ports.add(port);assert len(ports)==1,'more than one listener'
def admit_listener():
 global listener_write,listener_admitted
 ready=D/'first-request-listener.json'
 if ready.is_file() and not listener_admitted:
  value=json.loads(ready.read_text());assert value['host']=='127.0.0.1' and 1024<value['port']<65536
  assert len(children)==1 and children[0][1] is not None
  assert h.owns_listener(children[0][1],value['port']),'first-request listener is not owned by the verified birth'
  ports.add(value['port']);assert len(ports)==1
  report.update(listenerOwnedBeforeFirstRequest=True,listenerProof={'birth':children[0][1],'host':value['host'],'port':value['port'],'descriptor':value['descriptor'],'readySha256':hashlib.sha256(ready.read_bytes()).hexdigest()})
  os.write(listener_write,b'L');listener_admitted=True;os.close(listener_write);listener_write=None
def check():
 global peakrss,peakallocation,minimum,minmemory
 if cancelled:raise RuntimeError('owned run interrupted')
 live=h.members(known,unknown=unknown);observe_listeners(live);rss=0
 for row in [*live,{'pid':os.getpid()}]:
  try:rss+=int((Path('/proc')/str(row['pid'])/'stat').read_text().rsplit(')',1)[1].split()[21])*os.sysconf('SC_PAGE_SIZE')
  except FileNotFoundError:pass
 peakrss=max(peakrss,rss);available=memory();minmemory=min(minmemory,available);free=shutil.disk_usage(R).free;minimum=min(minimum,free)
 allocation=sum(p.lstat().st_blocks*512 for p in [D,*D.rglob('*')]);peakallocation=max(peakallocation,allocation)
 if rss>CAP or available<MEMORY_RESERVE:raise RuntimeError('memory cap/reserve reached')
 if free<FLOOR+128*1024**2 or allocation>OUTPUT_CAP:raise RuntimeError('filesystem cap/reserve reached')
 if time.monotonic()-started>90:raise RuntimeError('correctness deadline reached')
 admit_listener()
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
out=err=None;gate_read=gate_write=None;listener_read=listener_write=None;released=False;listener_admitted=False
try:
 approved_source()
 assert FORECAST<CAP and memory()>=MEMORY_RESERVE+FORECAST,'memory forecast/reserve does not fit'
 assert minimum>=FLOOR+256*1024**2 and minimum-OUTPUT_CAP>=FLOOR+128*1024**2,'filesystem projection does not fit'
 report['installedFilesVerifiedBefore']=installed();check()
 env={k:v for k,v in os.environ.items() if not k.startswith('PG') and k not in ['PYTHONOPTIMIZE','PYTHONPATH']};env.update(PYTHONDONTWRITEBYTECODE='1',NODE_RESULTS=str(D/'output'),PYTHONPATH=os.pathsep.join([str(DEPS),str(PROJECT/'bench/library-node'),str(PROJECT)]))
 out=(D/'stdout.log').open('w');err=(D/'stderr.log').open('w')
 gate_read,gate_write=os.pipe();listener_read,listener_write=os.pipe();env['COMPACT_OWNED_GATE_FD']=str(gate_read);env['MIXED_LISTENER_GATE_FD']=str(listener_read)
 child=subprocess.Popen([sys.executable,'-B',str(D/'launch-correctness.py')],stdout=out,stderr=err,env=env,cwd=PROJECT,stdin=subprocess.DEVNULL,start_new_session=True,pass_fds=(gate_read,listener_read))
 children.append((child,None));os.close(gate_read);os.close(listener_read);gate_read=listener_read=None
 row=h.row(child.pid);assert row and row['ppid']==os.getpid() and row['group']==child.pid and row['session']==child.pid
 ident=h.identity(row);children[-1]=(child,ident);known[(child.pid,row['start_ticks'])]=ident
 check();os.write(gate_write,b'G');released=True;os.close(gate_write);gate_write=None
 while child.poll() is None:check();time.sleep(.05)
 check();out.flush();err.flush();report['returnCode']=child.returncode;assert child.returncode==0,'controlled correctness process failed';assert listener_admitted,'first-request listener was never admitted'
 data=json.loads((D/'case-result.json').read_text());assert data=={'testsRun':1,'failures':0,'errors':0,'skipped':0,'success':True},'one-method result differs'
 http=json.loads((D/'http-observations.json').read_text());assert len(http)==9 and all(row['status']==200 for row in http),'HTTP observation mismatch'
 assert [row['method'] for row in http]==['GET','GET','POST']*3
 report.update(results=data,httpRequestCount=len(http),httpObservationSha256=hashlib.sha256((D/'http-observations.json').read_bytes()).hexdigest());report['installedFilesVerifiedAfter']=installed()
 assert all(hashlib.sha256((G/p).read_bytes()).hexdigest()==digest for p,digest in pins.items()),'source changed'
 closure=json.loads((D/'import-closure.json').read_text());assert all(pins['manga-protocol-lab/'+p]==digest and hashlib.sha256((PROJECT/p).read_bytes()).hexdigest()==digest for p,digest in closure.items()),'imported source changed or was not pre-pinned'
 report.update(success=True,sourceImportClosureSha256=hashlib.sha256((D/'import-closure.json').read_bytes()).hexdigest(),dependencyInventoryUnchanged=hashlib.sha256(INVENTORY.read_bytes()).hexdigest()==report['dependencyInventorySha256'])
except Exception as e:report.update(success=False,errorClass=type(e).__name__,error=str(e))
finally:
 forced=None;remaining=[];cleanup_error=None
 if gate_read is not None:os.close(gate_read)
 if gate_write is not None:os.close(gate_write)
 if listener_read is not None:os.close(listener_read)
 if listener_write is not None:os.close(listener_write)
 # A child which was never admitted sees EOF on its private pipe and exits
 # before loading the test; this does not signal an unverified PID.
 if children and not released:
  try:children[-1][0].wait(timeout=5)
  except Exception as e:cleanup_error='unreleased child did not abort: '+type(e).__name__
 try:forced,remaining=h.stop(children,known=known,unknown=unknown,state={})
 except Exception as e:cleanup_error=type(e).__name__+': '+str(e)
 if out:out.close()
 if err:err.close()
 port_checks=[{'port':port,'closed':h.closed(port)} for port in sorted(ports)]
 cleanup={'verifiedStopped':not remaining and not unknown and cleanup_error is None and len(ports)==1 and all(value['closed'] for value in port_checks),'ports':port_checks,'forced':forced,'remaining':remaining,'unconfirmedBirths':list(unknown.values()),'error':cleanup_error,'ownedBirths':len(known),'retainedOutputs':True,'correctnessGateReleased':released,'listenerGateReleased':listener_admitted}
 (D/'cleanup.json').write_text(json.dumps(cleanup,sort_keys=True,indent=2)+'\n');report.update(seconds=round(time.monotonic()-started,3),maximumOwnedRssBytes=peakrss,maximumNamedAllocationBytes=peakallocation,minimumAvailableMemoryBytes=minmemory,minimumFreeBytes=minimum,cleanup=cleanup,cleanupSha256=hashlib.sha256((D/'cleanup.json').read_bytes()).hexdigest())
 for name in ['stdout.log','stderr.log','runtime.json','case-result.json']:
  if (D/name).is_file():report[name+'Sha256']=hashlib.sha256((D/name).read_bytes()).hexdigest()
 (D/'result.json').write_text(json.dumps(report,sort_keys=True,indent=2)+'\n');print(json.dumps({'success':report.get('success'),'error':report.get('error'),'seconds':report['seconds'],'maximumOwnedRssBytes':peakrss,'cleanup':cleanup,'results':report.get('results')}))
sys.exit(0 if report.get('success') and cleanup['verifiedStopped'] else 1)
