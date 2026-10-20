"""Finite owned temp/process seams and injected spawn boundaries; no browser/services."""
import contextlib
import ctypes
import importlib.util
import io
import json
import errno
import os
from pathlib import Path
import signal
import socket
import stat
import subprocess
import sys
import tempfile
import threading
import time
from types import SimpleNamespace
import unittest
from unittest import mock

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location('application_temp_runner', ROOT / 'scripts/ci/run-hosted-browser.py')
runner = importlib.util.module_from_spec(spec)
spec.loader.exec_module(runner)
module = runner.browser_temp

class BrowserTempTests(unittest.TestCase):
    def temporary(self):
        value = module.BrowserTemp()
        self.addCleanup(value.close)
        value.create()
        # Keep handles until this exact admitted fixture has been removed.
        self.addCleanup(lambda: value.cleanup(time.monotonic() + 2) if not value.removed else None)
        return value

    def test_fresh_short_owned_0700_directory_and_nonreuse(self):
        value = self.temporary()
        self.assertTrue(value.created and value.admitted)
        self.assertLessEqual(len(str(value.path).encode('utf-8')), module.MAX_BROWSER_TMP_BYTES)
        self.assertLess(len(str(value.path).encode('utf-8')) + module.BROWSER_SOCKET_SUFFIX_BYTES,
                        module.LINUX_UNIX_PATH_BYTES)
        facts = value.path.lstat()
        self.assertEqual(stat.S_IMODE(facts.st_mode), 0o700)
        self.assertEqual(facts.st_uid, os.geteuid())
        self.assertEqual(module.BrowserTemp.identity_of(facts), value.identity)
        with self.assertRaises(module.Refused):
            value.create()

    def test_byte_length_guard_refuses_before_directory_creation(self):
        value = module.BrowserTemp()
        self.addCleanup(value.close)
        with mock.patch.object(module, 'MAX_BROWSER_TMP_BYTES', 20), \
                mock.patch.object(module.os, 'mkdir', side_effect=AssertionError('No creation admitted')):
            with self.assertRaises(module.Refused):
                value.create()
        self.assertFalse(value.created)

    def test_existing_directory_collision_is_never_reused_or_removed(self):
        prior = self.temporary()
        marker = prior.path / 'keep'
        marker.write_bytes(b'owned pre-existing fixture')
        value = module.BrowserTemp()
        self.addCleanup(value.close)
        with mock.patch.object(module.secrets, 'token_hex', return_value=prior.path.name[4:]) as token:
            with self.assertRaises(module.Refused):
                value.create()
        self.assertEqual(token.call_count, 8)
        self.assertFalse(value.created)
        value.cleanup(time.monotonic() + 1)
        self.assertEqual(marker.read_bytes(), b'owned pre-existing fixture')

    def test_symlink_parent_is_not_admitted(self):
        prior = self.temporary()
        alias = prior.path / 'alias'
        alias.symlink_to('/tmp', target_is_directory=True)
        value = module.BrowserTemp()
        value.parent = alias
        self.addCleanup(value.close)
        with self.assertRaises(OSError):
            value.create()
        self.assertFalse(value.created)
        self.assertTrue(alias.is_symlink())

    def test_root_symlink_replacement_refuses_without_touching_either_tree(self):
        value, outside = self.temporary(), self.temporary()
        marker = outside.path / 'keep'
        marker.write_bytes(b'outside fixture')
        held = value.path.with_name(value.path.name + '-held')
        value.path.rename(held)
        value.path.symlink_to(outside.path, target_is_directory=True)
        try:
            with self.assertRaises(module.Refused):
                value.cleanup(time.monotonic() + 1)
            self.assertTrue(value.path.is_symlink())
            self.assertEqual(marker.read_bytes(), b'outside fixture')
            self.assertTrue(held.is_dir())
        finally:
            value.path.unlink()
            held.rename(value.path)

    def test_root_directory_replacement_and_permission_drift_refuse(self):
        value = self.temporary()
        held = value.path.with_name(value.path.name + '-held')
        value.path.rename(held)
        value.path.mkdir(mode=0o700)
        marker = value.path / 'keep'
        marker.write_bytes(b'replacement fixture')
        try:
            with self.assertRaises(module.Refused):
                value.cleanup(time.monotonic() + 1)
            self.assertEqual(marker.read_bytes(), b'replacement fixture')
        finally:
            marker.unlink()
            value.path.rmdir()
            held.rename(value.path)
        value.path.chmod(0o750)
        try:
            with self.assertRaises(module.Refused):
                value.cleanup(time.monotonic() + 1)
            self.assertTrue(value.path.exists())
        finally:
            value.path.chmod(0o700)

    def test_foreign_owned_descendant_refuses_before_unlink(self):
        value = self.temporary()
        marker = value.path / 'keep'
        marker.write_bytes(b'owned test fixture')
        original = module.os.stat
        def foreign(path, *args, **kwargs):
            result = original(path, *args, **kwargs)
            if path == marker.name and kwargs.get('dir_fd') == value.fd:
                fields = list(result)
                fields[4] = os.geteuid() + 1
                return os.stat_result(fields)
            return result
        with mock.patch.object(module.os, 'stat', foreign):
            with self.assertRaises(module.Refused):
                value.cleanup(time.monotonic() + 1)
        self.assertEqual(marker.read_bytes(), b'owned test fixture')

    def test_nested_files_and_symlinks_are_removed_without_following_targets(self):
        value, outside = self.temporary(), self.temporary()
        marker = outside.path / 'keep'
        marker.write_bytes(b'outside fixture')
        nested = value.path / 'profile'
        nested.mkdir(mode=0o700)
        (nested / 'data').write_bytes(b'owned temporary data')
        (nested / 'SingletonSocket').symlink_to(marker)
        value.cleanup(time.monotonic() + 1)
        self.assertTrue(value.removed)
        self.assertFalse(value.path.exists())
        self.assertEqual(marker.read_bytes(), b'outside fixture')
        self.assertGreaterEqual(value.entries, 3)
        self.assertGreaterEqual(value.bytes, len(b'owned temporary data'))
        self.assertEqual(os.fstat(value.fd).st_nlink, 0)

    @unittest.skipUnless(hasattr(os, 'mkfifo'), 'POSIX owned FIFO control')
    def test_owned_child_special_entries_are_unlinked_without_opening_them(self):
        value, outside = self.temporary(), self.temporary()
        pipe = value.path / 'owned-pipe'
        external = outside.path / 'keep-pipe'
        os.mkfifo(pipe, 0o600)
        os.mkfifo(external, 0o600)
        (value.path / 'pipe-link').symlink_to(external)
        original = module.os.open
        def refuse_special_open(path, *args, **kwargs):
            if str(path) in ('owned-pipe', 'pipe-link', 'keep-pipe', str(pipe), str(external)):
                raise AssertionError('Special entry or symlink target must never be opened')
            return original(path, *args, **kwargs)
        started = time.monotonic()
        with mock.patch.object(module.os, 'open', refuse_special_open):
            value.cleanup(started + 1)
        self.assertLess(time.monotonic() - started, 1)
        self.assertTrue(value.removed)
        self.assertTrue(stat.S_ISFIFO(external.lstat().st_mode))
        self.assertEqual(value.entries, 2)

    def test_existing_deadline_entry_and_depth_limits_fail_closed(self):
        for constraint in ('deadline', 'entries', 'depth'):
            with self.subTest(constraint=constraint):
                value = self.temporary()
                nested = value.path / 'profile'
                nested.mkdir(mode=0o700)
                (nested / 'keep').write_bytes(b'owned fixture')
                deadline = time.monotonic() - 1 if constraint == 'deadline' else time.monotonic() + 1
                target = 'MAX_BROWSER_TMP_ENTRIES' if constraint == 'entries' else 'MAX_BROWSER_TMP_DEPTH'
                limit = 0 if constraint in ('entries', 'depth') else 4096
                with mock.patch.object(module, target, limit):
                    with self.assertRaises(module.Refused):
                        value.cleanup(deadline)
                self.assertFalse(value.removed)
                self.assertTrue(value.path.exists())

    @unittest.skipUnless(sys.platform == 'linux' and hasattr(socket, 'AF_UNIX'), 'Linux Unix socket witness')
    def test_actual_owned_unix_bind_rejects_old_128_bytes_and_accepts_new_short_base(self):
        value = self.temporary()
        old_base = value.path / ('x' * (83 - len(str(value.path).encode('utf-8')) - 1))
        old_base.mkdir(mode=0o700)
        suffix = Path('.org.chromium.Chromium.abcde') / 'SingletonSocket'
        old_path, new_path = old_base / suffix, value.path / suffix
        old_path.parent.mkdir(mode=0o700)
        new_path.parent.mkdir(mode=0o700)
        self.assertEqual(len(str(old_base).encode('utf-8')), 83)
        self.assertEqual(len(str(old_path).encode('utf-8')), 128)
        self.assertLess(len(str(new_path).encode('utf-8')), 108)
        try:
            owned_socket = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        except PermissionError as error:
            if error.errno in (errno.EPERM, errno.EACCES):
                self.skipTest('Local execution sandbox denies AF_UNIX socket creation; bind witness not run')
            raise
        with owned_socket:
            with self.assertRaises(OSError):
                owned_socket.bind(str(old_path))
        with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as owned_socket:
            owned_socket.bind(str(new_path))
        self.assertTrue(stat.S_ISSOCK(new_path.lstat().st_mode))
        value.cleanup(time.monotonic() + 1)
        self.assertTrue(value.removed)

    def test_unadmitted_created_directory_refuses_cleanup(self):
        value = self.temporary()
        value.admitted = False
        try:
            with self.assertRaises(module.Refused):
                value.cleanup(time.monotonic() + 1)
            self.assertTrue(value.path.is_dir())
        finally:
            value.admitted = True


class ApplicationBrowserTemp(unittest.TestCase):
    @contextlib.contextmanager
    def admitted(self):
        with tempfile.TemporaryDirectory(prefix='application-temp-seam-') as directory:
            owner = SimpleNamespace(process_roots={}, process_tracking_errors=[],
                process_members=mock.Mock(return_value=[]))
            with mock.patch.object(runner, 'STAGE', Path(directory)):
                runner.prepare_browser_temp(owner, 'startup')
            temp = owner.browser_temp
            held, parent = os.dup(temp.fd), os.dup(temp.parent_fd)
            try:
                yield owner, Path(directory)
            finally:
                # Only this exact fixture's retained handles are used for test recovery.
                temp.close()
                if not temp.removed:
                    temp.fd, temp.parent_fd = held, parent
                    held = parent = None
                    temp.cleanup(time.monotonic() + 2)
                temp.close()
                if held is not None:
                    os.close(held)
                if parent is not None:
                    os.close(parent)
                if not owner.browser_temp_record.closed:
                    owner.browser_temp_record.close()

    def batch(self):
        return dict(batch='startup', readiness=True,
            cleanup=dict(verified_stopped=True, lifecycle_clean=True, ports_closed=True,
                keys_removed=True, remaining_owned_process_count=0, final_teardown_clean=True,
                pg_fault_activated=False, pg_fault_terminal_clean=False),
            outer_cleanup=dict(forced=False, verified_stopped=True, remaining_owned_process_count=0),
            supervisor_terminal=True, supervisor_exit_zero=True)

    def test_private_admission_journal_and_identity_exist_before_child_admission(self):
        with self.admitted() as (owner, stage):
            value = json.loads(owner.browser_temp_record_path.read_text())
            self.assertTrue(value['created'] and value['admitted'])
            self.assertFalse(value['removed'] or value['cleanup_verified'] or value['process_tree_stopped'])
            self.assertEqual(tuple(value['identity']), owner.browser_temp.identity)
            self.assertEqual(stat.S_IMODE(owner.browser_temp_record_path.stat().st_mode), 0o600)
            self.assertNotIn(str(owner.browser_temp.path), owner.browser_temp_record_path.read_text())
            self.assertFalse(owner.process_roots)
            self.assertTrue(owner.browser_temp.path.is_dir())

    def test_fresh_journal_never_adopts_or_overwrites_prior_bytes(self):
        with tempfile.TemporaryDirectory(prefix='application-temp-stale-') as directory:
            stage = Path(directory)
            path = stage / 'browser-temp-startup.json'
            path.write_bytes(b'pre-existing fixture journal')
            owner = SimpleNamespace(process_roots={}, process_tracking_errors=[],
                process_members=mock.Mock(return_value=[]))
            with mock.patch.object(runner, 'STAGE', stage):
                with self.assertRaises(FileExistsError):
                    runner.prepare_browser_temp(owner, 'startup')
            temp_path = owner.browser_temp.path
            batch = self.batch()
            runner.finish_browser_temp(owner, None, batch, time.monotonic() + 2)
            self.assertFalse(temp_path.exists())
            self.assertFalse(batch['cleanup']['verified_stopped'])
            self.assertEqual(path.read_bytes(), b'pre-existing fixture journal')

    def test_clean_terminal_requires_removal_and_persists_before_handles_close(self):
        with self.admitted() as (owner, stage):
            path = owner.browser_temp.path
            profile = path / 'profile'
            profile.mkdir(mode=0o700)
            (profile / 'temporary-data').write_bytes(b'owned fixture bytes')
            batch = self.batch()
            process = SimpleNamespace(poll=lambda: 0)
            runner.finish_browser_temp(owner, process, batch, time.monotonic() + 2)
            self.assertTrue(batch['cleanup']['verified_stopped'])
            self.assertTrue(batch['outer_cleanup']['verified_stopped'])
            self.assertNotIn('cleanup_error_class', batch)
            self.assertFalse(path.exists())
            value = json.loads(owner.browser_temp_record_path.read_text())
            self.assertTrue(value['removed'] and value['cleanup_verified'] and value['process_tree_stopped'])
            self.assertEqual(value['entries'], 2)
            self.assertGreaterEqual(value['bytes'], len(b'owned fixture bytes'))
            self.assertTrue(owner.browser_temp_record.closed)
            self.assertIsNone(owner.browser_temp.fd)
            self.assertIsNone(owner.browser_temp.parent_fd)

    def test_live_unknown_or_unverified_tree_never_starts_deletion(self):
        cases = ('live', 'outer', 'members', 'tracking', 'unregistered')
        for case in cases:
            with self.subTest(case=case), self.admitted() as (owner, stage):
                batch = self.batch()
                process = SimpleNamespace(poll=lambda: None if case == 'live' else 0)
                if case == 'outer':
                    batch['outer_cleanup']['verified_stopped'] = False
                if case == 'members':
                    owner.process_members.return_value = [{'synthetic': True}]
                if case == 'tracking':
                    owner.process_tracking_errors = [{'synthetic': True}]
                if case == 'unregistered':
                    owner.process_roots = {'synthetic': True}
                    process = None
                with mock.patch.object(owner.browser_temp, 'cleanup', wraps=owner.browser_temp.cleanup) as remove:
                    runner.finish_browser_temp(owner, process, batch, time.monotonic() + 2)
                    remove.assert_not_called()
                self.assertTrue(owner.browser_temp.path.is_dir())
                self.assertFalse(batch['cleanup']['verified_stopped'])
                self.assertFalse(batch['outer_cleanup']['verified_stopped'])
                self.assertEqual(batch['cleanup_error_class'], 'RuntimeError')
                value = json.loads(owner.browser_temp_record_path.read_text())
                self.assertFalse(value['removed'] or value['cleanup_verified'])

    def test_expired_cleanup_deadline_cannot_be_reported_clean(self):
        with self.admitted() as (owner, stage):
            batch = self.batch()
            runner.finish_browser_temp(owner, SimpleNamespace(poll=lambda: 0),
                                       batch, time.monotonic() - 1)
            self.assertTrue(owner.browser_temp.path.exists())
            self.assertFalse(batch['cleanup']['verified_stopped'])
            value = json.loads(owner.browser_temp_record_path.read_text())
            self.assertTrue(value['process_tree_stopped'])
            self.assertFalse(value['cleanup_verified'] or value['removed'])

    def test_journal_uncertainty_blocks_clean_batch_even_after_actual_removal(self):
        with self.admitted() as (owner, stage):
            owner.browser_temp_record_path.unlink()
            owner.browser_temp_record_path.write_bytes(b'replacement fixture journal')
            batch = self.batch()
            runner.finish_browser_temp(owner, SimpleNamespace(poll=lambda: 0),
                                       batch, time.monotonic() + 2)
            self.assertTrue(owner.browser_temp.removed)
            self.assertFalse(batch['cleanup']['verified_stopped'])
            self.assertFalse(batch['outer_cleanup']['verified_stopped'])
            self.assertEqual(owner.browser_temp_record_path.read_bytes(), b'replacement fixture journal')

    def test_missing_temp_registration_cannot_promote_an_exited_child(self):
        owner = SimpleNamespace()
        batch = self.batch()
        runner.finish_browser_temp(owner, SimpleNamespace(poll=lambda: 0), batch, time.monotonic() + 2)
        self.assertFalse(batch['cleanup']['verified_stopped'])
        self.assertFalse(batch['outer_cleanup']['verified_stopped'])

    def test_failure_stays_in_the_existing_closed_public_schema_without_private_values(self):
        validator_spec = importlib.util.spec_from_file_location('application_temp_validator',
            ROOT / 'scripts/ci/validate-ci-evidence.py')
        validator = importlib.util.module_from_spec(validator_spec)
        validator_spec.loader.exec_module(validator)
        with self.admitted() as (owner, stage):
            batch = self.batch()
            runner.finish_browser_temp(owner, SimpleNamespace(poll=lambda: None),
                                       batch, time.monotonic() + 2)
            batch['passed'] = False
            journal = dict(schema=1, phases=[], batches=[batch])
            validator.phases(journal)
            encoded = json.dumps(journal)
            self.assertNotIn(str(owner.browser_temp.path), encoded)
            self.assertNotIn('identity', encoded)
            self.assertNotIn('browser_temp', encoded)
            self.assertNotIn('private', encoded)
            self.assertFalse(runner.summarize(journal, 'startup', ['startup'])['selected_scope_passed'])

    def test_actual_fixture_node_spawn_forwards_owned_override_and_filters_secrets(self):
        fixture_spec = importlib.util.spec_from_file_location('application_temp_fixture',
            ROOT / 'tests/runtime/browser_fixture.py')
        fixture = importlib.util.module_from_spec(fixture_spec)
        fixture_spec.loader.exec_module(fixture)
        with self.admitted() as (owner, stage):
            state = fixture.Fixture.__new__(fixture.Fixture)
            state.run = stage
            state.browser = None
            state.browser_completed = set()
            state.ui_dir = stage
            state.verify_frontend = lambda _: None
            state.origin, state.mail_origin = 'http://127.0.0.1:1', 'http://127.0.0.1:2'
            state.public_upload_controls = False  # Explicit native-profile stand-in.
            state.children, state.logs = {}, {}
            state.generation = 0
            state.receipt = dict(children={}, listener_lifecycle=[])
            state.process_ownership_lock = threading.RLock()
            seen = []
            child = SimpleNamespace(pid=12345)
            def register(name, process):
                seen.append(('registered-process', name))
                state.children[name] = process
                state.receipt['children'][name] = {'pid': process.pid}
            state.register_process = register
            def spawn(argv, **kwargs):
                self.assertTrue(owner.browser_temp.admitted)
                self.assertEqual(argv[1], str(ROOT / 'scripts/browser-acceptance.mjs'))
                self.assertEqual(kwargs['env']['TMPDIR'], str(owner.browser_temp.path))
                self.assertNotIn('never-forward-fixture-secret', json.dumps(kwargs['env']))
                self.assertTrue(kwargs['start_new_session'])
                seen.append(('popen', kwargs['env']['TMPDIR']))
                return child
            command = dict(phase='startup',
                argv=['node', str(ROOT / 'scripts/browser-acceptance.mjs')],
                timeout_seconds=150, env=dict(BROWSER_PHASE='startup', BROWSER_RUN_ID='finite'))
            fixture.write_json(stage / 'browser-command.json', command)
            try:
                with mock.patch.dict(os.environ, dict(TMPDIR=str(owner.browser_temp.path),
                        GITHUB_TOKEN='never-forward-fixture-secret', NODE_OPTIONS='never-forward-fixture-secret')), \
                        mock.patch.object(fixture.subprocess, 'Popen', side_effect=spawn), \
                        contextlib.redirect_stdout(io.StringIO()):
                    state.browser_commands()
            finally:
                for output in state.logs.values():
                    output.close()
            self.assertEqual([item[0] for item in seen], ['popen', 'registered-process'])
            self.assertIs(state.browser, child)
            launch = (ROOT / 'scripts/browser-acceptance.mjs').read_text()
            self.assertEqual(launch.count('await chromium.launch('), 1)
            self.assertIn('chromiumSandbox: true', launch)
            self.assertNotIn('--no-sandbox', launch)
            self.assertNotIn('TMPDIR', launch)  # Uses inherited, owner-bound process.env.

    def test_outer_batch_registers_and_overrides_before_spawn_and_gates_cleanup_failure(self):
        import importlib.abc
        import importlib.machinery
        for case in ('normal', 'admission-refused', 'cleanup-uncertain', 'forced'):
            with self.subTest(case=case), tempfile.TemporaryDirectory(prefix='application-batch-seam-') as directory:
                root = Path(directory).resolve()
                stage = root / '.build/hosted-browser'
                stage.mkdir(parents=True)
                owner = SimpleNamespace(process_roots={}, process_tracking_errors=[],
                    process_members=mock.Mock(return_value=[]), owned_members=mock.Mock(return_value=[]),
                    process_ownership_lock=threading.RLock())
                events = []
                class Child:
                    pid = 12345
                    returncode = 0
                    def poll(self):
                        return 0
                child = Child()
                def register(name, process):
                    self.assertTrue(owner.browser_temp.admitted)
                    owner.process_roots[process] = {'synthetic': True}
                    events.append('process-registered')
                owner.register_process = register
                class Loader(importlib.abc.Loader):
                    def create_module(self, spec):
                        return None
                    def exec_module(self, target):
                        target.OwnedFixtureProcesses = lambda: owner
                original_spec = importlib.util.spec_from_file_location
                def owned_spec(name, path, *args, **kwargs):
                    if name == 'outer_fixture_cleanup':
                        return importlib.machinery.ModuleSpec(name, Loader())
                    return original_spec(name, path, *args, **kwargs)
                plan = dict(normal_binary='not-executed', normal_sha256='a' * 64,
                    provisioner='not-executed', provisioner_sha256='b' * 64,
                    frontend_manifest='not-opened', frontend_manifest_sha256='c' * 64,
                    frontend_index_sha256='d' * 64, node='node')
                fixture = root / 'test-runtime/browser-final-finite'
                def spawn(argv, **kwargs):
                    temp = owner.browser_temp
                    admission = json.loads(owner.browser_temp_record_path.read_text())
                    self.assertTrue(admission['admitted'])
                    self.assertFalse(admission['removed'])
                    self.assertEqual(kwargs['env']['TMPDIR'], str(temp.path))
                    self.assertLessEqual(len(kwargs['env']['TMPDIR'].encode('utf-8')), 32)
                    self.assertNotIn('never-forward-fixture-secret', json.dumps(kwargs['env']))
                    self.assertEqual(argv[1], str(root / 'tests/runtime/browser_fixture.py'))
                    events.append('popen')
                    fixture.mkdir(mode=0o700)
                    runner.write(fixture / 'supervisor.json', dict(status='ready',
                        run_directory=str(fixture), protocol_seed=dict(fully_ready=True),
                        binary=dict(sha256=plan['normal_sha256']),
                        frontend=dict(artifact_manifest_sha256=plan['frontend_manifest_sha256'])))
                    runner.write(fixture / 'browser-phase-startup.json', {'synthetic': True})
                    return child
                def finish(current, process, fixture_path, batch):
                    self.assertIs(current, owner)
                    self.assertIs(process, child)
                    self.assertTrue(owner.browser_temp.path.is_dir())
                    batch.update(supervisor_terminal=True, supervisor_exit_zero=True,
                        outer_cleanup=dict(forced=case == 'forced', verified_stopped=True,
                                           remaining_owned_process_count=0),
                        cleanup=self.batch()['cleanup'])
                    events.append('tree-stopped')
                    if case == 'cleanup-uncertain':
                        original = owner.browser_temp.cleanup
                        def uncertain(deadline):
                            original(deadline)
                            raise RuntimeError('synthetic cleanup uncertainty')
                        owner.browser_temp.cleanup = uncertain
                original_wait = runner.wait_for
                def wait(process, predicate, seconds, current):
                    if (fixture / 'browser-command.json').exists():
                        acceptance = root / 'test-evidence/frontend/browser/startup-finite-startup/acceptance.json'
                        runner.write(acceptance, {'synthetic': True})
                    return original_wait(process, predicate, seconds, current)
                create = module.BrowserTemp.create
                def admission(temp):
                    if case == 'admission-refused':
                        raise module.Refused('synthetic admission refusal')
                    return create(temp)
                journal = dict(schema=1, phases=[], batches=[])
                try:
                    with mock.patch.object(runner, 'ROOT', root), mock.patch.object(runner, 'STAGE', stage), \
                            mock.patch.object(runner, 'PUBLIC', root / 'ci-evidence'), \
                            mock.patch.object(runner.importlib.util, 'spec_from_file_location', owned_spec), \
                            mock.patch.object(runner.subprocess, 'Popen', side_effect=spawn), \
                            mock.patch.object(runner, 'finish_supervisor', side_effect=finish), \
                            mock.patch.object(runner, 'wait_for', side_effect=wait), \
                            mock.patch.object(module.BrowserTemp, 'create', admission), \
                            mock.patch.object(runner, 'project_phase', side_effect=lambda phase, batch, *_:
                                dict(phase=phase, batch=batch, status='passed', actual_steps=1)), \
                            mock.patch.dict(os.environ, dict(TMPDIR='/tmp/' + 'x' * 78,
                                GITHUB_TOKEN='never-forward-fixture-secret')), \
                            contextlib.redirect_stdout(io.StringIO()):
                        value = runner.run_batch('startup', [], plan, journal, 'finite')
                    self.assertEqual(value['passed'], case == 'normal', json.dumps(value))
                    temp = owner.browser_temp
                    self.assertFalse(temp.created and not temp.removed)
                    self.assertIsNone(temp.fd)
                    self.assertIsNone(temp.parent_fd)
                    if case == 'admission-refused':
                        self.assertEqual(events, [])
                        self.assertEqual(value['error_class'], 'RuntimeError')
                    else:
                        self.assertEqual(events, ['popen', 'process-registered', 'tree-stopped'])
                        self.assertTrue(owner.browser_temp_record.closed)
                        self.assertTrue(json.loads(owner.browser_temp_record_path.read_text())['removed'])
                    if case == 'cleanup-uncertain':
                        self.assertFalse(value['cleanup']['verified_stopped'])
                        self.assertFalse(value['outer_cleanup']['verified_stopped'])
                    self.assertNotIn('never-forward-fixture-secret', json.dumps(journal))
                    if temp.path:
                        self.assertNotIn(str(temp.path), json.dumps(journal))
                finally:
                    if getattr(owner, 'browser_temp', None) is not None:
                        if owner.browser_temp.fd is not None:
                            owner.browser_temp.cleanup(time.monotonic() + 2)
                        owner.browser_temp.close()
                    output = getattr(owner, 'browser_temp_record', None)
                    if output is not None and not output.closed:
                        output.close()

    @unittest.skipUnless(sys.platform == 'linux' and hasattr(os, 'pidfd_open') and
                         hasattr(signal, 'pidfd_send_signal'), 'Linux owned Python stand-in')
    def test_real_owned_python_child_uses_short_root_and_is_joined_before_removal(self):
        helper_spec = importlib.util.spec_from_file_location('application_temp_process_owner',
            ROOT / 'scripts/ci/owned-fixture-cleanup.py')
        helper = importlib.util.module_from_spec(helper_spec)
        helper_spec.loader.exec_module(helper)
        owner = helper.OwnedFixtureProcesses()
        with tempfile.TemporaryDirectory(prefix='application-temp-process-') as directory:
            stage = Path(directory)
            with mock.patch.object(runner, 'STAGE', stage):
                runner.prepare_browser_temp(owner, 'startup')
            path = owner.browser_temp.path
            script = ('import os,pathlib; p=pathlib.Path(os.environ["TMPDIR"]);'
                      'assert len(str(p).encode("utf-8"))<=32;'
                      '(p/"profile").mkdir(mode=0o700);'
                      '(p/"profile"/"owned").write_bytes(b"finite Python stand-in")')
            child = None
            batch = self.batch()
            try:
                with owner.process_ownership_lock:
                    owner.browser_temp.check()
                    child = subprocess.Popen([sys.executable, '-I', '-c', script],
                        env=dict(PATH=os.environ.get('PATH', ''), TMPDIR=str(path)), start_new_session=True)
                    owner.register_process('supervisor', child)
                child.wait(timeout=3)
                self.assertEqual(child.returncode, 0)
                self.assertTrue((path / 'profile/owned').is_file())
                runner.finish_supervisor(owner, child, None, batch, seconds=.5)
                self.assertTrue(batch['supervisor_terminal'])
                self.assertTrue(batch['outer_cleanup']['verified_stopped'])
                runner.finish_browser_temp(owner, child, batch, time.monotonic() + 2)
                self.assertFalse(path.exists())
                self.assertTrue(json.loads(owner.browser_temp_record_path.read_text())['cleanup_verified'])
            finally:
                owner.fallback_cleanup()
                if not owner.browser_temp.removed:
                    owner.browser_temp.cleanup(time.monotonic() + 2)
                owner.browser_temp.close()
                if not owner.browser_temp_record.closed:
                    owner.browser_temp_record.close()


if __name__ == '__main__':
    unittest.main()
