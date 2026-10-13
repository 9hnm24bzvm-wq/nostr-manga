"""Actual spawned driver workers against controlled HTTP; no application load."""
import asyncio
import hashlib
import io
import json
import math
import os
from pathlib import Path
import signal
import socket
import tempfile
import time
import unittest
from mixed import arguments, plan, run_workload
from image_shards import ImageWorkers

class ImageSharding(unittest.IsolatedAsyncioTestCase):
    async def asyncSetUp(self):
        from aiohttp import web
        self.temp=tempfile.TemporaryDirectory();self.output=Path(self.temp.name)
        self.body=b'controlled HTTP body'*64
        self.images=[dict(sha256=hashlib.sha256(self.body).hexdigest(),bytes=len(self.body))]
        self.mode='good';self.delay=0;self.parent_delay=None;self.status=200;self.received=[];self.received_paths=[];self.active=0;self.peak=0
        async def image(request):
            self.received.append(time.monotonic());self.received_paths.append(request.path);self.active+=1;self.peak=max(self.peak,self.active)
            try:
                await asyncio.sleep(self.parent_delay if request.path=='/parent-delayed' and self.parent_delay is not None else self.delay)
                if self.mode=='truncated':
                    response=web.StreamResponse(headers={'Content-Type':'image/jpeg','Content-Length':str(len(self.body))})
                    await response.prepare(request);await response.write(self.body[:100]);request.transport.close();return response
                body=self.body[:-1] if self.mode=='wrong_length' else self.body
                return web.Response(body=body,status=self.status,content_type='text/plain' if self.mode=='wrong_mime' else 'image/jpeg')
            finally:self.active-=1
        self.app=web.Application();self.app.router.add_get('/{image}',image)
        self.runner=web.AppRunner(self.app);await self.runner.setup()
        sock=socket.socket();sock.bind(('127.0.0.1',0));sock.listen(128)
        self.origin=f'http://127.0.0.1:{sock.getsockname()[1]}'
        await web.SockSite(self.runner,sock).start();self.pools=[]

    async def asyncTearDown(self):
        for pool in self.pools:pool.terminate()
        await self.runner.cleanup();self.temp.cleanup()

    def pool(self,workers=4,extra=None):
        args=arguments(['--profile','reads-and-saves','--duration','.5','--image-rate','13','--peak-start','.1','--peak-duration','.2','--peak-multiplier','2','--image-workers',str(workers),'--images',self.origin,'--save-rate','0','--pageviews','0','--library-read-rate','0','--api-image-rate','0','--request-timeout','1','--closing-timeout','1']+(extra or []))
        out=self.output/str(len(self.pools));pool=ImageWorkers(args,plan(args),self.images,out);self.pools.append(pool);return pool

    async def ready(self,pool):
        pool.launch();await pool.wait_ready();return pool

    async def test_one_and_four_workers_preserve_nondivisible_peak_schedule(self):
        results=[]
        for workers in (1,4):
            pool=await self.ready(self.pool(workers));before=len(self.received)
            await asyncio.sleep(.05)
            self.assertEqual(len(self.received),before) # Ready workers cannot start their own clocks.
            start=time.monotonic()+.1;pool.start(start);result=await pool.finish()
            self.assertTrue(result['schedule_integrity']['valid'],result)
            self.assertEqual(result['schedule_integrity']['planned'],11)
            self.assertEqual(result['lane']['counts']['offered'],11)
            self.assertEqual(result['lane']['counts']['accepted_in_window'],11)
            self.assertEqual(result['lane']['client_latency']['n'],11)
            self.assertEqual(result['http_requests']['download_bytes'],11*len(self.body))
            self.assertTrue(all(t>=start for t in self.received[before:]))
            rows=[json.loads(line) for path in pool.output.glob('*-requests.jsonl') for line in path.read_text().splitlines()]
            latency=sorted(row['client_ms'] for row in rows if row['outcome']=='accepted')
            for percentile,fraction in (('p50_ms',.5),('p95_ms',.95),('p99_ms',.99)):
                exact=latency[math.ceil(len(latency)*fraction)-1]
                self.assertGreaterEqual(result['lane']['client_latency'][percentile],exact)
                self.assertLessEqual(result['lane']['client_latency'][percentile],(exact+1)*1.01-1+1e-9)
            results.append(sorted((r['sequence'],r['scheduled_s']) for r in rows))
        self.assertEqual(results[0],results[1])
        self.assertEqual([r[0] for r in results[0]],list(range(11)))

    async def test_global_credit_bounds_parent_and_children_without_closed_loop(self):
        import aiohttp
        self.delay=.08
        pool=await self.ready(self.pool(extra=['--duration','.3','--peak-start','0','--peak-duration','0','--image-rate','100','--pending-limit','1','--pageviews','100']))
        start=time.monotonic()+.1;pool.start(start)
        settings=dict(pool.settings,rates_per_second=dict(pool.settings['rates_per_second'],image=0))
        async with aiohttp.ClientSession() as session:
            async def request(kind,sequence,user):
                async with session.get(self.origin+'/parent') as response:await response.read();return response.status
            parent=await run_workload(pool.args,settings,request,io.StringIO(),start_at=start,shared_pending=pool.pending)
        result=await pool.finish()
        self.assertTrue(result['schedule_integrity']['valid'])
        self.assertEqual(result['lane']['counts']['offered'],30)
        self.assertLessEqual(self.peak,1)
        self.assertGreater(result['lane']['counts'].get('drop_reason_global_pending_limit',0)+parent['operations']['api']['counts'].get('drop_reason_global_pending_limit',0),0)

    async def test_wrong_length_mime_and_truncated_200_never_acknowledge(self):
        for mode in ('wrong_length','wrong_mime','truncated'):
            self.mode=mode
            pool=await self.ready(self.pool(extra=['--duration','.15','--peak-start','0','--peak-duration','0','--image-rate','20']))
            pool.start(time.monotonic()+.05);result=await pool.finish()
            self.assertTrue(result['schedule_integrity']['valid'],result)
            self.assertEqual(result['lane']['counts'].get('accepted',0),0)
            self.assertEqual(result['lane']['counts']['errors'],3)

    async def test_common_window_keeps_late_ack_separate(self):
        self.delay=.2
        pool=await self.ready(self.pool(extra=['--duration','.1','--peak-start','0','--peak-duration','0','--image-rate','1']))
        pool.start(time.monotonic()+.05);result=await pool.finish()
        self.assertTrue(result['schedule_integrity']['valid'])
        self.assertEqual(result['lane']['counts'].get('accepted_in_window',0),0)
        self.assertEqual(result['lane']['counts']['accepted_after_window'],1)

    async def test_common_closing_deadline_cancels_without_acceptance(self):
        self.delay=.5
        pool=await self.ready(self.pool(extra=['--duration','.1','--peak-start','0','--peak-duration','0','--image-rate','40','--closing-timeout','.05','--pending-limit','4','--image-inflight','4']))
        start=time.monotonic()+.05;pool.start(start);result=await pool.finish()
        self.assertTrue(result['schedule_integrity']['valid'])
        self.assertEqual(result['lane']['counts'].get('accepted',0),0)
        self.assertEqual(result['lane']['counts']['error_reason_closing_deadline_cancel'],4)
        records=[json.loads(line) for path in pool.output.glob('*-requests.jsonl') for line in path.read_text().splitlines()]
        self.assertTrue(all(.14<=row['completion_s']<.3 for row in records))
        # Reuse all four shared credits via actual HTTP, not a semaphore-state assertion.
        import aiohttp
        self.delay=.2
        settings=dict(pool.settings,rates_per_second={key:40 if key=='api' else 0 for key in pool.settings['rates_per_second']})
        pool.args.closing_timeout=1
        async with aiohttp.ClientSession() as session:
            async def request(kind,sequence,user):
                async with session.get(self.origin+'/credit-reuse') as response:await response.read();return response.status
            probe=await run_workload(pool.args,settings,request,io.StringIO(),shared_pending=pool.pending)
        self.assertEqual(probe['operations']['api']['counts']['accepted'],4)

    async def test_worker_crash_makes_missing_schedule_explicit(self):
        pool=await self.ready(self.pool(extra=['--duration','.3','--peak-start','0','--peak-duration','0','--image-rate','100']))
        pool.start(time.monotonic()+.05);await asyncio.sleep(.1)
        pool.processes[1].terminate();result=await pool.finish()
        self.assertFalse(result['schedule_integrity']['valid'])
        self.assertGreater(result['schedule_integrity']['missing'],0)
        self.assertTrue(result['errors'])

    async def test_orchestration_aborts_promptly_when_worker_dies_holding_credit(self):
        from mixed import run_sharded_workload, qualification_passes
        import aiohttp
        self.delay=2
        pool=self.pool(workers=1,extra=['--duration','4','--peak-start','0','--peak-duration','0','--image-rate','1','--pageviews','20','--pending-limit','4'])
        observations=[];parent_rows=io.StringIO()
        async def kill_occupied_worker():
            while '/'+self.images[0]['sha256']+'.jpg' not in self.received_paths:await asyncio.sleep(.005)
            pool.processes[0].kill()
        killer=asyncio.create_task(kill_occupied_worker())
        async with aiohttp.ClientSession() as session:
            async def request(kind,sequence,user):
                async with session.get(self.origin+'/parent') as response:await response.read();return response.status
            before=time.monotonic()
            result,images=await run_sharded_workload(pool,request,parent_rows,observations.append)
        await killer
        self.assertLess(time.monotonic()-before,2)
        self.assertTrue(result['aborted'])
        self.assertFalse(images['schedule_integrity']['valid'])
        self.assertFalse(qualification_passes(pool.args,pool.settings,result))
        pool.args.qualification_contract='full-dispatch-bounded-completion'
        self.assertFalse(qualification_passes(pool.args,pool.settings,result))
        self.assertTrue(all(not p.is_alive() for p in pool.processes))
        self.assertLess(result['operations']['api']['counts'].get('offered',0),20)
        with self.assertRaises(RuntimeError):pool.start(time.monotonic())
        count=len(observations);await asyncio.sleep(.05);self.assertEqual(len(observations),count)

    async def test_orchestration_strict_window_and_offer_conservation(self):
        from mixed import run_sharded_workload, qualification_passes
        for delay,expected in ((0,True),(.4,False)):
            self.delay=delay
            pool=self.pool(extra=['--duration','.15','--peak-start','0','--peak-duration','0','--image-rate','1'])
            async def unused(*args):raise AssertionError('disabled parent lane')
            from mixed_metrics import ScopedSampler
            sampler=ScopedSampler({'generator':os.getpid()});samples=[]
            def observe(backlog):samples.append(dict(backlog=backlog,resources=sampler.sample()))
            result,images=await run_sharded_workload(pool,unused,io.StringIO(),observe,sampler.register_roots)
            self.assertEqual(qualification_passes(pool.args,pool.settings,result),expected)
            self.assertTrue(samples)
            self.assertLessEqual(sum(sample['backlog'].get('phase')=='image_closing' for sample in samples),1)
            observed={p['pid']:p for sample in samples for p in sample['resources']['processes']}
            for index,worker in enumerate(pool.processes):
                self.assertEqual(observed[worker.pid]['group'],f'image_generator/{index}')
                self.assertGreater(observed[worker.pid]['start_ticks'],0)
            self.assertEqual(observed[os.getpid()]['group'],'generator')
            if expected:
                result['operations']['image']['counts']['offered']=0
                self.assertFalse(qualification_passes(pool.args,pool.settings,result))

    async def test_delayed_loop_preserves_eventual_ack_beyond_absolute_closing(self):
        import aiohttp
        pool=self.pool(extra=['--duration','.05','--closing-timeout','.05','--peak-start','0','--peak-duration','0','--image-rate','0','--pageviews','1'])
        stream=io.StringIO()
        async with aiohttp.ClientSession() as session:
            async def request(kind,sequence,user):
                async with session.get(self.origin+'/durable-ack') as response:
                    await response.read()
                    time.sleep(.15) # A stalled loop must not turn a post-cutoff observation into capacity.
                    return response.status
            result=await run_workload(pool.args,pool.settings,request,stream)
        counts=result['operations']['api']['counts']
        self.assertEqual(counts.get('accepted',0),0)
        self.assertEqual(counts['eventual_acknowledged_after_closing'],1)
        self.assertEqual(len(self.received),1)
        self.assertEqual(json.loads(stream.getvalue())['outcome'],'acknowledged_after_closing')

    async def test_worker_startup_pause_cannot_start_other_streams_early(self):
        pool=self.pool();pool.launch();os.kill(pool.processes[1].pid,signal.SIGSTOP)
        try:
            with self.assertRaises(TimeoutError):await pool.wait_ready(timeout=.2)
            self.assertEqual(self.received,[])
        finally:os.kill(pool.processes[1].pid,signal.SIGCONT)

    async def test_explicit_bounded_completion_accepts_normal_tail_not_strict_window(self):
        from mixed import run_sharded_workload, qualification_passes
        self.delay=.25
        pool=self.pool(extra=['--duration','.1','--peak-start','0','--peak-duration','0','--image-rate','40'])
        async def unused(*args):raise AssertionError('disabled parent lane')
        result,_=await run_sharded_workload(pool,unused,io.StringIO())
        self.assertEqual(result['operations']['image']['counts']['accepted_after_window'],4)
        self.assertFalse(qualification_passes(pool.args,pool.settings,result))
        pool.args.qualification_contract='full-dispatch-bounded-completion'
        self.assertTrue(qualification_passes(pool.args,pool.settings,result))
        self.assertFalse(qualification_passes(pool.args,pool.settings,result,require_final_heads=True))
        self.assertFalse(result['all_scheduled_operations_acknowledged_in_window'])
        # The selected contract must still reject missing accounting.
        result['operations']['image']['counts']['offered']-=1
        self.assertFalse(qualification_passes(pool.args,pool.settings,result))

    async def test_delayed_loop_preserves_known_ack_beyond_request_deadline(self):
        from mixed import run_sharded_workload, qualification_passes
        import aiohttp
        pool=self.pool(extra=['--duration','.3','--request-timeout','.05','--closing-timeout','.5','--peak-start','0','--peak-duration','0','--image-rate','0','--pageviews','1'])
        pool.args.qualification_contract='full-dispatch-bounded-completion'
        stream=io.StringIO()
        async with aiohttp.ClientSession() as session:
            async def request(kind,sequence,user):
                async with session.get(self.origin+'/known-ack') as response:
                    await response.read()
                    time.sleep(.15)
                    return response.status
            result,_=await run_sharded_workload(pool,request,stream)
        counts=result['operations']['api']['counts']
        self.assertEqual(counts.get('accepted',0),0)
        self.assertEqual(counts['eventual_acknowledged_after_request_deadline'],1)
        row=json.loads(stream.getvalue())
        self.assertEqual(row['outcome'],'acknowledged_after_request_deadline')
        self.assertTrue(row['durable_outcome_known'])
        self.assertGreater(row['acknowledged_s'],row['request_deadline_s'])
        self.assertLess(row['acknowledged_s'],pool.args.duration)
        self.assertEqual(len(self.received),1)
        self.assertFalse(qualification_passes(pool.args,pool.settings,result))

    async def test_bounded_completion_rejects_real_http_rejections_and_pending_drops(self):
        from mixed import run_sharded_workload, qualification_passes
        for status,delay,limit in ((429,0,16),(200,.2,1)):
            self.status=status;self.delay=delay
            pool=self.pool(extra=['--duration','.1','--peak-start','0','--peak-duration','0','--image-rate','40','--pending-limit',str(limit)])
            pool.args.qualification_contract='full-dispatch-bounded-completion'
            async def unused(*args):raise AssertionError('disabled parent lane')
            result,_=await run_sharded_workload(pool,unused,io.StringIO())
            self.assertFalse(qualification_passes(pool.args,pool.settings,result))
            counts=result['operations']['image']['counts']
            self.assertEqual(counts['offered'],4)
            self.assertGreater(counts.get('errors',0) if status==429 else counts.get('drop_reason_global_pending_limit',0),0)

    async def test_orchestration_rejects_nonfinite_worker_protocol_timestamps(self):
        from mixed import run_sharded_workload, qualification_passes
        import aiohttp
        self.delay=.2;self.parent_delay=.6
        cases=[('log',field,float('nan')) for field in ('scheduled_s','started_s','request_deadline_s','acknowledged_s')]
        cases += [('summary',field,float('nan')) for field in ('start_at','end_at','closing_deadline')]
        cases += [('log','acknowledged_s',float('inf'))]
        cases += [('log','started_s',2.0), ('lane','dispatch_timing',{'first_started_s':float('nan')})]
        for source,field,value in cases:
            with self.subTest(source=source,field=field,value=value):
                pool=self.pool(extra=['--duration','.1','--peak-start','0','--peak-duration','0','--image-rate','40','--pageviews','1'])
                pool.args.qualification_contract='full-dispatch-bounded-completion'
                before=len(self.received)
                async def corrupt_completed_worker():
                    summary=pool.output/'image-worker-0-summary.json'
                    while pool.processes[0].exitcode is None or not summary.exists():await asyncio.sleep(.005)
                    path=summary if source in ('summary','lane') else pool.output/'image-worker-0-requests.jsonl'
                    data=json.loads(path.read_text())
                    if source=='lane':data['operations']['image'][field]=value
                    else:data[field]=value
                    path.write_text(json.dumps(data)+'\n')
                corruption=asyncio.create_task(corrupt_completed_worker())
                async with aiohttp.ClientSession() as session:
                    async def request(kind,sequence,user):
                        async with session.get(self.origin+'/parent-delayed') as response:
                            await response.read();return response.status
                    result,images=await run_sharded_workload(pool,request,io.StringIO())
                await corruption
                self.assertFalse(images['schedule_integrity']['valid'])
                self.assertFalse(qualification_passes(pool.args,pool.settings,result))
                self.assertEqual(len(self.received)-before,5) # Known responses are never re-requested.

    async def test_real_clock_dispatch_tail_parent_and_four_workers(self):
        import aiohttp
        for contract, expected in [('strict-in-window',0),('full-dispatch-bounded-completion',4)]:
            with self.subTest(contract=contract):
                pool=await self.ready(self.pool(extra=['--duration','.05','--peak-start','0','--peak-duration','0','--image-rate','80','--pageviews','20','--api-per-page','1','--max-lateness','.5','--qualification-contract',contract]))
                # Workers are really spawned/ready. Deliver an already elapsed
                # common offer clock, still within the declared lateness bound.
                start=time.monotonic()-.08;before=len(self.received);pool.start(start)
                settings=dict(pool.settings,rates_per_second=dict(pool.settings['rates_per_second'],image=0))
                stream=io.StringIO()
                async with aiohttp.ClientSession() as session:
                    async def request(kind,sequence,user):
                        async with session.get(self.origin+'/tail-parent') as response:
                            await response.read();return response.status
                    parent=await run_workload(pool.args,settings,request,stream,start_at=start,shared_pending=pool.pending)
                merged=await pool.finish()
                self.assertTrue(merged['schedule_integrity']['valid'],merged)
                self.assertEqual(merged['lane']['counts'].get('accepted',0),expected)
                self.assertEqual(parent['operations']['api']['counts'].get('accepted',0),int(bool(expected)))
                self.assertEqual(len(self.received)-before,expected+int(bool(expected)))
                rows=[json.loads(line) for path in pool.output.glob('*-requests.jsonl') for line in path.read_text().splitlines()]
                self.assertEqual(sorted(r['sequence'] for r in rows),list(range(4)))
                if expected:
                    timing=merged['lane']['dispatch_timing']
                    self.assertEqual(timing['late_start_count'],4)
                    self.assertGreater(timing['first_started_s'],pool.args.duration)
                    self.assertLessEqual(timing['maximum_scheduling_lateness_ms'],500)
                    self.assertEqual(timing['dispatch_span_s'],timing['last_started_s']-timing['first_started_s'])
                    self.assertEqual(timing['effective_dispatch_rate_per_s'],3/timing['dispatch_span_s'])
                    self.assertTrue(all(r['scheduled_s']<.05<r['started_s']<=r['acknowledged_s']<=r['request_deadline_s'] for r in rows))

    async def test_dispatch_tail_rejects_stale_closing_and_pending_without_http(self):
        for lateness,closing,hold_credit,reason in [('.03','1',False,'scheduling_lateness'),('1','.02',False,'closing_deadline'),('1','1',True,'global_pending_limit')]:
            with self.subTest(reason=reason):
                pool=await self.ready(self.pool(extra=['--duration','.05','--peak-start','0','--peak-duration','0','--image-rate','80','--max-lateness',lateness,'--closing-timeout',closing,'--pending-limit','1','--qualification-contract','full-dispatch-bounded-completion']))
                if hold_credit:self.assertTrue(pool.pending.acquire(False))
                before=len(self.received);pool.start(time.monotonic()-.1)
                result=await pool.finish()
                if hold_credit:pool.pending.release()
                self.assertTrue(result['schedule_integrity']['valid'],result)
                self.assertEqual(result['lane']['counts']['offered'],4)
                self.assertEqual(result['lane']['counts'].get('started',0),0)
                self.assertEqual(result['lane']['counts'].get('drop_reason_'+reason,0),4)
                self.assertEqual(len(self.received),before)

    async def test_queued_reader_tail_still_owns_single_causal_write(self):
        import aiohttp
        pool=self.pool(extra=['--duration','.05','--closing-timeout','.15','--peak-start','0','--peak-duration','0','--image-rate','0','--save-rate','40','--users','1','--extreme-users','0','--qualification-contract','full-dispatch-bounded-completion'])
        self.delay=.08;stream=io.StringIO()
        async with aiohttp.ClientSession() as session:
            async def request(kind,sequence,user):
                async with session.get(self.origin+'/save-'+str(sequence)) as response:
                    await response.read();return response.status
            result=await run_workload(pool.args,pool.settings,request,stream)
        rows=[json.loads(line) for line in stream.getvalue().splitlines()]
        self.assertEqual(result['operations']['save']['counts'].get('accepted',0),2)
        self.assertEqual(self.received_paths,['/save-0','/save-1'])
        self.assertLessEqual(rows[0]['acknowledged_s'],rows[1]['started_s'])
        self.assertGreater(rows[1]['started_s'],.05)
        self.assertLessEqual(rows[1]['acknowledged_s'],rows[1]['request_deadline_s'])
        self.assertAlmostEqual(rows[1]['request_deadline_s'],.2,places=6)
        self.assertEqual(result['operations']['save']['dispatch_timing']['late_start_count'],1)

    async def test_actual_orchestrator_paused_workers_cross_dispatch_boundary(self):
        from mixed import run_sharded_workload, qualification_passes
        for contract, expected in [('strict-in-window',False),('full-dispatch-bounded-completion',True)]:
            with self.subTest(contract=contract):
                pool=self.pool(extra=['--duration','.1','--peak-start','0','--peak-duration','0','--image-rate','40','--max-lateness','.5','--qualification-contract',contract])
                def pause_ready_workers(roots):
                    for process in pool.processes:os.kill(process.pid,signal.SIGSTOP)
                async def resume_after_offer_window():
                    while pool.start_at is None:await asyncio.sleep(.001)
                    await asyncio.sleep(max(0,pool.start_at+.15-time.monotonic()))
                    for process in pool.processes:os.kill(process.pid,signal.SIGCONT)
                resume=asyncio.create_task(resume_after_offer_window())
                async def unused(*args):raise AssertionError('disabled parent lane')
                before=len(self.received)
                result,_=await run_sharded_workload(pool,unused,io.StringIO(),register_roots=pause_ready_workers)
                await resume
                self.assertEqual(qualification_passes(pool.args,pool.settings,result),expected)
                self.assertEqual(len(self.received)-before,4 if expected else 0)
                self.assertFalse(qualification_passes(pool.args,pool.settings,result,contract='strict-in-window'))
                self.assertEqual(result['operations']['image']['dispatch_timing']['late_start_count'],4 if expected else 0)
                self.assertEqual(result['operations']['image']['counts']['offered'],4)

if __name__=='__main__':unittest.main()
