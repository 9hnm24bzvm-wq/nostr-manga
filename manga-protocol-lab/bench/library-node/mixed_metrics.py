"""Scoped /proc diagnostics; never treats namespace network bytes as per-process."""
import math
import os
import time
from collections import Counter
from pathlib import Path
from process_metrics import parse_stat


class Histogram:
    """Fixed-range logarithmic bins; percentile upper bounds within ~1%."""
    def __init__(self):
        self.bins = Counter()
        self.n = 0
        self.maximum = 0.0

    def add(self, milliseconds):
        value = max(0.0, milliseconds)
        key = min(3000, max(0, math.ceil(math.log1p(value) / math.log(1.01))))
        self.bins[key] += 1
        self.n += 1
        self.maximum = max(self.maximum, value)

    def export(self):
        return dict(format='log1p-1.01-v1', n=self.n, maximum_ms=self.maximum,
                    bins=sorted(self.bins.items()))

    def merge(self, value):
        if value.get('format') != 'log1p-1.01-v1': raise ValueError('histogram format')
        bins={int(k):int(v) for k,v in value['bins']}
        if any(k<0 or k>3000 or v<0 for k,v in bins.items()) or sum(bins.values())!=value['n']:
            raise ValueError('histogram count integrity')
        self.bins.update(bins); self.n+=value['n']; self.maximum=max(self.maximum,value['maximum_ms'])

    def result(self):
        def percentile(fraction):
            threshold = max(1, math.ceil(self.n * fraction))
            seen = 0
            for key, count in sorted(self.bins.items()):
                seen += count
                if seen >= threshold:
                    return math.expm1(key * math.log(1.01))
            return None
        return dict(n=self.n, p50_ms=percentile(.50), p95_ms=percentile(.95),
                    p99_ms=percentile(.99), max_ms=self.maximum,
                    method="logarithmic-bin upper bounds, approximately1% resolution")


class ScopedSampler:
    def __init__(self, roots):
        self.roots = {}
        self.root_starts = {}
        self.register_roots(roots)
        self.previous = {}
        self.at = None
        self.ticks = os.sysconf('SC_CLK_TCK')
        self.page_kib = os.sysconf('SC_PAGE_SIZE') // 1024

    def register_roots(self, roots):
        for group,pid in roots.items():
            pid=int(pid); self.roots[pid]=group
            try: self.root_starts[pid] = parse_stat(Path(f"/proc/{pid}/stat").read_text())["start_ticks"]
            except (OSError, ValueError, IndexError): pass

    def sample(self):
        now = time.monotonic()
        elapsed = now - self.at if self.at else None
        rows = {}
        skipped = 0
        for path in Path('/proc').glob('[0-9]*/stat'):
            try:
                text = path.read_text()
                row = parse_stat(text, self.page_kib)
                fields = text[text.rindex(')') + 1:].split()
                row['ppid'] = int(fields[1])
                row['reaped_child_cpu_ticks'] = int(fields[13]) + int(fields[14])
                rows[row['pid']] = row
            except (OSError, ValueError, IndexError):
                skipped += 1
        def group_for(pid):
            lineage = set()
            cursor = pid
            while cursor in rows and cursor not in lineage:
                if cursor in self.roots:
                    if self.root_starts.get(cursor) != rows[cursor]['start_ticks']: return None
                    group = self.roots[cursor]
                    name = rows[pid]['name']
                    if group.startswith('node') and name in ('nginx', 'strfry'):
                        return group + '/' + name
                    if group.startswith('node') and name.startswith('python'):
                        return group + '/serving_helper'
                    return group
                lineage.add(cursor)
                cursor = rows[cursor]['ppid']
            return None
        current, processes, groups = {}, [], {}
        for pid, row in rows.items():
            group = group_for(pid)
            if group is None:
                continue
            key = (pid, row['start_ticks'])
            try:
                io = {line.split(':')[0]: int(line.split(':')[1])
                      for line in Path(f'/proc/{pid}/io').read_text().splitlines()}
            except (OSError, ValueError):
                io = None
            old = self.previous.get(key)
            cpu = ((row['cpu_ticks'] - old['cpu_ticks']) / self.ticks / elapsed
                   if elapsed and old else None)
            delta = ({name: max(0, value - old['io'].get(name, 0)) for name, value in io.items()}
                     if io is not None and old and old['io'] is not None else None)
            child_cpu = ((row['reaped_child_cpu_ticks'] - old['reaped_child_cpu_ticks']) / self.ticks / elapsed if elapsed and old else None)
            current[key] = dict(cpu_ticks=row['cpu_ticks'], reaped_child_cpu_ticks=row['reaped_child_cpu_ticks'], io=io)
            processes.append(dict(pid=pid, start_ticks=row['start_ticks'], group=group,
                                  cpu_cores_observed=cpu, reaped_child_cpu_cores=child_cpu, rss_kib=row['rss_kib'], io_delta=delta))
            total = groups.setdefault(group, dict(process_count=0, rss_sum_kib=0, cpu_cores_observed=0.0))
            total['process_count'] += 1
            total['rss_sum_kib'] += row['rss_kib']
            total['cpu_cores_observed'] += cpu or 0
        network = {}
        try:
            for line in Path('/proc/net/dev').read_text().splitlines()[2:]:
                interface, values = line.split(':'); values = values.split()
                network[interface.strip()] = dict(rx_bytes=int(values[0]), tx_bytes=int(values[8]))
        except (OSError, ValueError, IndexError):
            network = None
        self.previous, self.at = current, now
        return dict(at=time.time(), interval_s=elapsed, processes=processes, groups=groups,
                    unrelated_or_exited_stat_reads=skipped, namespace_network_cumulative=network,
                    caveats="Own CPU misses short-lived processes; separate reaped-child CPU recovers waited children but overlaps earlier live-child observations and must not be summed with them; RSS sums double-count shared pages; I/O may be unavailable; network is namespace-wide, not attributed to these processes")
