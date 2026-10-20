"""Closed private-receipt projection only; no native fixture is executed."""
import copy
import json
import unittest

from test_native_title_gate import contract, feed, fixture_receipt, title_pass


class NativeFixtureDiagnostics(unittest.TestCase):
    def test_normal_retained_receipt_preserves_the_existing_strict_gate(self):
        raw = fixture_receipt()
        result = contract.fixture_diagnostics(raw, feed.fixture_cleanup_verified(raw))
        self.assertEqual(result['receipt_state'], 'available')
        self.assertEqual(result['error_family'], 'none')
        self.assertTrue(result['normal_run_verified'])
        self.assertTrue(result['physical_stopped_reported'])
        self.assertEqual(result['test_exit'], 0)
        self.assertEqual(result['database_exit'], 0)
        self.assertFalse(result['forced'])
        contract.validate_fixture_diagnostics(result)

    def test_fixed_error_families_and_reasons_never_echo_private_values(self):
        cases = [('FileNotFoundError: PRIVATE_SENTINEL', 'file-not-found', 'other'),
                 ('PermissionError: PRIVATE_SENTINEL', 'permission', 'other'),
                 ('OSError: PRIVATE_SENTINEL', 'os', 'other'),
                 ('RuntimeError: isolated initdb failed', 'runtime', 'initdb'),
                 ('RuntimeError: owned postgres startup deadline', 'runtime', 'startup-deadline'),
                 ('interrupted', 'interrupted', 'interrupted'),
                 ('owned command deadline', 'deadline', 'deadline'),
                 ('PRIVATE_SENTINEL', 'other', 'other')]
        for error, family, reason in cases:
            with self.subTest(family=family):
                raw = fixture_receipt()
                raw.update(exit=72, error=error, test_returncode=None)
                result = contract.fixture_diagnostics(raw, feed.fixture_cleanup_verified(raw))
                self.assertEqual(result['error_family'], family)
                self.assertEqual(result['error_reason'], reason)
                self.assertTrue(result['physical_stopped_reported'])
                self.assertFalse(result['normal_run_verified'])
                self.assertNotIn('PRIVATE_SENTINEL', json.dumps(result))

    def test_reported_stop_does_not_promote_force_error_or_failed_test(self):
        for changes in [{'forced': True}, {'error': 'RuntimeError: isolated createdb failed'},
                        {'exit': 72, 'test_returncode': None}, {'test_returncode': 1},
                        {'postgres_returncode': -2}]:
            with self.subTest(changes=changes):
                raw = dict(fixture_receipt(), **changes)
                self.assertFalse(feed.fixture_cleanup_verified(raw))
                result = contract.fixture_diagnostics(raw, False)
                self.assertEqual(result['receipt_state'], 'available')
                self.assertTrue(result['physical_stopped_reported'])
                self.assertFalse(result['normal_run_verified'])

    def test_unknown_or_malformed_private_receipts_are_unverified_without_echo(self):
        for changes in [{'private': 'PRIVATE_SENTINEL'}, {'exit': True}, {'forced': 0},
                        {'test_returncode': True}, {'error': {'PRIVATE_SENTINEL': 1}},
                        {'error': 'X' * 65537}, {'captured_births': [{}]},
                        {'cleanup_errors': [1]}, {'unconfirmed_child_pids': [True]},
                        {'cleanup_phase_seconds': [True, 5, 5]}]:
            with self.subTest(keys=list(changes)):
                value = contract.fixture_diagnostics(dict(fixture_receipt(), **changes), False)
                self.assertEqual(value, contract.empty_fixture_diagnostics('invalid'))
                self.assertNotIn('PRIVATE_SENTINEL', json.dumps(value))
        self.assertEqual(contract.fixture_diagnostics(None), contract.empty_fixture_diagnostics('invalid'))

    def test_nonempty_terminal_ledgers_never_report_physical_stop(self):
        for key in ['remaining_owned_processes', 'unconfirmed_births', 'unconfirmed_child_pids', 'cleanup_errors']:
            raw = fixture_receipt()
            raw[key] = ([raw['captured_births'][0]] if key in ['remaining_owned_processes', 'unconfirmed_births'] else
                        [123] if key == 'unconfirmed_child_pids' else ['PRIVATE_SENTINEL'])
            value = contract.fixture_diagnostics(raw, False)
            self.assertEqual(value['receipt_state'], 'available')
            self.assertFalse(value['physical_stopped_reported'])
            self.assertNotIn('PRIVATE_SENTINEL', json.dumps(value))

    def test_public_shape_types_codes_and_normal_contradictions_are_closed(self):
        baseline = contract.fixture_diagnostics(fixture_receipt(), True)
        for key in baseline:
            value = dict(baseline)
            del value[key]
            with self.subTest(missing=key), self.assertRaises(ValueError):
                contract.validate_fixture_diagnostics(value)
        for changes in [{'private': 'PRIVATE_SENTINEL'}, {'schema': True},
                        {'error_family': 'PRIVATE_SENTINEL'}, {'error_reason': 'PRIVATE_SENTINEL'},
                        {'test_exit': True}, {'database_exit': 256}, {'forced': True},
                        {'physical_stopped_reported': False}, {'receipt_state': 'invalid'},
                        {'error_family': 'file-not-found', 'error_reason': 'createdb'}]:
            with self.subTest(keys=list(changes)), self.assertRaises(ValueError):
                contract.validate_fixture_diagnostics(dict(baseline, **changes))

    def test_diagnostic_cannot_satisfy_or_replace_the_existing_success_gate(self):
        baseline = title_pass()
        for field, replacement in [('fixture_diagnostics', contract.empty_fixture_diagnostics('unavailable')),
                                   ('http', dict(baseline['http'], fixture_cleanup_verified=False))]:
            with self.subTest(field=field), self.assertRaises(ValueError):
                contract.validate(dict(baseline, **{field: replacement}))
        for change in [{'exit': 72}, {'test_timeout_seconds': 181}, {'captured_births': []}]:
            self.assertEqual(contract.fixture_diagnostics(dict(fixture_receipt(), **change), True),
                             contract.empty_fixture_diagnostics('invalid'))


if __name__ == '__main__':
    unittest.main()
