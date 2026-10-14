"""Finite diagnostic projection/gate seams; no services or real browser."""
import copy
import importlib.util
import json
from pathlib import Path
import subprocess
import unittest

ROOT = Path(__file__).resolve().parents[2]


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, ROOT / path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


runner = load('diagnostic_runner', 'scripts/ci/run-hosted-browser.py')
validator = load('diagnostic_validator', 'scripts/ci/validate-ci-evidence.py')
contract = load('journey_contract', 'scripts/ci/journey-diagnostics.py')
startup = load('startup_contract', 'scripts/ci/startup-diagnostics.py')


class JourneyDiagnosticSeams(unittest.TestCase):
    def test_inventory_matches_actual_node_exports(self):
        code = '''import {accountRecoveryCheckIds as account} from './scripts/browser-account-recovery.mjs';
import {ui11To16DiagnosticChecks as ui} from './scripts/browser-ui11-16-journeys.mjs';
process.stdout.write(JSON.stringify({'account-recovery':account,...ui}));'''
        value = json.loads(subprocess.run(['node', '--input-type=module', '-e', code], cwd=ROOT, text=True, check=True, capture_output=True).stdout)
        self.assertEqual(value, contract.CHECKS)

    def phase(self, phase, statuses=None):
        statuses = statuses or ['passed'] * len(contract.CHECKS[phase])
        key = 'account_recovery_checks' if phase == 'account-recovery' else 'journey_checks'
        acceptance = {'phase': phase, 'passed': True, 'completed_utc': 'synthetic-finalized', 'runner_lifecycle': {'stage': 'complete', 'fatal_kind': 'none', 'fatal_stage': 'none', 'exit_observed': True, 'exit_code': 0}, 'steps': [{'status': 'passed'}], 'page_errors': [],
                      key: [{'check_id': check_id, 'status': status} for check_id, status in zip(contract.CHECKS[phase], statuses)]}
        owned = {'fixture_only': True, 'phase': phase, 'verified_stopped': True, 'forced': False, 'returncode': 0}
        return acceptance, owned, key

    def test_all_finite_journey_projections_validate(self):
        for phase in contract.CHECKS:
            acceptance, owned, _ = self.phase(phase)
            row = runner.project_phase(phase, 'ordinary-a', acceptance, owned)
            self.assertTrue(row['journey_diagnostics']['record_valid'])
            validator.phases({'schema': 1, 'phases': [row], 'batches': []})

    def test_unentered_and_interrupted_checks_are_honest(self):
        phase = 'ui11-device-progress'
        acceptance, owned, key = self.phase(phase)
        acceptance[key] = [acceptance[key][0], {'check_id': contract.CHECKS[phase][1], 'status': 'running'}]
        acceptance['passed'] = False; acceptance['steps'][0]['status'] = 'failed'; owned['returncode'] = 1
        row = runner.project_phase(phase, 'ui-feed', acceptance, owned)
        statuses = [check['status'] for check in row['journey_diagnostics']['check_results']]
        self.assertEqual(statuses[:3], ['passed', 'running', 'pending'])
        self.assertEqual(row['status'], 'failed')
        validator.phases({'schema': 1, 'phases': [row], 'batches': []})

    def test_private_unknown_duplicate_and_extra_values_never_project(self):
        secret = 'private@example.invalid?token=never-publish'
        phase = 'account-recovery'
        acceptance, _, key = self.phase(phase)
        for raw in [None, secret, [{'check_id': secret, 'status': 'passed'}],
                    [{'check_id': contract.CHECKS[phase][0], 'status': secret}],
                    [{'check_id': contract.CHECKS[phase][0], 'status': 'passed', 'error': secret}],
                    [acceptance[key][0], acceptance[key][0]]]:
            acceptance[key] = raw
            result = contract.project(phase, acceptance)
            self.assertFalse(result['record_valid'])
            self.assertNotIn(secret, json.dumps(result))

    def test_public_gate_rejects_adverse_journey_fields_and_false_success(self):
        acceptance, owned, _ = self.phase('ui12-compact-feed')
        baseline = runner.project_phase('ui12-compact-feed', 'ui-reader', acceptance, owned)
        for mutate in [lambda d: d.update(raw_error='secret'),
                       lambda d: d.update(record_valid=1),
                       lambda d: d['check_results'][0].update(check_id='private'),
                       lambda d: d['check_results'][0].update(status='private'),
                       lambda d: d['check_results'][0].update(status='pending'),
                       lambda d: d['check_results'][0].update(body='secret')]:
            row = copy.deepcopy(baseline); mutate(row['journey_diagnostics'])
            with self.assertRaises(ValueError): validator.phases({'schema': 1, 'phases': [row], 'batches': []})
        row = copy.deepcopy(baseline); row.pop('journey_diagnostics')
        with self.assertRaises(ValueError): validator.phases({'schema': 1, 'phases': [row], 'batches': []})

    def startup_row(self):
        acceptance = {'phase': 'startup', 'origin': 'http://127.0.0.1:31234', 'passed': False,
                      'steps': [{'name': name, 'status': 'passed'} for name in startup.CHECK_NAMES], 'page_errors': []}
        acceptance['steps'][15].update(status='failed', error='Page never rendered meaningful content')
        flags = {key: True for key in startup.READINESS_FLAGS}
        acceptance['seed_title_readiness'] = {'original_body_meaningful': False, 'initial': dict(flags, body_meaningful=False, fixture_heading_visible=False), 'late': flags, 'wait_outcome': 'ready'}
        owned = {'fixture_only': True, 'phase': 'startup', 'verified_stopped': True, 'forced': False, 'returncode': 1}
        return runner.project_phase('startup', 'startup', acceptance, owned), acceptance

    def test_late_ready_does_not_promote_original_failed_startup(self):
        row, _ = self.startup_row()
        self.assertEqual(row['status'], 'failed')
        self.assertEqual(row['failed_steps'], [{'index': 15, 'status': 'failed'}])
        self.assertEqual(row['startup_diagnostics']['seed_title_readiness']['wait_outcome'], 'ready')
        validator.phases({'schema': 1, 'phases': [row], 'batches': []})

    def test_navigation_failure_before_probe_keeps_unresolved_failed_evidence(self):
        _, acceptance = self.startup_row()
        acceptance.pop('seed_title_readiness')
        acceptance['steps'][15]['error'] = 'Navigation returned 503'
        owned = {'fixture_only': True, 'phase': 'startup', 'verified_stopped': True, 'forced': False, 'returncode': 1}
        row = runner.project_phase('startup', 'startup', acceptance, owned)
        probe = row['startup_diagnostics']['seed_title_readiness']
        self.assertFalse(probe['record_valid']); self.assertEqual(probe['wait_outcome'], 'probe-error')
        self.assertEqual(row['status'], 'failed')
        validator.phases({'schema': 1, 'phases': [row], 'batches': []})
        row.pop('startup_diagnostics')
        with self.assertRaises(ValueError): validator.phases({'schema': 1, 'phases': [row], 'batches': []})

    def test_readiness_projection_and_gate_reject_private_values(self):
        baseline, acceptance = self.startup_row()
        secret = 'private@example.invalid?token=never-publish'
        for mutate in [lambda d: d.update(wait_outcome=secret), lambda d: d.update(original_body_meaningful=secret),
                       lambda d: d['initial'].update(body_meaningful=secret), lambda d: d['late'].update(raw_html=secret)]:
            raw = copy.deepcopy(acceptance['seed_title_readiness']); mutate(raw)
            safe = startup.readiness_projection(raw)
            self.assertFalse(safe['record_valid']); self.assertNotIn(secret, json.dumps(safe))
        for mutate in [lambda d: d.update(wait_outcome=secret), lambda d: d.update(record_valid=1),
                       lambda d: d['initial'].update(body_meaningful=secret), lambda d: d['late'].update(raw_html=secret),
                       lambda d: d.update(original_body_meaningful=True), lambda d: d['late'].update(fixture_route=False)]:
            row = copy.deepcopy(baseline); mutate(row['startup_diagnostics']['seed_title_readiness'])
            with self.assertRaises(ValueError): validator.phases({'schema': 1, 'phases': [row], 'batches': []})
        row = copy.deepcopy(baseline); row['startup_diagnostics'].pop('seed_title_readiness')
        with self.assertRaises(ValueError): validator.phases({'schema': 1, 'phases': [row], 'batches': []})
