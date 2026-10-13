"""Bounded fixture disk guard. Never signals nodes/PG or guessed process names."""
import argparse, json, math, os, signal, time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]

def process(pid):
    text = Path(f'/proc/{pid}/stat').read_text()
    fields = text[text.rindex(')')+1:].split()
    return dict(pid=pid, state=fields[0], ppid=int(fields[1]), start_ticks=int(fields[19]))

def belongs(pid, fixture, start):
    seen = set()
    while pid > 0 and pid not in seen:
        seen.add(pid); row = process(pid)
        if pid == fixture: return row['start_ticks'] == start and row['state'] != 'Z'
        pid = row['ppid']
    return False

def last_sample(path):
    with path.open('rb') as source:
        source.seek(0, 2); length=source.tell(); source.seek(max(0,length-262144))
        data=source.read()
    rows=data.split(b'\n')[:-1]
    for line in reversed(rows):
        try:
            value=json.loads(line)
            if isinstance(value,dict) and isinstance(value.get('processes'),list): return value
        except ValueError: pass
    raise ValueError('no complete resource sample')

def stop_owned_worker(resources, fixture, fixture_start):
    if not hasattr(os,'pidfd_open') or not hasattr(signal,'pidfd_send_signal'):raise ValueError('pidfd signalling is required')
    rows=last_sample(resources)['processes']
    worker=next(row for row in rows if row.get('group')=='image_generator/0')
    generator=next(row for row in rows if row.get('group')=='generator')
    pid=worker['pid']; start=worker['start_ticks']
    if type(pid) is not int or type(start) is not int or pid<=0 or pid==fixture: raise ValueError('invalid worker identity')
    if type(generator['pid']) is not int or type(generator['start_ticks']) is not int or generator['pid']<=0 or generator['start_ticks']<=0: raise ValueError('invalid generator identity')
    # A pidfd anchors the selected process across PID reuse. Verify current start
    # identities and ancestry again after opening it, immediately before signal.
    fd=os.pidfd_open(pid)
    try:
        current=process(pid); parent=process(generator['pid'])
        command=Path(f'/proc/{pid}/cmdline').read_bytes()
        if current['state']=='Z' or parent['state']=='Z' or current['start_ticks']!=start or parent['start_ticks']!=generator['start_ticks'] or current['ppid']!=generator['pid']:
            raise ValueError('stale worker or generator identity')
        if b'spawn_main' not in command or b'--multiprocessing-fork' not in command: raise ValueError('not a spawned image worker')
        if not belongs(pid,fixture,fixture_start): raise ValueError('worker outside owned fixture')
        signal.pidfd_send_signal(fd,signal.SIGTERM)
        return dict(pid=pid,start_ticks=start,signal='SIGTERM',pidfd=True,owned_ancestry_verified=True)
    finally: os.close(fd)

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--preflight',action='store_true')
    parser.add_argument('--evidence',type=Path,required=True)
    parser.add_argument('--resources',type=Path)
    parser.add_argument('--fixture-pid',type=int)
    parser.add_argument('--fixture-start-ticks',type=int)
    parser.add_argument('--minimum-free-bytes',type=int,required=True)
    parser.add_argument('--interval',type=float,default=10)
    parser.add_argument('--max-seconds',type=float,default=2820)
    parser.add_argument('--target-wait-seconds',type=float,default=2)
    args=parser.parse_args()
    if args.minimum_free_bytes<1 or any(not math.isfinite(v) or v<=0 for v in (args.interval,args.max_seconds,args.target_wait_seconds)) or args.max_seconds>7200:
        parser.error('finite positive bounds required; maximum lifespan7200s')
    evidence=args.evidence.resolve()
    if not evidence.is_relative_to(ROOT/'results') or not evidence.parent.is_dir(): parser.error('evidence must be in an existing owned results directory')
    if not args.preflight and (not args.resources or not args.resources.resolve().is_relative_to(ROOT/'results') or not args.fixture_pid or args.fixture_pid<=0 or not args.fixture_start_ticks or args.fixture_start_ticks<=0):
        parser.error('owned fixture identity and results resource path required')
    began=time.monotonic(); summary=dict(status='starting',all_clear=False,minimum_free_bytes=args.minimum_free_bytes,
        fixture_pid=args.fixture_pid,fixture_start_ticks=args.fixture_start_ticks,samples=0,minimum_observed_free_bytes=None,stop_action=None,abort_reason=None)
    samples=evidence.with_suffix('.jsonl')
    if evidence.exists() or samples.exists():parser.error('guard evidence already exists; use a new attempt path')
    def save():
        summary['elapsed_s']=time.monotonic()-began
        temporary=evidence.with_suffix('.tmp');temporary.write_text(json.dumps(summary,indent=2)+'\n');temporary.replace(evidence)
    def observe(stream):
        stat=os.statvfs(ROOT);free=stat.f_bavail*stat.f_frsize
        stream.write(json.dumps(dict(unix_ms=time.time()*1000,elapsed_s=time.monotonic()-began,free_bytes=free))+'\n');stream.flush()
        summary['samples']+=1;summary['minimum_observed_free_bytes']=free if summary['minimum_observed_free_bytes'] is None else min(summary['minimum_observed_free_bytes'],free)
        return free
    try:
        with samples.open('x') as stream:
            if args.preflight:
                free=observe(stream);summary.update(status='preflight_pass' if free>=args.minimum_free_bytes else 'preflight_low_disk',all_clear=free>=args.minimum_free_bytes)
                if not summary['all_clear']:summary['abort_reason']='preflight_free_below_floor'
            else:
                initial=process(args.fixture_pid)
                if initial['start_ticks']!=args.fixture_start_ticks or initial['state']=='Z':raise ValueError('initial fixture identity unavailable')
                summary['status']='watching'
                abort_at=None;next_sample=began
                while True:
                    now=time.monotonic()
                    if now>=next_sample:
                        free=observe(stream);next_sample=now+args.interval
                        if free<=args.minimum_free_bytes and abort_at is None:
                            abort_at=now;summary.update(status='abort_pending_safe_worker',abort_reason='free_at_or_below_floor');save()
                        save()
                    try: current=process(args.fixture_pid)
                    except FileNotFoundError: current=None
                    if current is None or current['state']=='Z':
                        summary.update(status='all_clear' if abort_at is None else 'aborted_fixture_exited',all_clear=abort_at is None);break
                    if current['start_ticks']!=args.fixture_start_ticks:raise ValueError('fixture PID reused')
                    if abort_at is not None and summary['stop_action'] is None:
                        try:
                            summary['stop_action']=stop_owned_worker(args.resources,args.fixture_pid,args.fixture_start_ticks)
                            summary['status']='abort_signalled_owned_worker';save()
                        except (OSError,ValueError,KeyError,TypeError,StopIteration) as error:
                            summary['target_error']=type(error).__name__
                            if isinstance(error,ValueError):summary['target_error_detail']=str(error)
                            if now-abort_at>=args.target_wait_seconds:
                                summary['status']='abort_target_unavailable';break
                    if now-began>=args.max_seconds:
                        summary.update(status='guard_deadline',abort_reason=summary['abort_reason'] or 'fixture_exceeded_guard_lifespan');break
                    time.sleep(min(.1,max(.001,args.max_seconds-(time.monotonic()-began))))
                if observe(stream)<=args.minimum_free_bytes and summary['all_clear']:
                    summary.update(status='final_low_disk',all_clear=False,abort_reason='final_free_at_or_below_floor')
    except (OSError,ValueError,KeyError,TypeError) as error:
        summary.update(status='guard_error',all_clear=False,abort_reason=summary['abort_reason'] or type(error).__name__)
    save();print(json.dumps(summary),flush=True)
    return 0 if summary['all_clear'] else 2

if __name__=='__main__':raise SystemExit(main())
