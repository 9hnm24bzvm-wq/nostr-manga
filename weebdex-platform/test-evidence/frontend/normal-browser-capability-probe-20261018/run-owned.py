import pathlib,subprocess,os,time,signal,json,re
E=pathlib.Path(__file__).parent;R=E.parent;floor=4*1024**3;reserve=128*1024**2;import shutil
assert shutil.disk_usage(R).free>=floor+2*reserve
argv=['/opt/codex/runtimes/codex-primary-runtime/dependencies/node/bin/node',str(E/'probe.mjs')];env={k:v for k,v in os.environ.items()if k in['HOME','LANG','LC_ALL','LC_CTYPE','TMPDIR','TMP','TEMP','PATH']};births={};signals=[];started=time.monotonic();minimum=shutil.disk_usage(R).free;peak=0;stop=None
with(E/'probe.log').open('xb')as out:
 child=subprocess.Popen(argv,stdout=out,stderr=subprocess.STDOUT,cwd=E,env=env,start_new_session=True)
 def scan():
  all={}
  for p in pathlib.Path('/proc').glob('[0-9]*/stat'):
   try:
    a=p.read_text().rsplit(')',1)[1].split();pid=int(p.parent.name)
    if a[0]!='Z':all[pid]={'parent':int(a[1]),'birth':int(a[19]),'rss':int(a[21])*os.sysconf('SC_PAGE_SIZE')}
   except(OSError,ValueError,IndexError):pass
  owned={pid for(pid,b)in births if pid in all and all[pid]['birth']==b}
  if child.pid in all:owned.add(child.pid)
  while True:
   n={pid for pid,x in all.items()if x['parent']in owned};old=len(owned);owned|=n
   if len(owned)==old:break
  current=[]
  for pid in owned:
   x=all[pid];key=(pid,x['birth']);current.append((key,x['rss']))
   if key not in births:
    try:
     fd=os.pidfd_open(pid);a=pathlib.Path('/proc',str(pid),'stat').read_text().rsplit(')',1)[1].split()
     if int(a[19])!=key[1]:os.close(fd);continue
     births[key]=fd
    except(OSError,ValueError,IndexError):pass
  return current
 def send(sig):
  scan()
  for key,fd in births.items():
   try:signal.pidfd_send_signal(fd,sig,None,0);signals.append({'birth':list(key),'signal':sig.name})
   except ProcessLookupError:pass
 while child.poll()is None:
  live=scan();minimum=min(minimum,shutil.disk_usage(R).free);peak=max(peak,sum(x[1]for x in live))
  if minimum<floor+reserve:stop='disk cleanup reserve'
  elif peak>2*1024**3:stop='owned RSS2GiB'
  elif time.monotonic()-started>40:stop='40s deadline'
  if stop:send(signal.SIGTERM);break
  time.sleep(.025)
 try:child.wait(timeout=5)
 except subprocess.TimeoutExpired:send(signal.SIGKILL);child.wait(timeout=5)
 live=scan()
 if live:send(signal.SIGTERM);time.sleep(.5);live=scan()
 if live:send(signal.SIGKILL);time.sleep(.5);live=scan()
 for fd in births.values():os.close(fd)
value={'argv':argv,'exitCode':child.returncode,'elapsedSeconds':round(time.monotonic()-started,3),'stopReason':stop,'minimumFreeBytes':minimum,'maximumSampledOwnedRssBytes':peak,'ownedBirths':[list(k)for k in births],'ownedSurvivors':[list(k)for k,_ in live],'cleanupSignals':signals,'cleanupVerified':not live,'descendantMethod':'PPID lineage plus exact process birth and pidfd retention; detached browser descendants included'};(E/'owned.json').write_text(json.dumps(value,indent=2)+'\n');print(json.dumps(value,indent=2));raise SystemExit(child.returncode or(1 if stop or live else 0))
