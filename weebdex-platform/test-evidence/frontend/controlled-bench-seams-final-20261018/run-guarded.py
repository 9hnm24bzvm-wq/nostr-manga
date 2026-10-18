"""Bounded owned controlled-HTTP/process qualification; never signals unrelated groups."""
import json,os,pathlib,shutil,signal,subprocess,sys,time,traceback
receipt=pathlib.Path(sys.argv[1]).resolve();command=sys.argv[2:];assert command
GiB=1024**3;start=time.monotonic();reason=None;samples=[];error=None;process=None;code=None;survivors=[]
fixture=pathlib.Path('weebdex-platform/.build/controlled-bench-seams-final-20261018').resolve();compat=pathlib.Path('manga-protocol-lab/results/rebuild-mixed-prep').resolve()
def memory():return int(next(x.split()[1] for x in pathlib.Path('/proc/meminfo').read_text().splitlines()if x.startswith('MemAvailable:')))*1024
def members(group):
 rows=[]
 for path in pathlib.Path('/proc').iterdir():
  if not path.name.isdigit():continue
  try:
   fields=(path/'stat').read_text().rsplit(')',1)[1].split()
   if int(fields[2])==group and fields[0]!='Z':rows.append({'pid':int(path.name),'startTicks':int(fields[19]),'rssBytes':int(fields[21])*os.sysconf('SC_PAGE_SIZE'),'state':fields[0]})
  except (FileNotFoundError,ProcessLookupError):continue
 rows.sort(key=lambda x:x['pid']);return rows
def allocated():
 total=0
 for root in [fixture,compat]:
  for parent,dirs,files in os.walk(root,followlinks=False):
   for name in dirs+files:
    try:total+=(pathlib.Path(parent)/name).lstat().st_blocks*512
    except FileNotFoundError:continue # Test TemporaryDirectory legitimately removes entries concurrently.
 return total
def stop_owned():
 if process is None:return
 if members(process.pid):
  for sig in [signal.SIGCONT,signal.SIGTERM]:
   try:os.killpg(process.pid,sig)
   except ProcessLookupError:pass
  try:process.wait(timeout=3)
  except subprocess.TimeoutExpired:pass
  if members(process.pid):
   try:os.killpg(process.pid,signal.SIGKILL)
   except ProcessLookupError:pass
   try:process.wait(timeout=3)
   except subprocess.TimeoutExpired:pass
free=shutil.disk_usage('/workspace/shared').free
assert free>=4*GiB+256*1024**2,'Insufficient launch reserve: '+str(free)
try:
 process=subprocess.Popen(command,start_new_session=True)
 owner=members(process.pid);receipt.with_name('owner-birth.json').write_text(json.dumps({'group':process.pid,'command':command,'births':owner},indent=2)+'\n')
 while process.poll() is None:
  sample={'elapsedSeconds':round(time.monotonic()-start,3),'freeBytes':shutil.disk_usage('/workspace/shared').free,'availableMemoryBytes':memory(),'owned':members(process.pid),'ownedFixtureAllocatedBytes':allocated()};samples.append(sample)
  if sample['ownedFixtureAllocatedBytes']>64*1024**2:reason='64MiB owned fixture cap'
  elif sample['freeBytes']<4*GiB+128*1024**2:reason='4GiB+128MiB live disk floor'
  elif sample['availableMemoryBytes']<768*1024**2:reason='768MiB live memory floor'
  elif time.monotonic()-start>180:reason='180s owned command deadline'
  if reason:break
  time.sleep(.05)
except BaseException as e:
 reason=reason or 'Guard observation failed';error=repr(e);traceback.print_exc()
finally:
 if process is not None:
  if reason or process.poll() is None:stop_owned()
  code=process.wait(timeout=3);time.sleep(.2);survivors=members(process.pid)
  if survivors:reason=reason or 'Unexpected owned survivors';stop_owned();time.sleep(.2);survivors=members(process.pid)
 result={'command':command,'cwd':os.getcwd(),'exitCode':code,'stopReason':reason,'guardError':error,'elapsedSeconds':round(time.monotonic()-start,3),'minimumFreeBytes':min([free]+[s['freeBytes']for s in samples]),'minimumAvailableMemoryBytes':min([memory()]+[s['availableMemoryBytes']for s in samples]),'ownedSurvivors':survivors,'samples':samples,'launchDiskFloorBytes':4*GiB+256*1024**2,'liveDiskFloorBytes':4*GiB+128*1024**2,'fixtureCapBytes':64*1024**2,'deadlineSeconds':180,'sampleIntervalSeconds':.05,'status':'passed'if code==0 and reason is None and not survivors else'failed'}
 receipt.write_text(json.dumps(result,indent=2)+'\n');print(json.dumps({k:v for k,v in result.items()if k!='samples'}))
sys.exit(0 if result['status']=='passed'else 1)
