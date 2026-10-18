#!/usr/bin/env python3
"""Own one sandbox probe; publish only closed, secret-free observations.

Python 3.10 standard library only. No browser, fixture or package is imported.
The CLI admits only the adjacent fixed Node worker and explicit hosted runtime.
"""
import argparse
import ctypes
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import selectors
import signal
import stat
import subprocess
import sys
import time

try:
    _diagnostic_spec = importlib.util.spec_from_file_location('sandbox_closed_diagnostic', Path(__file__).with_name('sandbox_launch_diagnostics.py'))
    diagnostic = importlib.util.module_from_spec(_diagnostic_spec)
    _diagnostic_spec.loader.exec_module(diagnostic)
except Exception:
    raise SystemExit('Sandbox diagnostic module refused')

ROOT = Path(__file__).resolve().parents[2]
MAX_JSON = 65536
MAX_LOG = 4 * 1024 * 1024
MAX_BIRTHS = 256
MAX_RSS = 1536 * 1024 * 1024
STAGES = frozenset({'initializing', 'launching-browser', 'browser-launched',
                   'context-created', 'page-created', 'script-verified',
                   'closing-context', 'closing-browser', 'complete'})
CATEGORIES = frozenset({'none', 'sandbox-refused', 'launch-failed', 'context-failed',
                       'page-failed', 'script-failed', 'cleanup-failed', 'worker-failed'})
FLAGS = ('browser_created', 'context_created', 'page_created', 'script_verified',
         'context_closed', 'browser_closed', 'completed')
WORKER_KEYS = {'schema', 'stage', 'caught_stage', 'caught_category', 'launch_diagnostic', *FLAGS}
OUTCOMES = frozenset({'complete', 'admission-refused', 'worker-failed', 'record-invalid',
                     'deadline', 'output-limit', 'interrupted', 'owner-error',
                     'cleanup-unverified'})


class Refused(RuntimeError):
    """Fixed refusal: never retain a private exception or path in public JSON."""


def unique_object(pairs):
    value = {}
    for key, item in pairs:
        if key in value:
            raise Refused('duplicate-json-key')
        value[key] = item
    return value


def read_bytes(path, limit=MAX_JSON):
    """No symlink, nonregular input, duplicate key, oversized or partial read."""
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW)
    try:
        before = os.fstat(fd)
        if not stat.S_ISREG(before.st_mode) or before.st_size > limit:
            raise Refused('invalid-json-file')
        with os.fdopen(fd, 'rb', closefd=False) as stream:
            data = stream.read(limit + 1)
        after = os.fstat(fd)
        if len(data) > limit or before.st_size != len(data) or after.st_size != len(data):
            raise Refused('changed-json-file')
        return data
    finally:
        os.close(fd)


def read_json(path, limit=MAX_JSON):
    return json.loads(read_bytes(path, limit).decode('utf-8'), object_pairs_hook=unique_object)


def terminal_outcome(outcome, worker, cleanup, output_overflow):
    if not cleanup['verified_stopped'] or cleanup['forced']:
        return 'cleanup-unverified'
    if output_overflow:
        return 'output-limit'
    if outcome == 'complete' and not worker_valid(worker):
        return 'record-invalid'
    return outcome


def worker_valid(value):
    if type(value) is not dict or set(value) != WORKER_KEYS:
        return False
    if type(value['schema']) is not int or value['schema'] != 2:
        return False
    if (type(value['stage']) is not str or value['stage'] not in STAGES or
            type(value['caught_stage']) is not str or value['caught_stage'] not in STAGES | {'none'} or
            type(value['caught_category']) is not str or value['caught_category'] not in CATEGORIES or
            any(type(value[key]) is not bool for key in FLAGS)):
        return False
    site, category = value['caught_stage'], value['caught_category']
    if not diagnostic.worker_valid(value['launch_diagnostic'], site == 'launching-browser'):
        return False
    if site == 'launching-browser' and ((category == 'sandbox-refused') != (value['launch_diagnostic']['error']['signatures']['sandbox'] is True)):
        return False
    expected = {'none': {'none'}, 'launching-browser': {'sandbox-refused', 'launch-failed'},
                'browser-launched': {'context-failed'}, 'context-created': {'page-failed'},
                'page-created': {'script-failed'}, 'closing-context': {'cleanup-failed'},
                'closing-browser': {'cleanup-failed'}}
    if category not in expected.get(site, {'worker-failed'}):
        return False
    implications = [('context_created', 'browser_created'), ('page_created', 'context_created'),
                    ('script_verified', 'page_created'), ('context_closed', 'context_created'),
                    ('browser_closed', 'browser_created')]
    if any(value[a] and not value[b] for a, b in implications):
        return False
    # These sites precede the corresponding successfully returned handle.
    absent = {'initializing': ('browser_created',), 'launching-browser': ('browser_created',),
              'browser-launched': ('context_created',), 'context-created': ('page_created',),
              'page-created': ('script_verified',), 'closing-context': ('context_closed',),
              'closing-browser': ('browser_closed',)}
    if any(value[key] for key in absent.get(site, ())):
        return False
    present = {'browser-launched': ('browser_created',),
               'context-created': ('context_created',), 'page-created': ('page_created',),
               'script-verified': ('script_verified',), 'closing-context': ('context_created',),
               'closing-browser': ('browser_created',)}
    if any(not value[key] for key in present.get(site, ())):
        return False
    if value['completed'] and (value['stage'] != 'complete' or site != 'none' or
                               not all(value[key] for key in FLAGS)):
        return False
    return True


def public_projection(worker, exit_code, cleanup, outcome, elapsed_ms, executable_sha256=None, launch_log=None):
    cleanup = cleanup if type(cleanup) is dict else {}
    valid = worker_valid(worker)
    code = exit_code if type(exit_code) is int and -64 <= exit_code <= 255 else None
    clean = (type(cleanup) is dict and cleanup.get('verified_stopped') is True and
             cleanup.get('forced') is False and all(type(cleanup.get(key)) is int and
             cleanup[key] == 0 for key in ('remaining_owned', 'unknown_births', 'tracking_errors')))
    launch_log = launch_log if diagnostic.valid_log(launch_log) else diagnostic.empty_log()
    worker_diagnostic = worker['launch_diagnostic'] if valid else diagnostic.empty_worker()
    result = {'schema': 2, 'scope': 'sandbox-launch-preflight',
              'launch_diagnostic': {**worker_diagnostic, **launch_log},
              'status': 'failed', 'parent_outcome': outcome if type(outcome) is str and outcome in OUTCOMES else 'owner-error',
              'worker_record_valid': valid, 'worker_exit_code': code,
              'elapsed_ms': elapsed_ms if type(elapsed_ms) is int and 0 <= elapsed_ms <= 120000 else None,
              'playwright': '1.62.1', 'chromium_revision': '1234', 'sandbox_requested': True,
              'executable_sha256': executable_sha256 if type(executable_sha256) is str and
              len(executable_sha256) == 64 and all(c in '0123456789abcdef' for c in executable_sha256) else None,
              'stage': worker['stage'] if valid else 'unavailable',
              'caught_stage': worker['caught_stage'] if valid else 'unavailable',
              'caught_category': worker['caught_category'] if valid else 'unavailable',
              **{key: worker[key] if valid else None for key in FLAGS},
              'cleanup': {'verified_stopped': clean, 'forced': cleanup.get('forced') is True,
                          **{key: cleanup.get(key) if type(cleanup.get(key)) is int and
                             0 <= cleanup[key] <= MAX_BIRTHS else None
                             for key in ('remaining_owned', 'unknown_births', 'tracking_errors')}},
              'raw_messages_exported': False, 'private_values_omitted': True,
              'application_journey_qualified': False}
    if (valid and worker['completed'] and code == 0 and clean and outcome == 'complete' and
            result['elapsed_ms'] is not None and result['executable_sha256'] is not None):
        result['status'] = 'passed'
    return result


def validate_public(value):
    """Reject unknown/private fields even on a failed record; no normalization."""
    expected = public_projection(None, None, {}, 'owner-error', None)
    if type(value) is not dict or set(value) != set(expected):
        raise Refused('invalid-public-schema')
    for key in ('schema', 'scope', 'playwright', 'chromium_revision', 'sandbox_requested',
                'raw_messages_exported', 'private_values_omitted', 'application_journey_qualified'):
        if type(value[key]) is not type(expected[key]) or value[key] != expected[key]:
            raise Refused('invalid-public-constant')
    if type(value['worker_record_valid']) is not bool or value['status'] not in ('passed', 'failed'):
        raise Refused('invalid-public-status')
    cleanup = value['cleanup']
    if type(cleanup) is not dict or set(cleanup) != set(expected['cleanup']):
        raise Refused('invalid-public-cleanup')
    if any(type(cleanup[key]) is not bool for key in ('verified_stopped', 'forced')):
        raise Refused('invalid-public-cleanup')
    for key in ('remaining_owned', 'unknown_births', 'tracking_errors'):
        if cleanup[key] is not None and (type(cleanup[key]) is not int or not 0 <= cleanup[key] <= MAX_BIRTHS):
            raise Refused('invalid-public-cleanup')
    if cleanup['verified_stopped'] and any(cleanup[key] != 0 for key in
                                         ('remaining_owned', 'unknown_births', 'tracking_errors')):
        raise Refused('invalid-public-cleanup')
    details = value['launch_diagnostic']
    if type(details) is not dict or set(details) != {'error', 'host', 'process', 'log_signatures'}:
        raise Refused('invalid-public-diagnostic')
    launch_log = {key: details[key] for key in ('process', 'log_signatures')}
    if not diagnostic.valid_log(launch_log):
        raise Refused('invalid-public-diagnostic')
    worker = {'schema': 2, 'launch_diagnostic': {key: details[key] for key in ('error', 'host')}, **{key: value[key] for key in ('stage', 'caught_stage', 'caught_category', *FLAGS)}}
    reconstructed = public_projection(worker if value['worker_record_valid'] else None,
                                      value['worker_exit_code'], cleanup, value['parent_outcome'],
                                      value['elapsed_ms'], value['executable_sha256'], launch_log)
    if reconstructed != value:
        raise Refused('invalid-public-association')
    return value


def process(pid):
    try:
        text = Path('/proc/{}/stat'.format(pid)).read_text()
    except FileNotFoundError:
        return None
    fields = text.rsplit(')', 1)[1].split()
    return {'pid': pid, 'state': fields[0], 'ppid': int(fields[1]),
            'group': int(fields[2]), 'session': int(fields[3]), 'birth': int(fields[19]),
            'rss': int(fields[21]) * os.sysconf('SC_PAGE_SIZE')}


class Owner:
    """Dedicated subreaper; signals only pidfds registered to exact birth IDs."""
    def __init__(self, private, *, work_seconds=90, cleanup_seconds=25, total_seconds=120, started=None):
        if not (0 < work_seconds <= 90 and 0 < cleanup_seconds <= 25 and
                work_seconds + cleanup_seconds <= total_seconds <= 120):
            raise Refused('invalid-owner-bounds')
        self.private = private
        self.started = time.monotonic() if started is None else started
        self.work_deadline = self.started + work_seconds
        self.total_deadline = self.started + total_seconds
        self.cleanup_seconds = cleanup_seconds
        self.ledger = {}
        self.unknown = set()
        self.errors = 0
        self.forced = False
        self.interrupted = False
        self.child = None
        self.selector = selectors.DefaultSelector()
        self.log_bytes = 0
        self.output_overflow = False
        self.stdout_eof = False
        self.baseline = {(row['pid'], row['birth']) for row in self._rows()
                         if row['ppid'] == os.getpid()}

    def _rows(self):
        rows = []
        for entry in Path('/proc').iterdir():
            if entry.name.isdigit():
                row = process(int(entry.name))
                if row is not None:
                    rows.append(row)
        return rows

    def register(self, row):
        key = (row['pid'], row['birth'])
        if key in self.ledger:
            return
        if len(self.ledger) >= MAX_BIRTHS:
            self.unknown.add(key)
            raise Refused('birth-ledger-limit')
        try:
            fd = os.pidfd_open(row['pid'], 0)
            again = process(row['pid'])
            if again is None or again['birth'] != row['birth']:
                os.close(fd)
                self.unknown.add(key)
                return
            self.ledger[key] = fd
        except ProcessLookupError:
            self.unknown.add(key)

    def scan(self):
        try:
            rows = self._rows()
            known = {row['pid']: row for row in rows if (row['pid'], row['birth']) in self.ledger}
            session_alive = self.child is not None and any(row['session'] == self.child.pid for row in known.values())
            owned = {row['pid']: row for row in rows if
                     (row['pid'], row['birth']) in self.ledger or
                     (row['ppid'] == os.getpid() and (row['pid'], row['birth']) not in self.baseline) or
                     (session_alive and row['session'] == self.child.pid)}
            changed = True
            while changed:
                changed = False
                for row in rows:
                    if row['pid'] not in owned and row['ppid'] in owned:
                        owned[row['pid']] = row
                        changed = True
            for row in owned.values():
                self.register(row)
            # Reap only this subreaper's exact captured adopted zombies.
            for row in owned.values():
                if row['state'] == 'Z' and row['ppid'] == os.getpid() and self.child and row['pid'] != self.child.pid:
                    try:
                        os.waitpid(row['pid'], os.WNOHANG)
                    except ChildProcessError:
                        self.errors += 1
            return [row for row in owned.values() if row['state'] != 'Z']
        except Exception:
            self.errors += 1
            return None

    def signal_known(self, signum):
        self.scan()  # Census failure must not suppress captured-handle cleanup.
        self.forced = True
        for fd in tuple(self.ledger.values()):
            try:
                signal.pidfd_send_signal(fd, signum, None, 0)
            except ProcessLookupError:
                pass
            except OSError:
                self.errors += 1

    def drain(self):
        for key, _ in self.selector.select(0):
            data = os.read(key.fd, 4096)
            if not data:
                self.selector.unregister(key.fd)
                self.stdout_eof = True
                continue
            available = MAX_LOG - self.log_bytes
            self.log.write(data[:available])
            self.log_bytes += min(len(data), available)
            if len(data) > available:
                self.output_overflow = True

    def safe_drain(self):
        try:
            self.drain()
        except Exception:
            self.errors += 1

    def launch(self, argv, cwd, env):
        gate_read, gate_write = os.pipe()
        launcher = ('import os,sys; f=int(sys.argv[1]); g=os.read(f,1); os.close(f); '
                    'os._exit(73) if g!=b"G" else None; '
                    'os.execvpe(sys.argv[2],sys.argv[2:],os.environ)')
        self.log = (self.private / 'worker.log').open('xb')
        try:
            self.child = subprocess.Popen([sys.executable, '-I', '-c', launcher, str(gate_read), *argv],
                                          cwd=cwd, env=env, stdin=subprocess.DEVNULL,
                                          stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                                          start_new_session=True, pass_fds=(gate_read,))
            os.close(gate_read)
            gate_read = None
            row = process(self.child.pid)
            if (row is None or row['ppid'] != os.getpid() or
                    row['session'] != self.child.pid or row['group'] != self.child.pid):
                raise Refused('unregistered-worker')
            self.register(row)
            if (row['pid'], row['birth']) not in self.ledger or self.scan() is None:
                raise Refused('unregistered-worker')
            os.set_blocking(self.child.stdout.fileno(), False)
            self.selector.register(self.child.stdout, selectors.EVENT_READ)
            os.write(gate_write, b'G')
        finally:
            if gate_read is not None:
                os.close(gate_read)
            os.close(gate_write)

    def observe(self):
        while True:
            self.safe_drain()
            rows = self.scan()
            if self.interrupted:
                return 'interrupted'
            if rows is None or self.errors or self.unknown:
                return 'owner-error'
            if sum(row['rss'] for row in rows) > MAX_RSS:
                return 'owner-error'
            if self.output_overflow:
                return 'output-limit'
            if time.monotonic() >= self.work_deadline:
                return 'deadline'
            if self.child.poll() is not None:
                return 'complete' if self.child.returncode == 0 else 'worker-failed'
            time.sleep(.02)

    def cleanup(self):
        deadline = min(self.total_deadline, time.monotonic() + self.cleanup_seconds)
        # Allow only an already-exited worker's closing descendants a short grace.
        grace = min(deadline, time.monotonic() + (min(1, self.cleanup_seconds * .1)
                    if self.child and self.child.poll() is not None else 0))
        live = self.scan()
        while live and time.monotonic() < grace:
            self.safe_drain()
            time.sleep(.02)
            live = self.scan()
        if live is None or live or (self.child and self.child.poll() is None):
            self.signal_known(signal.SIGTERM)
        term = min(deadline, time.monotonic() + self.cleanup_seconds * .4)
        while time.monotonic() < term:
            self.safe_drain()
            live = self.scan()
            if live == [] and (self.child is None or self.child.poll() is not None):
                break
            time.sleep(.02)
        live = self.scan()
        if live is None or live or (self.child and self.child.poll() is None):
            self.signal_known(signal.SIGKILL)
        while time.monotonic() < deadline:
            self.safe_drain()
            live = self.scan()
            if live == [] and (self.child is None or self.child.poll() is not None):
                break
            time.sleep(.02)
        # A stopped child can leave more than a few chunks in its pipe. Observe
        # every buffered byte (retaining only MAX_LOG), and require actual EOF.
        while self.selector.get_map() and time.monotonic() < deadline:
            self.safe_drain()
            if self.selector.get_map():
                time.sleep(min(.02, max(0, deadline - time.monotonic())))
        if self.child and not self.stdout_eof:
            self.errors += 1
        live = self.scan()
        if self.child:
            self.child.poll()
        result = {'verified_stopped': live == [] and not self.unknown and not self.errors and
                  (self.child is None or self.child.returncode is not None),
                  'forced': self.forced, 'remaining_owned': len(live) if live is not None else MAX_BIRTHS,
                  'unknown_births': min(len(self.unknown), MAX_BIRTHS),
                  'tracking_errors': min(self.errors, MAX_BIRTHS),
                  'stdout_eof': self.stdout_eof or self.child is None}
        # Keep ownership handles until terminal private facts have been persisted.
        try:
            journal = dict(result, births=[{'pid': p, 'birth': b} for p, b in self.ledger])
            (self.private / 'owner.json').write_text(json.dumps(journal, sort_keys=True) + '\n')
        finally:
            self.selector.close()
            if self.child and self.child.stdout:
                self.child.stdout.close()
            if hasattr(self, 'log'):
                self.log.close()
            for fd in self.ledger.values():
                os.close(fd)
        return result


def child_environment(source, private):
    names = ('PATH', 'HOME', 'LANG', 'LC_ALL', 'LC_CTYPE', 'SSL_CERT_FILE', 'SSL_CERT_DIR')
    value = {key: source[key] for key in names if key in source and type(source[key]) is str and
             '\0' not in source[key] and len(source[key]) <= 4096}
    value.update(TMPDIR=str(private), NODE_OPTIONS='--max-old-space-size=512',
                 DEBUG='pw:browser', DEBUG_COLORS='0', DEBUG_HIDE_DATE='1')
    return value


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--node', type=Path)
    parser.add_argument('--validate', action='store_true')
    parser.add_argument('--expected-sha256')
    args = parser.parse_args()
    private = ROOT / '.build/sandbox-preflight'
    public = ROOT / 'ci-evidence/sandbox-preflight.json'
    if public.parent.resolve() != public.parent or private.parent.resolve() != private.parent:
        print('Fixed preflight output parent refused', file=sys.stderr)
        return 1
    owner = None
    worker = None
    code = None
    outcome = 'admission-refused'
    if args.validate:
        if args.node is not None:
            parser.error('Validation starts no worker and accepts no Node argument')
        if (args.expected_sha256 is None or len(args.expected_sha256) != 64 or
                any(c not in '0123456789abcdef' for c in args.expected_sha256)):
            parser.error('Validation requires the current invocation evidence SHA256')
        try:
            raw = read_bytes(public)
            if hashlib.sha256(raw).hexdigest() != args.expected_sha256:
                raise Refused('public-evidence-hash-mismatch')
            validate_public(json.loads(raw.decode('utf-8'), object_pairs_hook=unique_object))
        except Exception:
            print('Public preflight record refused', file=sys.stderr)
            return 1
        return 0
    if args.node is None:
        parser.error('An explicit Node binary is required')
    if args.expected_sha256 is not None:
        parser.error('Expected evidence SHA256 is accepted only for validation')
    cleanup = {'verified_stopped': True, 'forced': False, 'remaining_owned': 0,
               'unknown_births': 0, 'tracking_errors': 0}
    executable = None
    started = time.monotonic()
    os.umask(0o077)
    try:
        if (sys.platform != 'linux' or not hasattr(os, 'pidfd_open') or
                not hasattr(signal, 'pidfd_send_signal') or os.getuid() == 0):
            raise Refused('unsupported-owner')
        if ctypes.CDLL(None, use_errno=True).prctl(36, 1, 0, 0, 0) != 0:
            raise Refused('subreaper-required')
        node = args.node
        if not node.is_absolute() or not node.is_file() or node.is_symlink():
            raise Refused('explicit-node-required')
        runtime = read_json(ROOT / '.build/hosted-browser/runtime.json', 2 * 1024 * 1024)
        if (runtime.get('schema') != 1 or runtime.get('playwright') != '1.62.1' or
                runtime.get('chromium_revision') != '1234' or runtime.get('npm_hooks_executed') is not False):
            raise Refused('explicit-hosted-runtime-required')
        executable = runtime.get('executable_sha256')
        if type(executable) is not str or len(executable) != 64 or any(c not in '0123456789abcdef' for c in executable):
            raise Refused('invalid-executable-hash')
        if private.parent.is_symlink() or public.parent.is_symlink() or public.exists():
            raise Refused('unexpected-output-parent')
        private.mkdir(parents=True, mode=0o700, exist_ok=False)
        owner = Owner(private, started=started)
        def interrupted(*_):
            owner.interrupted = True
        signal.signal(signal.SIGINT, interrupted)
        signal.signal(signal.SIGTERM, interrupted)
        owner.launch([str(node), str(ROOT / 'scripts/ci/sandbox-preflight.mjs')], ROOT,
                     child_environment(os.environ, private))
        outcome = owner.observe()
    except Exception:
        outcome = 'owner-error' if owner else 'admission-refused'
    finally:
        if owner:
            try:
                cleanup = owner.cleanup()
            except Exception:
                cleanup['tracking_errors'] = 1
                outcome = 'cleanup-unverified'
            code = owner.child.returncode if owner.child else None
        try:
            worker = read_json(private / 'worker.json')
        except Exception:
            worker = None
        # A buffered tail can first cross the log cap during terminal draining.
        outcome = terminal_outcome(outcome, worker, cleanup, bool(owner and owner.output_overflow))
        try:
            launch_log = diagnostic.parse_log(read_bytes(private / 'worker.log', MAX_LOG)) if owner and not owner.output_overflow and cleanup.get('stdout_eof') is True else diagnostic.empty_log('oversized' if owner and owner.output_overflow else 'unavailable')
        except Exception:
            launch_log = diagnostic.empty_log()
        result = public_projection(worker, code, cleanup, outcome,
                                   int((time.monotonic() - started) * 1000), executable, launch_log)
        validate_public(result)
        data = (json.dumps(result, sort_keys=True) + '\n').encode()
        if len(data) > MAX_JSON:
            raise Refused('public-output-limit')
        public.parent.mkdir(parents=True, exist_ok=True)
        with public.open('xb') as output:
            output.write(data)
            output.flush()
            os.fsync(output.fileno())
        # Emit association only for this exclusively created, verified generation.
        written = read_bytes(public)
        if written != data:
            raise Refused('public-output-changed')
        validate_public(json.loads(written.decode('utf-8'), object_pairs_hook=unique_object))
        print('evidence_sha256=' + hashlib.sha256(written).hexdigest(), flush=True)
        print(json.dumps({'status': result['status'], 'parent_outcome': result['parent_outcome']}))
    return 0 if result['status'] == 'passed' else 1


if __name__ == '__main__':
    try:
        status = main()
    except Exception:
        # No private path/exception details in even the outer owner's log.
        print('Sandbox preflight owner refused', file=sys.stderr)
        status = 1
    sys.exit(status)
