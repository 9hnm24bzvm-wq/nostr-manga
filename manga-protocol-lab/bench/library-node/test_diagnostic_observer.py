"""Actual signed HTTP, CLI/process and driver-owned observer lifetime seams."""
import asyncio,base64,collections,io,json,os,socket,subprocess,sys,tempfile,time,unittest
from pathlib import Path
os.environ.setdefault('NODE_RESULTS',str(Path.cwd()/'results/rebuild-mixed-prep'))
import aiohttp
from aiohttp import web
from coincurve import PublicKeyXOnly
from run import Account,Wire,dump,sha
from mixed import CURRENT
from diagnostic_observer import observers,status_sample

ROOT=Path(__file__).resolve().parents[2]

def controlled_pg_snapshot():
 values=dict(fsync='on',synchronous_commit='on',full_page_writes='on',shared_buffers='128MB',max_wal_size='512MB',wal_buffers='4MB',wal_compression='off',checkpoint_timeout='5min',checkpoint_completion_target='0.9',wal_sync_method='fdatasync')
 return dict(activity=[],elapsed_s=0.0,sampled_at='2026-10-11T00:00:00+00:00',postmaster_started_at='2026-10-10T00:00:00+00:00',server_version_num=180006,
  settings={k:dict(value=v,setting=v,unit=None,source='default') for k,v in values.items()},
  pg_stat_wal=dict(wal_records=10,wal_fpi=10,wal_bytes=10,wal_buffers_full=10,stats_reset='2026-10-10T00:00:00+00:00'),
  pg_stat_checkpointer=dict(num_timed=10,num_requested=10,num_done=10,restartpoints_timed=10,restartpoints_req=10,restartpoints_done=10,write_time=10.0,sync_time=10.0,buffers_written=10,slru_written=10,stats_reset='2026-10-10T00:00:00+00:00'))

class DiagnosticObserver(unittest.IsolatedAsyncioTestCase):
 async def asyncSetUp(self):
  self.temp=tempfile.TemporaryDirectory(prefix='observer-test-');self.out=Path(self.temp.name)
  self.mode='good';self.delay=0;self.active=0;self.peak=0;self.requests=[];self.http=collections.Counter()
  async def status(request):
   raw=await request.read();event=json.loads(base64.b64decode(request.headers['Authorization'].removeprefix('Nostr ')))
   digest=sha(dump([0,event['pubkey'],event['created_at'],event['kind'],event['tags'],event['content']]))
   assert event['pubkey']==Account(1).pub and event['kind']==27235 and digest.hex()==event['id']
   assert PublicKeyXOnly(bytes.fromhex(event['pubkey'])).verify(bytes.fromhex(event['sig']),digest)
   tags={k:v for k,v in event['tags']};assert tags['u']==self.origin+'/admin/status' and tags['method']=='GET' and tags['payload']==sha(raw).hex()
   self.requests.append(time.monotonic());self.active+=1;self.peak=max(self.peak,self.active)
   try:
    await asyncio.sleep(self.delay)
    code=429 if self.mode=='429' else 200
    value={'error':'read_slots_busy'} if code==429 else {'pool':{'size':16,'available':3,'waiting':2},'not_retained':'secret-sentinel'}
    if self.mode=='malformed':value['pool']['waiting']=float('nan')
    body=dump(value);proof=Account(11).sign(20006,[['protocol','manga-federation-response-v1'],['u',self.origin+'/admin/status'],['method','GET'],['request',event['id']],['status',str(code)],['payload',sha(body).hex()]],'')
    headers={} if self.mode=='bad_proof' else {'X-Manga-Response-Proof':'Nostr '+base64.b64encode(dump(proof)).decode()}
    return web.Response(status=code,body=body,headers=headers,content_type='application/json')
   finally:self.active-=1
  app=web.Application();app.router.add_get('/admin/status',status)
  async def image(request):return web.Response(body=b'controlled observer image',content_type='image/jpeg')
  app.router.add_get('/{name}',image);self.runner=web.AppRunner(app);await self.runner.setup()
  sock=socket.socket();sock.bind(('127.0.0.1',0));sock.listen(16);self.origin=f'http://127.0.0.1:{sock.getsockname()[1]}'
  await web.SockSite(self.runner,sock).start();self.session=aiohttp.ClientSession(auto_decompress=False)
  self.wire=Wire(self.session,base_url=self.origin,service_secret=11,record_limit=0,observer=lambda record:self.http.update([CURRENT.get()]))
  self.admin=Account(1)
  self.command=[sys.executable,'-c','import sys;print(sys.argv[1])',json.dumps(controlled_pg_snapshot())]
 async def asyncTearDown(self):
  await self.session.close();await self.runner.cleanup();self.temp.cleanup()
 def rows(self):return [json.loads(line) for line in (self.out/'additional-diagnostics.jsonl').read_text().splitlines()]
 async def test_signed_status_filters_fields_and_keeps_diagnostic_context(self):
  token=CURRENT.set('api')
  try:
   value=await status_sample(self.wire,self.admin,CURRENT)
   self.assertEqual(value,{'pool':{'size':16,'available':3,'waiting':2},'http_status':200});self.assertEqual(CURRENT.get(),'api')
   self.assertEqual(dict(self.http),{'diagnostic_pool_status':1});self.assertNotIn('secret-sentinel',json.dumps(value))
  finally:CURRENT.reset(token)
 async def test_real_signed_rejection_timeout_malformed_and_bad_proof_are_gaps(self):
  for mode in ('429','malformed','bad_proof','timeout'):
   with self.subTest(mode=mode):
    directory=self.out/mode;directory.mkdir();self.mode=mode;self.delay=.08 if mode=='timeout' else 0
    async with observers(True,self.wire,self.admin,CURRENT,directory,lambda:'traffic',interval=1,status_timeout=.025,pg_command=self.command) as summary:
     await asyncio.sleep(.06)
    rows=[json.loads(x) for x in (directory/'additional-diagnostics.jsonl').read_text().splitlines()];status=[x for x in rows if x['kind']=='pool_status'];self.assertEqual(len(status),1)
    self.assertEqual(status[0]['outcome'],'error');self.assertNotIn('pool',status[0]);self.assertGreaterEqual(status[0]['elapsed_s'],0);self.assertTrue(summary['tasks_joined'])
    if mode=='429':self.assertEqual(status[0]['http_status'],429)
    if mode=='bad_proof':self.assertTrue(status[0]['response_verification_failed'])
    self.assertNotIn('secret-sentinel',json.dumps(rows));self.assertNotIn('Authorization',json.dumps(rows))
 async def test_single_flight_skips_catchup_and_stops_with_workload(self):
  self.delay=.09
  async with observers(True,self.wire,self.admin,CURRENT,self.out,lambda:'traffic',interval=.03,status_timeout=.5,pg_command=self.command) as summary:
   await asyncio.sleep(.25)
  count=len(self.requests);await asyncio.sleep(.1)
  self.assertEqual(len(self.requests),count);self.assertEqual(self.peak,1);self.assertTrue(summary['tasks_joined']);self.assertEqual(summary['task_errors'],[])
  rows=[r for r in self.rows() if r['kind']=='pool_status'];self.assertGreaterEqual(len(rows),2)
  self.assertGreater(sum(r['skipped_intervals_after_sample'] for r in rows),0)
  self.assertTrue(all(b-a>=.08 for a,b in zip(self.requests,self.requests[1:])))
 async def test_child_cancellation_is_reaped_and_does_not_block_event_loop(self):
  started=self.out/'child-started';done=self.out/'child-reaped'
  child=f'import os,time;from pathlib import Path;Path({str(started)!r}).write_text(str(os.getpid()));time.sleep(.2)'
  wrapper=f'import subprocess,sys,json;from pathlib import Path;subprocess.run([sys.executable,"-c",{child!r}],timeout=2,check=True);Path({str(done)!r}).write_text("reaped");print(json.dumps({{"activity":[],"elapsed_s":.2}}))'
  command=[sys.executable,'-c',wrapper];ticks=0
  async with observers(True,self.wire,self.admin,CURRENT,self.out,lambda:'traffic',interval=5,pg_command=command) as summary:
   deadline=time.monotonic()+2
   while not started.exists() and time.monotonic()<deadline:await asyncio.sleep(.005)
   self.assertTrue(started.exists());pid=int(started.read_text())
   until=time.monotonic()+.08
   while time.monotonic()<until:ticks+=1;await asyncio.sleep(.005)
  self.assertGreater(ticks,8);self.assertTrue(done.exists());self.assertTrue(summary['tasks_joined'])
  with self.assertRaises(ProcessLookupError):os.kill(pid,0)
  self.assertTrue(any(r['kind']=='pg_activity' and r['outcome']=='cancelled_by_owner' for r in self.rows()))
 async def test_real_four_worker_workload_owns_optional_observers(self):
  from image_shards import ImageWorkers
  from mixed import arguments,plan,run_sharded_workload
  body=b'controlled observer image'
  args=arguments(['--profile','reads-and-saves','--duration','.2','--image-rate','20','--images',self.origin,'--save-rate','0','--pageviews','0','--library-read-rate','0','--api-image-rate','0','--pending-limit','4','--qualification-contract','full-dispatch-bounded-completion','--diagnostic-waits'])
  workers=ImageWorkers(args,plan(args),[dict(sha256=sha(body).hex(),bytes=len(body))],self.out)
  async def unused(*args):raise AssertionError('disabled parent lane')
  async with observers(True,self.wire,self.admin,CURRENT,self.out,lambda:'traffic',interval=.05,pg_command=self.command) as summary:
   result,_=await run_sharded_workload(workers,unused,io.StringIO(),lambda backlog:None)
  self.assertEqual(result['operations']['image']['counts']['accepted'],4);self.assertEqual(result['operations']['image']['counts']['offered'],4)
  self.assertTrue(result['image_schedule_integrity']['valid']);self.assertTrue(all(not child.is_alive() for child in workers.processes))
  self.assertTrue(summary['tasks_joined']);self.assertEqual(summary['task_errors'],[]);self.assertGreater(summary['observations']['pool_status']['observed'],0)
  self.assertTrue(self.http);self.assertEqual(set(self.http),{'diagnostic_pool_status'})
  before=len(self.requests);await asyncio.sleep(.1);self.assertEqual(len(self.requests),before)
 async def test_default_off_has_no_request_file_or_child(self):
  async with observers(False,self.wire,self.admin,CURRENT,self.out,lambda:'traffic',pg_command=['nonexistent-observer']) as summary:
   await asyncio.sleep(.03)
  self.assertIsNone(summary);self.assertEqual(self.requests,[]);self.assertFalse((self.out/'additional-diagnostics.jsonl').exists())
 async def test_real_cli_counter_reset_decrease_gap_and_restart_never_fabricate_zero(self):
  state=self.out/'sample-number'
  source='''import datetime,json,sys
from pathlib import Path
path=Path(sys.argv[1]);n=int(path.read_text()) if path.exists() else 0;path.write_text(str(n+1))
v=json.loads(sys.argv[2]);v['sampled_at']=(datetime.datetime(2026,10,11,tzinfo=datetime.timezone.utc)+datetime.timedelta(seconds=n)).isoformat()
for name in ('pg_stat_wal','pg_stat_checkpointer'):
 for key in v[name]:
  if key!='stats_reset':v[name][key]=[10,15,3,1,0,8,9,10,11][min(n,8)]
 if n>=2:v[name]['stats_reset']='2026-10-10T01:00:00+00:00'
 if n>=8:v[name]['stats_reset']=None
if n>=7:v['postmaster_started_at']='2026-10-10T02:00:00+00:00'
if n==4:v={'activity':[],'error':'controlled_gap','elapsed_s':0.0}
v['unapproved']='secret-sentinel'
print(json.dumps(v))'''
  command=[sys.executable,'-c',source,str(state),json.dumps(controlled_pg_snapshot())]
  async with observers(True,self.wire,self.admin,CURRENT,self.out,lambda:'traffic',interval=.04,pg_command=command) as summary:
   deadline=time.monotonic()+4
   while summary['observations'].get('pg_activity',{}).get('attempted',0)<9 and time.monotonic()<deadline:await asyncio.sleep(.005)
  self.assertTrue(summary['tasks_joined']);self.assertEqual(summary['task_errors'],[])
  rows=[r for r in self.rows() if r['kind']=='pg_activity'];self.assertGreaterEqual(len(rows),9)
  expected=['first_sample','continuous','stats_reset_changed','counter_decreased',None,'previous_sample_missing','continuous','postmaster_changed','reset_identity_unavailable']
  for index,status in enumerate(expected):
   row=rows[index]
   if status is None:
    self.assertEqual(row['outcome'],'error');self.assertNotIn('counter_deltas',row);continue
   self.assertEqual(row['outcome'],'observed')
   for name in ('pg_stat_wal','pg_stat_checkpointer'):
    delta=row['counter_deltas'][name];self.assertEqual(delta['status'],status)
    if status=='continuous':self.assertTrue(all(value=={1:5,6:1}[index] for value in delta['values'].values()))
    else:self.assertIsNone(delta['values'])
  self.assertNotIn('secret-sentinel',json.dumps(rows))
 async def test_real_cli_malformed_counter_and_timestamp_are_sample_gaps(self):
  for field in ('counter','timestamp'):
   directory=self.out/field;directory.mkdir();value=controlled_pg_snapshot()
   if field=='counter':value['pg_stat_wal']['wal_bytes']=float('nan')
   else:value['sampled_at']='not-a-clock'
   command=[sys.executable,'-c','import sys;print(sys.argv[1])',json.dumps(value)]
   async with observers(True,self.wire,self.admin,CURRENT,directory,lambda:'traffic',interval=5,pg_command=command) as summary:
    deadline=time.monotonic()+2
    while not summary['observations'].get('pg_activity') and time.monotonic()<deadline:await asyncio.sleep(.005)
   rows=[json.loads(x) for x in (directory/'additional-diagnostics.jsonl').read_text().splitlines()]
   row=next(r for r in rows if r['kind']=='pg_activity');self.assertEqual(row['outcome'],'error');self.assertNotIn('counter_deltas',row)
   self.assertTrue(summary['tasks_joined']);self.assertEqual(summary['task_errors'],[])
 async def test_cli_matches_reviewed_current_plan_and_observer_only_adds_diagnostics(self):
  base=[sys.executable,str(ROOT/'bench/library-node/mixed.py'),'--plan','--duration','210','--qualification-contract','full-dispatch-bounded-completion']
  # This versioned current contract is not the unrecovered historical run plan.
  expected=json.loads((Path(__file__).with_name('fixtures')/'mixed-210s-bounded-plan-v2.json').read_text())
  default=json.loads(subprocess.run(base,capture_output=True,text=True,check=True).stdout);self.assertEqual(default,expected)
  enabled=json.loads(subprocess.run(base+['--diagnostic-waits'],capture_output=True,text=True,check=True).stdout)
  settings=enabled.pop('additional_diagnostics');self.assertEqual(enabled,default);self.assertEqual(settings['interval_s'],5);self.assertFalse(settings['affects_target_offer_counts']);self.assertFalse(settings['missing_samples_are_zero'])

if __name__=='__main__':unittest.main()
