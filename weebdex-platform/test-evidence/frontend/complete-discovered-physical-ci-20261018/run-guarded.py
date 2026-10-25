"""Run only the owned frontend command with disk/memory/process-tree receipts."""
import json,os,pathlib,shutil,signal,subprocess,sys,time
receipt=pathlib.Path(sys.argv[1]).resolve();command=sys.argv[2:]
assert command
GiB=1024**3;start=time.monotonic();reason=None;samples=[]
def available_memory():
 return int(next(line.split()[1] for line in pathlib.Path('/proc/meminfo').read_text().splitlines() if line.startswith('MemAvailable:')))*1024
def group_members(group):
 result=[]
 for path in pathlib.Path('/proc').iterdir():
  if not path.name.isdigit():continue
  try:
   fields=(path/'stat').read_text().rsplit(')',1)[1].split()
   if int(fields[2])==group and fields[0]!='Z':result.append({'pid':int(path.name),'rssBytes':int(fields[21])*os.sysconf('SC_PAGE_SIZE')})
  except (OSError,ValueError,IndexError):pass
 return result
free=shutil.disk_usage('/workspace/shared').free
if free<4*GiB+256*1024**2:raise SystemExit('Insufficient start reserve: '+str(free))
process=subprocess.Popen(command,start_new_session=True)
while process.poll() is None:
 sample={'elapsedSeconds':round(time.monotonic()-start,3),'freeBytes':shutil.disk_usage('/workspace/shared').free,'availableMemoryBytes':available_memory(),'owned':group_members(process.pid)};samples.append(sample)
 if sample['freeBytes']<4*GiB+128*1024**2:reason='4GiB +128MiB live disk floor'
 elif sample['availableMemoryBytes']<768*1024**2:reason='768MiB live memory floor'
 elif time.monotonic()-start>600:reason='600s owned command timeout'
 if reason:
  os.killpg(process.pid,signal.SIGTERM)
  try:process.wait(timeout=5)
  except subprocess.TimeoutExpired:os.killpg(process.pid,signal.SIGKILL);process.wait()
  break
 time.sleep(.05)
code=process.wait();time.sleep(.2);survivors=group_members(process.pid)
if survivors:
 reason=reason or 'Unexpected owned survivors';os.killpg(process.pid,signal.SIGTERM);time.sleep(.2)
 survivors=group_members(process.pid)
 if survivors:os.killpg(process.pid,signal.SIGKILL);time.sleep(.2)
 survivors=group_members(process.pid)
result={'command':command,'cwd':os.getcwd(),'exitCode':code,'stopReason':reason,'elapsedSeconds':round(time.monotonic()-start,3),'minimumFreeBytes':min([free]+[s['freeBytes']for s in samples]),'minimumAvailableMemoryBytes':min([available_memory()]+[s['availableMemoryBytes']for s in samples]),'ownedSurvivors':survivors,'samples':samples,'launchDiskFloorBytes':4*GiB+256*1024**2,'liveDiskFloorBytes':4*GiB+128*1024**2,'sampleIntervalSeconds':.05,'status':'passed' if code==0 and reason is None and not survivors else 'failed'}
receipt.write_text(json.dumps(result,indent=2)+'\n');print(json.dumps({k:v for k,v in result.items() if k!='samples'}));sys.exit(0 if result['status']=='passed' else 1)
