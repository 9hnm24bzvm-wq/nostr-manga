"""Closed synthetic records and short owned Python processes; no browser/network."""
import copy
import contextlib
import ctypes
import hashlib
import importlib.util
import json
import io
import os
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import time
import unittest
from unittest import mock

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location('sandbox_owner', ROOT / 'scripts/ci/run-sandbox-preflight.py')
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)
HASH = 'a' * 64
PRIVATE = 'https://synthetic-secret.invalid/private?token=never-export'


def worker():
    return dict(schema=2, launch_diagnostic=module.diagnostic.empty_worker(), stage='complete', caught_stage='none', caught_category='none',
                **dict.fromkeys(module.FLAGS, True))


def launching(category='launch-failed'):
    value = module.diagnostic.empty_worker()
    sandbox = category == 'sandbox-refused'
    value['error'] = dict(reason='sandbox-refused' if sandbox else 'unknown', code='none',
        code_source='none', message_state='bounded', signatures=dict.fromkeys(module.diagnostic.SIGNATURES, False))
    value['error']['signatures']['sandbox'] = sandbox
    return value


def cleanup():
    return dict(verified_stopped=True, forced=False, remaining_owned=0,
                unknown_births=0, tracking_errors=0)


class ClosedPreflightRecords(unittest.TestCase):
    def test_complete_record_and_physical_cleanup_are_both_required(self):
        value = module.public_projection(worker(), 0, cleanup(), 'complete', 100, HASH)
        self.assertEqual(value['status'], 'passed')
        self.assertEqual(module.validate_public(value), value)
        for code in [1, -9, None, False, 256, PRIVATE]:
            failed = module.public_projection(worker(), code, cleanup(), 'complete', 100, HASH)
            self.assertEqual(failed['status'], 'failed')
            module.validate_public(failed)
        for facts in [dict(cleanup(), forced=True), dict(cleanup(), remaining_owned=1),
                      dict(cleanup(), unknown_births=1), dict(cleanup(), tracking_errors=1),
                      dict(cleanup(), verified_stopped=False)]:
            failed = module.public_projection(worker(), 0, facts, 'complete', 100, HASH)
            self.assertEqual(failed['status'], 'failed')
            module.validate_public(failed)

    def test_unknown_missing_extra_or_private_worker_fields_never_project(self):
        base = worker()
        for raw in [None, [], PRIVATE, dict(base, message=PRIVATE), dict(base, stage=PRIVATE),
                    dict(base, caught_stage=PRIVATE), dict(base, caught_category=PRIVATE),
                    dict(base, browser_created=1), dict(base, schema=True)]:
            with self.subTest(raw=type(raw).__name__):
                value = module.public_projection(raw, 1, cleanup(), 'worker-failed', 1)
                self.assertFalse(value['worker_record_valid'])
                self.assertEqual(value['stage'], 'unavailable')
                self.assertTrue(all(value[key] is None for key in module.FLAGS))
                self.assertNotIn(PRIVATE, json.dumps(value))
                module.validate_public(value)
        for key in base:
            raw = dict(base)
            raw.pop(key)
            self.assertFalse(module.worker_valid(raw))

    def test_caught_stage_category_association_is_closed(self):
        expected = {'none': {'none'}, 'launching-browser': {'sandbox-refused', 'launch-failed'},
                    'browser-launched': {'context-failed'}, 'context-created': {'page-failed'},
                    'page-created': {'script-failed'}, 'closing-context': {'cleanup-failed'},
                    'closing-browser': {'cleanup-failed'}, 'initializing': {'worker-failed'},
                    'script-verified': {'worker-failed'}, 'complete': {'worker-failed'}}
        for site, categories in expected.items():
            for category in module.CATEGORIES:
                raw = dict(schema=2, launch_diagnostic=module.diagnostic.empty_worker(), stage='complete', caught_stage=site, caught_category=category,
                           **dict.fromkeys(module.FLAGS, False))
                if site == 'launching-browser':
                    raw['launch_diagnostic'] = launching(category)
                if site in ('browser-launched', 'context-created', 'page-created', 'script-verified',
                            'closing-context', 'closing-browser'):
                    raw['browser_created'] = True
                if site in ('context-created', 'page-created', 'script-verified', 'closing-context'):
                    raw['context_created'] = True
                if site in ('page-created', 'script-verified'):
                    raw['page_created'] = True
                if site == 'script-verified':
                    raw['script_verified'] = True
                self.assertEqual(module.worker_valid(raw), category in categories)
                value = module.public_projection(raw, 1, cleanup(), 'worker-failed', 1)
                self.assertEqual(value['status'], 'failed')
                module.validate_public(value)

    def test_handle_flags_and_completion_cannot_invent_acceptance(self):
        for changes in [dict(browser_created=False), dict(context_created=False), dict(page_created=False),
                        dict(context_closed=False), dict(browser_closed=False), dict(script_verified=False),
                        dict(caught_stage='launching-browser', caught_category='sandbox-refused'),
                        dict(stage='script-verified')]:
            self.assertFalse(module.worker_valid(dict(worker(), **changes)))
        for field in module.FLAGS:
            raw = worker()
            raw[field] = False
            self.assertEqual(module.public_projection(raw, 0, cleanup(), 'complete', 1, HASH)['status'], 'failed')
        raw = dict(worker(), completed=False, caught_stage='launching-browser',
                   caught_category='sandbox-refused', launch_diagnostic=launching('sandbox-refused'), **dict.fromkeys(module.FLAGS[:-1], False))
        self.assertTrue(module.worker_valid(raw))
        value = module.public_projection(raw, 1, cleanup(), 'worker-failed', 100, HASH)
        self.assertEqual(value['caught_category'], 'sandbox-refused')
        self.assertFalse(value['application_journey_qualified'])

    def test_serialized_public_schema_rejects_private_or_forged_fields(self):
        baseline = module.public_projection(worker(), 0, cleanup(), 'complete', 1, HASH)
        changes = [lambda d: d.update(raw_error=PRIVATE), lambda d: d.update(stage=PRIVATE),
                   lambda d: d.update(caught_category=PRIVATE), lambda d: d.update(worker_exit_code=True),
                   lambda d: d.update(elapsed_ms=120001), lambda d: d.update(executable_sha256=PRIVATE),
                   lambda d: d.update(parent_outcome=PRIVATE), lambda d: d.update(worker_record_valid=False),
                   lambda d: d['cleanup'].update(unknown_births=1),
                   lambda d: d['cleanup'].update(forced=True),
                   lambda d: d['cleanup'].update(raw_error=PRIVATE),
                   lambda d: d.update(application_journey_qualified=True)]
        for change in changes:
            value = copy.deepcopy(baseline)
            change(value)
            with self.assertRaises(module.Refused):
                module.validate_public(value)
        for key in baseline:
            value = copy.deepcopy(baseline)
            value.pop(key)
            with self.assertRaises(module.Refused):
                module.validate_public(value)

    def test_json_reader_rejects_duplicate_oversize_and_symlink(self):
        with tempfile.TemporaryDirectory(prefix='sandbox-schema-') as directory:
            root = Path(directory)
            file = root / 'record.json'
            for raw in [b'{"schema":1,"schema":1}', b' ' * (module.MAX_JSON + 1)]:
                file.write_bytes(raw)
                with self.assertRaises((module.Refused, ValueError)):
                    module.read_json(file)
            file.write_text('{}')
            alias = root / 'link'
            alias.symlink_to(file)
            with self.assertRaises(OSError):
                module.read_json(alias)

    def test_child_environment_drops_every_unapproved_secret_and_injection(self):
        value = module.child_environment(dict(PATH='/usr/bin', HOME='/synthetic/home',
            LANG='C.UTF-8', GITHUB_TOKEN=PRIVATE, ACTIONS_RUNTIME_TOKEN=PRIVATE,
            NODE_OPTIONS=PRIVATE, NODE_PATH=PRIVATE, LD_PRELOAD=PRIVATE,
            PYTHONPATH=PRIVATE, HTTPS_PROXY=PRIVATE), Path('/synthetic/private'))
        self.assertNotIn(PRIVATE, json.dumps(value))
        self.assertEqual(value['NODE_OPTIONS'], '--max-old-space-size=512')
        self.assertEqual(set(value), {'PATH', 'HOME', 'LANG', 'TMPDIR', 'NODE_OPTIONS', 'DEBUG', 'DEBUG_COLORS', 'DEBUG_HIDE_DATE'})

    def test_output_validation_cli_does_not_require_node_or_launch(self):
        with tempfile.TemporaryDirectory(prefix='sandbox-validate-') as directory:
            root = Path(directory)
            public = root / 'ci-evidence/sandbox-preflight.json'
            public.parent.mkdir()
            public.write_text(json.dumps(module.public_projection(None, 1, cleanup(), 'worker-failed', 1)))
            digest = hashlib.sha256(public.read_bytes()).hexdigest()
            with mock.patch.object(module, 'ROOT', root), mock.patch.object(sys, 'argv',
                    ['owner', '--validate', '--expected-sha256', digest]), \
                    mock.patch.object(module.Owner, 'launch', side_effect=AssertionError('No worker allowed')):
                self.assertEqual(module.main(), 0)

    def test_validation_requires_current_exact_byte_hash(self):
        with tempfile.TemporaryDirectory(prefix='sandbox-hash-') as directory:
            root = Path(directory)
            public = root / 'ci-evidence/sandbox-preflight.json'
            public.parent.mkdir()
            public.write_text(json.dumps(module.public_projection(worker(), 0, cleanup(), 'complete', 1, HASH)))
            digest = hashlib.sha256(public.read_bytes()).hexdigest()
            with mock.patch.object(module, 'ROOT', root), contextlib.redirect_stderr(io.StringIO()):
                for expected in [None, '0' * 64, PRIVATE]:
                    args = ['owner', '--validate'] + (['--expected-sha256', expected] if expected else [])
                    with mock.patch.object(sys, 'argv', args):
                        if expected == '0' * 64:
                            self.assertEqual(module.main(), 1)
                        else:
                            with self.assertRaises(SystemExit):
                                module.main()
                # Even schema-equivalent whitespace changes invalidate association.
                public.write_bytes(public.read_bytes() + b'\n')
                with mock.patch.object(sys, 'argv', ['owner', '--validate', '--expected-sha256', digest]):
                    self.assertEqual(module.main(), 1)

    def test_stale_passed_record_cannot_emit_current_invocation_marker(self):
        with tempfile.TemporaryDirectory(prefix='sandbox-stale-') as directory:
            root = Path(directory)
            public = root / 'ci-evidence/sandbox-preflight.json'
            public.parent.mkdir()
            prior = json.dumps(module.public_projection(worker(), 0, cleanup(), 'complete', 1, HASH)).encode()
            public.write_bytes(prior)
            stdout = io.StringIO()
            with mock.patch.object(module, 'ROOT', root), mock.patch.object(sys, 'argv',
                    ['owner', '--node', sys.executable]), contextlib.redirect_stdout(stdout), \
                    mock.patch.object(module.Owner, 'launch', side_effect=AssertionError('No worker allowed')):
                with self.assertRaises(FileExistsError):
                    module.main()
            self.assertNotIn('evidence_sha256=', stdout.getvalue())
            self.assertEqual(public.read_bytes(), prior)

    def test_new_failure_record_emits_only_its_verified_exact_hash(self):
        with tempfile.TemporaryDirectory(prefix='sandbox-new-') as directory:
            root = Path(directory)
            stdout = io.StringIO()
            with mock.patch.object(module, 'ROOT', root), mock.patch.object(sys, 'argv',
                    ['owner', '--node', sys.executable]), contextlib.redirect_stdout(stdout), \
                    mock.patch.object(module.Owner, 'launch', side_effect=AssertionError('No worker allowed')):
                # No runtime receipt exists: publish a bounded admission refusal.
                self.assertEqual(module.main(), 1)
            public = root / 'ci-evidence/sandbox-preflight.json'
            digest = hashlib.sha256(public.read_bytes()).hexdigest()
            self.assertEqual(stdout.getvalue().splitlines()[0], 'evidence_sha256=' + digest)
            self.assertEqual(module.validate_public(module.read_json(public))['status'], 'failed')


@unittest.skipUnless(sys.platform == 'linux' and hasattr(os, 'pidfd_open') and
                     hasattr(signal, 'pidfd_send_signal'), 'Linux pidfd ownership required')
class SyntheticOwnedProcesses(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        # Process-local subreaper ownership only; no system/service setting.
        if ctypes.CDLL(None, use_errno=True).prctl(36, 1, 0, 0, 0) != 0:
            raise unittest.SkipTest('Process-local subreaper unavailable')

    def run_owned(self, script, *, work=1, cap=None):
        with tempfile.TemporaryDirectory(prefix='sandbox-owned-') as directory:
            private = Path(directory)
            owner = module.Owner(private, work_seconds=work, cleanup_seconds=.6, total_seconds=3)
            env = module.child_environment(os.environ, private)
            patch = mock.patch.object(module, 'MAX_LOG', cap) if cap else mock.patch.object(module, 'MAX_LOG', module.MAX_LOG)
            with patch:
                try:
                    owner.launch([sys.executable, '-I', '-c', script], private, env)
                    result = owner.observe()
                finally:
                    terminal = owner.cleanup()
            self.assertLess(time.monotonic() - owner.started, 3)
            self.assertTrue((private / 'owner.json').is_file())
            return owner, result, terminal

    def test_registered_python_standin_exits_without_forced_cleanup(self):
        owner, result, terminal = self.run_owned('print("synthetic normal",flush=True)')
        self.assertEqual(result, 'complete')
        self.assertEqual(owner.child.returncode, 0)
        self.assertTrue(terminal['verified_stopped'])
        self.assertFalse(terminal['forced'])
        self.assertTrue(terminal['stdout_eof'])
        self.assertEqual(len(owner.ledger), 1)

    def test_deadline_signals_only_captured_birth_and_fails_closed(self):
        owner, result, terminal = self.run_owned('import time;time.sleep(20)', work=.08)
        self.assertEqual(result, 'deadline')
        self.assertTrue(terminal['verified_stopped'])
        self.assertTrue(terminal['forced'])
        self.assertEqual(terminal['remaining_owned'], 0)

    def test_output_limit_is_bounded_and_cannot_pass(self):
        owner, result, terminal = self.run_owned('print("x"*100,flush=True)', cap=16)
        self.assertEqual(result, 'output-limit')
        self.assertEqual(owner.log_bytes, 16)
        self.assertTrue(terminal['verified_stopped'])

    def test_exited_child_buffered_tail_is_checked_after_terminal_drain(self):
        with tempfile.TemporaryDirectory(prefix='sandbox-late-output-') as directory:
            private = Path(directory)
            owner = module.Owner(private, work_seconds=1, cleanup_seconds=.6, total_seconds=3)
            with mock.patch.object(module, 'MAX_LOG', 4096):
                try:
                    owner.launch([sys.executable, '-I', '-c',
                                  'import os;os.write(1,b"x"*5000)'], private,
                                 module.child_environment(os.environ, private))
                    owner.child.wait(timeout=2)
                    outcome = owner.observe()
                    self.assertEqual(outcome, 'complete')
                    self.assertEqual(owner.log_bytes, 4096)
                    self.assertFalse(owner.output_overflow)
                finally:
                    terminal = owner.cleanup()
            self.assertTrue(owner.output_overflow)
            self.assertTrue(terminal['verified_stopped'])
            self.assertFalse(terminal['forced'])
            outcome = module.terminal_outcome(outcome, worker(), terminal, owner.output_overflow)
            self.assertEqual(outcome, 'output-limit')
            self.assertEqual(module.public_projection(worker(), 0, terminal, outcome, 1, HASH)['status'], 'failed')

    def test_many_buffered_chunks_are_drained_through_eof_without_retaining_over_cap(self):
        with tempfile.TemporaryDirectory(prefix='sandbox-many-chunks-') as directory:
            private = Path(directory)
            owner = module.Owner(private, work_seconds=1, cleanup_seconds=.6, total_seconds=3)
            owner.launch([sys.executable, '-I', '-c', 'pass'], private,
                         module.child_environment(os.environ, private))
            owner.child.wait(timeout=2)
            fd, original = owner.child.stdout.fileno(), os.read
            pending, observed = bytearray(b'x' * 20000), []
            def buffered_read(descriptor, limit):
                if descriptor != fd:
                    return original(descriptor, limit)
                value = bytes(pending[:limit])
                del pending[:limit]
                observed.append(len(value))
                return value
            with mock.patch.object(module, 'MAX_LOG', 16384), mock.patch.object(module.os, 'read', buffered_read):
                try:
                    outcome = owner.observe()
                    self.assertEqual(outcome, 'complete')
                    self.assertEqual(sum(observed), 4096)
                    self.assertFalse(owner.output_overflow)
                finally:
                    terminal = owner.cleanup()
            self.assertEqual(sum(observed), 20000)
            self.assertEqual(observed[-1], 0)
            self.assertTrue(terminal['stdout_eof'])
            self.assertTrue(terminal['verified_stopped'])
            self.assertTrue(owner.output_overflow)
            self.assertEqual(owner.log_bytes, 16384)
            self.assertEqual(module.terminal_outcome(outcome, worker(), terminal, owner.output_overflow), 'output-limit')

    def test_unverified_terminal_pipe_eof_fails_closed_within_existing_deadline(self):
        with tempfile.TemporaryDirectory(prefix='sandbox-no-eof-') as directory:
            private = Path(directory)
            owner = module.Owner(private, work_seconds=1, cleanup_seconds=.12, total_seconds=2)
            owner.launch([sys.executable, '-I', '-c', 'pass'], private,
                         module.child_environment(os.environ, private))
            owner.child.wait(timeout=1)
            fd, original = owner.child.stdout.fileno(), os.read
            def no_eof(descriptor, limit):
                return b'x' if descriptor == fd else original(descriptor, limit)
            started = time.monotonic()
            with mock.patch.object(module.os, 'read', no_eof):
                terminal = owner.cleanup()
            self.assertLess(time.monotonic() - started, .5)
            self.assertFalse(terminal['stdout_eof'])
            self.assertFalse(terminal['verified_stopped'])
            self.assertGreater(terminal['tracking_errors'], 0)

    def test_adopted_term_resistant_descendant_is_reaped_by_exact_handle(self):
        script = ('import subprocess,sys,time;'
                  'subprocess.Popen([sys.executable,"-I","-c",'
                  '"import signal,time;signal.signal(signal.SIGTERM,signal.SIG_IGN);time.sleep(20)"],'
                  'start_new_session=True);time.sleep(.15)')
        owner, result, terminal = self.run_owned(script)
        self.assertEqual(result, 'complete')
        self.assertGreaterEqual(len(owner.ledger), 2)
        self.assertTrue(terminal['verified_stopped'])
        self.assertTrue(terminal['forced'])
        self.assertEqual(terminal['unknown_births'], 0)

    def test_census_failure_still_signals_the_captured_worker(self):
        with tempfile.TemporaryDirectory(prefix='sandbox-census-') as directory:
            private = Path(directory)
            owner = module.Owner(private, work_seconds=1, cleanup_seconds=.3, total_seconds=2)
            owner.launch([sys.executable, '-I', '-c', 'import time;time.sleep(20)'], private,
                         module.child_environment(os.environ, private))
            with mock.patch.object(owner, '_rows', side_effect=OSError('synthetic private census error')):
                terminal = owner.cleanup()
            self.assertIsNotNone(owner.child.poll())
            self.assertFalse(terminal['verified_stopped'])
            self.assertTrue(terminal['forced'])
            self.assertGreater(terminal['tracking_errors'], 0)

    def test_preexisting_unrelated_sibling_is_never_registered_or_signalled(self):
        sibling = subprocess.Popen([sys.executable, '-I', '-c', 'import time;time.sleep(20)'],
                                   stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL,
                                   stderr=subprocess.DEVNULL, start_new_session=True)
        try:
            owner, result, terminal = self.run_owned('import time;time.sleep(20)', work=.08)
            self.assertEqual(result, 'deadline')
            self.assertTrue(terminal['verified_stopped'])
            self.assertIsNone(sibling.poll())
            self.assertNotIn(sibling.pid, [pid for pid, _ in owner.ledger])
        finally:
            sibling.terminate()
            sibling.wait(timeout=3)


if __name__ == '__main__':
    unittest.main()
