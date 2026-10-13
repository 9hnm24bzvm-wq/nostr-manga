"""Acceptance through the sampler CLI, real children and counter-file faults."""
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import time
import unittest

CLI = Path(__file__).with_name('scheduler_probe.py')


def identity(pid):
    fields = Path(f'/proc/{pid}/stat').read_text().rsplit(')', 1)[1].split()
    return int(fields[19])


class SamplerCli(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='scheduler-probe-test-')
        self.root = Path(self.temp.name)
        self.children = []
        self.addCleanup(self.temp.cleanup)
        self.addCleanup(self.stop_children)

    def child(self, code='import time; time.sleep(20)'):
        child = subprocess.Popen([sys.executable, '-c', code], stdout=subprocess.DEVNULL,
                                 stderr=subprocess.DEVNULL)
        self.children.append(child)
        return child

    def stop_children(self):
        for child in self.children:
            if child.poll() is None:
                child.terminate()
            try:
                child.wait(timeout=2)
            except subprocess.TimeoutExpired:
                child.kill()
                child.wait(timeout=2)

    def command(self, child, *options, start=None):
        start = identity(child.pid) if start is None else start
        return [sys.executable, str(CLI), '--process', f'worker:{child.pid}:{start}',
                '--output', str(self.root / 'samples.jsonl'),
                '--summary', str(self.root / 'summary.json'),
                '--duration-seconds', '.7', *options]

    def run_probe(self, child, *options, start=None):
        run = subprocess.run(self.command(child, *options, start=start),
                             capture_output=True, text=True, timeout=5)
        self.assertTrue((self.root / 'summary.json').exists(), run.stderr)
        summary = json.loads((self.root / 'summary.json').read_text())
        rows = [json.loads(line) for line in (self.root / 'samples.jsonl').read_text().splitlines()]
        return run, summary, rows

    def test_known_child_workload_is_observed_without_changing_child(self):
        child = self.child('import time\nend=time.monotonic()+2\nx=0\n'
                           'while time.monotonic()<end: x+=1\ntime.sleep(10)')
        run, summary, rows = self.run_probe(child)
        self.assertEqual(run.returncode, 0, run.stderr)
        self.assertEqual(summary['stop_reason'], 'duration')
        self.assertGreaterEqual(len(rows), 5)
        self.assertLessEqual(len(rows), 8)
        samples = [r['processes'][0] for r in rows]
        self.assertTrue(all(p['identity_status'] == 'verified' for p in samples))
        self.assertGreater(samples[-1]['cpu_ticks'], samples[0]['cpu_ticks'])
        self.assertTrue(all('scheduler' in p and 'context_switches' in p for p in samples))
        self.assertGreater(summary['observer_cpu_seconds'], 0)
        self.assertLess(summary['output_bytes'], 1024 * 1024)
        self.assertIsNone(child.poll(), 'Observation must not signal or reap the workload')

    def test_wrong_start_identity_never_samples_a_reused_pid(self):
        child = self.child()
        run, summary, rows = self.run_probe(child, start=identity(child.pid) + 1)
        self.assertEqual(run.returncode, 2)
        self.assertEqual(summary['stop_reason'], 'targets_unavailable')
        self.assertEqual(len(rows), 1)
        process = rows[0]['processes'][0]
        self.assertEqual(process['reason'], 'identity_changed')
        self.assertIsNone(process['cpu_ticks'])
        self.assertIsNone(child.poll())

    def test_exited_target_stops_with_explicit_missing_identity(self):
        child = self.child()
        start = identity(child.pid)
        child.terminate()
        child.wait(timeout=2)
        run, summary, rows = self.run_probe(child, start=start)
        self.assertEqual(run.returncode, 2)
        self.assertEqual(summary['stop_reason'], 'targets_unavailable')
        self.assertEqual(rows[0]['processes'][0]['reason'], 'missing')
        self.assertIsNone(rows[0]['processes'][0]['scheduler'])

    def test_signal_stops_sampler_promptly_without_signaling_workload(self):
        child = self.child()
        probe = subprocess.Popen(self.command(child, '--duration-seconds', '10'),
                                 stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
        self.children.append(probe)
        deadline = time.monotonic() + 2
        while time.monotonic() < deadline:
            if (self.root / 'samples.jsonl').exists() and (self.root / 'samples.jsonl').stat().st_size:
                break
            time.sleep(.01)
        began = time.monotonic()
        probe.send_signal(signal.SIGTERM)
        stdout, stderr = probe.communicate(timeout=2)
        self.assertEqual(probe.returncode, 0, stderr)
        self.assertLess(time.monotonic() - began, .8)
        self.assertEqual(json.loads(stdout)['stop_reason'], 'signal')
        self.assertIsNone(child.poll())

    def test_output_budget_stops_instead_of_growing_or_silently_sampling_less(self):
        child = self.child()
        run, summary, rows = self.run_probe(child, '--max-output-bytes', '4096',
                                             '--duration-seconds', '3')
        self.assertEqual(run.returncode, 2)
        self.assertEqual(summary['stop_reason'], 'output_limit')
        self.assertLessEqual((self.root / 'samples.jsonl').stat().st_size, 4096)
        self.assertEqual(summary['samples'], len(rows))
        self.assertLess(summary['duration_s'], 2)

    def test_missing_and_restricted_counter_files_are_unavailable_not_zero(self):
        child = self.child()
        capture = self.root / 'captured-proc'
        process = capture / str(child.pid)
        task = process / 'task' / str(child.pid)
        task.mkdir(parents=True)
        for path in (process, task):
            (path / 'stat').write_bytes(Path(f'/proc/{child.pid}/stat').read_bytes())
        restricted = task / 'status'
        restricted.write_text('voluntary_ctxt_switches: 77\nnonvoluntary_ctxt_switches: 88\n')
        restricted.chmod(0)
        self.addCleanup(restricted.chmod, 0o600)
        run, summary, rows = self.run_probe(child, '--proc-root', str(capture))
        self.assertEqual(run.returncode, 0, run.stderr)
        self.assertEqual(summary['proc_root'], str(capture))
        for row in rows:
            self.assertEqual(row['pressure']['cpu']['status'], 'unavailable')
            item = row['processes'][0]
            self.assertEqual(item['identity_status'], 'verified')
            self.assertEqual(item['scheduler']['status'], 'unavailable')
            self.assertIsNone(item['scheduler']['totals']['wait_ns'])
            self.assertEqual(item['context_switches']['status'], 'unavailable')
            self.assertIsNone(item['context_switches']['totals']['voluntary'])
            self.assertEqual(item['cgroup']['status'], 'unavailable')

    def test_tree_finds_real_descendant_without_inspecting_command_lines(self):
        ready = self.root / 'child.pid'
        code = ('import subprocess,sys,time,signal\nfrom pathlib import Path\n'
                'def stop(*_): raise SystemExit()\n'
                'signal.signal(signal.SIGTERM,stop)\n'
                'p=subprocess.Popen([sys.executable,"-c","import time;time.sleep(10)"])\n'
                f'Path({str(ready)!r}).write_text(str(p.pid))\n'
                'try: time.sleep(20)\nfinally: p.terminate();p.wait()')
        parent = self.child(code)
        deadline = time.monotonic() + 2
        while not ready.exists() and time.monotonic() < deadline:
            time.sleep(.01)
        descendant = int(ready.read_text())
        command = self.command(parent)
        command[command.index('--process')] = '--tree'
        run = subprocess.run(command, capture_output=True, text=True, timeout=5)
        self.assertEqual(run.returncode, 0, run.stderr)
        rows = [json.loads(line) for line in (self.root / 'samples.jsonl').read_text().splitlines()]
        expected = {(parent.pid, identity(parent.pid)), (descendant, identity(descendant))}
        self.assertTrue(all({(p['pid'], p['start_ticks']) for p in row['processes']} == expected
                            for row in rows), rows)

    def test_thread_limit_marks_partial_observation(self):
        ready = self.root / 'ready'
        child = self.child('import threading,time\nfrom pathlib import Path\n'
                           'threading.Thread(target=lambda:time.sleep(10),daemon=True).start()\n'
                           f'Path({str(ready)!r}).touch()\ntime.sleep(20)')
        deadline = time.monotonic() + 2
        while not ready.exists() and time.monotonic() < deadline:
            time.sleep(.01)
        run, summary, rows = self.run_probe(child, '--max-threads', '1')
        self.assertEqual(run.returncode, 0, run.stderr)
        self.assertEqual(summary['maximum_threads'], 1)
        self.assertEqual(summary['coverage']['thread_limit_samples'], len(rows))
        self.assertTrue(all(row['processes'][0]['threads_truncated'] for row in rows))
        self.assertTrue(all(row['processes'][0]['scheduler']['status'] == 'partial' for row in rows))

    def test_saved_counter_tree_reports_leaf_and_ancestor_throttling_with_missing_pressure(self):
        child = self.child()
        capture = self.root / 'captured-proc'
        process = capture / str(child.pid)
        task = process / 'task' / str(child.pid)
        task.mkdir(parents=True)
        for path in (process, task):
            (path / 'stat').write_bytes(Path(f'/proc/{child.pid}/stat').read_bytes())
        (task / 'schedstat').write_text('100 20 3\n')
        (task / 'status').write_text('voluntary_ctxt_switches: 5\nnonvoluntary_ctxt_switches: 7\n')
        groups = self.root / 'captured-cgroup'
        leaf = groups / 'tenant' / 'worker'
        leaf.mkdir(parents=True)
        for index, path in enumerate((leaf, leaf.parent, groups)):
            (path / 'cpu.stat').write_text(f'usage_usec 12\nuser_usec 9\nsystem_usec 3\nnr_periods 20\nnr_throttled {index}\nthrottled_usec {index * 500}\n')
            (path / 'cpu.max').write_text('50000 100000\n')
            (path / 'cpu.pressure').write_text('some avg10=0.2 avg60=0.1 avg300=0.0 total=19\n')
        (capture / 'self').mkdir()
        (capture / 'self' / 'mountinfo').write_text(f'10 1 0:1 / {groups} rw - cgroup2 cgroup rw\n')
        (process / 'cgroup').write_text('0::/tenant/worker\n')
        run, summary, rows = self.run_probe(child, '--proc-root', str(capture))
        self.assertEqual(run.returncode, 0, run.stderr)
        for row in rows:
            membership = row['processes'][0]['cgroup']
            self.assertTrue(membership['ancestors_complete'])
            ordered = [membership['id'], *membership['ancestor_ids']]
            self.assertEqual(len(ordered), 3)
            self.assertEqual([row['cgroups'][key]['cpu']['values']['throttled_usec']
                              for key in ordered], [0, 500, 1000])
            group = row['cgroups'][ordered[0]]
            self.assertEqual(group['cpu_max']['quota_usec'], 50000)
            self.assertEqual(group['pressure']['cpu']['some']['total'], 19)
            self.assertEqual(group['pressure']['io']['status'], 'unavailable')
        self.assertIn('cgroup_pressure_io', summary['unavailable_or_partial_fields'])


if __name__ == '__main__':
    unittest.main()
