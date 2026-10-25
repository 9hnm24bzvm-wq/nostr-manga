"""Finite source/seam checks only: no real processes, sockets or databases."""
import errno
import importlib.util
import json
import os
from pathlib import Path
import signal
import subprocess
import tempfile
import unittest
from unittest.mock import Mock, patch

SOURCE = Path(__file__).resolve().parents[1] / 'runtime/auth_fixture_guard.py'
SPEC = importlib.util.spec_from_file_location('auth_fixture_guard_under_test', SOURCE)
guard = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(guard)


class Clock:
    def __init__(self):
        self.value, self.steps = 0., 0

    def now(self):
        self.steps += 1
        if self.steps > 10000:
            raise AssertionError('finite clock seam exhausted')
        return self.value

    def sleep(self, seconds):
        self.value += seconds


class GuardTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.proc = Path(self.temporary.name) / 'proc'
        (self.proc / 'net').mkdir(parents=True)
        self.stat(101, 42, 101, 100)
        (self.proc / '101/fd').mkdir()
        (self.proc / '101/fd/3').symlink_to('socket:[900]')
        self.expected = dict(pid=101, start_ticks=100, group=101, session=101)
        self.tables()

    def stat(self, pid, ppid, session, start, state='S'):
        path = self.proc / str(pid)
        path.mkdir(exist_ok=True)
        fields = [state, str(ppid), str(session), str(session)] + ['0'] * 15 + [str(start)]
        (path / 'stat').write_text(f'{pid} (fake process) ' + ' '.join(fields))

    def tables(self, address='0100007F', inode='900', extra6=''):
        line = f'0: {address}:9C40 00000000:0000 0A 0:0 0:0 0 0 0 {inode}\n'
        (self.proc / 'net/tcp').write_text('header\n' + line)
        (self.proc / 'net/tcp6').write_text('header\n' + extra6)

    def test_exact_postmaster_birth_and_loopback_inode_are_required(self):
        self.assertTrue(guard.owns_listener(self.expected, 40000, self.proc))
        self.tables(inode='901')
        self.assertFalse(guard.owns_listener(self.expected, 40000, self.proc))
        self.tables()
        self.stat(101, 42, 101, 101)
        self.assertFalse(guard.owns_listener(self.expected, 40000, self.proc))

    def test_wildcard_ipv6_duplicate_and_zombie_listeners_are_rejected(self):
        self.tables(address='00000000')
        self.assertFalse(guard.owns_listener(self.expected, 40000, self.proc))
        self.tables(extra6='0: ' + '0' * 32 + ':9C40 0:0 0A 0:0 0:0 0 0 0 901\n')
        self.assertFalse(guard.owns_listener(self.expected, 40000, self.proc))
        self.tables()
        self.stat(101, 42, 101, 100, 'Z')
        self.assertFalse(guard.owns_listener(self.expected, 40000, self.proc))

    def test_members_need_exact_lineage_or_owned_adoption(self):
        self.stat(102, 101, 101, 101)
        self.stat(103, 42, 101, 102)
        self.stat(104, 99, 101, 103)
        self.stat(105, 42, 105, 104)
        self.assertEqual({p['pid'] for p in guard.members(self.expected, self.proc, 42)}, {101, 102, 103})
        self.stat(101, 99, 101, 200)
        self.stat(102, 101, 101, 201)
        self.assertEqual({p['pid'] for p in guard.members(self.expected, self.proc, 42)}, {103})

    def test_pidfd_rechecks_birth_after_open_and_never_signals_reuse(self):
        def reuse(_pid):
            self.stat(101, 42, 101, 999)
            return 888
        with patch.object(guard.os, 'pidfd_open', side_effect=reuse) as opened, \
             patch.object(guard.signal, 'pidfd_send_signal') as sent, patch.object(guard.os, 'close') as closed:
            guard.send(self.expected, signal.SIGINT, self.proc)
            opened.assert_called_once_with(101)
            sent.assert_not_called()
            closed.assert_called_once_with(888)

    def stop_fixture(self, disappear_on=None, stuck=False, zombie=False):
        clock, sent, reaped = Clock(), [], []
        root = dict(self.expected, state='Z' if zombie else 'S', ppid=42)
        descendant = dict(root, pid=102, start_ticks=101)
        remaining = [root, descendant]
        child = Mock(pid=101)
        def emit(value, signum):
            sent.append((value['pid'], signum))
            if signum == disappear_on and not stuck:
                remaining.clear()
        with patch.object(guard.os, 'getpid', return_value=42):
            result = guard.stop([(child, self.expected)], clock=clock.now, sleep=clock.sleep,
                                inspect=lambda _expected: list(remaining), emit=emit,
                                reap=lambda pid, _options: reaped.append(pid))
        return result, clock, sent, reaped, child

    def test_clean_fast_shutdown_only_signals_owned_leader(self):
        result, clock, sent, _, child = self.stop_fixture(signal.SIGINT)
        self.assertEqual(result, (False, []))
        self.assertEqual(sent, [(101, signal.SIGINT)])
        self.assertLess(clock.value, 1)
        child.wait.assert_called_once_with(timeout=0)

    def test_escalation_keeps_forced_truth_and_exact_individual_targets(self):
        result, clock, sent, _, child = self.stop_fixture(signal.SIGKILL)
        self.assertEqual(result, (True, []))
        self.assertEqual(sent, [(101, signal.SIGINT), (101, signal.SIGTERM), (102, signal.SIGTERM),
                                (101, signal.SIGKILL), (102, signal.SIGKILL)])
        self.assertLess(clock.value, 25.2)
        child.wait.assert_called_once_with(timeout=0)

    def test_stuck_owned_processes_return_failure_at_fixed_deadline(self):
        result, clock, _, _, child = self.stop_fixture(stuck=True)
        self.assertTrue(result[0])
        self.assertEqual({p['pid'] for p in result[1]}, {101, 102})
        self.assertLess(clock.value, 25.2)
        child.wait.assert_not_called()

    def test_owned_zombie_descendant_is_reaped_without_real_waitpid(self):
        result, _, sent, reaped, child = self.stop_fixture(zombie=True)
        self.assertEqual(result, (False, []))
        self.assertEqual(reaped, [102])
        self.assertEqual(sent, [])
        child.wait.assert_called_once_with(timeout=0)

    def test_signal_failure_still_retains_forced_cleanup_truth(self):
        clock, state = Clock(), {}
        child = Mock(pid=101)
        current = dict(self.expected, state='S', ppid=42)
        def emit(_value, signum):
            if signum == signal.SIGTERM:
                raise PermissionError('synthetic denial')
        with self.assertRaises(PermissionError):
            guard.stop([(child, self.expected)], clock=clock.now, sleep=clock.sleep,
                       inspect=lambda _expected: [current], emit=emit, reap=Mock(), state=state)
        self.assertTrue(state['forced'])
        self.assertLess(clock.value, 15.2)

    def test_closed_port_requires_refusal_not_timeout_or_success(self):
        probe = Mock()
        probe.__enter__ = Mock(return_value=probe)
        probe.__exit__ = Mock(return_value=False)
        with patch.object(guard.socket, 'socket', return_value=probe):
            for code, expected in [(0, False), (errno.ETIMEDOUT, False), (errno.ECONNREFUSED, True)]:
                probe.connect_ex.return_value = code
                self.assertEqual(guard.closed(40000), expected)
        probe.settimeout.assert_called_with(.2)
        probe.connect_ex.assert_called_with(('127.0.0.1', 40000))

    def run_seam(self, *, forced=False, port_closed=True, listener=True, capture=True, test_timeout=False,
                 row_raises=False, final_same_raises=False, cancel_on_test_wait=False):
        root, clock, launched = Path(self.temporary.name) / 'project', Clock(), []
        root.mkdir()
        reservation = Mock()
        reservation.__enter__ = Mock(return_value=reservation)
        reservation.__exit__ = Mock(return_value=False)
        reservation.getsockname.return_value = ('127.0.0.1', 40000)
        def popen(command, **kwargs):
            child = Mock(pid=200 + len(launched), args=command, returncode=0)
            if command[0] == 'synthetic-test' and test_timeout:
                child.returncode = None
                def wait(timeout):
                    clock.sleep(timeout)
                    raise subprocess.TimeoutExpired(command, timeout)
                child.wait.side_effect = wait
            else:
                def wait(timeout):
                    if cancel_on_test_wait and command[0] == 'synthetic-test':
                        callbacks[signal.SIGTERM](signal.SIGTERM, None)
                    return 0
                child.wait.side_effect = wait
            launched.append((command, kwargs, child))
            return child
        def process(pid):
            if row_raises:
                raise PermissionError('synthetic procfs capture denial')
            return dict(pid=pid, ppid=os.getpid(), group=pid, session=pid, start_ticks=pid, state='S') if capture else None
        same_calls, callbacks = [], {}
        def same(value):
            same_calls.append(value)
            if final_same_raises and len(same_calls) == 2:
                raise PermissionError('synthetic pre-cleanup observation denial')
            return process(value['pid'])
        def handler(signum, callback):
            callbacks[signum] = callback
        with patch.object(guard, 'ROOT', root), patch.object(guard.sys, 'argv', [str(SOURCE), '/fake/pg', 'synthetic-test']), \
             patch.object(guard.ctypes, 'CDLL') as libc, patch.object(guard.socket, 'socket', return_value=reservation), \
             patch.object(guard.subprocess, 'Popen', side_effect=popen), patch.object(guard, 'row', side_effect=process), \
             patch.object(guard, 'same', side_effect=same), \
             patch.object(guard, 'owns_listener', return_value=listener), patch.object(guard, 'closed', return_value=port_closed), \
             patch.object(guard, 'stop', return_value=(forced, [])) as stopped, patch.object(guard.signal, 'signal', side_effect=handler), \
             patch.object(guard.time, 'monotonic', side_effect=clock.now), patch.object(guard.time, 'sleep', side_effect=clock.sleep):
            libc.return_value.prctl.return_value = 0
            result = guard.run()
        receipt = json.loads(next((root / 'test-runtime').glob('auth-*/cleanup.json')).read_text())
        return result, receipt, launched, stopped

    def test_mocked_orchestration_retains_original_exit_when_cleanup_forced(self):
        result, receipt, launched, _ = self.run_seam(forced=True)
        self.assertEqual(result, 71)
        self.assertEqual(receipt['exit'], 0)
        self.assertEqual(receipt['test_returncode'], 0)
        self.assertTrue(receipt['forced'])
        self.assertEqual(receipt['postgres_cleanup'], 'verified_stopped')
        for _, kwargs, _ in launched:
            self.assertTrue(kwargs['start_new_session'])
        postgres = next(command for command, _, _ in launched if command[0].endswith('/postgres'))
        self.assertEqual(postgres[postgres.index('-k') + 1], '')
        self.assertEqual(postgres[postgres.index('-h') + 1], '127.0.0.1')

    def test_mocked_open_port_prevents_verified_cleanup(self):
        result, receipt, _, _ = self.run_seam(port_closed=False)
        self.assertEqual(result, 71)
        self.assertFalse(receipt['closed_port'])
        self.assertEqual(receipt['postgres_cleanup'], 'cleanup_unconfirmed')

    def test_mocked_foreign_listener_never_reaches_createdb_or_tests(self):
        result, receipt, launched, _ = self.run_seam(listener=False)
        self.assertEqual(result, 72)
        self.assertIn('startup deadline', receipt['error'])
        self.assertFalse(any(command[0].endswith('/createdb') or command[0] == 'synthetic-test' for command, _, _ in launched))

    def test_mocked_missing_birth_capture_cannot_claim_cleanup(self):
        result, receipt, _, stopped = self.run_seam(capture=False)
        self.assertEqual(result, 71)
        self.assertIn('child ownership unconfirmed', receipt['cleanup_errors'])
        self.assertEqual(stopped.call_args.args, ([],))

    def test_mocked_test_timeout_keeps_original_deadline_result(self):
        result, receipt, _, _ = self.run_seam(test_timeout=True)
        self.assertEqual(result, 124)
        self.assertEqual(receipt['exit'], 124)
        self.assertEqual(receipt['test_timeout_seconds'], 180)
        self.assertIsNone(receipt['test_returncode'])

    def test_mocked_capture_exception_keeps_child_registered_and_cleanup_unconfirmed(self):
        result, receipt, launched, stopped = self.run_seam(row_raises=True)
        self.assertEqual(result, 71)
        self.assertEqual(receipt['unconfirmed_child_pids'], [launched[0][2].pid])
        self.assertEqual(stopped.call_args.args, ([],))

    def test_mocked_final_observation_exception_still_attempts_cleanup_and_writes_receipt(self):
        result, receipt, _, stopped = self.run_seam(final_same_raises=True)
        self.assertEqual(result, 71)
        self.assertIn('PermissionError', receipt['cleanup_errors'])
        self.assertEqual(receipt['postgres_cleanup'], 'cleanup_unconfirmed')
        stopped.assert_called_once()

    def test_mocked_cancel_racing_successful_wait_cannot_become_success(self):
        result, receipt, _, _ = self.run_seam(cancel_on_test_wait=True)
        self.assertEqual(result, 143)
        self.assertEqual(receipt['exit'], 143)
        self.assertEqual(receipt['error'], 'interrupted')


if __name__ == '__main__':
    unittest.main()
