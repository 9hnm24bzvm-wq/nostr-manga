"""Bounded open-loop mixed public-seam driver. Default action is plan, never load."""
import argparse
import base64
import asyncio
import collections
import contextvars
import hashlib
import heapq
import json
import math
import os
import time
import sys
from pathlib import Path
from urllib.parse import urlparse
from email.utils import parsedate_to_datetime
from mixed_metrics import Histogram, ScopedSampler

CURRENT = contextvars.ContextVar('operation', default='setup')
BASE_RATES = dict(save=120.0, api=405.0, image=3087.0, api_image=1.0,
                  social=2.0, upload=.1, sync=.05, library_read=10.0)


def arguments(argv=None):
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--execute', action='store_true')
    p.add_argument('--plan', action='store_true')
    p.add_argument('--duration', type=float, default=60)
    p.add_argument('--scale', type=float, default=1)
    p.add_argument('--max-new-upload-bytes', type=int, default=536870912)
    p.add_argument('--seed', type=int, default=20261010)
    p.add_argument('--users', type=int, default=128)
    p.add_argument('--extreme-users', type=int, default=2)
    p.add_argument('--profile', choices=['all-operations', 'reads-and-saves'], default='all-operations')
    p.add_argument('--pageviews', type=float, default=135)
    p.add_argument('--api-per-page', type=int, default=3)
    for name, rate in BASE_RATES.items():
        if name != 'api': p.add_argument('--' + name.replace('_', '-') + '-rate', type=float, default=rate)
    p.add_argument('--hot-fraction', type=float, default=.2)
    p.add_argument('--hot-probability', type=float, default=.8)
    p.add_argument('--image-hash', action='store_true', help='Hash timed image responses (off by default)')
    p.add_argument('--diagnostic-waits', action='store_true', help='Optional5s main-pool/PG wait snapshots; extra observer overhead, off by default')
    p.add_argument('--peak-start', type=float, default=0)
    p.add_argument('--peak-duration', type=float, default=0)
    p.add_argument('--peak-multiplier', type=float, default=1)
    p.add_argument('--max-offers', type=int, default=2000000)
    p.add_argument('--pending-limit', type=int, default=512)
    p.add_argument('--image-workers', type=int, default=4)
    p.add_argument('--image-inflight', type=int, default=256)
    p.add_argument('--api-inflight', type=int, default=128)
    p.add_argument('--save-pending', type=int, default=128)
    p.add_argument('--max-lateness', type=float, default=1)
    p.add_argument('--request-timeout', type=float, default=30)
    p.add_argument('--closing-timeout', type=float, default=60)
    p.add_argument('--qualification-contract', choices=['strict-in-window', 'full-dispatch-bounded-completion'], default='strict-in-window',
                   help='Explicit exit contract; in-window counts are always retained. Timeouts are safety bounds, not latency SLOs.')
    p.add_argument('--base', default='http://127.0.0.1:58200')
    p.add_argument('--peer', default='http://127.0.0.1:58201')
    p.add_argument('--images', default='http://127.0.0.1:58202')
    p.add_argument('--corpus', type=Path, default=Path('server/tests/fixtures/real-images/MANIFEST.json'))
    args = p.parse_args(argv)
    if not (0 < args.duration <= 3600 and 0 < args.scale <= 20 and 1 <= args.users <= 1024
            and 0 <= args.extreme_users <= args.users and 1 <= args.pending_limit <= 8192):
        p.error('duration/scale/user/pending bounds exceeded')
    if not 1 <= args.max_offers <= 20000000: p.error('max-offers must be between 1 and 20000000')
    if not 1 <= args.image_workers <= 8 or args.image_workers > args.image_inflight: p.error('image workers must be1..8 and fit aggregate image inflight')
    if args.max_new_upload_bytes <= 0: p.error('fresh upload byte budget must be positive')
    if args.api_per_page < 1 or args.pageviews < 0 or any(getattr(args, k + '_rate') < 0 for k in BASE_RATES if k != 'api'):
        p.error('rates must be nonnegative')
    if not (0 <= args.peak_start <= args.duration and 0 <= args.peak_duration <= args.duration - args.peak_start
            and 1 <= args.peak_multiplier <= 20): p.error('invalid peak window')
    if min(args.image_inflight, args.api_inflight, args.save_pending) < 1 or any(
            not math.isfinite(value) or value <= 0
            for value in (args.request_timeout, args.closing_timeout, args.max_lateness)):
        p.error('limits must be positive and timeouts/lateness finite positive')
    for endpoint in (args.base, args.peer, args.images):
        parsed = urlparse(endpoint)
        if parsed.scheme != 'http' or parsed.hostname not in ('127.0.0.1', 'localhost', '::1') or parsed.username or parsed.password or parsed.path not in ('', '/'):
            p.error('this synthetic fixture is restricted to plain loopback origins')
    if not (0 < args.hot_fraction <= 1 and 0 <= args.hot_probability <= 1): p.error('invalid hot/cold skew')
    if len({args.base, args.peer, args.images}) != 3:
        p.error('main API, peer API and managed nginx origins must differ')
    return args


def plan(args):
    rates = {k: args.pageviews * args.api_per_page if k == 'api' else getattr(args, k + '_rate') for k in BASE_RATES}
    if args.profile == 'reads-and-saves': rates.update(social=0, upload=0, sync=0)
    rates = {k: value * args.scale for k, value in rates.items()}
    if args.profile == 'all-operations' and any(rates[k] <= 0 for k in rates):
        raise ValueError('all-operations requires every lane enabled')
    phases=rate_phases(args.duration,dict(start=args.peak_start,duration=args.peak_duration,multiplier=args.peak_multiplier))
    planned_offers={k:sum(phase_count(rate*multiplier,begin,end) for begin,end,multiplier in phases) for k,rate in rates.items()}
    if sum(planned_offers.values()) > args.max_offers: raise ValueError('planned offers exceed bounded request-log budget')
    result = dict(planned_offers=planned_offers, max_offers=args.max_offers, origins=dict(api=args.base, peer=args.peer, managed_nginx=args.images), profile=args.profile, rates_per_second=rates, duration_s=args.duration,
                readers=dict(total=args.users, histories_30000=args.users - args.extreme_users, histories_500000=args.extreme_users),
                pageview_equivalent_rate=args.pageviews * args.scale, api_calls_per_page=args.api_per_page,
                seed=args.seed, timed_image_hashing=args.image_hash,
                qualification_contract=args.qualification_contract,
                dispatch_start_policy=dispatch_start_policy(args),
                completion_bounds=dict(request_timeout_s=args.request_timeout, closing_timeout_s=args.closing_timeout,
                                       maximum_scheduling_lateness_s=args.max_lateness, latency_slo_declared=False),
                upload_semantics='unique JPEG comment variant, signed original upload, signed chapter publication, approved peer import; retained on both nodes for the full observation',
                max_new_upload_bytes_per_node=args.max_new_upload_bytes,
                hot_fraction=args.hot_fraction, hot_probability=args.hot_probability,
                peak=dict(start=args.peak_start, duration=args.peak_duration, multiplier=args.peak_multiplier),
                pending_limit=args.pending_limit, image_workers=args.image_workers, aggregate_image_inflight=args.image_inflight, reader_model='seeded round-robin synthetic contention, not human think time',
                semantics='save acceptance requires signed durable HTTP acknowledgement for the exact event; image acceptance requires complete body length/mime; sync acceptance requires terminal completed job and exact imported event',
                caveats=['All but the final manifest image are static assets; the final image is the decoded-image-preserving JPEG metadata-variant source for fresh signed publication/import. Corpus size and skew are recorded, never treated as cold-storage proof.',
                         'Pageviews mean scheduled catalogue/discovery API calls, not browser/HTML navigation. Separate library_read operations fetch a signed head, root and up to three chunks; they are selective restores, not full-tree downloads.',
                         'Finite windows begin each enabled lane at time0; rare lanes round up and actual offered counts/rates are reported.',
                         'No real WAN, TLS, cold100TB corpus, independent generator host or capacity claim follows from harness preparation.'])
    if args.diagnostic_waits:
        from diagnostic_observer import settings
        result['additional_diagnostics']=settings()
    return result


def rate_phases(duration, peak):
    return [(0,peak['start'],1),(peak['start'],peak['start']+peak['duration'],peak['multiplier']),
            (peak['start']+peak['duration'],duration,1)]


def phase_count(rate, begin, end):
    if rate<=0 or end<=begin: return 0
    count=max(1,math.ceil((end-begin)*rate))
    while count>1 and begin+(count-1)/rate>=end-1e-12: count-=1
    while begin+count/rate<end-1e-12: count+=1
    return count


def schedule(rates, duration, peak):
    sequence=collections.Counter()
    for begin,end,multiplier in rate_phases(duration,peak):
        heap=[]
        for name,rate in rates.items():
            rate*=multiplier; count=phase_count(rate,begin,end)
            if count: heap.append((begin,name,0,rate,count))
        heapq.heapify(heap)
        while heap:
            when,name,offset,rate,count=heapq.heappop(heap)
            yield when,name,sequence[name]
            sequence[name]+=1
            if offset+1<count:heapq.heappush(heap,(begin+(offset+1)/rate,name,offset+1,rate,count))


def jpeg_variant(source, seed, sequence):
    if not source.startswith(b'\xff\xd8'): raise ValueError('fresh upload fixture requires a real JPEG source')
    comment = f'manga mixed fixture seed={seed} sequence={sequence}'.encode()
    return source[:2] + b'\xff\xfe' + (len(comment)+2).to_bytes(2, 'big') + comment + source[2:]


def select_image(sequence, seed, count, hot_fraction, hot_probability):
    # Stable digest gives repeatable selection independent of task completion order.
    raw = hashlib.sha256(f"{seed}:image:{sequence}".encode()).digest()
    hot = max(1, math.ceil(count * hot_fraction))
    if hot == count or int.from_bytes(raw[:8], 'big') / 2**64 < hot_probability:
        return int.from_bytes(raw[8:16], 'big') % hot
    return hot + int.from_bytes(raw[8:16], 'big') % (count - hot)


def dispatch_start_policy(args):
    return ('scheduled-before-end-bounded-start-v1' if args.qualification_contract == 'full-dispatch-bounded-completion'
            else 'strict-window-start-v1')


def dispatch_drop_reason(args, scheduled_s, started_s):
    if not 0 <= scheduled_s < args.duration: return 'window_closed'
    if args.qualification_contract != 'full-dispatch-bounded-completion' and started_s >= args.duration:
        return 'window_closed'
    if started_s >= args.duration + args.closing_timeout: return 'closing_deadline'
    if started_s - scheduled_s > args.max_lateness: return 'scheduling_lateness'
    return None


class Lane:
    def __init__(self):
        self.counts = collections.Counter()
        self.statuses = collections.Counter()
        self.lateness = Histogram(); self.client = Histogram(); self.scheduled = Histogram()
        self.error_samples = []; self.readers = set()
        self.first_started = None; self.last_started = None
        self.maximum_start_lateness_ms = 0; self.maximum_start_after_window_ms = 0

    def started(self, scheduled_s, started_s, duration):
        self.counts['started'] += 1
        self.counts['started_after_window'] += started_s >= duration
        self.first_started = started_s if self.first_started is None else min(self.first_started, started_s)
        self.last_started = started_s if self.last_started is None else max(self.last_started, started_s)
        self.maximum_start_lateness_ms = max(self.maximum_start_lateness_ms, (started_s-scheduled_s)*1000)
        self.maximum_start_after_window_ms = max(self.maximum_start_after_window_ms, (started_s-duration)*1000)

    def dispatch_timing(self):
        span=None if self.first_started is None else self.last_started-self.first_started
        return dict(first_started_s=self.first_started, last_started_s=self.last_started,
                    dispatch_span_s=span,
                    effective_dispatch_rate_per_s=(self.counts['started']-1)/span if span and self.counts['started']>1 else None,
                    effective_dispatch_rate_basis='(started-1)/(last_started_s-first_started_s); inter-start cadence, not offer-window ACK throughput',
                    late_start_count=self.counts['started_after_window'],
                    maximum_scheduling_lateness_ms=self.maximum_start_lateness_ms,
                    maximum_start_after_window_ms=self.maximum_start_after_window_ms)

    def result(self, duration):
        return dict(exercised_readers=sorted(self.readers), counts=dict(self.counts), acknowledged_per_s=self.counts['accepted_in_window'] / duration,
                    total_accepted_per_s=self.counts['accepted'] / duration,
                    offered_per_s=self.counts['offered'] / duration, sent_per_s=self.counts['started'] / duration,
                    dispatch_timing=self.dispatch_timing(),
                    http_statuses=dict(self.statuses), histogram_bins=dict(scheduling_lateness=self.lateness.export(),client_latency=self.client.export(),schedule_to_ack=self.scheduled.export()), scheduling_lateness=self.lateness.result(),
                    client_latency=self.client.result(), schedule_to_ack=self.scheduled.result(), errors=self.error_samples)


async def run_workload(args, settings, operation, stream, observe=None, start_at=None, sequence_partition=None, shared_pending=None):
    """Run bounded open-loop operations; return acknowledgement/drop evidence.

    operation(kind, sequence, reader) must return only after its actual public
    acknowledgement. observe receives backlog snapshots for resource sampling.
    No queued operation is counted as accepted.
    """
    lane = {name: Lane() for name in BASE_RATES}
    locks = [asyncio.Lock() for _ in range(args.users)]
    user_pending = collections.Counter(); poisoned = set()
    pending = set(); class_pending = collections.Counter(); maximum_pending = 0
    class_limit = dict(save=args.save_pending, api=args.api_inflight, image=args.image_inflight, api_image=4, social=16, upload=1, sync=1, library_read=16)
    start = time.monotonic() if start_at is None else start_at; stop = start + args.duration
    done = asyncio.Event(); monitor_errors = []
    async def monitor():
        while not done.is_set():
            try:
                if observe: observe(dict(elapsed_s=time.monotonic()-start, pending=len(pending), pending_by_class=dict(class_pending), reader_backlog=dict(user_pending)))
                stream.flush()
            except Exception as error: monitor_errors.append(type(error).__name__ + ': ' + str(error)[:200])
            try: await asyncio.wait_for(done.wait(), 1)
            except asyncio.TimeoutError: pass
    monitor_task = asyncio.create_task(monitor())
    async def execute(when, kind, sequence, user):
        record = dict(kind=kind, sequence=sequence, user=user, scheduled_s=when)
        token = CURRENT.set(kind); began = None
        try:
            async with locks[user] if kind == 'save' else _NoLock():
                began = time.monotonic(); late = began - start - when
                lane[kind].lateness.add(late * 1000)
                reason = dispatch_drop_reason(args, when, began-start)
                if reason is None and kind == 'save' and user in poisoned: reason='ambiguous_reader'
                if reason:
                    lane[kind].counts['drop_reason_' + reason] += 1
                    lane[kind].counts['generator_drop_after_queue'] += 1; record.update(outcome='generator_drop_after_queue', reason=reason); return
                lane[kind].started(when, began-start, args.duration)
                lane[kind].readers.add(user)
                request_deadline = min(began + args.request_timeout, stop + args.closing_timeout)
                record.update(started_s=began-start, request_deadline_s=request_deadline-start)
                status = await asyncio.wait_for(operation(kind, sequence, user), max(0, request_deadline-time.monotonic()))
                ended = time.monotonic()
                if ended > stop + args.closing_timeout:
                    lane[kind].counts['eventual_acknowledged_after_closing'] += 1
                    lane[kind].statuses[str(status)] += 1
                    record.update(outcome='acknowledged_after_closing', status=status, acknowledged_s=ended-start, durable_outcome_known=True)
                    return
                if ended > request_deadline:
                    lane[kind].counts['eventual_acknowledged_after_request_deadline'] += 1
                    lane[kind].statuses[str(status)] += 1
                    record.update(outcome='acknowledged_after_request_deadline', status=status, acknowledged_s=ended-start, durable_outcome_known=True)
                    return
                lane[kind].counts['accepted'] += 1
                lane[kind].counts['accepted_in_window' if ended <= stop else 'accepted_after_window'] += 1
                lane[kind].statuses[str(status)] += 1
                lane[kind].client.add((ended - began) * 1000); lane[kind].scheduled.add((ended - start - when) * 1000)
                record.update(outcome='accepted', status=status, acknowledged_s=ended-start, client_ms=(ended - began) * 1000, scheduled_ack_ms=(ended - start - when) * 1000)
        except BaseException as error:
            if not isinstance(error, (Exception, asyncio.CancelledError)): raise
            lane[kind].counts['errors'] += 1
            reason = (('workload_abort_cancel' if aborted else 'closing_deadline_cancel') if isinstance(error, asyncio.CancelledError) else ('closing_deadline_cancel' if time.monotonic() >= stop + args.closing_timeout else 'request_timeout') if isinstance(error, asyncio.TimeoutError) else 'response_verification_http_' + str(error.status) if getattr(error, 'response_verification', False) else 'server_' + str(error.status) if getattr(error, 'status', None) else 'operation_error')
            lane[kind].counts['error_reason_' + reason] += 1
            if kind == 'save': poisoned.add(user)
            record.update(outcome='unacknowledged_or_error', reason=reason, error=type(error).__name__, detail=str(error)[:240])
            if len(lane[kind].error_samples) < 16: lane[kind].error_samples.append(record.copy())
        finally:
            record['completion_s'] = time.monotonic() - start
            try: stream.write(json.dumps(record) + '\n')
            finally:
                class_pending[kind] -= 1
                if kind == 'save': user_pending[user] -= 1
                if shared_pending is not None: shared_pending.release()
                CURRENT.reset(token)
    aborted = False
    try:
        for when, kind, sequence in schedule(settings['rates_per_second'], args.duration, settings['peak']):
            if sequence_partition and sequence % sequence_partition[1] != sequence_partition[0]: continue
            await asyncio.sleep(max(0, start + when - time.monotonic()))
            lane[kind].counts['offered'] += 1
            user = (sequence + args.seed) % args.users
            now=time.monotonic()
            reason = dispatch_drop_reason(args, when, now-start)
            reason = reason or ('global_pending_limit' if len(pending) >= args.pending_limit else
                      'class_pending_limit' if class_pending[kind] >= class_limit[kind] else
                      'reader_pending_limit' if kind == 'save' and user_pending[user] >= 2 else None)
            if reason is None and shared_pending is not None and not shared_pending.acquire(False): reason='global_pending_limit'
            if reason:
                lane[kind].counts['drop_reason_' + reason] += 1
                lane[kind].counts['generator_drop_before_queue'] += 1
                lane[kind].lateness.add((time.monotonic() - start - when) * 1000)
                stream.write(json.dumps(dict(kind=kind, sequence=sequence, scheduled_s=when, outcome='generator_drop_before_queue', reason=reason)) + '\n')
                continue
            class_pending[kind] += 1
            if kind == 'save': user_pending[user] += 1
            task = asyncio.create_task(execute(when, kind, sequence, user)); pending.add(task); task.add_done_callback(pending.discard)
            maximum_pending = max(maximum_pending, args.pending_limit-shared_pending.get_value() if shared_pending is not None else len(pending))
        await asyncio.sleep(max(0, stop - time.monotonic()))
        outstanding_at_close = len(pending)
        outstanding_observed_s = time.monotonic()-start
        if pending:
            _, incomplete = await asyncio.wait(list(pending), timeout=max(0,stop+args.closing_timeout-time.monotonic()))
            for task in incomplete: task.cancel()
            await asyncio.gather(*incomplete, return_exceptions=True)
    except asyncio.CancelledError:
        aborted = True
        outstanding_at_close = len(pending)
        outstanding_observed_s = time.monotonic()-start
    finally:
        for task in list(pending): task.cancel()
        await asyncio.gather(*list(pending), return_exceptions=True)
        done.set()
        await monitor_task
    closing_elapsed = time.monotonic() - stop
    return dict(aborted=aborted, dispatch_start_policy=dispatch_start_policy(args), operations={k: v.result(args.duration) for k, v in lane.items()},
                poisoned_readers=sorted(poisoned), maximum_pending=maximum_pending,
                outstanding_at_window_end=outstanding_at_close, closing_elapsed_s=closing_elapsed,
                outstanding_at_window_end_observed_s=outstanding_observed_s,
                monitor_errors=monitor_errors,
                all_operations_exercised=all(lane[k].counts['accepted'] > 0 for k in BASE_RATES),
                all_scheduled_operations_acknowledged_in_window=all(v.counts['accepted_in_window']==v.counts['offered'] for v in lane.values()),
                accepted_window_rates={k:v.counts['accepted_in_window']/args.duration for k,v in lane.items()},
                no_generator_underoffer=not any(v.counts['generator_drop_before_queue'] + v.counts['generator_drop_after_queue'] for v in lane.values()))


async def run_sharded_workload(image_workers, operation, stream, observe=None, register_roots=None):
    """Actual mixed-driver orchestration; fail closed on lost worker ownership."""
    args=image_workers.args;settings=image_workers.settings
    parent_task=None;watcher=None;abort_reason=None
    try:
        image_workers.launch()
        roots=await image_workers.wait_ready()
        if register_roots:register_roots(roots)
        common_start=time.monotonic()+.5
        common_start_unix_ms=(time.time()+common_start-time.monotonic())*1000
        image_workers.start(common_start)
        parent_settings=dict(settings,rates_per_second=dict(settings['rates_per_second'],image=0))
        parent_task=asyncio.create_task(run_workload(args,parent_settings,operation,stream,observe,start_at=common_start,shared_pending=image_workers.pending))
        watcher=asyncio.create_task(image_workers.watch())
        finished,_=await asyncio.wait((parent_task,watcher),return_when=asyncio.FIRST_COMPLETED)
        if watcher in finished:
            try:await watcher
            except Exception as error:abort_reason=str(error)
        if abort_reason:
            image_workers.compromised=True
            parent_task.cancel()
            image_workers.terminate()
            workload=await parent_task
            image_result=await asyncio.to_thread(image_workers.merge)
        else:
            workload=await parent_task
            image_result=await image_workers.finish(observe)
            if image_result['errors']:abort_reason='image worker evidence failed'
        workload['abort_reason']=abort_reason
        workload['aborted']=bool(workload.get('aborted') or abort_reason)
    finally:
        if parent_task and not parent_task.done():
            parent_task.cancel();await asyncio.gather(parent_task,return_exceptions=True)
        if watcher:
            watcher.cancel();await asyncio.gather(watcher,return_exceptions=True)
        image_workers.terminate()
    workload['operations']['image']=image_result['lane']
    workload['image_schedule_integrity']=image_result['schedule_integrity']
    workload['image_worker_errors']=image_result['errors']
    workload['image_worker_pids']=image_result['worker_pids']
    workload['common_start_monotonic']=common_start
    workload['common_end_monotonic']=common_start+args.duration
    workload['common_start_unix_ms']=common_start_unix_ms
    workload['common_end_unix_ms']=common_start_unix_ms+args.duration*1000
    workload['maximum_pending']=max(workload['maximum_pending'],image_result['maximum_pending'])
    workload['outstanding_at_window_end']+=image_result['outstanding_at_window_end']
    workload['outstanding_at_window_end_observed_s']=max(workload['outstanding_at_window_end_observed_s'],image_result['outstanding_at_window_end_observed_s'])
    workload['closing_elapsed_s']=max(workload['closing_elapsed_s'],time.monotonic()-common_start-args.duration)
    workload['all_operations_exercised']=all(v['counts'].get('accepted',0)>0 for v in workload['operations'].values())
    workload['planned_offered_conservation']={k:v['counts'].get('offered',0)==settings['planned_offers'][k] for k,v in workload['operations'].items()}
    workload['all_scheduled_operations_acknowledged_in_window']=not workload.get('aborted') and image_result['schedule_integrity']['valid'] and all(workload['planned_offered_conservation'].values()) and all(v['counts'].get('accepted_in_window',0)==v['counts'].get('offered',0) for v in workload['operations'].values())
    image_counts=image_result['lane']['counts']
    workload['no_generator_underoffer']=workload['no_generator_underoffer'] and image_result['schedule_integrity']['valid'] and not(image_counts.get('generator_drop_before_queue',0)+image_counts.get('generator_drop_after_queue',0))
    workload['accepted_window_rates']['image']=image_result['lane']['acknowledged_per_s']
    return workload,image_result


def qualification_passes(args, settings, result, *, contract=None, require_final_heads=False):
    """Explicit contracts share conservation and failure gates, never a latency SLO.

    Controlled traffic seams omit final heads; actual main requires all readers.
    Known acknowledgements beyond either absolute deadline are never acceptance.
    """
    contract=contract or args.qualification_contract
    if contract not in ('strict-in-window','full-dispatch-bounded-completion'):
        raise ValueError('unknown qualification contract')
    heads=result.get('final_heads',[])
    if require_final_heads and (len(heads)!=args.users or
                               {row['user'] for row in heads}!=set(range(args.users)) or
                               len({row['publisher'] for row in heads})!=args.users):
        return False
    count='accepted_in_window' if contract=='strict-in-window' else 'accepted'
    return (not result.get('aborted') and
            result.get('image_schedule_integrity',{}).get('valid',False) and
            (contract!='strict-in-window' or result.get('all_scheduled_operations_acknowledged_in_window',False)) and
            all(all(result['operations'][k]['counts'].get(field,0)==n for field in ('offered','started',count))
                for k,n in settings['planned_offers'].items()) and
            (args.profile!='all-operations' or result['all_operations_exercised']) and
            result['no_generator_underoffer'] and not result.get('image_worker_errors') and
            not any(v['counts'].get('errors',0) or v['counts'].get('eventual_acknowledged_after_closing',0) or
                    v['counts'].get('eventual_acknowledged_after_request_deadline',0) or
                    any(n for k,n in v['counts'].items() if k.startswith(('generator_drop','drop_reason_')))
                    for v in result['operations'].values()) and
            not result['monitor_errors'] and not result.get('server_diagnostic_errors') and
            all(row['projected'] and not row['ambiguous'] and row.get('acknowledged_head') for row in heads))


async def verify_projected_head(wire, publisher, expected_head, timeout=30):
    """Bounded read-only observation; throttling never counts as head visibility."""
    from run import verify_head
    attempts=0; throttled=0
    async with asyncio.timeout(timeout):
        while True:
            attempts+=1
            try:
                value, _ = await wire.call('GET', '/libraries/' + publisher)
            except Exception as error:
                if getattr(error, 'status', None) != 429: raise
                throttled+=1
                retry=getattr(error, 'retry_after', None)
                try: delay=max(.05,float(retry)) if retry is not None else .1
                except ValueError:
                    try: delay=max(.05,parsedate_to_datetime(retry).timestamp()-time.time())
                    except (TypeError,ValueError,OverflowError): delay=.1
                await asyncio.sleep(delay)
                continue
            verify_head(value['event'],publisher)
            if value['event']['id'] != expected_head: raise RuntimeError('acknowledged head identity mismatch')
            if value.get('projected') is True: return dict(attempts=attempts,throttled_responses=throttled)
            await asyncio.sleep(.05)


async def read_library_snapshot(wire, publisher, sequence):
    """Verify a public signed head, addressed root and bounded selective restore."""
    import cbor2
    from run import sha, verify_head
    head, _ = await wire.call('GET', '/libraries/' + publisher)
    verify_head(head['event'], publisher)
    root_hash = json.loads(head['event']['content'])['manifest']
    root, _ = await wire.call('GET', '/library/blobs/' + root_hash, raw=True)
    if sha(root).hex() != root_hash: raise RuntimeError('library root mismatch')
    descriptors = [item for family in cbor2.loads(root)['families'] for item in family['chunks'] if item]
    selected = [descriptors[(sequence + offset) % len(descriptors)] for offset in range(min(3, len(descriptors)))]
    result, record = await wire.call('POST', '/library/read', dict(hashes=[item[0].hex() for item in selected]))
    if len(result.get('objects', [])) != len(selected): raise RuntimeError('selected library objects missing')
    for obj, descriptor in zip(result['objects'], selected):
        body = base64.b64decode(obj['data'], validate=True)
        if sha(body) != descriptor[0] or len(body) != descriptor[1] or len(cbor2.loads(body)['entries']) != descriptor[2]: raise RuntimeError('selected library chunk mismatch')
    return record['status']


async def main(args):
    if os.environ.get('MIXED_FUNCTIONAL_GATE') != 'passed':
        raise RuntimeError('Execution blocked: complete functional gate and explicitly set MIXED_FUNCTIONAL_GATE=passed')
    import aiohttp
    import cbor2
    from run import Account, Library, Wire, b64, dump, sha, verify_head
    settings = plan(args)
    out = Path(os.environ['NODE_RESULTS'])
    out.mkdir(parents=True, exist_ok=True)
    (out / 'mixed-plan.json').write_text(json.dumps(settings, indent=2))
    http = collections.defaultdict(collections.Counter)
    def observed(record):
        kind = CURRENT.get(); http[kind]['requests'] += 1
        http[kind]['status_' + str(record['status'])] += 1
        http[kind]['upload_bytes'] += record['upload_bytes']; http[kind]['download_bytes'] += record['download_bytes']
    manifest = json.loads(args.corpus.read_text())
    images = []
    for item in manifest['images']:
        root = args.corpus.parent.resolve(); path = (root / item['file']).resolve()
        if not path.is_relative_to(root): raise ValueError('corpus path escapes manifest directory')
        with path.open('rb') as source: digest = hashlib.file_digest(source, 'sha256').hexdigest()
        if not item.get('public_domain') or path.stat().st_size != item['bytes'] or digest != item['sha256']:
            raise ValueError('corpus provenance/bytes verification failed')
        images.append((item, path))
    if len(images) > 10000: raise ValueError('fixture corpus is bounded to 10000 files; larger cold working sets need a separate provisioner')
    if len(images) < 2: raise ValueError('separate retained image and fresh-upload original required')
    served, upload_image = images[:-1], images[-1]
    hot_count=max(1, math.ceil(len(served)*args.hot_fraction))
    hot_mean=sum(item['bytes'] for item,_ in served[:hot_count])/hot_count
    cold_mean=sum(item['bytes'] for item,_ in served[hot_count:])/len(served[hot_count:]) if hot_count<len(served) else hot_mean
    expected_image_bytes=hot_mean*args.hot_probability+cold_mean*(1-args.hot_probability)
    async with aiohttp.ClientSession(auto_decompress=False, connector=aiohttp.TCPConnector(limit=args.pending_limit + 16),
                                    timeout=aiohttp.ClientTimeout(total=args.request_timeout)) as session:
        local = Wire(session, base_url=args.base, service_secret=11, record_limit=0, observer=observed)
        peer = Wire(session, base_url=args.peer, service_secret=12, record_limit=0, observer=observed)
        admin = Account(1)
        async def signed_original(method, descriptor, body=b''):
            action = 'upload' if method == 'PUT' else 'delete'
            event = admin.sign(24242, [['t', action], ['server', args.base], ['x', descriptor['sha256']], ['nonce', os.urandom(16).hex()], ['expiration', str(int(time.time()) + 120)]], action, int(time.time()) - 1)
            headers = {'Authorization': 'Nostr ' + b64(dump(event)), 'Content-Type': 'application/octet-stream'}
            path = '/upload/' if method == 'PUT' else '/blobs/'
            async with session.request(method, args.base + path + descriptor['sha256'], data=body, headers=headers) as response:
                result = await response.read()
                observed(dict(status=response.status, upload_bytes=len(body), download_bytes=len(result)))
                local.proofs.verify_response(response, args.base + path + descriptor['sha256'], method, event, result)
                if response.status != 200: raise RuntimeError(f'original {method}: HTTP {response.status}')
                value = json.loads(result)
                if method == 'PUT' and (value.get('sha256') != descriptor['sha256'] or value.get('size') != len(body)):
                    raise RuntimeError('original acknowledgement mismatch')
        async def image_request(item, static=True, verify=False, origin=None):
            origin = origin or (args.images if static else args.base)
            path = '/' + item['sha256'] + '.jpg' if static else '/originals/' + item['sha256']
            size = 0; digest = hashlib.sha256() if verify else None
            async with session.get(origin + path, allow_redirects=False) as response:
                status = response.status
                async for part in response.content.iter_chunked(65536):
                    size += len(part)
                    if digest: digest.update(part)
                observed(dict(status=status, upload_bytes=0, download_bytes=size))
                if status != 200 or size != item['bytes'] or response.headers.get('Content-Type', '').split(';')[0] != 'image/jpeg':
                    raise RuntimeError(f'original read status/size/mime: HTTP {status}, bytes{size}')
                if digest and digest.hexdigest() != item['sha256']: raise RuntimeError('original digest mismatch')
            return status
        async def projected(wire, event_id):
            deadline = time.monotonic() + 60
            while time.monotonic() < deadline:
                value, record = await wire.call('GET', '/events/' + event_id)
                if record['status'] == 200 and value.get('id') == event_id and 'sig' in value: return
                await asyncio.sleep(.05)
            raise RuntimeError('projection deadline')
        async def publish(kind, content):
            descriptor = dict(sha256=sha(content).hex(), bytes=len(content))
            await signed_original('PUT', descriptor, content)
            event = admin.sign(kind, [], dump(dict(manifest=descriptor['sha256'], version=1)).decode())
            ack, _ = await local.call('POST', '/publications', event, admin)
            if ack.get('id') != event['id']: raise RuntimeError('publication acknowledgement mismatch')
            await projected(local, event['id'])
            return event
        for item, path in served: await signed_original('PUT', item, await asyncio.to_thread(path.read_bytes))
        series = await publish(1002, cbor2.dumps(dict(version=1, titles={'en': 'Public-domain Hokusai mixed fixture'}), canonical=True))
        pages = [dict(sha256=item['sha256'], size=item['bytes'], mime='image/jpeg', width=item['width'], height=item['height']) for item, _ in served]
        chapters = []
        for offset in range(0, len(pages), 32):
            chapters.append(await publish(1001, cbor2.dumps(dict(version=1, series=series['id'], language='ja', chapter=str(offset // 32 + 1), pages=pages[offset:offset+32]), canonical=True)))
        # Initial reads always verify exact digests. Timed image hashing is opt-in.
        for item, _ in served:
            deadline = time.monotonic() + 30
            while True:
                try: await image_request(item, verify=True); break
                except RuntimeError:
                    if time.monotonic() >= deadline: raise
                    await asyncio.sleep(.1)
        if args.profile == 'all-operations':
            await local.call('POST', '/admin/peers', dict(key=Account(12).pub, endpoint=args.peer, allowed=True), admin)
            await peer.call('POST', '/admin/peers', dict(key=Account(11).pub, endpoint=args.base, allowed=True), admin)
            await peer.call('POST', '/admin/sync', dict(peer=Account(11).pub, event=series['id'], filter='', fallbacks=[]), admin)
            await projected(peer, series['id'])
        planned_uploads = settings['planned_offers']['upload']
        if planned_uploads * (upload_image[0]['bytes'] + 4096) > args.max_new_upload_bytes:
            raise ValueError('planned retained fresh uploads exceed explicit per-node byte budget')
        published_uploads = []
        libraries = [None] * args.users
        sem = asyncio.Semaphore(4)
        class AcknowledgedLibrary(Library):
            async def commit(self, wire, items):
                event = self.account.pointer(self.root)
                response, record = await wire.call('POST', '/libraries', dict(event=event, expected_head=self.head, objects=self.objects(items)), self.account, True)
                if response.get('id') != event['id'] or not isinstance(response.get('inserted'), bool):
                    raise RuntimeError('library durable acknowledgement missing or mismatched')
                self.head = event['id']
                return record
        async def seed(index):
            async with sem:
                account = Account(1000 + index)
                await local.call('POST', '/admin/accounts/' + account.pub, dict(allowed=True), admin)
                library = await asyncio.to_thread(AcknowledgedLibrary, 500000 if index < args.extreme_users else 30000, account)
                await library.initialize(local); await library.projected(local); library.compact(); libraries[index] = library
        await asyncio.gather(*(seed(i) for i in range(args.users)))
        paths = ['/series?limit=20&order=asc', '/catalogue/' + series['id'], '/query?limit=20&order=asc']
        async def api_read(index):
            value, record = await local.call('GET', paths[index])
            if index == 0: valid = any(entry.get('event') == series for entry in value.get('entries', []))
            elif index == 1: valid = value.get('event') == series and value.get('metadata', {}).get('titles', {}).get('en') == 'Public-domain Hokusai mixed fixture'
            else: valid = any(event in chapters for event in value.get('events', []))
            if not valid: raise RuntimeError('API read omitted or changed seeded publication')
            return record['status']
        for index in range(len(paths)): await api_read(index)
        (out / 'mixed-corpus.json').write_text(json.dumps(dict(scope=manifest['scope'], static_images=[x[0] for x in served], static_working_set_bytes=sum(x[0]['bytes'] for x in served), expected_image_bytes=expected_image_bytes, offered_static_bytes_per_s=expected_image_bytes*settings['rates_per_second']['image'], offered_static_gigabits_per_s=expected_image_bytes*settings['rates_per_second']['image']*8/1e9, hot_set_images=max(1, math.ceil(len(served)*args.hot_fraction)), hot_probability=args.hot_probability, cold_storage_claim=False, fresh_upload_image=upload_image[0], chapters=[chapter['id'] for chapter in chapters], series=series['id']), indent=2))
        limits, _ = await local.call('GET', '/admin/status', account=admin)
        (out / 'public-status-before.json').write_text(json.dumps(limits, indent=2))
        async def disk_snapshot(label):
            runtime = os.environ.get('MIXED_RUNTIME')
            if runtime:
                process = await asyncio.create_subprocess_exec('du', '-sb', *(str(Path(runtime)/part) for part in ('data', 'node-0', 'node-1')), stdout=asyncio.subprocess.PIPE, stderr=asyncio.subprocess.PIPE)
                stdout, stderr = await process.communicate()
                (out / ('disk-' + label + '.txt')).write_bytes(stdout + stderr)
        await disk_snapshot('before-traffic')
        stream = (out / 'mixed-requests.jsonl').open('w', buffering=1024 * 1024)
        metric_stream = (out / 'mixed-resources.jsonl').open('w')
        sampler = ScopedSampler(dict(node=int(os.environ['NODE_PID']), node_peer=int(os.environ['NODE_PEER_PID']), postgres=int(os.environ['NODE_PG_PID']), generator=os.getpid()))
        def observe(backlog):
            sample = sampler.sample(); sample.update(phase='traffic'); sample.update(backlog)
            metric_stream.write(json.dumps(sample) + '\n'); metric_stream.flush()
        CURRENT.set('traffic')
        async def operation(kind, sequence, user):
            if kind == 'save': return (await libraries[user].commit(local, libraries[user].changed()))['status']
            if kind == 'api': return await api_read(sequence % len(paths))
            if kind in ('image', 'api_image'):
                selected = select_image(sequence, args.seed, len(served), args.hot_fraction, args.hot_probability)
                item = served[selected][0]
                return await image_request(item, static=kind == 'image', verify=args.image_hash)
            if kind == 'library_read': return await read_library_snapshot(local, libraries[user].account.pub, sequence)
            if kind == 'social':
                account = libraries[user].account
                tags = [['E', series['id']], ['K', '1002'], ['P', admin.pub], ['e', series['id']], ['k', '1002'], ['p', admin.pub]]
                event = account.sign(1111, tags, f'Synthetic mixed comment seed{args.seed} sequence{sequence}')
                ack, rec = await local.call('POST', '/social', event, account)
                if ack.get('id') != event['id']: raise RuntimeError('social acknowledgement mismatch')
                return rec['status']
            if kind == 'upload':
                source = await asyncio.to_thread(upload_image[1].read_bytes)
                variant = jpeg_variant(source, args.seed, sequence)
                item = dict(upload_image[0], sha256=sha(variant).hex(), bytes=len(variant))
                await signed_original('PUT', item, variant)
                page = dict(sha256=item['sha256'], size=item['bytes'], mime='image/jpeg', width=item['width'], height=item['height'])
                manifest_bytes = cbor2.dumps(dict(version=1, series=series['id'], language='ja', chapter=f'upload-{args.seed}-{sequence}', pages=[page]), canonical=True)
                event = await publish(1001, manifest_bytes)
                await peer.call('POST', '/admin/sync', dict(peer=Account(11).pub, event=event['id'], filter='', fallbacks=[]), admin)
                await projected(peer, event['id'])
                fetched, _ = await peer.call('GET', '/events/' + event['id'])
                if fetched != event: raise RuntimeError('fresh chapter import differs')
                await image_request(item, static=False, verify=True, origin=args.peer)
                published_uploads.append(dict(event=event['id'], original=item['sha256'], bytes=item['bytes'], manifest_bytes=len(manifest_bytes), retained_on_nodes=2))
                return 200
            if kind == 'sync':
                sync_author = Account(9000 + sequence)
                await peer.call('POST', '/admin/accounts/' + sync_author.pub, dict(allowed=True), admin)
                event = sync_author.sign(0, [], dump(dict(name=f'Synthetic sync {args.seed}/{sequence}')).decode(), int(time.time()))
                ack, _ = await peer.call('POST', '/account-metadata', event, sync_author)
                if ack.get('id') != event['id']: raise RuntimeError('peer metadata acknowledgement mismatch')
                await projected(peer, event['id'])
                batch, _ = await local.call('POST', '/admin/sync-jobs', dict(peer=Account(12).pub, inventory='profiles', filter=f'publisher="{sync_author.pub}"'), admin)
                while True:
                    state, _ = await local.call('GET', '/admin/sync-jobs/' + batch['id'], account=admin)
                    if state['state'] == 'completed': break
                    if state['state'] in ('failed', 'cancelled', 'completed_with_skips'): raise RuntimeError('sync did not import cleanly')
                    await asyncio.sleep(.05)
                fetched, _ = await local.call('GET', '/events/' + event['id'])
                if fetched != event: raise RuntimeError('imported event differs')
                return 200
        diagnostics = (out / 'server-backlog.jsonl').open('w')
        diagnostic_errors = []
        phase = ['traffic']
        async def sample_server_backlog():
            for database in ('manga_mixed_0','manga_mixed_1'):
                try:
                    from pg_diagnostics import collect_sample
                    value = await collect_sample([sys.executable, str(Path(__file__).with_name('pg_diagnostics.py')), '--psql', os.environ.get('MIXED_PSQL', 'vendor/pgsql/bin/psql'), '--database', database], database)
                    if value.get('error'): diagnostic_errors.append(value)
                except OSError as error:
                    value=dict(database=database,error=type(error).__name__)
                    diagnostic_errors.append(value)
                value['phase']=phase[0]
                diagnostics.write(json.dumps(value)+'\n'); diagnostics.flush()
        async def server_diagnostics():
            while True:
                await asyncio.sleep(2)
                await sample_server_backlog()
        phase[0]='before_traffic'
        await sample_server_backlog()
        phase[0]='traffic'
        diagnostics_task=asyncio.create_task(server_diagnostics())
        from image_shards import ImageWorkers
        image_workers=ImageWorkers(args,settings,[dict(sha256=item['sha256'],bytes=item['bytes']) for item,_ in served],out)
        from diagnostic_observer import observers
        async with observers(args.diagnostic_waits,local,admin,CURRENT,out,lambda:phase[0]) as extra_diagnostics:
            workload,image_result=await run_sharded_workload(image_workers,operation,stream,observe,sampler.register_roots)
        http['image'].update(image_result['http_requests'])
        stream.close()
        poisoned = set(workload['poisoned_readers'])
        await disk_snapshot('after-traffic')
        phase[0]='final_projection_verification'
        CURRENT.set('final_verification')
        verification = []
        verification_slots = asyncio.Semaphore(2)
        async def verify_library(index, library):
            try:
                async with verification_slots:
                    observation=await verify_projected_head(local,library.account.pub,library.head)
                verification.append(dict(user=index, publisher=library.account.pub, acknowledged_head=library.head, projected=True, ambiguous=index in poisoned, **observation))
            except Exception as error:
                verification.append(dict(user=index, publisher=library.account.pub, acknowledged_head=library.head, projected=False, ambiguous=index in poisoned, error=str(error)[:240]))
        async def observe_projection_drain():
            while True:
                try: observe(dict(phase='final_projection_verification', verified_heads=len(verification)))
                except Exception as error: workload['monitor_errors'].append(type(error).__name__ + ': ' + str(error)[:200])
                await asyncio.sleep(1)
        drain_monitor=asyncio.create_task(observe_projection_drain())
        try:
            await asyncio.gather(*(verify_library(i, library) for i, library in enumerate(libraries)))
        finally:
            drain_monitor.cancel()
            await asyncio.gather(drain_monitor, return_exceptions=True)
            metric_stream.close()
            diagnostics_task.cancel()
            diagnostic_result=await asyncio.gather(diagnostics_task, return_exceptions=True)
            for value in diagnostic_result:
                if isinstance(value, Exception): diagnostic_errors.append(dict(error=type(value).__name__))
            phase[0]='after_projection_verification'
            await sample_server_backlog()
            diagnostics.close()
        verification.sort(key=lambda row: row['user'])
        result = dict(workload, plan=settings, http_requests={k: dict(v) for k, v in http.items()},
                      fresh_upload_publications=published_uploads, retained_upload_bytes_per_node=sum(x['bytes']+x['manifest_bytes'] for x in published_uploads),
                      cleanup_semantics='new chapters/originals stay retained on both nodes during observation; disposable fixture teardown removes the test instance afterwards, not evidence of online reclamation',
                      final_heads=verification, server_diagnostic_errors=diagnostic_errors,
                      target_attainment_is_not_inferred_from_exit_code=True,
                      notes=['Queued, timed out and cancelled writes are never counted as durable acceptance.',
                             'Library failures poison that reader for the rest of the run; ambiguous commits are not retried.',
                             'Client backlog, acknowledged-head visibility and read-only internal server backlog diagnostics are distinct. A short run does not establish sustained retirement throughput.',
                             'CPU/RSS/I/O sampling is scoped to fixture process ancestry; network counters are namespace-wide.',
                             'Monthly arithmetic averages are baselines, not measured peak capacity. Final head projection is verified; it is not a full predecessor-retirement or disk-growth drain proof.',
                             'An all-operations label requires every lane actually acknowledged at least once.'])
        if args.diagnostic_waits: result['additional_diagnostics']=extra_diagnostics
        result['strict_in_window_qualification']=qualification_passes(args,settings,result,contract='strict-in-window',require_final_heads=True)
        result['full_dispatch_bounded_completion']=qualification_passes(args,settings,result,contract='full-dispatch-bounded-completion',require_final_heads=True)
        result['qualification_contract']=args.qualification_contract
        result['qualification_passed']=qualification_passes(args,settings,result,require_final_heads=True)
        (out / 'mixed-summary.json').write_text(json.dumps(result, indent=2))
        after, _ = await local.call('GET', '/admin/status', account=admin)
        (out / 'public-status-after.json').write_text(json.dumps(after, indent=2))
        print(json.dumps({k: v for k, v in result.items() if k != 'final_heads'}), flush=True)
        if not result['qualification_passed']:
            raise SystemExit(2)


class _NoLock:
    async def __aenter__(self): return self
    async def __aexit__(self, *args): return False


if __name__ == '__main__':
    args = arguments()
    if not args.execute or args.plan: print(json.dumps(plan(args), indent=2))
    else:
        try: asyncio.run(main(args))
        except Exception as error:
            if os.environ.get('NODE_RESULTS'):
                out=Path(os.environ['NODE_RESULTS']); out.mkdir(parents=True, exist_ok=True)
                (out/'mixed-fatal.json').write_text(json.dumps(dict(error=type(error).__name__, detail=str(error)[:500], capacity_acceptance=False), indent=2))
            raise
