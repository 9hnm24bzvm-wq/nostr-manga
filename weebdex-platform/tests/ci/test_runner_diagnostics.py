"""Closed finite exit evidence; no browser, server or database."""
import copy
import importlib.util
import json
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[2]


def load(name, filename):
    spec = importlib.util.spec_from_file_location(name, ROOT / 'scripts/ci' / filename)
    module = importlib.util.module_from_spec(spec); spec.loader.exec_module(module)
    return module


contract = load('exit_contract', 'runner-diagnostics.py')
runner = load('exit_runner', 'run-hosted-browser.py')
validator = load('exit_validator', 'validate-ci-evidence.py')


class RunnerDiagnosticSeams(unittest.TestCase):
    def inputs(self):
        acceptance = {'phase': 'account', 'passed': True, 'completed_utc': 'synthetic-only', 'proxy_lifecycle': {'connect_denials': 0, 'expected_connect_resets': 0, 'remaining_socket_count': 0, 'observations_truncated': False, 'shutdown_started': True, 'closed': True},
                      'steps': [{'status': 'passed'}], 'page_errors': [],
                      'cleanup': {'browser_created': True, 'context_created': True, 'page_created': True}, 'runner_lifecycle': {'stage': 'complete', 'fatal_kind': 'none', 'fatal_code': 'none', 'fatal_stage': 'none', 'caught_stage': 'none', 'caught_category': 'none', 'exit_observed': True, 'exit_code': 0}}
        owned = {'fixture_only': True, 'phase': 'account', 'verified_stopped': True, 'forced': False, 'returncode': 0}
        return acceptance, owned

    def row(self, acceptance, owned):
        row = runner.project_phase('account', 'ordinary-a', acceptance, owned)
        validator.phases({'schema': 1, 'phases': [row], 'batches': []})
        return row

    def test_normal_terminal_report_and_exit_are_required(self):
        acceptance, owned = self.inputs()
        self.assertEqual(self.row(acceptance, owned)['status'], 'passed')
        for key in ['runner_lifecycle', 'completed_utc', 'passed']:
            adverse = copy.deepcopy(acceptance); adverse.pop(key)
            self.assertEqual(self.row(adverse, owned)['status'], 'failed')

    def test_all_passed_steps_with_nonzero_exit_still_fail(self):
        acceptance, owned = self.inputs()
        for code in [1, 7, -9, -15, None, False, 999, 'private']:
            row = self.row(acceptance, dict(owned, returncode=code))
            self.assertEqual(row['status'], 'failed')
            self.assertEqual(row['failed_steps'], [])
            self.assertFalse(row['runner_exit_zero'])
            self.assertEqual(row['runner_diagnostics']['owned_exit_code'], code if type(code) is int and -64 <= code <= 255 else None)

    def test_unfinalized_teardown_and_late_rejection_are_distinct(self):
        acceptance, owned = self.inputs()
        for stage, finalized in [('closing-browser', False), ('complete', True)]:
            adverse = copy.deepcopy(acceptance)
            if not finalized:
                adverse.pop('passed'); adverse.pop('completed_utc')
            adverse['runner_lifecycle'].update(stage=stage, fatal_kind='unhandled-rejection', fatal_code='unknown', fatal_stage=stage, exit_code=1)
            row = self.row(adverse, dict(owned, returncode=1))
            self.assertEqual(row['status'], 'failed')
            self.assertEqual(row['runner_diagnostics']['report_finalized'], finalized)
            self.assertEqual(row['runner_diagnostics']['fatal_stage'], stage)

    def test_unknown_private_or_extra_raw_fields_never_project(self):
        acceptance, owned = self.inputs(); secret = 'private@example.invalid?token=do-not-export'
        for value in [secret, None, {}, dict(acceptance['runner_lifecycle'], raw_error=secret),
                      dict(acceptance['runner_lifecycle'], stage=secret), dict(acceptance['runner_lifecycle'], exit_code=secret),
                      dict(acceptance['runner_lifecycle'], fatal_stage='closing-browser')]:
            adverse = dict(acceptance, runner_lifecycle=value)
            row = self.row(adverse, owned)
            self.assertEqual(row['status'], 'failed')
            self.assertFalse(row['runner_diagnostics']['record_valid'])
            self.assertNotIn(secret, json.dumps(row))

    def test_proxy_ownership_evidence_is_closed_and_required_for_pass(self):
        acceptance, owned = self.inputs()
        secret = 'private@example.invalid?token=do-not-export'
        for raw in [None, secret, {}, dict(acceptance['proxy_lifecycle'], raw_error=secret),
                    dict(acceptance['proxy_lifecycle'], connect_denials=True),
                    dict(acceptance['proxy_lifecycle'], expected_connect_resets=65536),
                    dict(acceptance['proxy_lifecycle'], shutdown_started=False),
                    dict(acceptance['proxy_lifecycle'], remaining_socket_count=1)]:
            row = self.row(dict(acceptance, proxy_lifecycle=raw), owned)
            self.assertEqual(row['status'], 'failed')
            self.assertNotIn(secret, json.dumps(row))
        adverse = copy.deepcopy(acceptance); adverse['proxy_lifecycle']['closed'] = False
        self.assertEqual(self.row(adverse, owned)['status'], 'failed')
        for code in ['ECONNRESET', 'EPIPE', 'ERR_STREAM_DESTROYED', 'ERR_SERVER_NOT_RUNNING', 'unknown']:
            adverse = copy.deepcopy(acceptance)
            adverse['runner_lifecycle'].update(stage='closing-browser', fatal_kind='uncaught-exception',
                                               fatal_code=code, fatal_stage='closing-browser', exit_code=1)
            row = self.row(adverse, dict(owned, returncode=1))
            self.assertEqual(row['status'], 'failed')
            self.assertEqual(row['runner_diagnostics']['fatal_code'], code)

    def test_serialized_proxy_and_fatal_code_omissions_cannot_pass(self):
        acceptance, owned = self.inputs(); baseline = self.row(acceptance, owned)
        mutations = [lambda d: d.pop('fatal_code'), lambda d: d.update(fatal_code='private'),
                     lambda d: d.update(fatal_code='ECONNRESET'), lambda d: d.pop('proxy_diagnostics'),
                     lambda d: d['proxy_diagnostics'].pop('closed'),
                     lambda d: d['proxy_diagnostics'].update(raw_error='private'),
                     lambda d: d['proxy_diagnostics'].update(record_valid=False),
                     lambda d: d['proxy_diagnostics'].update(closed=False),
                     lambda d: d['proxy_diagnostics'].update(shutdown_started=False),
                     lambda d: d['proxy_diagnostics'].update(remaining_socket_count=1),
                     lambda d: d['proxy_diagnostics'].update(connect_denials=True),
                     lambda d: d['proxy_diagnostics'].update(expected_connect_resets=65536)]
        for mutate in mutations:
            row = copy.deepcopy(baseline); mutate(row['runner_diagnostics'])
            with self.assertRaises(ValueError): validator.phases({'schema': 1, 'phases': [row], 'batches': []})

    def test_signal_or_missing_report_preserves_failed_observation(self):
        row = self.row({}, {'returncode': -15})
        self.assertFalse(row['runner_report_present'])
        self.assertFalse(row['runner_diagnostics']['exit_observed'])
        self.assertEqual(row['runner_diagnostics']['owned_exit_code'], -15)

    def test_caught_pre_step_failure_survives_final_cleanup_without_a_fatal_exit(self):
        acceptance, owned = self.inputs()
        acceptance.update(passed=False, steps=[], harness_error='private-value?token=never-export')
        acceptance['runner_lifecycle'].update(caught_stage='launching-browser', caught_category='browser-sandbox-refused', exit_code=1)
        row = self.row(acceptance, dict(owned, returncode=1))
        value = row['runner_diagnostics']
        self.assertEqual(row['status'], 'failed')
        self.assertEqual(row['actual_steps'], 0)
        self.assertEqual(value['stage'], 'complete')
        self.assertEqual(value['fatal_kind'], 'none')
        self.assertEqual(value['caught_stage'], 'launching-browser')
        self.assertEqual(value['caught_category'], 'browser-sandbox-refused')
        self.assertNotIn('private-value', json.dumps(row))
        # Even a forged success exit/report cannot turn an observed catch into a pass.
        acceptance.update(passed=True, steps=[{'status': 'passed'}]); acceptance.pop('harness_error')
        acceptance['runner_lifecycle']['exit_code'] = 0
        self.assertEqual(self.row(acceptance, owned)['status'], 'failed')

    def test_caught_records_refuse_unknown_missing_extra_or_cross_stage_facts(self):
        acceptance, owned = self.inputs()
        baseline = acceptance['runner_lifecycle']
        private = 'https://private.invalid/path?nsec=do-not-export'
        for changes in [{'caught_stage': private}, {'caught_category': private},
                        {'caught_stage': 'none', 'caught_category': 'browser-launch-failed'},
                        {'caught_stage': 'launching-browser', 'caught_category': 'none'},
                        {'caught_stage': 'creating-context', 'caught_category': 'browser-sandbox-refused'},
                        {'caught_stage': [], 'caught_category': {}}, {'raw_error': private}]:
            with self.subTest(changes=changes):
                raw = dict(baseline, **changes)
                row = self.row(dict(acceptance, runner_lifecycle=raw), owned)
                self.assertFalse(row['runner_diagnostics']['record_valid'])
                self.assertEqual(row['status'], 'failed')
                self.assertNotIn(private, json.dumps(row))
        for key in ['caught_stage', 'caught_category']:
            raw = dict(baseline); raw.pop(key)
            row = self.row(dict(acceptance, runner_lifecycle=raw), owned)
            self.assertFalse(row['runner_diagnostics']['record_valid'])
            self.assertEqual(row['runner_diagnostics']['caught_stage'], 'unavailable')
            self.assertEqual(row['runner_diagnostics']['caught_category'], 'unavailable')

    def test_creation_facts_are_actual_booleans_or_explicitly_unavailable(self):
        acceptance, owned = self.inputs()
        for setup in [None, {}, {'browser_created': True},
                      {'browser_created': 1, 'context_created': True, 'page_created': True},
                      {'browser_created': False, 'context_created': True, 'page_created': False},
                      {'browser_created': True, 'context_created': False, 'page_created': True}]:
            with self.subTest(setup=setup):
                row = self.row(dict(acceptance, cleanup=setup), owned)
                self.assertFalse(row['runner_diagnostics']['record_valid'])
                self.assertEqual(row['status'], 'failed')
                for key in ['browser_created', 'context_created', 'page_created']:
                    self.assertIsNone(row['runner_diagnostics'][key])
        for flags in [(False, False, False), (True, False, False), (True, True, False)]:
            setup = dict(zip(['browser_created', 'context_created', 'page_created'], flags))
            row = self.row(dict(acceptance, cleanup=setup), owned)
            self.assertTrue(row['runner_diagnostics']['record_valid'])
            self.assertEqual(row['status'], 'failed')
            self.assertEqual(tuple(row['runner_diagnostics'][key] for key in setup), flags)

    def test_serialized_caught_projection_refuses_omissions_unknowns_and_cross_stage_claims(self):
        acceptance, owned = self.inputs(); baseline = self.row(acceptance, owned)
        for mutate in [lambda d: d.pop('caught_stage'), lambda d: d.pop('caught_category'),
                       lambda d: d.update(caught_category='private'),
                       lambda d: d.update(caught_stage='private'),
                       lambda d: d.update(caught_stage='none', caught_category='browser-launch-failed'),
                       lambda d: d.update(caught_stage='creating-context', caught_category='browser-sandbox-refused'),
                       lambda d: d.update(caught_stage='unavailable', caught_category='none'),
                       lambda d: d.update(browser_created=False, context_created=True),
                       lambda d: d.update(context_created=None),
                       lambda d: d.update(page_created='private'),
                       lambda d: d.update(raw_error='private')]:
            row = copy.deepcopy(baseline); mutate(row['runner_diagnostics'])
            # Invalid serialized associations refuse even when explicitly failed.
            row.update(status='failed')
            row['runner_diagnostics']['record_valid'] = False
            with self.assertRaises(ValueError): validator.phases({'schema': 1, 'phases': [row], 'batches': []})

    def test_serialized_gate_rejects_omission_unknowns_and_forged_pass(self):
        acceptance, owned = self.inputs(); baseline = self.row(acceptance, owned)
        mutations = [lambda row: row.pop('runner_diagnostics'),
                     lambda row: row['runner_diagnostics'].update(raw_error='private'),
                     lambda row: row['runner_diagnostics'].update(exit_code=True),
                     lambda row: row['runner_diagnostics'].update(owned_exit_code=1),
                     lambda row: row['runner_diagnostics'].update(stage='private'),
                     lambda row: row['runner_diagnostics'].update(report_finalized=False),
                     lambda row: row['runner_diagnostics'].update(record_valid=False),
                     lambda row: row['runner_diagnostics'].update(fatal_kind='unhandled-rejection', fatal_code='unknown', fatal_stage='complete')]
        for mutate in mutations:
            row = copy.deepcopy(baseline); mutate(row)
            with self.assertRaises(ValueError): validator.phases({'schema': 1, 'phases': [row], 'batches': []})


if __name__ == '__main__': unittest.main()
