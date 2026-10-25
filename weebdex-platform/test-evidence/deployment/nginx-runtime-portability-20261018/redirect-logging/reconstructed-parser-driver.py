#!/usr/bin/env python3
"""Newly reconstructed driver after workspace reset; approved 64MiB/420s scope."""
import ctypes,hashlib,importlib.util,json,os,shutil,signal,socket,subprocess,sys,time
from pathlib import Path
D=Path(__file__).resolve().parent;R=D.parent;SETUP=R/'nginx-qualification-20261018T1329';G=R/'nginx-runtime-fix-worktree';H=G/'weebdex-platform/tests/runtime/auth_fixture_guard.py'
FLOOR=4*1024**3;START=FLOOR+256*1024**2;LIVE=FLOOR+128*1024**2;CAP=64*1024**2
URL='https://nginx.org/packages/debian/pool/nginx/n/nginx/nginx_1.30.6-1~trixie_amd64.deb';SHA='8dc9a5da6d62b33aaef20c047760a82c7b18c90100251290ef5f553b9de3bc3c';SIZE=1087236
spec=importlib.util.spec_from_file_location('owned_guard',H);h=importlib.util.module_from_spec(spec);spec.loader.exec_module(h)
assert os.geteuid()!=0 and ctypes.CDLL(None,use_errno=True).prctl(36,1,0,0,0)==0
os.umask(0o077);(D/'runtime').mkdir(mode=0o700,exist_ok=True)
children=[];known={};unknown={};handles=[];ports=set();steps=[];cancelled=None;began=time.monotonic();minimum=shutil.disk_usage(R).free;peakbytes=0;peakrss=0
paths=['manga-protocol-lab/tests/test_serving_supervisor.py','manga-protocol-lab/tests/operator_support.py','manga-protocol-lab/src/serving_supervisor.py','weebdex-platform/deployment/nginx.example.conf','weebdex-platform/tests/runtime/auth_fixture_guard.py']
pins={p:hashlib.sha256((G/p).read_bytes()).hexdigest() for p in paths}
prior_seconds=sum(json.loads((p/'result.json').read_text()).get('seconds',0) for p in R.glob('nginx-qualification-20261018T13*') if p!=D and (p/'result.json').is_file());remaining_seconds=420-prior_seconds
report={'priorGuardedExecutionSeconds':round(prior_seconds,3),'remainingExecutionSeconds':round(remaining_seconds,3),'schema':'official-nginx-isolated-qualification-v2','provenance':'Fresh reconstructed driver after old runtime disappeared; this is a new run on verified restored source','sourceCommit':'4351752c3188aea9199383aa27b865e193f58ea7','sourceTree':'dc920d3d28eb491ceb693e5b83e074f77182d0db','package':{'url':URL,'sha256':SHA,'bytes':SIZE},'sourcePins':pins,'driverSha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),'testLauncherSha256':hashlib.sha256((D/'run-eight.py').read_bytes()).hexdigest(),'steps':steps,'startFreeBytes':minimum,'limits':{'newAllocatedBytes':CAP,'startFreeBytes':START,'liveFreeBytes':LIVE,'seconds':420,'rssBytes':512*1024**2,'hostAvailableBytes':1024**3},'oldSyntheticDatabaseCleanup':{'deletionPerformed':False,'candidateAndManifestAbsentAfterWorkspaceReset':True}}
def interrupt(sig,_):
 global cancelled
 cancelled=sig
signal.signal(signal.SIGINT,interrupt);signal.signal(signal.SIGTERM,interrupt)
def allocated():
 seen=set();n=0
 for p in [p for root in sorted(R.glob('nginx-qualification-20261018T13*')) for p in [root,*root.rglob('*')]]:
  try:st=p.lstat()
  except FileNotFoundError:continue
  k=(st.st_dev,st.st_ino)
  if k not in seen:n+=st.st_blocks*512;seen.add(k)
 counter=D/'unlinked-block-accounting.json'
 if counter.is_file():n+=json.loads(counter.read_text())['maximumAllocatedBeforeUnlinkBytes']
 return n
def observe_ports(live):
 path=D/'test-loopback-ports.json'
 if path.is_file():
  for value in json.loads(path.read_text()):
   if value['bind']!='127.0.0.1' or not 1024<int(value['port'])<65536:raise RuntimeError('unexpected test listener target')
   ports.add(int(value['port']))
def check():
 global minimum,peakbytes,peakrss
 if cancelled:raise RuntimeError('owned qualification interrupted')
 live=h.members(known,unknown=unknown);observe_ports(live);free=shutil.disk_usage(R).free;minimum=min(minimum,free);size=allocated();peakbytes=max(peakbytes,size);rss=0
 for row in live:
  try:rss+=int((Path('/proc')/str(row['pid'])/'stat').read_text().rsplit(')',1)[1].split()[21])*os.sysconf('SC_PAGE_SIZE')
  except FileNotFoundError:pass
 peakrss=max(peakrss,rss);available=int(next(v.split()[1] for v in Path('/proc/meminfo').read_text().splitlines() if v.startswith('MemAvailable:')))*1024
 if free<LIVE or size>CAP:raise RuntimeError('disk reserve or64MiB allocation cap reached')
 if rss>512*1024**2 or available<1024**3:raise RuntimeError('memory cap reached')
 if time.monotonic()-began>remaining_seconds:raise RuntimeError('overall deadline reached')
env={k:v for k,v in os.environ.items() if not k.startswith('PG')};env.update(TMPDIR=str(D/'runtime'),PYTHONDONTWRITEBYTECODE='1',LC_ALL='C',LANG='C')
def run(label,args,timeout,expected=0):
 start=time.monotonic();f=(D/(label+'.log')).open('w');handles.append(f)
 p=subprocess.Popen(args,stdout=f,stderr=subprocess.STDOUT,stdin=subprocess.DEVNULL,env=env,start_new_session=True)
 row=h.row(p.pid)
 if not row or row['ppid']!=os.getpid() or row['group']!=p.pid or row['session']!=p.pid:raise RuntimeError('owned birth unavailable')
 ident=h.identity(row);children.append((p,ident));known[(p.pid,row['start_ticks'])]=ident
 while p.poll() is None:
  check()
  if time.monotonic()-start>timeout:raise RuntimeError('phase deadline at '+label)
  time.sleep(.05)
 check();f.flush();data=(D/(label+'.log')).read_bytes();steps.append({'name':label,'exit':p.returncode,'seconds':round(time.monotonic()-start,3),'logSha256':hashlib.sha256(data).hexdigest()})
 if (expected=='nonzero' and p.returncode==0) or (expected!='nonzero' and p.returncode!=expected):raise RuntimeError('unexpected command exit at '+label)
 return data.decode(errors='replace')
try:
 assert minimum>=START and minimum-CAP>=LIVE,'insufficient projected start reserve'
 fetch="import hashlib,pathlib,urllib.request;u="+repr(URL)+";p=pathlib.Path("+repr(str(D/'nginx.deb'))+");r=urllib.request.urlopen(u,timeout=30);b=r.read("+str(SIZE+1)+");assert len(b)=="+str(SIZE)+" and hashlib.sha256(b).hexdigest()=="+repr(SHA)+";p.write_bytes(b)"
 assert (SETUP/'nginx.deb').stat().st_size==SIZE and hashlib.sha256((SETUP/'nginx.deb').read_bytes()).hexdigest()==SHA
 report['reusesVerifiedSetupFrom']='nginx-qualification-20261018T1329; initial parser failure preserved'
 binary=SETUP/'official-package/usr/sbin/nginx';assert binary.is_file() and not binary.is_symlink() and binary.read_bytes()[:4]==b'\x7fELF'
 version=run('actual-nginx-version',[str(binary),'-V'],10);assert 'nginx/1.30.6' in version
 libs=run('actual-nginx-dynamic-libraries',['ldd',str(binary)],10);assert 'not found' not in libs
 report.update(actualNginxBinarySha256=hashlib.sha256(binary.read_bytes()).hexdigest(),actualVersion='1.30.6')
 parser=D/'parser';parser.mkdir(mode=0o700)
 run('synthetic-parser-certificate',['openssl','req','-x509','-newkey','rsa:2048','-nodes','-keyout',str(parser/'synthetic.key'),'-out',str(parser/'synthetic.crt'),'-days','1','-subj','/CN=synthetic.invalid'],15)
 source=(G/'weebdex-platform/deployment/nginx.example.conf').read_text();patched=source
 for old in ['/etc/ssl/weebdex/fullchain.pem','/etc/ssl/protocol/fullchain.pem']:patched=patched.replace(old,str(parser/'synthetic.crt'))
 for old in ['/etc/ssl/weebdex/private.key','/etc/ssl/protocol/private.key']:patched=patched.replace(old,str(parser/'synthetic.key'))
 for i,old in enumerate(['/var/log/nginx/weebdex.log','/var/log/nginx/weebdex-protocol.log']):patched=patched.replace(old,str(parser/('access-'+str(i)+'.log')))
 held=[];private_ports=[]
 for _ in range(2):
  reservation=socket.socket();reservation.bind(('127.0.0.1',0));held.append(reservation);private_ports.append(reservation.getsockname()[1])
 assert len(set(private_ports))==2 and all(p>1024 for p in private_ports)
 patched=patched.replace('listen 80;','listen 127.0.0.1:'+str(private_ports[0])+';').replace('listen 443 ssl;','listen 127.0.0.1:'+str(private_ports[1])+' ssl;')
 ports.update(private_ports)
 for reservation in held:reservation.close()
 (parser/'example.conf').write_text(patched);prefix='pid '+str(parser/'nginx.pid')+';\nerror_log '+str(parser/'error.log')+';\nevents {}\nhttp {\naccess_log '+str(parser/'inherited-default-access.log')+' combined;\n'
 for directive in ['client_body_temp_path','proxy_temp_path','fastcgi_temp_path','uwsgi_temp_path','scgi_temp_path']:prefix+=directive+' '+str(parser/directive)+';\n'
 wrapper=prefix+'include '+str(parser/'example.conf')+';\n}\n';(parser/'wrapper.conf').write_text(wrapper)
 run('official-nginx-parser',[str(binary),'-t','-e',str(parser/'startup-error.log'),'-p',str(parser)+'/', '-c',str(parser/'wrapper.conf')],15)
 negative=patched.replace('location ~ "^/originals/[a-f0-9]{64}$" {','location ~ ^/originals/[a-f0-9]{64}$ {',1);assert negative!=patched
 (parser/'negative-example.conf').write_text(negative);(parser/'negative-wrapper.conf').write_text(wrapper.replace(str(parser/'example.conf'),str(parser/'negative-example.conf')))
 run('official-nginx-rejects-unquoted-braces',[str(binary),'-t','-e',str(parser/'startup-error.log'),'-p',str(parser)+'/', '-c',str(parser/'negative-wrapper.conf')],15,expected='nonzero')
 report['parser']={'success':True,'negativeUnquotedBracesRefused':True,'sourceSha256':pins['weebdex-platform/deployment/nginx.example.conf'],'patchedCopySha256':hashlib.sha256(patched.encode()).hexdigest(),'wrapperSha256':hashlib.sha256(wrapper.encode()).hexdigest(),'sourceEdits':'four TLS certificate/key paths, two access-log paths and three listen directives mapped to unprivileged numeric loopback ports','wrapperAdditions':'private prefix/pid/startup-error log/temp directories, explicit inherited combined-format access log in private file, events/http contexts','noServeOrReload':True,'nginxTestInitializesSockets':True,'privateUnprivilegedLoopbackListeners':True}
 report['scope']='Redirect privacy follow-up: mapped parser plus negative only; no lifecycle methods rerun'
 assert all(hashlib.sha256((G/p).read_bytes()).hexdigest()==v for p,v in pins.items()),'source drift'
 report.update(success=True,sourceUnchanged=True,allocationScope='All named files in every retained attempt plus conservative separately measured blocks of the deliberately unlinked original')
except Exception as e:
 report.update(success=False,errorClass=type(e).__name__,error=str(e))
 if (D/'eight-result.json').is_file():report['eightCases']=json.loads((D/'eight-result.json').read_text())
finally:
 cleanup_error=None;forced=None;remaining=[]
 try:forced,remaining=h.stop(children,known=known,unknown=unknown,state={})
 except Exception as e:cleanup_error=type(e).__name__+': '+str(e)
 for f in handles:f.close()
 closed=all(h.closed(p) for p in ports);cleanup={'verifiedStopped':cleanup_error is None and not remaining and not unknown and closed,'allObservedPortsClosed':closed,'recordedLoopbackPortCount':len(ports),'socketFdInventory':'not available for some owned children; exact source/argv port records and closed-port proofs used','forced':forced,'remaining':remaining,'unconfirmedBirths':list(unknown.values()),'error':cleanup_error,'ownedBirthCount':len(known),'sourcePackageLogsAndEvidenceRetained':True}
 (D/'cleanup.json').write_text(json.dumps(cleanup,sort_keys=True,indent=2)+'\n');report.update(physicalCleanup=cleanup,seconds=round(time.monotonic()-began,3),cumulativeGuardedExecutionSeconds=round(prior_seconds+time.monotonic()-began,3),minimumActiveFreeBytes=minimum,endFreeBytes=shutil.disk_usage(R).free,maximumAllocatedBytes=peakbytes,maximumOwnedRssBytes=peakrss,cleanupSha256=hashlib.sha256((D/'cleanup.json').read_bytes()).hexdigest())
 (D/'result.json').write_text(json.dumps(report,sort_keys=True,indent=2)+'\n');print(json.dumps({'success':report.get('success'),'seconds':report['seconds'],'error':report.get('error'),'parser':report.get('parser'),'eightCases':report.get('eightCases'),'physicalCleanup':cleanup,'minimumActiveFreeBytes':minimum,'maximumAllocatedBytes':peakbytes}))
sys.exit(0 if report.get('success') and cleanup['verifiedStopped'] else 1)
