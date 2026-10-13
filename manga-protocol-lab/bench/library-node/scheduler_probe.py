"""Bounded, read-only Linux scheduler observer for explicitly owned processes.

Uses only allowlisted counters, never command lines, environments or open files.
The default proc root is live /proc; an alternate mount or saved counter tree is
accepted explicitly and identified in the summary. Optional counters may be
unavailable. Cgroup/pressure observations are shared-scope, not process costs.
"""
import argparse
import errno
import hashlib
import itertools
import json
import math
import os
from pathlib import Path
import re
import resource
import signal
import stat
import time


class CounterUnavailable(Exception):
    pass


def read(path, limit=16384):
    try:
        fd = os.open(path, os.O_RDONLY | os.O_NONBLOCK | os.O_CLOEXEC | os.O_NOFOLLOW)
        try:
            if not stat.S_ISREG(os.fstat(fd).st_mode):
                raise CounterUnavailable('not_regular')
            value = os.read(fd, limit + 1)
        finally:
            os.close(fd)
        if len(value) > limit:
            raise CounterUnavailable('oversize')
        return value.decode('ascii')
    except OSError as error:
        reason = ('missing' if error.errno in (errno.ENOENT, errno.ESRCH) else
                  'restricted' if error.errno in (errno.EACCES, errno.EPERM) else 'io_error')
        raise CounterUnavailable(reason) from None
    except UnicodeError:
        raise CounterUnavailable('invalid_encoding') from None


def process_stat(path):
    text = read(path / 'stat')
    try:
        fields = text[text.rindex(')') + 1:].split()
        return dict(pid=int(text.split('(', 1)[0]), parent_pid=int(fields[1]),
                    state=fields[0], cpu_ticks=int(fields[11]) + int(fields[12]),
                    start_ticks=int(fields[19]))
    except (ValueError, IndexError):
        raise CounterUnavailable('invalid_stat') from None


def numbers(path, names):
    try:
        found = {}
        for line in read(path).splitlines():
            key, _, value = line.replace(':', ' ', 1).partition(' ')
            if key in names:
                number = int(value.strip())
                if number < 0:
                    raise ValueError()
                found[key] = number
        return dict(status='available' if len(found) == len(names) else 'partial',
                    values={key: found.get(key) for key in names})
    except (CounterUnavailable, ValueError) as error:
        return dict(status='unavailable', reason=str(error) if isinstance(error, CounterUnavailable)
                    else 'invalid_counter', values={key: None for key in names})


def pressure(path):
    try:
        rows = {}
        for line in read(path, 4096).splitlines():
            kind, *values = line.split()
            if kind not in ('some', 'full'):
                continue
            fields = dict(v.split('=', 1) for v in values)
            row = {key: (int(fields[key]) if key == 'total' else float(fields[key]))
                   for key in ('avg10', 'avg60', 'avg300', 'total')}
            if any(v < 0 or not math.isfinite(v) for v in row.values()):
                raise ValueError()
            rows[kind] = row
        if not rows:
            raise ValueError()
        return dict(status='available', some=rows.get('some'), full=rows.get('full'))
    except (CounterUnavailable, ValueError, KeyError) as error:
        return dict(status='unavailable', reason=str(error) if isinstance(error, CounterUnavailable)
                    else 'invalid_pressure', some=None, full=None)


def target(value):
    try:
        label, pid, start = value.split(':')
        if not re.fullmatch(r'[a-zA-Z][a-zA-Z0-9_-]{0,31}', label):
            raise ValueError()
        pid, start = int(pid), int(start)
        if pid <= 0 or start < 0:
            raise ValueError()
        return label, pid, start
    except ValueError:
        raise argparse.ArgumentTypeError('expected label:positive_pid:nonnegative_start_ticks') from None


class Observer:
    def __init__(self, root, targets, trees, process_limit, thread_limit):
        self.root, self.targets, self.trees = root, targets, set(trees)
        self.process_limit, self.thread_limit = process_limit, thread_limit
        self.previous_cpu, self.previous_threads = {}, {}
        self.discovery, self.discovery_at = None, 0.0
        self.discovery_truncated = False
        self.child_cache = {}
        self.mounts = []
        self.mount_error = None
        try:
            for line in read(root / 'self/mountinfo', 65536).splitlines():
                left, right = line.split(' - ', 1)
                if right.split()[0] == 'cgroup2':
                    fields = left.split()
                    decode = lambda s: re.sub(r'\\([0-7]{3})', lambda m: chr(int(m[1], 8)), s)
                    self.mounts.append((Path(decode(fields[3])), Path(decode(fields[4]))))
        except (CounterUnavailable, ValueError, IndexError) as error:
            self.mount_error = str(error) if isinstance(error, CounterUnavailable) else 'invalid_mountinfo'

    def cgroup(self, pid, captured):
        try:
            lines = read(self.root / str(pid) / 'cgroup', 4096).splitlines()
            members = [line[3:] for line in lines if line.startswith('0::')]
            if len(members) != 1:
                raise CounterUnavailable('cgroup_v2_unavailable')
            member = Path(members[0])
            if not member.is_absolute() or '..' in member.parts:
                raise CounterUnavailable('invalid_membership')
            matches = [(base, mount) for base, mount in self.mounts if member.is_relative_to(base)]
            if not matches:
                raise CounterUnavailable(self.mount_error or 'cgroup_mount_unavailable')
            base, mount = max(matches, key=lambda pair: len(pair[0].parts))
            path = mount / member.relative_to(base)
            identities = []
            ancestors_complete = False
            for _ in range(8):
                metadata = path.stat()
                key = hashlib.sha256(f'{metadata.st_dev}:{metadata.st_ino}'.encode()).hexdigest()[:16]
                identities.append(key)
                if key in captured:
                    if path == mount:
                        ancestors_complete = True
                        break
                    path = path.parent
                    continue
                cpu = numbers(path / 'cpu.stat', ('usage_usec', 'user_usec', 'system_usec',
                              'nr_periods', 'nr_throttled', 'throttled_usec'))
                try:
                    quota, period = read(path / 'cpu.max', 256).split()
                    quota = None if quota == 'max' else int(quota)
                    maximum = dict(status='available', quota_usec=quota, period_usec=int(period),
                                   unlimited=quota is None)
                except (CounterUnavailable, ValueError):
                    maximum = dict(status='unavailable', quota_usec=None, period_usec=None, unlimited=None)
                captured[key] = dict(scope='shared_cgroup_not_process', cpu=cpu, cpu_max=maximum,
                    pressure={kind: pressure(path / f'{kind}.pressure') for kind in ('cpu', 'io', 'memory')})
                if path == mount:
                    ancestors_complete = True
                    break
                path = path.parent
            return dict(status='available', id=identities[0], ancestor_ids=identities[1:],
                        ancestors_complete=ancestors_complete)
        except (CounterUnavailable, OSError) as error:
            return dict(status='unavailable', id=None,
                        reason=str(error) if isinstance(error, CounterUnavailable) else 'cgroup_path_unavailable')

    def process(self, label, pid, expected, cgroups, thread_budget):
        path = self.root / str(pid)
        row = dict(label=label, pid=pid, start_ticks=expected)
        try:
            before = process_stat(path)
            if before['pid'] != pid or before['start_ticks'] != expected:
                raise CounterUnavailable('identity_changed')
            if before['state'] in ('Z', 'X', 'x'):
                raise CounterUnavailable('exited')
            row.update(identity_status='verified', parent_pid=before['parent_pid'],
                       cpu_ticks=before['cpu_ticks'])
            old_cpu = self.previous_cpu.get((pid, expected))
            row['cpu_delta_ticks'] = (before['cpu_ticks'] - old_cpu if old_cpu is not None
                                      and before['cpu_ticks'] >= old_cpu else None)
            tasks = []
            try:
                with os.scandir(path / 'task') as entries:
                    tasks = list(itertools.islice((v.name for v in entries if v.name.isdigit()),
                                                  thread_budget + 1))
                truncated = len(tasks) > thread_budget
            except OSError:
                truncated = False
            observed, current = [], {}
            for tid in tasks[:thread_budget]:
                thread = path / 'task' / tid
                try:
                    first = process_stat(thread)
                    try:
                        values = [int(v) for v in read(thread / 'schedstat', 256).split()]
                        if len(values) != 3 or min(values) < 0:
                            raise ValueError()
                        scheduling = values
                    except (CounterUnavailable, ValueError):
                        scheduling = None
                    switches = numbers(thread / 'status',
                                       ('voluntary_ctxt_switches', 'nonvoluntary_ctxt_switches'))['values']
                    last = process_stat(thread)
                    if first['pid'] != int(tid) or last['pid'] != int(tid) or first['start_ticks'] != last['start_ticks']:
                        continue
                    key = (pid, expected, int(tid), first['start_ticks'])
                    item = dict(scheduling=scheduling, switches=list(switches.values()))
                    current[key] = item
                    observed.append((item, self.previous_threads.get(key)))
                except CounterUnavailable:
                    continue
            def aggregate(field, names):
                good = [(new[field], old[field] if old else None) for new, old in observed
                        if new[field] is not None and all(v is not None for v in new[field])]
                complete = bool(tasks) and not truncated and len(good) == len(tasks)
                continued = [(new, old) for new, old in good
                             if old is not None and all(a is not None and b is not None and a >= b
                                                       for a, b in zip(new, old))]
                return dict(status='available' if complete else 'partial' if good else 'unavailable',
                            measured_threads=len(good), continued_threads=len(continued),
                            totals={name: sum(new[i] for new, _ in good) if good else None
                                    for i, name in enumerate(names)},
                            deltas={name: sum(new[i] - old[i] for new, old in continued)
                                    if continued else None for i, name in enumerate(names)})
            row['scheduler'] = aggregate('scheduling', ('runtime_ns', 'wait_ns', 'timeslices'))
            row['context_switches'] = aggregate('switches', ('voluntary', 'involuntary'))
            row['threads_observed'] = len(observed)
            row['threads_truncated'] = truncated
            row['cgroup'] = self.cgroup(pid, cgroups)
            after = process_stat(path)
            if after['pid'] != pid or after['start_ticks'] != expected or after['state'] in ('Z', 'X', 'x'):
                raise CounterUnavailable('identity_changed_or_exited_during_sample')
            self.current_cpu[(pid, expected)] = before['cpu_ticks']
            self.current_threads.update(current)
            return row, len(tasks[:thread_budget])
        except CounterUnavailable as error:
            return dict(label=label, pid=pid, start_ticks=expected,
                        identity_status='unavailable', reason=str(error), cpu_ticks=None,
                        cpu_delta_ticks=None, scheduler=None, context_switches=None, cgroup=None), 0

    def sample(self):
        self.current_cpu, self.current_threads = {}, {}
        processes, cgroups, seen = [], {}, set()
        discovery = self.discovery if time.monotonic() - self.discovery_at < 1 else None
        discovery_scanned = 0
        discovery_truncated = self.discovery_truncated if discovery is not None else False
        pending = [(label, pid, start, label in self.trees) for label, pid, start in self.targets]
        remaining_threads = self.thread_limit
        truncated = False
        while pending and len(processes) < self.process_limit:
            label, pid, expected, tree = pending.pop(0)
            if (pid, expected) in seen:
                continue
            seen.add((pid, expected))
            row, used = self.process(label, pid, expected, cgroups, remaining_threads)
            remaining_threads -= used
            processes.append(row)
            if tree and row['identity_status'] == 'verified':
                try:
                    cached = self.child_cache.get((pid, expected))
                    if cached is not None and time.monotonic() - cached[0] < 1:
                        _, descendants, source, children_count = cached
                        room = max(0, self.process_limit - len(processes) - len(pending))
                        truncated |= children_count > room
                        pending.extend(descendants[:room])
                        row['descendant_source'] = source
                        continue
                    try:
                        children = read(self.root / str(pid) / 'task' / str(pid) / 'children', 4096).split()
                        row['descendant_source'] = 'children'
                    except CounterUnavailable as error:
                        # Some procfs implementations omit children. A bounded
                        # stat-only index is an alternative for that absence;
                        # permission restrictions are never worked around.
                        if str(error) != 'missing':
                            raise
                        if discovery is None:
                            discovery = {}
                            with os.scandir(self.root) as entries:
                                paths = list(itertools.islice((v.name for v in entries if v.name.isdigit()), 1025))
                            discovery_truncated = len(paths) > 1024
                            for name in paths[:1024]:
                                discovery_scanned += 1
                                try:
                                    candidate = process_stat(self.root / name)
                                    discovery[candidate['pid']] = candidate
                                except CounterUnavailable:
                                    pass
                            self.discovery = discovery
                            self.discovery_at = time.monotonic()
                            self.discovery_truncated = discovery_truncated
                        if discovery.get(pid, {}).get('start_ticks') != expected:
                            raise CounterUnavailable('discovery_identity_unavailable')
                        children = [str(p) for p, data in discovery.items() if data['parent_pid'] == pid]
                        row['descendant_source'] = 'bounded_stat_index'
                    room = self.process_limit - len(processes) - len(pending)
                    truncated |= len(children) > max(0, room)
                    descendants = []
                    for raw in children[:max(0, room)]:
                        child = process_stat(self.root / str(int(raw)))
                        if child['parent_pid'] == pid:
                            descendants.append((label, child['pid'], child['start_ticks'], True))
                    if process_stat(self.root / str(pid))['start_ticks'] != expected:
                        raise CounterUnavailable('identity_changed')
                    self.child_cache[(pid, expected)] = (time.monotonic(), descendants,
                                                        row['descendant_source'], len(children))
                    pending.extend(descendants)
                except (CounterUnavailable, OSError, ValueError) as error:
                    row['descendants_status'] = 'unavailable'
                    row['descendants_reason'] = str(error) if isinstance(error, CounterUnavailable) else 'discovery_io_error'
        self.previous_cpu, self.previous_threads = self.current_cpu, self.current_threads
        self.child_cache = {key: value for key, value in self.child_cache.items()
                            if key in self.current_cpu}
        referenced = set()
        for process in processes:
            membership = process.get('cgroup') or {}
            if membership.get('status') == 'available':
                referenced.update([membership['id'], *membership['ancestor_ids']])
        cgroups = {key: value for key, value in cgroups.items() if key in referenced}
        try:
            lines = read(self.root / 'stat', 65536).splitlines()
            fields = next(line.split()[1:] for line in lines if line.startswith('cpu '))
            keys = ('user', 'nice', 'system', 'idle', 'iowait', 'irq', 'softirq', 'steal')
            cpu = dict(status='available', ticks={key: int(value) for key, value in zip(keys, fields)})
            if len(cpu['ticks']) != len(keys) or min(cpu['ticks'].values()) < 0:
                raise ValueError()
        except (CounterUnavailable, StopIteration, ValueError) as error:
            cpu = dict(status='unavailable', ticks=None,
                       reason=str(error) if isinstance(error, CounterUnavailable) else 'invalid_cpu')
        return dict(processes=processes, cgroups=cgroups,
                    process_limit_reached=bool(pending) or truncated,
                    descendant_discovery=dict(stat_files_scanned=discovery_scanned,
                                              truncated=discovery_truncated,
                                              cache_age_s=(time.monotonic() - self.discovery_at)
                                              if self.discovery is not None else None),
                    namespace_cpu=cpu,
                    pressure_scope='observer_namespace_shared_not_process',
                    pressure={kind: pressure(self.root / 'pressure' / kind)
                              for kind in ('cpu', 'io', 'memory')})


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--process', action='append', default=[], type=target)
    parser.add_argument('--tree', action='append', default=[], type=target)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--summary', type=Path, required=True)
    parser.add_argument('--proc-root', type=Path, default=Path('/proc'))
    parser.add_argument('--duration-seconds', type=float, default=480)
    parser.add_argument('--interval-ms', type=int, default=100)
    parser.add_argument('--max-processes', type=int, default=64)
    parser.add_argument('--max-threads', type=int, default=128)
    parser.add_argument('--max-output-bytes', type=int, default=128 * 1024 * 1024)
    args = parser.parse_args()
    targets = args.process + args.tree
    if (not 1 <= len(targets) <= 12 or len({t[0] for t in targets}) != len(targets)
            or not .1 <= args.duration_seconds <= 900 or not 100 <= args.interval_ms <= 1000
            or not len(targets) <= args.max_processes <= 64 or not 1 <= args.max_threads <= 256
            or not 4096 <= args.max_output_bytes <= 128 * 1024 * 1024):
        parser.error('invalid bounded observer configuration')
    # Exclusive creation keeps prior evidence recoverable; no workload is signaled.
    with args.output.open('x') as output, args.summary.open('x') as summary_file:
        stopped = []
        for signum in (signal.SIGINT, signal.SIGTERM):
            signal.signal(signum, lambda value, _frame: stopped.append(value))
        observer = Observer(args.proc_root, targets, [t[0] for t in args.tree],
                            args.max_processes, args.max_threads)
        start, cpu_start = time.monotonic(), time.process_time()
        try:
            enabled = read(args.proc_root / 'sys/kernel/sched_schedstats', 64).strip()
            if enabled not in ('0', '1'):
                raise CounterUnavailable('invalid_setting')
            scheduler_accounting = dict(status='available', enabled=enabled == '1')
        except CounterUnavailable as error:
            scheduler_accounting = dict(status='unavailable', enabled=None, reason=str(error))
        interval = args.interval_ms / 1000
        count = total_bytes = missed = 0
        maximum_cost = total_cost = maximum_lateness = 0.0
        maximum_processes = maximum_threads = 0
        optional_unavailable = set()
        coverage = dict(process_limit_samples=0, thread_limit_samples=0,
                        discovery_limit_samples=0, identity_unavailable_samples=0,
                        descendants_unavailable_samples=0)
        stop_reason = 'duration'
        index = 0
        while time.monotonic() - start < args.duration_seconds:
            if stopped:
                stop_reason = 'signal'
                break
            due = start + index * interval
            now = time.monotonic()
            if now < due:
                time.sleep(min(due - now, interval))
                continue
            began = time.monotonic()
            began_unix_ms = time.time() * 1000
            lateness = began - due
            sample = observer.sample()
            cost = time.monotonic() - began
            sample.update(type='sample', sequence=count, at_unix_ms=began_unix_ms,
                          finished_unix_ms=time.time() * 1000,
                          monotonic_s=began, elapsed_s=began - start,
                          scheduled_elapsed_s=index * interval, lateness_ms=lateness * 1000,
                          collection_ms=cost * 1000)
            line = json.dumps(sample, separators=(',', ':'), allow_nan=False) + '\n'
            encoded = len(line.encode())
            if encoded > 65536 or total_bytes + encoded > args.max_output_bytes:
                stop_reason = 'output_limit'
                break
            output.write(line)
            output.flush()
            count += 1
            total_bytes += encoded
            total_cost += cost
            maximum_cost = max(maximum_cost, cost)
            maximum_lateness = max(maximum_lateness, lateness)
            maximum_processes = max(maximum_processes, len(sample['processes']))
            maximum_threads = max(maximum_threads, sum(p.get('threads_observed', 0) for p in sample['processes']))
            coverage['process_limit_samples'] += bool(sample['process_limit_reached'])
            coverage['thread_limit_samples'] += any(p.get('threads_truncated') for p in sample['processes'])
            coverage['discovery_limit_samples'] += bool(sample['descendant_discovery']['truncated'])
            coverage['identity_unavailable_samples'] += any(p['identity_status'] != 'verified' for p in sample['processes'])
            coverage['descendants_unavailable_samples'] += any(p.get('descendants_status') == 'unavailable' for p in sample['processes'])
            for kind, value in sample['pressure'].items():
                if value['status'] != 'available':
                    optional_unavailable.add(f'namespace_pressure_{kind}')
            if sample['namespace_cpu']['status'] != 'available':
                optional_unavailable.add('namespace_cpu')
            for group in sample['cgroups'].values():
                for name in ('cpu', 'cpu_max'):
                    if group[name]['status'] != 'available':
                        optional_unavailable.add(f'cgroup_{name}')
                for kind, value in group['pressure'].items():
                    if value['status'] != 'available':
                        optional_unavailable.add(f'cgroup_pressure_{kind}')
            for process in sample['processes']:
                for name in ('scheduler', 'context_switches', 'cgroup'):
                    if process.get(name) is None or process[name].get('status') != 'available':
                        optional_unavailable.add(name)
            if not any(p['identity_status'] == 'verified' for p in sample['processes']):
                stop_reason = 'targets_unavailable'
                break
            next_index = max(index + 1, math.floor((time.monotonic() - start) / interval) + 1)
            missed += next_index - index - 1
            index = next_index
        elapsed = time.monotonic() - start
        cpu = time.process_time() - cpu_start
        summary = dict(format='scheduler-probe-v1', read_only=True, proc_root=str(args.proc_root),
            clock_ticks_per_second=os.sysconf('SC_CLK_TCK'), scheduler_accounting=scheduler_accounting,
            stop_reason=stop_reason, signal=stopped[0] if stopped else None, samples=count,
            duration_s=elapsed, interval_ms=args.interval_ms, missed_sample_slots=missed,
            output_bytes=total_bytes, output_limit_bytes=args.max_output_bytes,
            observer_cpu_seconds=cpu, observer_average_cores=cpu / elapsed if elapsed else None,
            observer_peak_rss_kib=resource.getrusage(resource.RUSAGE_SELF).ru_maxrss,
            collection_mean_ms=total_cost * 1000 / count if count else None,
            collection_max_ms=maximum_cost * 1000, scheduling_lateness_max_ms=maximum_lateness * 1000,
            maximum_processes=maximum_processes, maximum_threads=maximum_threads,
            coverage=coverage,
            unavailable_or_partial_fields=sorted(optional_unavailable),
            limitations=['Only explicit PID/start identities and verified descendants are observed.',
                'Process CPU includes its threads; scheduler/context counters sum sampled surviving threads. Do not add these CPU measures together.',
                'Thread/process churn, truncation and missing counters make deltas incomplete; missing values are null, not zero.',
                'Scheduler counters may be disabled or virtualized even when files exist. Zero values alone do not prove no scheduling wait.',
                'Cgroup counters (leaf and up to seven ancestors) and namespace CPU/pressure are shared-scope. They cannot attribute contention to this workload. Ancestor truncation is explicit.',
                'Sampling latency includes observer scheduling and counter reads. It does not directly measure the application reactor.',
                'Output and duration are bounded. Missed intervals are skipped, never replayed as catch-up samples.',
                'Descendant discovery is cached for one second. Missing children files permit a stat-only index capped at 1024 processes. New descendants can be absent for one second. Restricted children is reported, never bypassed.',
                'No command lines, environments, credentials or file-descriptor targets are read.'])
        summary_file.write(json.dumps(summary, indent=2, allow_nan=False) + '\n')
        print(json.dumps(summary, allow_nan=False))
        return 0 if stop_reason in ('duration', 'signal') else 2


if __name__ == '__main__':
    raise SystemExit(main())
