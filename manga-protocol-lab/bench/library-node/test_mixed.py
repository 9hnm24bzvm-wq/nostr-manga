"""Lightweight harness invariants; no server, database or offered load."""
import asyncio
import io
import os
import subprocess
import sys
import tempfile
import hashlib
import json
import unittest
from pathlib import Path
from mixed import arguments, jpeg_variant, plan, run_workload
from mixed_metrics import Histogram

class MixedHarness(unittest.TestCase):
    def test_default_plan_routes_images_separately(self):
        value = plan(arguments([]))
        self.assertEqual(value['rates_per_second']['api'], 405)
        self.assertEqual(value['rates_per_second']['image'], 3087)
        self.assertEqual(value['readers']['histories_500000'], 2)
        self.assertNotEqual(value['origins']['api'], value['origins']['managed_nginx'])
        self.assertFalse(value['timed_image_hashing'])
        self.assertEqual(value['qualification_contract'],'strict-in-window')
        self.assertEqual(value['dispatch_start_policy'],'strict-window-start-v1')
        self.assertFalse(value['completion_bounds']['latency_slo_declared'])
        self.assertTrue(all(value['rates_per_second'].values()))

    def test_cli_plan_and_execution_gate(self):
        command=[sys.executable, 'bench/library-node/mixed.py']
        result=subprocess.run(command+['--plan'],capture_output=True,text=True,check=True)
        self.assertEqual(json.loads(result.stdout)['rates_per_second']['save'],120)
        scaled=subprocess.run(command+['--plan','--duration','60','--scale','.1'],capture_output=True,text=True,check=True)
        self.assertEqual(json.loads(scaled.stdout)['planned_offers']['image'],18522)
        bounded=subprocess.run(command+['--plan','--qualification-contract','full-dispatch-bounded-completion'],capture_output=True,text=True,check=True)
        bounded_plan=json.loads(bounded.stdout)
        self.assertEqual(bounded_plan['qualification_contract'],'full-dispatch-bounded-completion')
        self.assertEqual(bounded_plan['dispatch_start_policy'],'scheduled-before-end-bounded-start-v1')
        self.assertEqual(bounded_plan['completion_bounds']['request_timeout_s'],30)
        self.assertEqual(bounded_plan['completion_bounds']['closing_timeout_s'],60)
        with tempfile.TemporaryDirectory() as directory:
            env=dict(os.environ, NODE_RESULTS=directory);env.pop('MIXED_FUNCTIONAL_GATE',None)
            blocked=subprocess.run(command+['--execute'],env=env,capture_output=True,text=True)
            self.assertNotEqual(blocked.returncode,0)
            fatal=json.loads((Path(directory)/'mixed-fatal.json').read_text())
            self.assertFalse(fatal['capacity_acceptance'])
            self.assertIn('Execution blocked',fatal['detail'])

    def test_disabled_features_are_explicit(self):
        value=plan(arguments(['--profile','reads-and-saves']))
        self.assertEqual([value['rates_per_second'][k] for k in ('social','upload','sync')], [0,0,0])
        with self.assertRaises(ValueError): plan(arguments(['--social-rate','0']))

    def test_cli_rejects_nonfinite_safety_bounds(self):
        for option in ('request-timeout','closing-timeout','max-lateness'):
            for value in ('inf','-inf','nan'):
                with self.subTest(option=option,value=value):
                    result=subprocess.run([sys.executable,'bench/library-node/mixed.py','--plan',f'--{option}={value}'],capture_output=True,text=True)
                    self.assertNotEqual(result.returncode,0)
                    self.assertFalse(result.stdout.strip())

    def test_unique_jpeg_metadata_preserves_original_stream(self):
        manifest=Path('server/tests/fixtures/real-images/MANIFEST.json')
        source=(manifest.parent/json.loads(manifest.read_text())['images'][-1]['file']).read_bytes()
        a=jpeg_variant(source,19,1);b=jpeg_variant(source,19,2)
        self.assertNotEqual(hashlib.sha256(a).digest(),hashlib.sha256(b).digest())
        self.assertEqual(a[:4], b'\xff\xd8\xff\xfe')
        self.assertEqual(a[:2]+a[4+int.from_bytes(a[4:6],'big'):],source)
        with self.assertRaises(ValueError):jpeg_variant(b'not jpeg',19,1)

    def test_percentile_output_contract(self):
        h=Histogram()
        for i in range(1,1001):h.add(i)
        value=h.result()
        self.assertEqual(value['n'],1000)
        self.assertTrue(500 <= value['p50_ms'] < 507)
        self.assertTrue(950 <= value['p95_ms'] < 961)
        self.assertTrue(990 <= value['p99_ms'] < 1002)
        self.assertIsNone(Histogram().result()['p99_ms'])

class ControlledHttpWorkload(unittest.IsolatedAsyncioTestCase):
    async def run_fixture(self, delay, closing=1, status=200, users=1):
        import aiohttp
        from aiohttp import web
        received=[]
        async def handle(request):
            value=await request.json();received.append(value)
            await asyncio.sleep(delay)
            return web.json_response({'ack':value['sequence']}, status=status)
        app=web.Application();app.router.add_post('/save',handle)
        runner=web.AppRunner(app);await runner.setup()
        # The bound socket is fixture arrangement; all workload observations use HTTP.
        import socket
        sock=socket.socket();sock.bind(('127.0.0.1',0));sock.listen(16)
        port=sock.getsockname()[1]
        site=web.SockSite(runner,sock);await site.start()
        args=arguments(['--profile','reads-and-saves','--duration','.2','--users',str(users),'--extreme-users','0','--save-rate','50','--pageviews','0','--image-rate','0','--api-image-rate','0','--library-read-rate','0','--pending-limit','1','--closing-timeout',str(closing)])
        stream=io.StringIO()
        try:
            async with aiohttp.ClientSession() as session:
                async def operation(kind,sequence,user):
                    async with session.post(f'http://127.0.0.1:{port}/save',json={'sequence':sequence,'user':user}) as response:
                        value=await response.json()
                        if response.status!=200 or value.get('ack')!=sequence:
                            error=RuntimeError('no exact HTTP acknowledgement');error.status=response.status;raise error
                        return response.status
                result=await run_workload(args,plan(args),operation,stream)
            return result,received,[json.loads(line) for line in stream.getvalue().splitlines()]
        finally:await runner.cleanup()

    async def test_actual_http_acknowledgements_match_offered_work(self):
        result,received,records=await self.run_fixture(0)
        counts=result['operations']['save']['counts']
        self.assertEqual(counts['offered'],10)
        self.assertEqual(counts['accepted'],len(received))
        self.assertEqual(counts['accepted'],10)
        self.assertTrue(result['no_generator_underoffer'])
        self.assertTrue(result['all_scheduled_operations_acknowledged_in_window'])
        self.assertEqual(result['operations']['save']['client_latency']['n'],10)
        self.assertTrue(all(row['outcome']=='accepted' for row in records))

    async def test_non_power_of_two_reader_mix_uses_all_identities(self):
        result,received,_=await self.run_fixture(0,users=5)
        self.assertEqual({row['user'] for row in received},set(range(5)))
        self.assertEqual(result['operations']['save']['exercised_readers'],list(range(5)))

    async def test_delayed_ack_reports_open_loop_underoffer(self):
        result,received,records=await self.run_fixture(.3)
        counts=result['operations']['save']['counts']
        self.assertEqual(counts['offered'],10)
        self.assertEqual(counts['accepted'],len(received))
        self.assertEqual(counts['accepted'],1)
        self.assertEqual(counts.get('accepted_in_window',0),0)
        self.assertEqual(counts['accepted_after_window'],1)
        self.assertFalse(result['no_generator_underoffer'])
        self.assertFalse(result['all_scheduled_operations_acknowledged_in_window'])
        self.assertEqual(result['maximum_pending'],1)
        self.assertEqual(sum(row['outcome']=='accepted' for row in records),1)
        self.assertEqual(len(records),10)

    async def test_cutoff_never_counts_queued_unacknowledged_write(self):
        result,received,records=await self.run_fixture(.4,closing=.02)
        counts=result['operations']['save']['counts']
        self.assertEqual(len(received),1)
        self.assertEqual(counts.get('accepted',0),0)
        self.assertEqual(counts['errors'],1)
        self.assertEqual(result['outstanding_at_window_end'],1)
        self.assertTrue(any(row['outcome']=='unacknowledged_or_error' for row in records))

    async def test_http_rejection_poisoning_prevents_false_acceptance(self):
        result,received,_=await self.run_fixture(0,status=503)
        counts=result['operations']['save']['counts']
        self.assertEqual(len(received),1)
        self.assertEqual(counts.get('accepted',0),0)
        self.assertEqual(counts['errors'],1)
        self.assertEqual(counts['error_reason_server_503'],1)
        self.assertEqual(result['poisoned_readers'],[0])

if __name__=='__main__':unittest.main()
