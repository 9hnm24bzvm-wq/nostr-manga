from pathlib import Path
import os, subprocess, json, time, signal, select, shutil, hashlib, re
R=Path('/workspace/shared/nostr-manga-recovery-20261017');W=R/'account-intent-boundaries-worktree-20261017';D=R/'account-intent-runtime-evidence-20261017'
name='final-compile-02';floor=4*1024**3;reserve=128*1024**2
assert shutil.disk_usage(R).free>=floor+2*reserve
assert not (D/(name+'.json')).exists()
h=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
tracked=subprocess.check_output(['git','ls-files','-z','--','weebdex-platform/backend','manga-protocol-lab/server','weebdex-platform/platform-integrations','weebdex-platform/frontend','weebdex-platform/scripts/run-auth-tests.sh','weebdex-platform/tests/runtime/auth_fixture_guard.py'],cwd=W,text=True).split('\0')
pins={p:h(W/p) for p in tracked if p and (W/p).is_file()};(D/'final-source-before.json').write_text(json.dumps(pins,indent=2)+'\n')
env=os.environ.copy();env.update(CARGO_HOME=str(R/'toolchains/cargo'),RUSTUP_HOME=str(R/'toolchains/rustup'),CARGO_TARGET_DIR=str(R/'pipeline-cargo-target'),CARGO_BUILD_JOBS='1',CARGO_INCREMENTAL='0',RUSTUP_TOOLCHAIN='1.90.0-x86_64-unknown-linux-gnu',PATH=str(R/'toolchains/cargo/bin')+':'+str(R/'toolchains/cmake-3.31.6/installed/bin')+':'+env['PATH'])
cmd=[str(R/'toolchains/cargo/bin/cargo'),'test','--locked','--offline','--manifest-path','weebdex-platform/backend/Cargo.toml','--test','public_chapter_writes','--no-run','-j','1']
started=time.monotonic();minimum=shutil.disk_usage(R).free;min_mem=10**20;max_rss=0;stop=None;births={};signals=[]
def scan(group):
 current=[]
 for p in Path('/proc').glob('[0-9]*/stat'):
  try:
   s=p.read_text().rsplit(')',1)[1].split();pid=int(p.parent.name)
   if int(s[2])!=group or int(s[3])!=group or s[0]=='Z':continue
   key=(pid,int(s[19]));rss=int(s[21])*os.sysconf('SC_PAGE_SIZE');current.append((key,rss))
   if key not in births:
    fd=os.pidfd_open(pid,0)
    check=p.read_text().rsplit(')',1)[1].split()
    if int(check[19])!=key[1] or int(check[2])!=group or int(check[3])!=group:os.close(fd);continue
    births[key]=fd
  except (OSError,ValueError,IndexError):continue
 return current
def send(sig,group):
 scan(group)
 for key,fd in births.items():
  try:signal.pidfd_send_signal(fd,sig,None,0);signals.append({'birth':list(key),'signal':sig.name})
  except ProcessLookupError:pass
with (D/(name+'.log')).open('xb') as out:
 p=subprocess.Popen(cmd,cwd=W,env=env,stdout=out,stderr=subprocess.STDOUT,start_new_session=True)
 try:
  while p.poll() is None:
   current=scan(p.pid);free=shutil.disk_usage(R).free;minimum=min(minimum,free);rss=sum(rss for _,rss in current);max_rss=max(max_rss,rss)
   available=int(re.search(r'^MemAvailable:\s+(\d+)',Path('/proc/meminfo').read_text(),re.M)[1])*1024;min_mem=min(min_mem,available)
   if free<floor+reserve:stop='cleanup disk reserve reached'
   elif rss>4*1024**3:stop='owned compile RSS4GiB cap'
   elif available<1024**3:stop='shared available memory1GiB reserve'
   elif time.monotonic()-started>600:stop='600s compile deadline'
   if stop:send(signal.SIGTERM,p.pid);break
   time.sleep(.1)
  try:p.wait(timeout=10)
  except subprocess.TimeoutExpired:send(signal.SIGKILL,p.pid);p.wait(timeout=10)
  survivors=scan(p.pid)
  if survivors:send(signal.SIGTERM,p.pid);time.sleep(.5);survivors=scan(p.pid)
  if survivors:send(signal.SIGKILL,p.pid);time.sleep(.5);survivors=scan(p.pid)
 finally:
  for fd in births.values():os.close(fd)
after={p:h(W/p) for p in pins};changed=[p for p in pins if after[p]!=pins[p]];(D/'final-source-after.json').write_text(json.dumps(after,indent=2)+'\n')
result={'sourceCommit':subprocess.check_output(['git','rev-parse','HEAD'],cwd=W,text=True).strip(),'command':cmd,'exitCode':p.returncode,'elapsedSeconds':round(time.monotonic()-started,3),'minimumFreeBytes':minimum,'floorBytes':floor,'cleanupReserveBytes':reserve,'maxOwnedRssBytes':max_rss,'minimumAvailableMemoryBytes':min_mem,'stopReason':stop,'sourceFiles':len(pins),'sourceChanged':changed,'ownedProcessBirths':[list(k) for k in births],'cleanupSignals':signals,'ownedSurvivors':[list(k) for k,_ in survivors],'executableFiles':{}}
if p.returncode==0:
 for target,path in re.findall(r'Executable (unittests src/lib.rs|tests/public_chapter_writes.rs) \(([^)]+)\)',(D/(name+'.log')).read_text()):
  exe=Path(path);result['executableFiles'][target]={'path':str(exe),'sha256':h(exe),'bytes':exe.stat().st_size}
result['passed']=p.returncode==0 and not stop and not changed and not survivors and len(result['executableFiles'])==1
(D/(name+'.json')).write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result,indent=2));print((D/(name+'.log')).read_text()[-4500:]);raise SystemExit(0 if result['passed'] else 1)
