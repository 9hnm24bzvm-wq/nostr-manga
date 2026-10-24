"""Finite qualification owner reusing the reviewed pidfd/subreaper lifecycle.

Importing this module never calls the sandbox-preflight browser entrypoint.
Each invocation owns one gated child and excludes preexisting child births.
"""
import ctypes
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import shutil
import signal
import sys
import time

LIFECYCLE_SOURCE = Path(__file__).resolve().parents[2] / 'scripts/ci/run-sandbox-preflight.py'
spec = importlib.util.spec_from_file_location('qualification_reused_lifecycle', LIFECYCLE_SOURCE)
lifecycle = importlib.util.module_from_spec(spec)
spec.loader.exec_module(lifecycle)
# This private module instance is not the browser entrypoint. Retain its 4 MiB
# pipe cap; the finite suite needs more births than the 90-second browser probe.
lifecycle.MAX_BIRTHS = 1024
MAX_SAMPLE_BYTES = 8 * 1024**2
MAX_EVIDENCE_GROWTH = 256 * 1024**2
LAUNCH_DISK = 4 * 1024**3 + 256 * 1024**2
LIVE_DISK = 4 * 1024**3 + 128 * 1024**2
MEMORY_FLOOR = 768 * 1024**2
TREE_RSS_LIMIT = 4 * 1024**3


def available_memory():
    values = [line.split() for line in Path('/proc/meminfo').read_text().splitlines()
              if line.startswith('MemAvailable:')]
    if len(values) != 1 or len(values[0]) != 3 or values[0][2] != 'kB':
        raise ValueError('Missing or malformed available-memory census')
    return int(values[0][1]) * 1024


def fail_walk(error):
    raise error


def evidence_size(roots):
    size = 0
    for root in roots:
        for parent, dirs, files in os.walk(root, followlinks=False, onerror=fail_walk):
            for name in dirs + files:
                path = Path(parent) / name
                try:
                    size += path.lstat().st_size
                except FileNotFoundError:
                    pass  # An owned generation may atomically rename a temporary file.
    return size


class QualificationOwner(lifecycle.Owner):
    def __init__(self, private, *, deadline, roots):
        if not 0 < deadline <= 600:
            raise ValueError('Qualification deadline must be in (0, 600] seconds')
        cleanup = min(25, deadline * .2)
        work = deadline - cleanup
        super().__init__(private, work_seconds=min(90, work), cleanup_seconds=cleanup,
                         total_seconds=min(120, deadline))
        # Qualification has a separately stated total bound, including cleanup.
        self.work_deadline = self.started + work
        self.total_deadline = self.started + deadline
        self.roots = roots
        self.initial_evidence_bytes = evidence_size(roots)
        self.samples = []
        self.sample_bytes = 0

    def resource_sample(self, rows):
        return {'elapsedSeconds': time.monotonic() - self.started,
                'freeBytes': shutil.disk_usage(self.private).free,
                'availableMemoryBytes': available_memory(),
                'evidenceBytes': evidence_size(self.roots),
                'owned': [{'pid': row['pid'], 'birth': row['birth'], 'ppid': row['ppid'],
                           'group': row['group'], 'session': row['session'], 'rssBytes': row['rss']}
                          for row in rows]}

    def observe(self):
        while True:
            self.safe_drain()
            rows = self.scan()
            if self.interrupted:
                return 'interrupted'
            if rows is None or self.errors or self.unknown:
                return 'owner-error'
            sample = self.resource_sample(rows)
            size = len(json.dumps(sample, separators=(',', ':')).encode())
            if self.sample_bytes + size > MAX_SAMPLE_BYTES:
                return 'sample-evidence-limit'
            self.samples.append(sample)
            self.sample_bytes += size
            if sample['freeBytes'] < LIVE_DISK:
                return 'disk-floor'
            if sample['availableMemoryBytes'] < MEMORY_FLOOR:
                return 'memory-floor'
            if sample['evidenceBytes'] - self.initial_evidence_bytes > MAX_EVIDENCE_GROWTH:
                return 'evidence-growth-limit'
            if sum(row['rss'] for row in rows) > TREE_RSS_LIMIT:
                return 'owned-memory-limit'
            if self.output_overflow:
                return 'output-limit'
            if time.monotonic() >= self.work_deadline:
                return 'deadline'
            if self.child.poll() is not None:
                return 'complete' if self.child.returncode == 0 else 'worker-failed'
            time.sleep(.05)


def run(command, cwd, env, out, name, deadline=600, evidence_roots=()):
    """Always persist failed/uncertain cleanup; never turn forced cleanup into pass."""
    private = out / (name + '-owner')
    private.mkdir(exist_ok=False)
    owner, outcome, raised = None, 'admission-refused', None
    cleanup = {'verified_stopped': False, 'forced': False, 'remaining_owned': None,
               'unknown_births': None, 'tracking_errors': None, 'stdout_eof': False}
    previous = {}
    try:
        if (sys.platform != 'linux' or not hasattr(os, 'pidfd_open') or
                not hasattr(signal, 'pidfd_send_signal')):
            raise RuntimeError('Linux pidfd lifecycle required')
        if ctypes.CDLL(None, use_errno=True).prctl(36, 1, 0, 0, 0) != 0:
            raise OSError(ctypes.get_errno(), 'Process-local subreaper required')
        if shutil.disk_usage(out).free < LAUNCH_DISK:
            raise ValueError('Insufficient 4 GiB + 256 MiB launch disk reserve')
        if available_memory() < MEMORY_FLOOR:
            raise ValueError('Insufficient 768 MiB available-memory launch reserve')
        owner = QualificationOwner(private, deadline=deadline, roots=[out, *evidence_roots])
        def interrupted(*_):
            owner.interrupted = True
        for signum in (signal.SIGINT, signal.SIGTERM):
            previous[signum] = signal.signal(signum, interrupted)
        owner.launch(command, cwd, env)
        outcome = owner.observe()
    except BaseException as error:
        raised = error
        outcome = 'interrupted' if isinstance(error, KeyboardInterrupt) else 'owner-error'
    finally:
        # Capture the initiating observation before cleanup can add failures or
        # replace the outward outcome with cleanup-unverified. Private only.
        pre_cleanup_outcome = outcome
        pre_cleanup_tracking_error_trace = list(owner.tracking_error_trace) if owner else []
        if owner is not None:
            try:
                cleanup = owner.cleanup()
            except BaseException as error:
                # The reused owner closes pidfds only after its terminal journal
                # attempt. Uncertain cleanup is never represented as no survivors.
                raised = raised or error
                outcome = 'cleanup-unverified'
        for signum, handler in previous.items():
            signal.signal(signum, handler)
        if owner and owner.output_overflow:
            outcome = 'output-limit'
        if not cleanup['verified_stopped']:
            outcome = 'cleanup-unverified'
        elif cleanup['forced'] and outcome == 'complete':
            outcome = 'forced-cleanup'
        code = owner.child.returncode if owner and owner.child else None
        passed = outcome == 'complete' and code == 0 and cleanup['verified_stopped'] and not cleanup['forced']
        report = {'command': command, 'cwd': str(cwd), 'exitCode': code,
                  'stopReason': None if passed else outcome,
                  'elapsedSeconds': time.monotonic() - owner.started if owner else None,
                  'ownedSurvivors': [] if cleanup['verified_stopped'] else None,
                  'cleanup': cleanup, 'samples': owner.samples if owner else [],
                  'status': 'passed' if passed else 'failed',
                  'preCleanupOutcome': pre_cleanup_outcome,
                  'preCleanupTrackingErrorTrace': pre_cleanup_tracking_error_trace,
                  'errorType': type(raised).__name__ if raised else None,
                  'lifecycleSource': {'path': str(LIFECYCLE_SOURCE),
                                      'sha256': hashlib.sha256(LIFECYCLE_SOURCE.read_bytes()).hexdigest()},
                  'limits': {'totalSeconds': deadline, 'cleanupSeconds': min(25, deadline * .2),
                             'logBytes': lifecycle.MAX_LOG, 'births': lifecycle.MAX_BIRTHS,
                             'sampleBytes': MAX_SAMPLE_BYTES, 'evidenceGrowthBytes': MAX_EVIDENCE_GROWTH}}
        log = private / 'worker.log'
        if log.exists():
            log.rename(out / (name + '.log'))
        (out / (name + '-resource-receipt.json')).write_text(json.dumps(report, indent=2) + '\n')
    if isinstance(raised, (KeyboardInterrupt, SystemExit)) or (owner and owner.interrupted):
        raise raised if raised else KeyboardInterrupt('Qualification interrupted')
    if raised or not cleanup['verified_stopped']:
        raise RuntimeError('Qualification owner failed; inspect ' + name + '-resource-receipt.json') from raised
    return report
