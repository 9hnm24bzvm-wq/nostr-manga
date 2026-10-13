"""Bounded benchmark diagnostics for known local processes, without command lines.

CPU is an observed lower bound: a process that starts and exits between samples
is absent. Summed RSS double-counts shared mappings and is not host memory use.
"""
import os
import time
from pathlib import Path

NAMES = {'manga-node': 'node', 'postgres': 'postgres', 'strfry': 'strfry', 'python3': 'generator'}


def parse_stat(text, page_kib=4):
    left, right = text.index('('), text.rindex(')')
    pid = int(text[:left].strip())
    name = text[left + 1:right]
    fields = text[right + 1:].split()
    return {'pid': pid, 'name': name, 'start_ticks': int(fields[19]),
            'cpu_ticks': int(fields[11]) + int(fields[12]),
            'rss_kib': max(0, int(fields[21])) * page_kib}


class ProcessSampler:
    def __init__(self, root='/proc', clock=time.monotonic, ticks=None, page_kib=None):
        self.root = Path(root)
        self.clock = clock
        self.ticks = ticks or os.sysconf('SC_CLK_TCK')
        self.page_kib = page_kib or os.sysconf('SC_PAGE_SIZE') // 1024
        self.previous = {}
        self.at = None

    def sample(self):
        now = self.clock()
        elapsed = None if self.at is None else now - self.at
        current, groups, skipped = {}, {}, 0
        for path in self.root.glob('[0-9]*/stat'):
            try:
                row = parse_stat(path.read_text(), self.page_kib)
            except (OSError, ValueError, IndexError):
                skipped += 1
                continue
            group = NAMES.get(row['name'])
            if group is None:
                continue
            key = (row['pid'], row['start_ticks'])
            current[key] = row['cpu_ticks']
            data = groups.setdefault(group, {'process_count': 0, 'rss_sum_kib': 0,
                                             'cpu_cores_observed': None if elapsed is None or elapsed <= 0 else 0.0})
            data['process_count'] += 1
            data['rss_sum_kib'] += row['rss_kib']
            if elapsed is not None and elapsed > 0 and key in self.previous:
                data['cpu_cores_observed'] += max(0, row['cpu_ticks'] - self.previous[key]) / self.ticks / elapsed
        self.previous, self.at = current, now
        return {'processes': groups, 'process_stat_reads_skipped': skipped}
