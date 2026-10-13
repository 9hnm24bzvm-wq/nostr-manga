"""Image-only spawned workers with one shared open-loop schedule and budget."""
import argparse
import asyncio
import collections
import hashlib
import json
import math
import multiprocessing
import os
from pathlib import Path
import time
from mixed import BASE_RATES, Lane, dispatch_drop_reason, dispatch_start_policy, run_workload, select_image, phase_count, rate_phases
from mixed_metrics import Histogram

async def _run_worker(config, connection, shared_pending):
    import aiohttp
    args=argparse.Namespace(**config['args'])
    args.image_inflight=config['image_inflight']
    settings=dict(rates_per_second={k:config['image_rate'] if k=='image' else 0 for k in BASE_RATES},peak=config['peak'])
    out=Path(config['output']); out.mkdir(parents=True,exist_ok=True)
    prefix=f"image-worker-{config['index']}"
    http=collections.Counter(upload_bytes=0)
    async with aiohttp.ClientSession(auto_decompress=False,connector=aiohttp.TCPConnector(limit=args.image_inflight),timeout=aiohttp.ClientTimeout(total=args.request_timeout)) as session:
        connection.send(dict(ready=True,pid=os.getpid(),worker=config['index']))
        if not connection.poll(30):raise RuntimeError('worker did not receive common start')
        start=connection.recv()['start_at']
        async def operation(kind,sequence,user):
            selected=select_image(sequence,args.seed,len(config['images']),args.hot_fraction,args.hot_probability)
            item=config['images'][selected];size=0;digest=hashlib.sha256() if args.image_hash else None
            async with session.get(config['origin']+'/'+item['sha256']+'.jpg',allow_redirects=False) as response:
                http['requests']+=1;http['status_'+str(response.status)]+=1
                try:
                    async for part in response.content.iter_chunked(65536):
                        size+=len(part)
                        if digest:digest.update(part)
                finally:http['download_bytes']+=size
                if response.status!=200 or size!=item['bytes'] or response.headers.get('Content-Type','').split(';')[0]!='image/jpeg':
                    error=RuntimeError(f'image status/size/MIME mismatch: HTTP{response.status}, bytes{size}');error.status=response.status;raise error
                if digest and digest.hexdigest()!=item['sha256']:raise RuntimeError('image digest mismatch')
                return response.status
        with (out/(prefix+'-requests.jsonl')).open('w',buffering=1024*1024) as stream:
            result=await run_workload(args,settings,operation,stream,start_at=start,
                                      sequence_partition=(config['index'],config['workers']),shared_pending=shared_pending)
    result.update(worker=config['index'],pid=os.getpid(),start_at=start,end_at=start+args.duration,
                  closing_deadline=start+args.duration+args.closing_timeout,http_requests=dict(http))
    (out/(prefix+'-summary.json')).write_text(json.dumps(result,indent=2))


def _worker(config, connection, shared_pending):
    try:asyncio.run(_run_worker(config,connection,shared_pending))
    except BaseException as error:
        out=Path(config['output']);out.mkdir(parents=True,exist_ok=True)
        (out/f"image-worker-{config['index']}-fatal.json").write_text(json.dumps(dict(error=type(error).__name__,detail=str(error)[:250])))
        raise
    finally:connection.close()


class ImageWorkers:
    """Lifecycle used by the actual mixed driver and controlled HTTP fixtures."""
    def __init__(self,args,settings,images,output):
        self.args=args;self.settings=settings;self.images=images;self.output=Path(output)
        self.context=multiprocessing.get_context('spawn')
        self.pending=self.context.BoundedSemaphore(args.pending_limit)
        self.processes=[];self.connections=[];self.ready=[];self.start_at=None;self.compromised=False

    def launch(self):
        if self.compromised or self.processes:raise RuntimeError('worker pool is single-use')
        public_args={k:v for k,v in vars(self.args).items() if k in (
            'duration','users','seed','pending_limit','image_inflight','api_inflight','save_pending',
            'max_lateness','request_timeout','closing_timeout','hot_fraction','hot_probability','image_hash','qualification_contract')}
        for index in range(self.args.image_workers):
            parent,child=self.context.Pipe()
            config=dict(index=index,workers=self.args.image_workers,args=public_args,
                        image_inflight=self.args.image_inflight//self.args.image_workers+(index<self.args.image_inflight%self.args.image_workers),
                        image_rate=self.settings['rates_per_second']['image'],peak=self.settings['peak'],
                        images=self.images,origin=self.args.images,output=str(self.output))
            process=self.context.Process(target=_worker,args=(config,child,self.pending),name=f'mixed-image-{index}')
            # Only public fixture descriptors/origin are passed. Do not inherit DB secrets.
            removed={k:os.environ.pop(k) for k in ('PGPASSWORD','MANGA_DATABASE_URL') if k in os.environ}
            try:process.start()
            finally:os.environ.update(removed)
            child.close();self.processes.append(process);self.connections.append(parent)

    async def wait_ready(self,timeout=15):
        deadline=time.monotonic()+timeout;ready={}
        while len(ready)<len(self.processes):
            for index,(process,connection) in enumerate(zip(self.processes,self.connections)):
                if index in ready:continue
                if connection.poll():
                    value=connection.recv()
                    if value.get('ready') is not True or value.get('pid')!=process.pid:raise RuntimeError('invalid image readiness')
                    ready[index]=value
                elif process.exitcode is not None:raise RuntimeError(f'image worker{index} exited before ready')
            if time.monotonic()>=deadline:raise TimeoutError('image worker readiness deadline')
            if len(ready)<len(self.processes):await asyncio.sleep(.005)
        self.ready=[ready[i] for i in sorted(ready)]
        return {f'image_generator/{i}':process.pid for i,process in enumerate(self.processes)}

    def start(self,start_at):
        if self.compromised or self.start_at is not None:raise RuntimeError('worker pool cannot restart or reuse credits')
        if len(self.ready)!=len(self.processes):raise RuntimeError('all image workers must be ready')
        if type(start_at) not in (int,float) or not math.isfinite(start_at):raise ValueError('common start must be finite')
        self.start_at=start_at
        for connection in self.connections:connection.send(dict(start_at=start_at))

    def unexpected_exit(self):
        for index,process in enumerate(self.processes):
            if process.exitcode is not None and (process.exitcode!=0 or not (self.output/f'image-worker-{index}-summary.json').exists()):
                self.compromised=True
                return f'image worker{index} exited unexpectedly ({process.exitcode}); credits discarded'
        return None

    async def watch(self):
        while True:
            error=self.unexpected_exit()
            if error:raise RuntimeError(error)
            await asyncio.sleep(.02)

    async def finish(self,observe=None):
        deadline=self.start_at+self.args.duration+self.args.closing_timeout+5
        next_observation=0
        while any(process.is_alive() for process in self.processes):
            if self.unexpected_exit():
                self.terminate();break
            if time.monotonic()>deadline:
                self.terminate();break
            if observe and time.monotonic()>=next_observation:
                observe(dict(phase='image_closing',pending=self.args.pending_limit-self.pending.get_value()))
                next_observation=time.monotonic()+1
            await asyncio.sleep(.05)
        for process in self.processes:process.join(timeout=1)
        for connection in self.connections:connection.close()
        return await asyncio.to_thread(self.merge)

    def terminate(self):
        for process in self.processes:
            if process.is_alive():process.terminate()
        for process in self.processes:
            process.join(timeout=2)
            if process.is_alive():process.kill();process.join(timeout=2)
        for connection in self.connections:connection.close()

    def merge(self):
        expected=self.settings['planned_offers']['image'];seen=bytearray(expected)
        duplicate=outside=reported=0;errors=[];summaries=[]
        totals=collections.Counter();statuses=collections.Counter();http=collections.Counter(upload_bytes=0);readers=set();samples=[]
        hist={name:Histogram() for name in ('scheduling_lateness','client_latency','schedule_to_ack')}
        dispatched=Lane()
        for index,process in enumerate(self.processes):
            summary=self.output/f'image-worker-{index}-summary.json';log=self.output/f'image-worker-{index}-requests.jsonl'
            if process.exitcode!=0:errors.append(f'worker{index}:exit{process.exitcode}')
            if not summary.exists() or not log.exists():errors.append(f'worker{index}:missing evidence');continue
            try:
                value=json.loads(summary.read_text())
                if any(type(value[field]) not in (int,float) or not math.isfinite(value[field]) for field in ('start_at','end_at','closing_deadline','outstanding_at_window_end_observed_s')):
                    raise ValueError('nonfinite common clock')
                if value['start_at']!=self.start_at or value['end_at']!=self.start_at+self.args.duration or value['closing_deadline']!=self.start_at+self.args.duration+self.args.closing_timeout:raise ValueError('common window mismatch')
                if value.get('dispatch_start_policy')!=dispatch_start_policy(self.args):raise ValueError('dispatch start policy mismatch')
                lane=value['operations']['image'];summaries.append(value)
                if value.get('monitor_errors'):errors.append(f'worker{index}:monitor error')
                if lane['histogram_bins']['client_latency']['n']!=lane['counts'].get('accepted',0):raise ValueError('accepted histogram count')
                totals.update(lane['counts']);statuses.update(lane['http_statuses']);http.update(value['http_requests']);readers.update(lane['exercised_readers'])
                samples.extend(lane['errors'][:max(0,16-len(samples))])
                for name in hist:hist[name].merge(lane['histogram_bins'][name])
                count=0;acknowledged=0;in_window=0;worker_dispatch=Lane()
                with log.open() as source:
                    for line in source:
                        row=json.loads(line);seq=row['sequence'];count+=1;reported+=1
                        if any(type(row[field]) not in (int,float) or not math.isfinite(row[field]) for field in ('scheduled_s','started_s','request_deadline_s','acknowledged_s','completion_s') if field in row):
                            raise ValueError('nonfinite operation clock')
                        if row['kind']!='image' or type(seq) is not int or seq<0 or seq>=expected or seq%self.args.image_workers!=index:outside+=1;continue
                        if seen[seq]:duplicate+=1
                        seen[seq]=1
                        remaining=seq
                        for begin,end,multiplier in rate_phases(self.args.duration,self.settings['peak']):
                            rate=self.settings['rates_per_second']['image']*multiplier;n=phase_count(rate,begin,end)
                            if remaining<n:
                                if abs(row['scheduled_s']-(begin+remaining/rate))>1e-9:raise ValueError('global deadline changed')
                                break
                            remaining-=n
                        if 'started_s' in row:
                            if row['started_s']<row['scheduled_s'] or dispatch_drop_reason(self.args,row['scheduled_s'],row['started_s']):
                                raise ValueError('dispatch start outside selected bounds')
                            worker_dispatch.started(row['scheduled_s'],row['started_s'],self.args.duration)
                            dispatched.started(row['scheduled_s'],row['started_s'],self.args.duration)
                        if row['outcome']=='accepted':
                            if row['acknowledged_s']<row['started_s'] or row['acknowledged_s']>row['request_deadline_s'] or abs(row['request_deadline_s']-min(row['started_s']+self.args.request_timeout,self.args.duration+self.args.closing_timeout))>1e-6:
                                raise ValueError('absolute request deadline mismatch')
                            acknowledged+=1;in_window+=row['acknowledged_s']<=self.args.duration
                if acknowledged!=lane['counts'].get('accepted',0) or in_window!=lane['counts'].get('accepted_in_window',0):raise ValueError('acknowledgement/window mismatch')
                if worker_dispatch.counts['started']!=lane['counts'].get('started',0) or worker_dispatch.dispatch_timing()!=lane.get('dispatch_timing') or worker_dispatch.counts['started_after_window']!=lane['counts'].get('started_after_window',0):raise ValueError('dispatch timing/count mismatch')
                if count!=lane['counts'].get('offered',0):raise ValueError('log/offered mismatch')
            except (OSError,ValueError,KeyError,TypeError) as error:errors.append(f'worker{index}:{type(error).__name__}')
        missing=expected-sum(seen)
        integrity=dict(planned=expected,reported=reported,duplicates=duplicate,out_of_range=outside,missing=missing,
                       valid=not(errors or duplicate or outside or missing))
        lane=dict(exercised_readers=sorted(readers),counts=dict(totals),
                  acknowledged_per_s=totals['accepted_in_window']/self.args.duration,
                  total_accepted_per_s=totals['accepted']/self.args.duration,
                  offered_per_s=totals['offered']/self.args.duration,sent_per_s=totals['started']/self.args.duration,
                  dispatch_timing=dispatched.dispatch_timing(),
                  http_statuses=dict(statuses),errors=samples,histogram_bins={k:v.export() for k,v in hist.items()},
                  **{k:v.result() for k,v in hist.items()})
        return dict(lane=lane,http_requests=dict(http),schedule_integrity=integrity,errors=errors,
                    maximum_pending=max((v['maximum_pending'] for v in summaries),default=0),
                    outstanding_at_window_end=sum(v['outstanding_at_window_end'] for v in summaries),
                    outstanding_at_window_end_observed_s=max((v['outstanding_at_window_end_observed_s'] for v in summaries),default=0),
                    worker_pids=[p.pid for p in self.processes],start_at=self.start_at,end_at=self.start_at+self.args.duration)
