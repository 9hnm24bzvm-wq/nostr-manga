"""Actual finite Python-child fault/escape controls. No browser or network."""
import importlib.util
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import unittest
from types import SimpleNamespace
from unittest.mock import patch

spec = importlib.util.spec_from_file_location('qualification_owner_tested', Path(__file__).with_name('qualification_process_owner.py'))
owner_module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(owner_module)


class QualificationOwnerTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix='qualification-owner-')
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.addCleanup(patch.stopall)
        patch.object(owner_module.shutil, 'disk_usage', return_value=SimpleNamespace(free=owner_module.LAUNCH_DISK + 1)).start()
        patch.object(owner_module, 'available_memory', return_value=owner_module.MEMORY_FLOOR + 1).start()

    def run_owned(self, source, deadline=2):
        return owner_module.run([sys.executable, '-I', '-c', source], self.root,
                                dict(os.environ), self.root, 'fixture', deadline)

    def receipt(self):
        return json.loads((self.root / 'fixture-resource-receipt.json').read_text())

    def assert_no_live_births(self):
        journal = json.loads((self.root / 'fixture-owner/owner.json').read_text())
        for birth in journal['births']:
            current = owner_module.lifecycle.process(birth['pid'])
            self.assertTrue(current is None or current['birth'] != birth['birth'], birth)
        return journal

    def test_normal_completion_requires_terminal_eof_and_unforced_cleanup(self):
        result = self.run_owned('print("owned complete",flush=True)')
        self.assertEqual(result['status'], 'passed')
        self.assertEqual(result['exitCode'], 0)
        self.assertTrue(result['cleanup']['verified_stopped'])
        self.assertTrue(result['cleanup']['stdout_eof'])
        self.assertFalse(result['cleanup']['forced'])
        self.assertEqual(result['ownedSurvivors'], [])
        self.assertGreater(len(result['samples']), 0)
        self.assert_no_live_births()

    def test_zero_exit_detached_reparented_descendant_prevents_false_pass(self):
        child = ("import signal,time,pathlib; signal.signal(signal.SIGTERM,signal.SIG_IGN); "
                 "pathlib.Path('ready').write_text('ready'); time.sleep(20)")
        source = ("import subprocess,sys,time,pathlib; subprocess.Popen([sys.executable,'-I','-c',"
                  + repr(child) + "],start_new_session=True)\n"
                  "while not pathlib.Path('ready').exists(): time.sleep(.005)")
        result = self.run_owned(source)
        self.assertEqual(result['exitCode'], 0)
        self.assertEqual(result['status'], 'failed')
        self.assertEqual(result['stopReason'], 'forced-cleanup')
        self.assertTrue(result['cleanup']['forced'])
        self.assertTrue(result['cleanup']['verified_stopped'])
        self.assertGreaterEqual(len(self.assert_no_live_births()['births']), 2)

    def test_unrelated_preexisting_sibling_is_not_signalled(self):
        sibling = subprocess.Popen([sys.executable, '-I', '-c', 'import time; time.sleep(20)'],
                                   start_new_session=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        try:
            result = self.run_owned('import time; time.sleep(20)', deadline=.5)
            self.assertEqual(result['stopReason'], 'deadline')
            self.assertEqual(result['status'], 'failed')
            self.assertTrue(result['cleanup']['verified_stopped'])
            self.assertIsNone(sibling.poll())
            self.assertNotIn(sibling.pid, [b['pid'] for b in self.assert_no_live_births()['births']])
        finally:
            sibling.terminate()
            sibling.wait(timeout=2)

    def test_memory_read_exception_after_launch_still_cleans_registered_birth(self):
        with patch.object(owner_module, 'available_memory', side_effect=[owner_module.MEMORY_FLOOR + 1, OSError('synthetic meminfo denial')]):
            with self.assertRaisesRegex(RuntimeError, 'owner failed'):
                self.run_owned('import time; time.sleep(20)')
        result = self.receipt()
        self.assertEqual(result['status'], 'failed')
        self.assertEqual(result['errorType'], 'OSError')
        self.assertTrue(result['cleanup']['verified_stopped'])
        self.assertTrue(result['cleanup']['forced'])
        self.assert_no_live_births()

    def test_disk_read_exception_after_launch_still_cleans_registered_birth(self):
        with patch.object(owner_module.shutil, 'disk_usage', side_effect=[SimpleNamespace(free=owner_module.LAUNCH_DISK + 1), OSError('synthetic disk failure')]):
            with self.assertRaisesRegex(RuntimeError, 'owner failed'):
                self.run_owned('import time; time.sleep(20)')
        self.assertEqual(self.receipt()['errorType'], 'OSError')
        self.assertTrue(self.receipt()['cleanup']['verified_stopped'])
        self.assert_no_live_births()

    def test_keyboard_interrupt_records_failure_then_propagates_only_after_cleanup(self):
        with patch.object(owner_module.QualificationOwner, 'resource_sample', side_effect=KeyboardInterrupt):
            with self.assertRaises(KeyboardInterrupt):
                self.run_owned('import time; time.sleep(20)')
        result = self.receipt()
        self.assertEqual(result['status'], 'failed')
        self.assertEqual(result['stopReason'], 'interrupted')
        self.assertTrue(result['cleanup']['verified_stopped'])
        self.assert_no_live_births()

    def test_sigterm_marks_interruption_without_abandoning_child(self):
        original = owner_module.QualificationOwner.resource_sample
        def interrupt(owner, rows):
            os.kill(os.getpid(), signal.SIGTERM)
            return original(owner, rows)
        with patch.object(owner_module.QualificationOwner, 'resource_sample', interrupt):
            with self.assertRaises(KeyboardInterrupt):
                self.run_owned('import time; time.sleep(20)')
        self.assertEqual(self.receipt()['status'], 'failed')
        self.assertTrue(self.receipt()['cleanup']['verified_stopped'])
        self.assert_no_live_births()

    def census_fault(self, error):
        original = owner_module.QualificationOwner.observe
        # Assign after launch so the gated birth has already been registered.
        def observe(owner):
            owner._rows = lambda: (_ for _ in ()).throw(error)
            return original(owner)
        with patch.object(owner_module.QualificationOwner, 'observe', observe):
            with self.assertRaisesRegex(RuntimeError, 'owner failed'):
                self.run_owned('import time; time.sleep(20)', deadline=.8)
        result = self.receipt()
        self.assertEqual(result['status'], 'failed')
        self.assertEqual(result['stopReason'], 'cleanup-unverified')
        self.assertIsNone(result['ownedSurvivors'])
        self.assertFalse(result['cleanup']['verified_stopped'])
        self.assertGreater(result['cleanup']['tracking_errors'], 0)
        self.assert_no_live_births()  # Captured pidfds still terminated the child.

    def test_unreadable_census_is_uncertain_not_silently_empty(self):
        self.census_fault(PermissionError('synthetic proc denial'))

    def test_malformed_census_is_uncertain_not_silently_empty(self):
        self.census_fault(ValueError('synthetic malformed stat'))

    def test_retained_stdout_cap_fails_closed_including_buffered_tail(self):
        with patch.object(owner_module.lifecycle, 'MAX_LOG', 16):
            result = self.run_owned('print("x"*5000,flush=True)')
        self.assertEqual(result['status'], 'failed')
        self.assertEqual(result['stopReason'], 'output-limit')
        self.assertEqual((self.root / 'fixture.log').stat().st_size, 16)
        self.assertTrue(result['cleanup']['stdout_eof'])
        self.assert_no_live_births()

    def test_sample_and_evidence_growth_caps_cannot_be_reported_as_pass(self):
        with patch.object(owner_module, 'MAX_SAMPLE_BYTES', 1):
            result = self.run_owned('import time; time.sleep(20)')
        self.assertEqual(result['stopReason'], 'sample-evidence-limit')
        self.assertEqual(result['status'], 'failed')
        self.assertEqual(result['samples'], [])
        self.assert_no_live_births()

    def test_generated_evidence_growth_is_sampled_and_bounded(self):
        with patch.object(owner_module, 'MAX_EVIDENCE_GROWTH', 1), \
                patch.object(owner_module, 'evidence_size', side_effect=[0, 2]):
            result = self.run_owned('import time; time.sleep(20)')
        self.assertEqual(result['stopReason'], 'evidence-growth-limit')
        self.assertEqual(result['status'], 'failed')
        self.assert_no_live_births()
