"""Owned local Unix sockets and finite stand-ins only; no browser or network."""
import contextlib
import errno
import importlib.util
import io
import json
import os
from pathlib import Path
import signal
import socket
import stat
import sys
import tempfile
import time
import unittest
from unittest import mock

spec = importlib.util.spec_from_file_location('short_tmp_owner',
    Path(__file__).parents[2] / 'scripts/ci/run-sandbox-preflight.py')
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


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


@unittest.skipUnless(sys.platform == 'linux' and hasattr(os, 'pidfd_open') and
                     hasattr(signal, 'pidfd_send_signal'), 'Linux owned stand-in')
class BrowserTempOwnerTests(unittest.TestCase):
    def test_temp_admission_refusal_still_emits_fresh_failed_record_without_receipt(self):
        with tempfile.TemporaryDirectory(prefix='short-tmp-refusal-') as directory:
            root = Path(directory).resolve()
            runtime = root / '.build/hosted-browser/runtime.json'
            runtime.parent.mkdir(parents=True)
            runtime.write_text(json.dumps(dict(schema=1, playwright='1.62.1',
                chromium_revision='1234', npm_hooks_executed=False, executable_sha256='a' * 64)))
            node = root / 'synthetic-node'
            node.write_text('not executed')
            stdout = io.StringIO()
            with mock.patch.object(module, 'ROOT', root), \
                    mock.patch.object(module.BrowserTemp, 'create', side_effect=module.Refused('synthetic-admission-refusal')), \
                    mock.patch.object(module.Owner, 'launch', side_effect=AssertionError('No worker admitted')), \
                    mock.patch.object(sys, 'argv', ['owner', '--node', str(node)]), \
                    contextlib.redirect_stdout(stdout):
                self.assertEqual(module.main(), 1)
            value = module.validate_public(json.loads((root / 'ci-evidence/sandbox-preflight.json').read_bytes()))
            self.assertEqual(value['status'], 'failed')
            self.assertEqual(value['parent_outcome'], 'owner-error')
            self.assertTrue(stdout.getvalue().startswith('evidence_sha256='))
            self.assertFalse((root / '.build/sandbox-preflight/seal-ready.json').exists())

    def test_wrong_temp_environment_refuses_before_worker_creation(self):
        with tempfile.TemporaryDirectory(prefix='short-tmp-owner-') as directory:
            owner = module.Owner(Path(directory), work_seconds=1, cleanup_seconds=.6, total_seconds=3)
            owner.browser_temp = module.BrowserTemp()
            owner.browser_temp.create()
            path = owner.browser_temp.path
            try:
                with self.assertRaises(module.Refused):
                    owner.launch([sys.executable, '-I', '-c', 'pass'], Path(directory), {'TMPDIR': '/tmp'})
                self.assertIsNone(owner.child)
                self.assertFalse((Path(directory) / 'worker.log').exists())
            finally:
                clean = owner.cleanup()
            self.assertTrue(clean['verified_stopped'])
            self.assertFalse(path.exists())
            self.assertTrue(json.loads((Path(directory) / 'owner.json').read_text())['browser_temp']['removed'])

    def test_unverified_process_cleanup_never_starts_temp_deletion(self):
        with tempfile.TemporaryDirectory(prefix='short-tmp-uncertain-') as directory:
            owner = module.Owner(Path(directory), work_seconds=.1, cleanup_seconds=.02, total_seconds=.2)
            temp = mock.Mock()
            temp.journal.return_value = {'created': True, 'admitted': True, 'removed': False}
            owner.browser_temp = temp
            with mock.patch.object(owner, 'scan', return_value=None):
                clean = owner.cleanup()
            temp.cleanup.assert_not_called()
            temp.close.assert_called_once_with()
            self.assertFalse(clean['verified_stopped'])
            self.assertGreater(clean['tracking_errors'], 0)

    def run_main_standin(self, uncertain=False):
        events, temps = [], []
        with tempfile.TemporaryDirectory(prefix='short-tmp-main-') as directory:
            root = Path(directory).resolve()
            private = root / '.build/sandbox-preflight'
            public = root / 'ci-evidence/sandbox-preflight.json'
            runtime = root / '.build/hosted-browser/runtime.json'
            runtime.parent.mkdir(parents=True)
            runtime.write_text(json.dumps(dict(schema=1, playwright='1.62.1',
                chromium_revision='1234', npm_hooks_executed=False, executable_sha256='a' * 64)))
            node = root / 'synthetic-node'
            node.write_text('not executed')
            record = dict(schema=2, launch_diagnostic=module.diagnostic.empty_worker(),
                stage='complete', caught_stage='none', caught_category='none',
                **dict.fromkeys(module.FLAGS, True))
            script = ('import pathlib;pathlib.Path(' + repr(str(private / 'worker.json')) +
                      ').write_text(' + repr(json.dumps(record)) + ');print("synthetic owned log",flush=True)')
            base = module.BrowserTemp
            class RecordedTemp(base):
                def create(self):
                    result = super().create()
                    temps.append(self)
                    events.append('registered')
                    return result
                def cleanup(self, deadline):
                    if public.exists():
                        raise AssertionError('Public record precedes temp teardown')
                    super().cleanup(deadline)
                    events.append('removed')
                    if uncertain:
                        raise module.Refused('synthetic-cleanup-uncertainty')
            original_launch = module.Owner.launch
            def launch(owner, argv, cwd, env):
                self.assertTrue(owner.browser_temp.admitted)
                self.assertEqual(env['TMPDIR'], str(owner.browser_temp.path))
                self.assertNotEqual(env['TMPDIR'], str(private))
                events.append('launched')
                original_launch(owner, [sys.executable, '-I', '-c', script], cwd, env)
            stdout = io.StringIO()
            with mock.patch.object(module, 'ROOT', root), mock.patch.object(module, 'BrowserTemp', RecordedTemp), \
                    mock.patch.object(module.Owner, 'launch', launch), \
                    mock.patch.object(sys, 'argv', ['owner', '--node', str(node)]), \
                    contextlib.redirect_stdout(stdout):
                result = module.main()
            self.assertEqual(events, ['registered', 'launched', 'removed'])
            self.assertEqual(len(temps), 1)
            self.assertTrue(temps[0].removed)
            self.assertIsNone(temps[0].fd)
            self.assertIsNone(temps[0].parent_fd)
            value = module.validate_public(json.loads(public.read_bytes()))
            self.assertNotIn(str(temps[0].path), public.read_text())
            self.assertNotIn('synthetic-cleanup-uncertainty', stdout.getvalue())
            journal = json.loads((private / 'owner.json').read_text())
            self.assertTrue(journal['browser_temp']['admitted'])
            self.assertTrue(journal['browser_temp']['removed'])
            self.assertGreaterEqual(len(journal['births']), 1)
            receipt = private / 'seal-ready.json'
            self.assertEqual(result, 1 if uncertain else 0)
            self.assertEqual(value['status'], 'failed' if uncertain else 'passed')
            self.assertEqual(value['parent_outcome'], 'cleanup-unverified' if uncertain else 'complete')
            self.assertEqual(value['cleanup']['verified_stopped'], not uncertain)
            self.assertEqual(receipt.exists(), not uncertain)
            self.assertEqual((private / 'worker.log').read_bytes(), b'synthetic owned log\n')
            if receipt.exists():
                self.assertEqual(json.loads(receipt.read_text())['log_bytes'], len(b'synthetic owned log\n'))

    def test_main_removes_registered_temp_before_clean_public_and_seal_ready(self):
        self.run_main_standin()

    def test_temp_cleanup_uncertainty_prevents_clean_terminal_and_seal_ready(self):
        self.run_main_standin(uncertain=True)


if __name__ == '__main__':
    unittest.main()
