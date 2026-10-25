from pathlib import Path
import subprocess,os,json,time,signal,shutil
R=Path('/workspace/shared/nostr-manga-recovery-20261017');W=R/'account-intent-boundaries-worktree-20261017';D=R/'account-intent-runtime-evidence-20261017';floor=4*1024**3
assert shutil.disk_usage(R).free>=floor
assert not (D/'baseline-compile.json').exists()
env=os.environ.copy();env.update(CARGO_HOME=str(R/'toolchains/cargo'),RUSTUP_HOME=str(R/'toolchains/rustup'),CARGO_TARGET_DIR=str(R/'pipeline-cargo-target'),CARGO_BUILD_JOBS='1',CARGO_INCREMENTAL='0',PATH=str(R/'toolchains/cargo/bin')+':'+str(R/'toolchains/cmake-3.31.6/installed/bin')+':'+env['PATH'])
command=[str(R/'toolchains/cargo/bin/cargo'),'test','--offline','--manifest-path','weebdex-platform/backend/Cargo.toml','--test','public_chapter_writes','--no-run','-j','1'];started=time.monotonic();minimum=shutil.disk_usage(R).free;reason=None;signals=[]
def members(group):
 out=[]
 for p in Path('/proc').glob('[0-9]*/stat'):
  try:
   s=p.read_text().rsplit(')',1)[1].split()
   if int(s[2])==group and s[0]!='Z':out.append(int(p.parent.name))
  except (ValueError,OSError,IndexError):pass
 return sorted(out)
with (D/'baseline-compile.log').open('wb') as log:
 proc=subprocess.Popen(command,cwd=W,env=env,stdout=log,stderr=subprocess.STDOUT,start_new_session=True)
 while proc.poll() is None:
  minimum=min(minimum,shutil.disk_usage(R).free)
  if minimum<floor:reason='4GiB shared disk floor';break
  if time.monotonic()-started>600:reason='600s compile deadline';break
  time.sleep(.25)
 if reason:
  os.killpg(proc.pid,signal.SIGTERM);signals.append('SIGTERM newly owned compile group')
  try:proc.wait(timeout=5)
  except subprocess.TimeoutExpired:os.killpg(proc.pid,signal.SIGKILL);signals.append('SIGKILL newly owned compile group');proc.wait(timeout=5)
 survivors=members(proc.pid)
 if survivors:
  os.killpg(proc.pid,signal.SIGKILL);signals.append('SIGKILL remaining owned group');time.sleep(.2);survivors=members(proc.pid)
result=dict(command=command,cwd=str(W),exitCode=proc.returncode,elapsedSeconds=round(time.monotonic()-started,3),floorBytes=floor,minimumFreeBytes=minimum,freeBytesAfter=shutil.disk_usage(R).free,stopReason=reason,ownedGroup=proc.pid,cleanupSignals=signals,survivors=survivors,cleanupVerified=not survivors,offline=True,jobs=1,incremental=False)
(D/'baseline-compile.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result),flush=True)
raise SystemExit(0 if proc.returncode==0 and not reason and not survivors else 1)
