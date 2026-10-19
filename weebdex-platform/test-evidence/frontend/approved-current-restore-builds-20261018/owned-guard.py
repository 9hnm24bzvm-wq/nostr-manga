#!/usr/bin/env python3
"""Bounded owned session; immediate stopped-leader registration, pidfd signals only."""
import argparse,json,os,pathlib,re,shutil,signal,subprocess,sys,time
GIB=1024**3; RESERVE=128*1024**2

def process(pid):
 try:
  fields=pathlib.Path(f'/proc/{pid}/stat').read_text().rsplit(')',1)[1].split()
  return {'pid':pid,'state':fields[0],'ppid':int(fields[1]),'group':int(fields[2]),'session':int(fields[3]),'birth':int(fields[19]),'rss':int(fields[21])*os.sysconf('SC_PAGE_SIZE')}
 except (OSError,ValueError,IndexError):return None

def allocated(roots):
 seen=set(); total=0
 for root in roots:
  if not root.exists():continue
  for parent,dirs,files in os.walk(root,followlinks=False):
   for name in dirs+files:
    p=pathlib.Path(parent)/name
    try:s=p.lstat()
    except FileNotFoundError:continue
    key=(s.st_dev,s.st_ino)
    if key not in seen:seen.add(key);total+=s.st_blocks*512
 return total

def run(argv,cwd,out,seconds,growth_roots=(),growth_baseline=0):
 out=pathlib.Path(out);out.parent.mkdir(parents=True,exist_ok=True)
 assert not out.exists() and not out.with_suffix('.log').exists(),'Refuse existing receipt or log'
 minimum=shutil.disk_usage(cwd).free;assert minimum>=4*GIB+2*RESERVE,'Insufficient start reserve'
 births={};signals=[];stop=None;guard_error=False;census_errors=0;max_rss=0;min_mem=10**30;peak_growth=0;started=time.monotonic()
 assert seconds>=15,'Total deadline must include12s cleanup reserve'
 def available_memory():return int(re.search(r'^MemAvailable:\s+(\d+)',pathlib.Path('/proc/meminfo').read_text(),re.M)[1])*1024
 assert available_memory()>=GIB,'Insufficient memory launch reserve'
 def register(row):
  key=(row['pid'],row['birth'])
  if key in births:return
  try:
   fd=os.pidfd_open(row['pid'],0);again=process(row['pid'])
   if not again or again['birth']!=row['birth']:os.close(fd);return
   births[key]=fd
  except ProcessLookupError:pass
 def scan():
  rows=[r for p in pathlib.Path('/proc').glob('[0-9]*/stat')if(r:=process(int(p.parent.name)))and r['state']!='Z']
  owned={r['pid']:r for r in rows if r['session']==child.pid or (r['pid'],r['birth'])in births}
  changed=True
  while changed:
   changed=False
   for row in rows:
    if row['pid']not in owned and row['ppid']in owned:owned[row['pid']]=row;changed=True
  for row in owned.values():register(row)
  return list(owned.values())
 def safe_scan():
  nonlocal census_errors
  try:return scan()
  except Exception:census_errors+=1;return None
 def send(sig):
  safe_scan()
  for key,fd in list(births.items()):
   try:signal.pidfd_send_signal(fd,sig,None,0);signals.append({'pid':key[0],'birth':key[1],'signal':sig.name})
   except ProcessLookupError:pass
 launcher='import os,signal,sys; os.kill(os.getpid(),signal.SIGSTOP); os.execvpe(sys.argv[1],sys.argv[1:],os.environ)'
 with out.with_suffix('.log').open('xb')as log:
  child=subprocess.Popen([sys.executable,'-c',launcher,*argv],cwd=cwd,env=os.environ.copy(),stdout=log,stderr=subprocess.STDOUT,start_new_session=True)
  try:
   deadline=time.monotonic()+5
   while True:
    row=process(child.pid)
    if row and row['state']=='T':break
    if child.poll()is not None or time.monotonic()>deadline:raise RuntimeError('Stopped leader handshake failed')
    time.sleep(.005)
   assert row['session']==child.pid and row['group']==child.pid
   register(row);assert(child.pid,row['birth'])in births
   assert shutil.disk_usage(cwd).free>=4*GIB+2*RESERVE and available_memory()>=GIB,'Launch reserve changed'
   signal.pidfd_send_signal(births[(child.pid,row['birth'])],signal.SIGCONT,None,0)
   while True:
    current=safe_scan();free=shutil.disk_usage(cwd).free;minimum=min(minimum,free);rss=sum(r['rss']for r in (current or []));max_rss=max(max_rss,rss)
    if current is None:stop='census-failure'
    available=available_memory();min_mem=min(min_mem,available)
    growth=max(0,allocated(growth_roots)-growth_baseline)if growth_roots else 0;peak_growth=max(peak_growth,growth)
    if free<4*GIB+RESERVE:stop='disk-reserve'
    elif rss>4*GIB:stop='owned-rss-cap'
    elif available<GIB:stop='available-memory-reserve'
    elif growth>4*GIB:stop='owned-dependency-growth-cap'
    elif time.monotonic()-started>seconds-12:stop='deadline-work-budget'
    if stop:send(signal.SIGTERM);break
    if child.poll()is not None:break
    time.sleep(.1)
  except BaseException:
   guard_error=True;stop=stop or 'guard-internal-failure'
  finally:
   # Cleanup is identical for success, resource stop and an exception. Census
   # failures never suppress signals to already birth-verified pidfds.
   survivors=safe_scan()
   if child.poll()is None or survivors is None or survivors:send(signal.SIGTERM)
   try:child.wait(timeout=3)
   except subprocess.TimeoutExpired:send(signal.SIGKILL)
   time.sleep(.1);survivors=safe_scan()
   if child.poll()is None or survivors is None or survivors:send(signal.SIGKILL)
   try:child.wait(timeout=3)
   except subprocess.TimeoutExpired:stop=stop or 'leader-survived-cleanup'
   time.sleep(.1);survivors=safe_scan()
   cleanup_verified=survivors==[] and child.poll()is not None and census_errors==0
   # Persist terminal facts while ownership handles are still held.
   terminal={'cleanup_verified':cleanup_verified,'surviving_owned_processes':survivors,'census_errors':census_errors,'process_exit_code':child.returncode,'signals':signals}
   out.with_suffix('.terminal.json').write_text(json.dumps(terminal,indent=2)+'\n')
   for fd in births.values():os.close(fd)
 value={'command':argv,'cwd':str(cwd),'deadline_seconds':seconds,'elapsed_seconds':round(time.monotonic()-started,3),'exit_code':child.returncode,'stop_reason':stop,'minimum_free_bytes':minimum,'minimum_available_memory_bytes':min_mem,'maximum_owned_rss_bytes':max_rss,'peak_new_dependency_allocated_bytes':peak_growth,'floor_bytes':4*GIB,'cleanup_reserve_bytes':RESERVE,'dependency_growth_cap_bytes':4*GIB,'owned_process_births':[{'pid':p,'start_time':b}for p,b in births],'cleanup_signals':signals,'surviving_owned_processes':survivors,'cleanup_verified':cleanup_verified,'census_errors':census_errors,'guard_internal_failure':guard_error,'cleanup_reserve_seconds':12,'leader_registered_before_exec':True,'identity':'exact birth recheck plus pidfd-owned session and sampled descendants'}
 out.write_text(json.dumps(value,indent=2)+'\n');print(json.dumps(value),flush=True)
 return 0 if child.returncode==0 and stop is None and cleanup_verified and not guard_error else 1
if __name__=='__main__':
 p=argparse.ArgumentParser();p.add_argument('--cwd',type=pathlib.Path,required=True);p.add_argument('--receipt',type=pathlib.Path,required=True);p.add_argument('--seconds',type=int,required=True);p.add_argument('--growth-root',type=pathlib.Path,action='append',default=[]);p.add_argument('command',nargs=argparse.REMAINDER);a=p.parse_args();command=a.command[1:]if a.command[:1]==['--']else a.command
 assert command and 15<=a.seconds<=900
 sys.exit(run(command,a.cwd,a.receipt,a.seconds,a.growth_root,0))
