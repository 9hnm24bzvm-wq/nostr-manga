"""Separate Rust processes against a real ephemeral PostgreSQL database."""
import asyncio,json,os,pathlib
async def main():
    binary=os.environ['MANGA_TEST_WORKER']
    async def worker(mode):
        p=await asyncio.create_subprocess_exec(binary,mode,stdout=asyncio.subprocess.PIPE,stderr=asyncio.subprocess.PIPE)
        out,err=await p.communicate();assert p.returncode==0,(mode,err.decode()[:2000]);return json.loads(out)
    event=await worker('event');pathlib.Path(os.environ['MANGA_TEST_EVENT_PATH']).write_text(json.dumps(event))
    writes=await asyncio.gather(*(worker('write') for _ in range(12)));assert sum(w['created'] for w in writes)==1,writes
    await worker('seed-jobs')
    workers=await asyncio.gather(*(worker('work') for _ in range(4)))
    completed=[x for w in workers for x in w['completed']];assert len(completed)==len(set(completed))==32,workers
    outage=await worker('cache-outage');assert outage['outage_failed_closed'] and outage['pool_recovered']
    assert (await worker('read'))['visible']
    report={'scope':__doc__,'writer_processes':12,'unique_event_inserts':sum(w['created'] for w in writes),'worker_processes':4,'unique_jobs_completed':len(completed),'jobs_per_process':[len(w['completed']) for w in workers],'postgresql_immediate_stop_and_restart':outage,'fresh_process_read_after_recovery':True,'not_claimed':'No full manga API, strfry projection, blob admission or target-scale capacity validation.'}
    pathlib.Path(os.environ['MANGA_PROCESS_REPORT']).write_text(json.dumps(report,indent=2));print(json.dumps(report,indent=2))
asyncio.run(main())
