"""Actual guard CLI and controlled processes/HTTP; no application or database."""
import asyncio,io,json,os,signal,socket,subprocess,sys,tempfile,time,unittest
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
sys.path.insert(0,str(ROOT/'bench/library-node'))
from disk_guard import process

def command(directory,fixture=None,**options):
    argv=[sys.executable,str(ROOT/'bench/library-node/disk_guard.py'),'--evidence',str(directory/'guard.json'),'--minimum-free-bytes',str(options.pop('floor',1)),'--interval','.02','--max-seconds',str(options.pop('maximum',8)),'--target-wait-seconds','.12']
    if fixture:argv+=['--fixture-pid',str(fixture.pid),'--fixture-start-ticks',str(process(fixture.pid)['start_ticks']),'--resources',str(directory/'mixed-resources.jsonl')]
    else:argv+=['--preflight']
    return argv

async def controlled_fixture(directory):
    import aiohttp
    from aiohttp import web
    from image_shards import ImageWorkers
    from mixed import arguments,plan,run_sharded_workload
    from mixed_metrics import ScopedSampler
    import hashlib
    body=b'disk guard real HTTP body';received=[]
    async def handle(request):
        received.append(request.path);(directory/'request-observed').write_text('yes')
        await asyncio.sleep(.5)
        return web.Response(body=body,content_type='image/jpeg')
    app=web.Application();app.router.add_get('/{name}',handle);runner=web.AppRunner(app);await runner.setup()
    sock=socket.socket();sock.bind(('127.0.0.1',0));sock.listen(16);origin=f'http://127.0.0.1:{sock.getsockname()[1]}';await web.SockSite(runner,sock).start()
    args=arguments(['--profile','reads-and-saves','--duration','2','--image-rate','8','--images',origin,'--save-rate','0','--pageviews','0','--library-read-rate','0','--api-image-rate','0','--pending-limit','4'])
    pool=ImageWorkers(args,plan(args),[dict(sha256=hashlib.sha256(body).hexdigest(),bytes=len(body))],directory)
    sampler=ScopedSampler({'generator':os.getpid()})
    with (directory/'mixed-resources.jsonl').open('w') as resources:
        def observe(backlog):resources.write(json.dumps(sampler.sample())+'\n');resources.flush()
        async def unused(*args):raise AssertionError('disabled parent lane')
        result,_=await run_sharded_workload(pool,unused,io.StringIO(),observe,sampler.register_roots)
    (directory/'fixture-result.json').write_text(json.dumps(dict(aborted=result['aborted'],worker_errors=result['image_worker_errors'],received=len(received),all_workers_stopped=all(not p.is_alive() for p in pool.processes))))
    await runner.cleanup()

class DiskGuard(unittest.TestCase):
    def setUp(self):
        self.temp=tempfile.TemporaryDirectory(prefix='disk-guard-test-',dir=ROOT/'results/rebuild-mixed-prep');self.directory=Path(self.temp.name);self.children=[]
    def tearDown(self):
        for child in self.children:
            if child.poll() is None:child.terminate()
            child.wait(timeout=5)
        self.temp.cleanup()
    def child(self,*args):
        env=dict(os.environ,PYTHONPATH=str(ROOT/'vendor/pydeps')+os.pathsep+str(ROOT/'bench/library-node'))
        p=subprocess.Popen([sys.executable,str(Path(__file__).resolve()),*args],env=env,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL);self.children.append(p);return p
    def guard(self,fixture=None,**options):
        result=subprocess.run(command(self.directory,fixture,**options),capture_output=True,text=True,timeout=10)
        return result,json.loads((self.directory/'guard.json').read_text())
    def test_preflight_low_disk_never_launches_or_signals(self):
        result,value=self.guard(floor=10**18)
        self.assertEqual(result.returncode,2);self.assertFalse(value['all_clear']);self.assertIsNone(value['stop_action']);self.assertEqual(value['status'],'preflight_low_disk')
    def test_actual_fixture_exit_requires_explicit_guard_clear(self):
        fixture=self.child('--idle','.2');result,value=self.guard(fixture)
        self.assertEqual(result.returncode,0);self.assertTrue(value['all_clear']);self.assertGreaterEqual(value['samples'],2)
    def test_actual_http_worker_abort_is_owned_and_reaped(self):
        fixture=self.child('--controlled',str(self.directory));deadline=time.monotonic()+6
        while not (self.directory/'request-observed').exists() and time.monotonic()<deadline:time.sleep(.01)
        self.assertTrue((self.directory/'request-observed').exists())
        result,value=self.guard(fixture,floor=10**18)
        self.assertEqual(result.returncode,2);self.assertFalse(value['all_clear']);self.assertTrue(value['stop_action']['owned_ancestry_verified']);self.assertTrue(value['stop_action']['pidfd'])
        fixture.wait(timeout=5);out=json.loads((self.directory/'fixture-result.json').read_text());self.assertTrue(out['aborted']);self.assertTrue(out['all_workers_stopped']);self.assertGreater(out['received'],0)
    def test_missing_or_unrelated_target_never_signals_guess(self):
        fixture=self.child('--idle','1');other=self.child('--idle','1')
        (self.directory/'mixed-resources.jsonl').write_text(json.dumps({'processes':[dict(process(other.pid),group='image_generator/0'),dict(process(fixture.pid),group='generator')]})+'\n')
        result,value=self.guard(fixture,floor=10**18)
        self.assertEqual(result.returncode,2);self.assertEqual(value['status'],'abort_target_unavailable');self.assertIsNone(value['stop_action']);self.assertIsNone(other.poll());self.assertIsNone(fixture.poll())
    def test_pretraffic_worker_unavailable_reports_no_automatic_stop(self):
        fixture=self.child('--idle','1');result,value=self.guard(fixture,floor=10**18)
        self.assertEqual(result.returncode,2);self.assertEqual(value['status'],'abort_target_unavailable');self.assertIsNone(value['stop_action']);self.assertIsNone(fixture.poll())
    def test_stale_worker_start_refused_without_replaying_or_signalling(self):
        fixture=self.child('--controlled',str(self.directory));deadline=time.monotonic()+6
        while not (self.directory/'request-observed').exists() and time.monotonic()<deadline:time.sleep(.01)
        self.assertTrue((self.directory/'request-observed').exists())
        path=self.directory/'mixed-resources.jsonl';value=json.loads(path.read_text().splitlines()[-1])
        for row in value['processes']:
            if row['group']=='image_generator/0':row['start_ticks']+=1
        path.write_text(json.dumps(value)+'\n')
        result,value=self.guard(fixture,floor=10**18)
        self.assertEqual(result.returncode,2);self.assertIsNone(value['stop_action']);self.assertEqual(value['status'],'abort_target_unavailable')
        fixture.wait(timeout=6);out=json.loads((self.directory/'fixture-result.json').read_text());self.assertFalse(out['aborted']);self.assertTrue(out['all_workers_stopped'])
    def test_stale_fixture_identity_rejected_without_signal(self):
        fixture=self.child('--idle','1');argv=command(self.directory,fixture,floor=10**18);i=argv.index('--fixture-start-ticks');argv[i+1]=str(int(argv[i+1])+1)
        result=subprocess.run(argv,capture_output=True,text=True,timeout=5);value=json.loads((self.directory/'guard.json').read_text())
        self.assertEqual(result.returncode,2);self.assertEqual(value['status'],'guard_error');self.assertFalse(value['all_clear']);self.assertIsNone(value['stop_action']);self.assertIsNone(fixture.poll())
    def test_guard_lifespan_is_bounded(self):
        fixture=self.child('--idle','1');before=time.monotonic();result,value=self.guard(fixture,maximum=.1)
        self.assertLess(time.monotonic()-before,.6);self.assertEqual(result.returncode,2);self.assertEqual(value['status'],'guard_deadline');self.assertIsNone(value['stop_action']);self.assertIsNone(fixture.poll())
    def test_final_actual_filesystem_sample_can_fail_after_healthy_first_sample(self):
        stat=os.statvfs(ROOT);floor=stat.f_bavail*stat.f_frsize-16*1024*1024
        fixture=self.child('--consume-after-sample',str(self.directory))
        argv=command(self.directory,fixture,floor=floor,maximum=3);argv[argv.index('--interval')+1]='10'
        result=subprocess.run(argv,capture_output=True,text=True,timeout=5)
        value=json.loads((self.directory/'guard.json').read_text());samples=[json.loads(line) for line in (self.directory/'guard.jsonl').read_text().splitlines()]
        self.assertEqual(len(samples),2);self.assertGreater(samples[0]['free_bytes'],floor);self.assertLessEqual(samples[-1]['free_bytes'],floor)
        self.assertEqual(result.returncode,2);self.assertFalse(value['all_clear']);self.assertEqual(value['status'],'final_low_disk');self.assertIsNone(value['stop_action'])

if __name__=='__main__':
    if len(sys.argv)>1 and sys.argv[1]=='--idle':time.sleep(float(sys.argv[2]))
    elif len(sys.argv)>1 and sys.argv[1]=='--controlled':asyncio.run(controlled_fixture(Path(sys.argv[2])))
    elif len(sys.argv)>1 and sys.argv[1]=='--consume-after-sample':
        directory=Path(sys.argv[2]);sample=directory/'guard.jsonl';deadline=time.monotonic()+2
        while (not sample.exists() or not sample.read_text().strip()) and time.monotonic()<deadline:time.sleep(.005)
        with (directory/'owned-test-allocation').open('wb') as file:os.posix_fallocate(file.fileno(),0,32*1024*1024)
    else:unittest.main()
