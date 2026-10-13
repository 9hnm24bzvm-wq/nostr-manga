"""Open-loop scheduled saves, one sequential client per synthetic reader."""
import asyncio,json,os,time,subprocess
from pathlib import Path
import aiohttp
from run import Account,Library,Wire,OUT,stats

def pg_stats():
 sql="SELECT json_build_object('wal',pg_current_wal_lsn()::text,'db_bytes',pg_database_size(current_database()),'pending',(SELECT count(*) FROM backend_outbox WHERE completed_at IS NULL));"
 return json.loads(subprocess.check_output(['vendor/pgsql/bin/psql','-h127.0.0.1','-p55441','-d','manga_node_bench','-At','-c',sql]))
async def main():
 users=int(os.environ.get('NODE_USERS','32'));rate=float(os.environ.get('NODE_RATE','24'));duration=float(os.environ.get('NODE_DURATION','20'));wan=os.environ.get('NODE_WAN','0')=='1';n=int(os.environ.get('NODE_CHAPTERS','30000'))
 results=[];errors=[];rss=[];finished=False;sample_outbox=os.environ.get("NODE_SAMPLE_OUTBOX","0")=="1";profile=os.environ.get("NODE_PROFILE","0")=="1"
 async def monitor():
  while not finished:
   try:
    data=Path('/proc/'+os.environ['NODE_PID']+'/status').read_text();sample=dict(at=time.time(),rss_kib=int(next(x for x in data.splitlines() if x.startswith('VmRSS:')).split()[1]))
    if sample_outbox:
     proc=await asyncio.create_subprocess_exec('vendor/pgsql/bin/psql','-h127.0.0.1','-p55441','-d','manga_node_bench','-At','-c',"SELECT json_build_object('pending',count(*),'oldest_seconds',COALESCE(EXTRACT(EPOCH FROM statement_timestamp()-min(created_at)),0)) FROM backend_outbox WHERE completed_at IS NULL",stdout=asyncio.subprocess.PIPE,stderr=asyncio.subprocess.PIPE)
     output,error=await proc.communicate()
     if proc.returncode: raise RuntimeError(error.decode())
     sample.update(json.loads(output))
    if profile:
     proc=await asyncio.create_subprocess_exec('vendor/pgsql/bin/psql','-h127.0.0.1','-p55441','-d','manga_node_bench','-At','-c',"SELECT COALESCE(json_agg(json_build_object('pid',pid,'wait_type',wait_event_type,'wait',wait_event,'query',query)),'[]') FROM pg_stat_activity WHERE datname=current_database() AND pid<>pg_backend_pid() AND state='active'",stdout=asyncio.subprocess.PIPE,stderr=asyncio.subprocess.PIPE)
     output,error=await proc.communicate()
     if proc.returncode: raise RuntimeError(error.decode())
     sample['activity']=json.loads(output)
    rss.append(sample)
   except Exception as e: rss.append(dict(at=time.time(),monitor_error=str(e)))
   await asyncio.sleep(.1 if profile else (5 if sample_outbox else 1))
 async with aiohttp.ClientSession(auto_decompress=False,connector=aiohttp.TCPConnector(limit=users+16),timeout=aiohttp.ClientTimeout(total=180)) as session:
  local=Wire(session);admin=Account(1);libraries=[]
  for i in range(users):
   a=Account(1000+i);await local.call('POST','/admin/accounts/'+a.pub,dict(allowed=True),admin);lib=Library(n,a);await lib.initialize(local);await lib.projected(local);libraries.append(lib)
   if i%16==15:print(f'Initialized {i+1}/{users} readers',flush=True)
  before=pg_stats();before['object_bytes']=int(subprocess.check_output(['du','-sb',os.environ['NODE_OBJECT_DIR']]).split()[0]);task=asyncio.create_task(monitor());start=time.perf_counter();total=int(rate*duration)
  async def reader(i,lib):
   wire=Wire(session,wan)
   for sequence in range(i,total,users):
    scheduled=start+sequence/rate;await asyncio.sleep(max(0,scheduled-time.perf_counter()));began=time.perf_counter()
    try:
     rec=await lib.commit(wire,lib.changed());rec.update(user=i,sequence=sequence,scheduled_accept_ms=(time.perf_counter()-scheduled)*1000,client_total_ms=(time.perf_counter()-began)*1000);results.append(rec)
    except Exception as e:errors.append(dict(user=i,sequence=sequence,error=str(e),scheduled_ms=(time.perf_counter()-scheduled)*1000))
  await asyncio.gather(*(reader(i,lib) for i,lib in enumerate(libraries)));accepted_end=time.perf_counter();pending_before_drain=pg_stats();drain_start=time.perf_counter()
  for lib in libraries:await lib.projected(local)
  after=pg_stats()
  while after['pending']:
   if time.perf_counter()-drain_start>120: raise RuntimeError('outbox retirement drain timeout')
   await asyncio.sleep(.05);after=pg_stats()
  drain_elapsed=time.perf_counter()-drain_start;after['object_bytes']=int(subprocess.check_output(['du','-sb',os.environ['NODE_OBJECT_DIR']]).split()[0]);finished=True;await task
  summary=dict(readers=users,completed_chapters=n,target_rate=rate,duration_s=duration,wan=wan,attempted=total,accepted=len(results),errors=errors,accepted_elapsed_s=accepted_end-start,acceptance=stats([r['elapsed_ms'] for r in results]) if results else None,scheduled_acceptance=stats([r['scheduled_accept_ms'] for r in results]) if results else None,drain_s=drain_elapsed,pending_before_drain=pending_before_drain,before=before,after=after,peak_node_rss_kib=max((x['rss_kib'] for x in rss if 'rss_kib' in x),default=0),sampled_outbox=sample_outbox,monitor_errors=[x for x in rss if 'monitor_error' in x],final_heads=[dict(publisher=l.account.pub,id=l.head) for l in libraries])
  (OUT/'load-summary.json').write_text(json.dumps(summary,indent=2));(OUT/'requests.json').write_text(json.dumps(results));(OUT/'rss.json').write_text(json.dumps(rss));print(json.dumps({k:v for k,v in summary.items() if k not in ('final_heads','errors')} | {'error_count':len(errors)}),flush=True)
asyncio.run(main())
