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
        acceptance = {'phase': 'account', 'passed': True, 'completed_utc': 'synthetic-only',
                      'steps': [{'status': 'passed'}], 'page_errors': [],
                      'runner_lifecycle': {'stage': 'complete', 'fatal_kind': 'none', 'fatal_stage': 'none', 'exit_observed': True, 'exit_code': 0}}
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
            adverse['runner_lifecycle'].update(stage=stage, fatal_kind='unhandled-rejection', fatal_stage=stage, exit_code=1)
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

    def test_signal_or_missing_report_preserves_failed_observation(self):
        row = self.row({}, {'returncode': -15})
        self.assertFalse(row['runner_report_present'])
        self.assertFalse(row['runner_diagnostics']['exit_observed'])
        self.assertEqual(row['runner_diagnostics']['owned_exit_code'], -15)

    def test_serialized_gate_rejects_omission_unknowns_and_forged_pass(self):
        acceptance, owned = self.inputs(); baseline = self.row(acceptance, owned)
        mutations = [lambda row: row.pop('runner_diagnostics'),
                     lambda row: row['runner_diagnostics'].update(raw_error='private'),
                     lambda row: row['runner_diagnostics'].update(exit_code=True),
                     lambda row: row['runner_diagnostics'].update(owned_exit_code=1),
                     lambda row: row['runner_diagnostics'].update(stage='private'),
                     lambda row: row['runner_diagnostics'].update(report_finalized=False),
                     lambda row: row['runner_diagnostics'].update(record_valid=False),
                     lambda row: row['runner_diagnostics'].update(fatal_kind='unhandled-rejection', fatal_stage='complete')]
        for mutate in mutations:
            row = copy.deepcopy(baseline); mutate(row)
            with self.assertRaises(ValueError): validator.phases({'schema': 1, 'phases': [row], 'batches': []})


if __name__ == '__main__': unittest.main()
