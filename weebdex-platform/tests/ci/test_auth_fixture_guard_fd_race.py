"""Finite fake-proc and orchestration seams; no native child, socket or DB."""
import contextlib
import errno
import hashlib
import importlib.util
import io
from pathlib import Path
import types
import unittest
from unittest.mock import patch

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
BASELINE_SHA = '17ff9c2eb04a4bdefaa3b61e975e77567f873208a83454afcdd6ca19c5cfc33e'
RETAINED_RECEIPTS = {}


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


seams = load('auth_fd_existing_seams', HERE / 'test_auth_fixture_guard_recovery.py')
guard = seams.guard


class DescriptorRace(unittest.TestCase):
    def harness(self):
        value = seams.GuardTests()
        value.setUp()
        self.addCleanup(value.doCleanups)
        return value

    def setUp(self):
        self.fixture = self.harness()
        self.directory = self.fixture.proc / '101/fd'
        self.listener = self.directory / '3'
        self.unrelated = self.directory / '4'
        self.unrelated.symlink_to('unrelated-target')
        self.readlink = guard.os.readlink

    def owns(self):
        return guard.owns_listener(self.fixture.expected, 40000, self.fixture.proc)

    def vanish(self, target):
        def observe(path):
            if path == target:
                raise FileNotFoundError(errno.ENOENT, 'synthetic closed descriptor')
            return self.readlink(path)
        return observe

    def test_unrelated_descriptor_disappears_without_losing_positive_listener(self):
        with patch.object(guard.os, 'readlink', side_effect=self.vanish(self.unrelated)):
            self.assertTrue(self.owns())

    def test_vanished_listener_cannot_supply_positive_inode(self):
        with patch.object(guard.os, 'readlink', side_effect=self.vanish(self.listener)):
            self.assertFalse(self.owns())

    def test_reused_descriptor_with_other_target_cannot_supply_listener(self):
        self.listener.unlink()
        self.listener.symlink_to('socket:[901]')
        with patch.object(guard.os, 'readlink', side_effect=self.vanish(self.unrelated)):
            self.assertFalse(self.owns())

    def test_permission_denial_is_not_ignored(self):
        with patch.object(guard.os, 'readlink', side_effect=PermissionError(errno.EACCES, 'synthetic denial')):
            with self.assertRaises(PermissionError):
                self.owns()

    def test_unknown_os_error_is_not_ignored(self):
        with patch.object(guard.os, 'readlink', side_effect=OSError(errno.EIO, 'synthetic failure')):
            with self.assertRaises(OSError) as observed:
                self.owns()
        self.assertEqual(observed.exception.errno, errno.EIO)

    def test_non_enoent_file_not_found_is_not_ignored(self):
        with patch.object(guard.os, 'readlink', side_effect=FileNotFoundError('synthetic non-ENOENT')):
            with self.assertRaises(FileNotFoundError):
                self.owns()

    def test_directory_enumeration_failure_is_not_ignored(self):
        for descriptor in self.directory.iterdir():
            descriptor.unlink()
        self.directory.rmdir()
        with self.assertRaises(FileNotFoundError):
            self.owns()

    def test_parent_directory_disappearing_during_read_is_not_ignored(self):
        def disappear(_path):
            for descriptor in self.directory.iterdir():
                descriptor.unlink()
            self.directory.rmdir()
            raise FileNotFoundError(errno.ENOENT, 'synthetic parent disappearance')
        with patch.object(guard.os, 'readlink', side_effect=disappear):
            with self.assertRaises(FileNotFoundError):
                self.owns()

    def test_owner_disappearance_during_enoent_refuses_ownership(self):
        def disappear(path):
            if path == self.unrelated:
                (self.fixture.proc / '101/stat').unlink()
            return self.vanish(self.unrelated)(path)
        with patch.object(guard.os, 'readlink', side_effect=disappear):
            self.assertFalse(self.owns())

    def test_pid_birth_reuse_during_enoent_refuses_ownership(self):
        def reuse(path):
            if path == self.unrelated:
                self.fixture.stat(101, 42, 101, 999)
            return self.vanish(self.unrelated)(path)
        with patch.object(guard.os, 'readlink', side_effect=reuse):
            self.assertFalse(self.owns())

    def test_final_birth_recheck_still_refuses_identity_change(self):
        def reuse(path):
            value = self.readlink(path)
            self.fixture.stat(101, 42, 101, 999)
            return value
        with patch.object(guard.os, 'readlink', side_effect=reuse):
            self.assertFalse(self.owns())

    def test_final_live_owner_recheck_refuses_zombie_after_positive_read(self):
        def zombie(path):
            value = self.readlink(path)
            self.fixture.stat(101, 42, 101, 100, 'Z')
            return value
        with patch.object(guard.os, 'readlink', side_effect=zombie):
            self.assertFalse(self.owns())

    def test_old_exact_source_counterfactual_exits_72_with_normal_physical_cleanup(self):
        # Reverse only this additive FD-read seam, then bind the complete old
        # producer bytes. This also works in shallow source-only checkouts.
        text = (ROOT / 'tests/runtime/auth_fixture_guard.py').read_bytes()
        start = text.index(b'    sockets = set()\n')
        end = text.index(b'    return (len(listeners) == 1', start)
        text = (text[:start] + b"    sockets = {os.readlink(p) for p in (proc / str(expected['pid']) / 'fd').iterdir()}\n"
                + text[end:])
        text = text.replace(b"and (current := same(expected, proc)) is not None\n            and current['state'] != 'Z')",
                            b'and same(expected, proc) is not None)')
        self.assertEqual(hashlib.sha256(text).hexdigest(), BASELINE_SHA)
        old = types.ModuleType('exact_baseline_auth_guard')
        old.__file__ = str(ROOT / 'tests/runtime/auth_fixture_guard.py')
        exec(compile(text, old.__file__, 'exec'), old.__dict__)
        for name, module, expected_exit in [('baseline', old, 72), ('candidate', guard, 0)]:
            with self.subTest(source=name):
                actual_owns, actual_same, actual_row = module.owns_listener, module.same, module.row
                fixture = self.fixture
                readlink = self.vanish(self.unrelated)
                class Observation:
                    def __bool__(self):
                        # Re-enter the actual source's fake-proc listener read;
                        # the surrounding orchestration still mocks every child.
                        with patch.object(module, 'same', actual_same), patch.object(module, 'row', actual_row), \
                             patch.object(module.os, 'readlink', side_effect=readlink):
                            return actual_owns(fixture.expected, 40000, fixture.proc)
                harness = self.harness()
                with patch.object(seams, 'guard', module), contextlib.redirect_stderr(io.StringIO()):
                    result, receipt, launched, stopped = harness.run_seam(listener=Observation())
                RETAINED_RECEIPTS[name] = receipt
                self.assertEqual(result, expected_exit)
                self.assertEqual(receipt['postgres_cleanup'], 'verified_stopped')
                self.assertFalse(receipt['forced'])
                self.assertTrue(receipt['closed_port'])
                for field in ['remaining_owned_processes', 'cleanup_errors', 'unconfirmed_births', 'unconfirmed_child_pids']:
                    self.assertEqual(receipt[field], [])
                self.assertEqual(receipt['postgres_returncode'], 0)
                stopped.assert_called_once()
                if name == 'baseline':
                    self.assertEqual(receipt['exit'], 72)
                    self.assertTrue(receipt['error'].startswith('FileNotFoundError:'))
                    self.assertIsNone(receipt['test_returncode'])
                    self.assertFalse(any(command[0] == 'synthetic-test' for command, _, _ in launched))
                else:
                    self.assertEqual(receipt['exit'], 0)
                    self.assertIsNone(receipt['error'])
                    self.assertEqual(receipt['test_returncode'], 0)
                    self.assertTrue(any(command[0] == 'synthetic-test' for command, _, _ in launched))


if __name__ == '__main__':
    unittest.main()
