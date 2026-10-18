"""Independent exhaustive caught-diagnostic associations; no browser or fixture."""
import copy
import importlib.util
import itertools
import json
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[2]


def load(name, file):
    spec = importlib.util.spec_from_file_location(name, ROOT / 'scripts/ci' / file)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


contract = load('caught_matrix_contract', 'runner-diagnostics.py')
runner = load('caught_matrix_runner', 'run-hosted-browser.py')
validator = load('caught_matrix_validator', 'validate-ci-evidence.py')
MARKER = 'caught-matrix-secret-marker'
KEY = 'bc' * 32
URL = 'https://closed-private.invalid/private-path?token=' + MARKER + '&key=' + KEY
EXPECTED = {
    'none': {'none'},
    'initializing': {'proxy-bind-permission', 'proxy-listen-failed', 'harness-failed'},
    'launching-browser': {'browser-sandbox-refused', 'browser-launch-failed', 'harness-failed'},
    'creating-context': {'context-setup-failed', 'harness-failed'},
    'running-checks': {'journey-setup-failed', 'journey-failed', 'harness-failed'},
    **{stage: {'journey-setup-failed', 'harness-failed'} for stage in
       ['public-upload-fixture', 'public-upload-signer', 'public-upload-observers']},
    **{stage: {'harness-failed'} for stage in
       ['closing-context', 'closing-browser', 'closing-proxy', 'finalizing-report', 'complete']},
}
CATEGORIES = set().union(*EXPECTED.values())


class CaughtDiagnosticMatrix(unittest.TestCase):
    def inputs(self):
        acceptance = {
            'phase': 'account', 'passed': True, 'completed_utc': 'synthetic-only',
            'steps': [{'status': 'passed'}], 'page_errors': [],
            'cleanup': {'browser_created': True, 'context_created': True, 'page_created': True},
            'proxy_lifecycle': {'connect_denials': 0, 'expected_connect_resets': 0,
                                'remaining_socket_count': 0, 'observations_truncated': False,
                                'shutdown_started': True, 'closed': True},
            'runner_lifecycle': {'stage': 'complete', 'fatal_kind': 'none', 'fatal_code': 'none',
                                 'fatal_stage': 'none', 'caught_stage': 'none', 'caught_category': 'none',
                                 'exit_observed': True, 'exit_code': 0},
        }
        owned = {'fixture_only': True, 'phase': 'account', 'verified_stopped': True,
                 'forced': False, 'returncode': 0}
        return acceptance, owned

    def validate(self, row):
        validator.phases({'schema': 1, 'phases': [row], 'batches': []})

    def safe(self, value):
        encoded = json.dumps(value)
        for token in [MARKER, KEY, URL, 'closed-private.invalid', '/private-path']:
            self.assertNotIn(token, encoded)

    def row(self, acceptance, owned):
        row = runner.project_phase('account', 'ordinary-a', acceptance, owned)
        self.validate(row)
        self.safe(row)
        return row

    def test_full_closed_stage_category_matrix_projects_without_invented_success(self):
        self.assertEqual(contract.STAGES | {'none'}, set(EXPECTED))
        self.assertEqual(contract.CAUGHT_CATEGORIES, CATEGORIES)
        for stage, category in itertools.product(sorted(EXPECTED), sorted(CATEGORIES)):
            valid = category in EXPECTED[stage]
            with self.subTest(stage=stage, category=category):
                acceptance, owned = self.inputs()
                acceptance['runner_lifecycle'].update(caught_stage=stage, caught_category=category)
                row = self.row(acceptance, owned)
                self.assertEqual(contract.caught_pair(stage, category), valid)
                self.assertEqual(row['runner_diagnostics']['record_valid'], valid)
                self.assertEqual(row['status'] == 'passed', stage == category == 'none')
                if not valid:
                    self.assertEqual(row['runner_diagnostics']['caught_stage'], 'unavailable')
                    self.assertEqual(row['runner_diagnostics']['caught_category'], 'unavailable')

    def test_serialized_matrix_rejects_bad_associations_even_when_marked_failed_invalid(self):
        acceptance, owned = self.inputs()
        baseline = self.row(acceptance, owned)
        for stage, category in itertools.product(sorted(EXPECTED), sorted(CATEGORIES)):
            with self.subTest(stage=stage, category=category):
                row = copy.deepcopy(baseline)
                row['status'] = 'failed'
                row['runner_diagnostics'].update(record_valid=False, caught_stage=stage, caught_category=category)
                if category in EXPECTED[stage]:
                    self.validate(row)
                else:
                    with self.assertRaisesRegex(ValueError, '^Evidence failed its closed public contract$'):
                        self.validate(row)

    def test_unsafe_unknown_and_nonstrings_collapse_in_projection_and_refuse_serialization(self):
        acceptance, owned = self.inputs()
        baseline = self.row(acceptance, owned)
        for field in ['caught_stage', 'caught_category']:
            for value in [URL, KEY, 'future-stage', '', True, 0, None, [], {}, ['launching-browser']]:
                with self.subTest(field=field, value=value):
                    raw = copy.deepcopy(acceptance)
                    raw['runner_lifecycle'][field] = value
                    row = self.row(raw, owned)
                    self.assertFalse(row['runner_diagnostics']['record_valid'])
                    self.assertEqual(row['runner_diagnostics']['caught_stage'], 'unavailable')
                    self.assertEqual(row['runner_diagnostics']['caught_category'], 'unavailable')
                    direct = copy.deepcopy(baseline)
                    direct['status'] = 'failed'
                    direct['runner_diagnostics'].update(record_valid=False, **{field: value})
                    with self.assertRaisesRegex(ValueError, '^Evidence failed its closed public contract$'):
                        self.validate(direct)

    def test_extra_raw_message_stack_url_key_class_and_code_fields_never_export(self):
        acceptance, owned = self.inputs()
        baseline = self.row(acceptance, owned)
        for field in ['raw_error', 'message', 'stack', 'url', 'key', 'error_class', 'code']:
            with self.subTest(field=field):
                raw = copy.deepcopy(acceptance)
                raw['runner_lifecycle'][field] = URL
                row = self.row(raw, owned)
                self.assertFalse(row['runner_diagnostics']['record_valid'])
                self.assertEqual(row['status'], 'failed')
                for target in ['phase', 'runner_diagnostics']:
                    direct = copy.deepcopy(baseline)
                    direct['status'] = 'failed'
                    container = direct if target == 'phase' else direct['runner_diagnostics']
                    container[field] = URL
                    with self.assertRaisesRegex(ValueError, '^Evidence failed its closed public contract$'):
                        self.validate(direct)

    def test_creation_flags_are_observations_not_an_acceptance_substitute(self):
        for created in itertools.product([False, True], repeat=3):
            with self.subTest(created=created):
                acceptance, owned = self.inputs()
                acceptance['cleanup'] = dict(zip(['browser_created', 'context_created', 'page_created'], created))
                row = self.row(acceptance, owned)
                valid = (not created[1] or created[0]) and (not created[2] or created[1])
                self.assertEqual(row['runner_diagnostics']['record_valid'], valid)
                self.assertEqual(row['status'] == 'passed', all(created))
                if not valid:
                    self.assertTrue(all(row['runner_diagnostics'][key] is None for key in acceptance['cleanup']))
        acceptance, owned = self.inputs()
        acceptance['runner_lifecycle'].update(caught_stage='launching-browser', caught_category='browser-launch-failed')
        self.assertEqual(self.row(acceptance, owned)['status'], 'failed')

    def test_failed_app_check_and_zero_check_launcher_failure_remain_distinct(self):
        acceptance, owned = self.inputs()
        acceptance.update(passed=False, steps=[{'status': 'failed', 'error': URL}])
        acceptance['runner_lifecycle']['exit_code'] = 1
        app = self.row(acceptance, dict(owned, returncode=1))
        self.assertEqual(app['runner_diagnostics']['caught_category'], 'none')
        self.assertFalse(app['harness_error_observed'])
        self.assertEqual(app['failed_steps'], [{'index': 0, 'status': 'failed'}])
        acceptance.update(steps=[], harness_error=URL)
        acceptance['cleanup'] = dict.fromkeys(acceptance['cleanup'], False)
        acceptance['runner_lifecycle'].update(caught_stage='launching-browser', caught_category='browser-launch-failed', exit_code=1)
        launch = self.row(acceptance, dict(owned, returncode=1))
        self.assertEqual(launch['actual_steps'], 0)
        self.assertEqual(launch['failed_steps'], [])
        self.assertTrue(launch['harness_error_observed'])
        self.assertEqual(launch['runner_diagnostics']['caught_stage'], 'launching-browser')
        self.assertFalse(launch['runner_diagnostics']['browser_created'])

    def test_duplicate_caught_fields_are_rejected_by_public_json_parser(self):
        acceptance, owned = self.inputs()
        row = self.row(acceptance, owned)
        encoded = json.dumps({'schema': 1, 'phases': [row], 'batches': []})
        for field in ['caught_stage', 'caught_category']:
            original = '"' + field + '": "none"'
            duplicate = '"' + field + '": "' + MARKER + '", ' + original
            self.assertEqual(encoded.count(original), 1)
            with self.assertRaisesRegex(ValueError, '^Evidence failed its closed public contract$'):
                json.loads(encoded.replace(original, duplicate), object_pairs_hook=validator.unique_object)


if __name__ == '__main__':
    unittest.main()
