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
import {ui10DiagnosticChecks as ui10} from './scripts/browser-ui10-journeys.mjs';
import {localLibraryDiagnosticChecks as local, ordinaryUploadDiagnosticChecks as upload, ordinaryReaderDiagnosticChecks as reader, ordinaryEditorialDiagnosticChecks as editorial} from './scripts/browser-journeys.mjs';
import {signedLibraryDiagnosticChecks as signed} from './scripts/browser-signed-journeys.mjs';
process.stdout.write(JSON.stringify({'account-recovery':account,...ui10,...ui,...local,...upload,...reader,...editorial,...signed}));'''
        value = json.loads(subprocess.run(['node', '--input-type=module', '-e', code], cwd=ROOT, text=True, check=True, capture_output=True).stdout)
        self.assertEqual(value, contract.CHECKS)

    def phase(self, phase, statuses=None):
        statuses = statuses or ['passed'] * len(contract.CHECKS[phase])
        key = 'account_recovery_checks' if phase == 'account-recovery' else 'journey_checks'
        acceptance = {'phase': phase, 'passed': True, 'completed_utc': 'synthetic-finalized', 'proxy_lifecycle': {'connect_denials': 0, 'expected_connect_resets': 0, 'remaining_socket_count': 0, 'observations_truncated': False, 'shutdown_started': True, 'closed': True}, 'cleanup': {'browser_created': True, 'context_created': True, 'page_created': True}, 'runner_lifecycle': {'stage': 'complete', 'fatal_kind': 'none', 'fatal_code': 'none', 'fatal_stage': 'none', 'caught_stage': 'none', 'caught_category': 'none', 'exit_observed': True, 'exit_code': 0}, 'steps': [{'status': 'passed'}], 'page_errors': [],
                      key: [{'check_id': check_id, 'status': status} for check_id, status in zip(contract.CHECKS[phase], statuses)]}
        if phase == 'ui12-compact-feed':
            acceptance['compact_feed_http_outcome'] = 'success200'
            acceptance['compact_feed_database_failure'] = 'none'
        if phase == 'ui14-actor-follow':
            acceptance['actor_follow_initial_observations'] = self.actor_observations()
        owned = {'fixture_only': True, 'phase': phase, 'verified_stopped': True, 'forced': False, 'returncode': 0}
        return acceptance, owned, key

    def actor_observations(self):
        return [dict(kind=kind, route='current', header='one', follow='disabled',
                     unfollow='absent', retry='absent', block='absent', unblock='absent',
                     get='one', post='zero', delete='zero', put='zero',
                     request_actor='prerequisite-actor', held_status='native204', delivery='pending')
                for kind in ['group', 'user']]

    def test_actor_observation_vocabulary_matches_actual_frozen_node_export(self):
        code = "import {actorInitialObservationValues as values} from './scripts/browser-actor-follow-observations.mjs';process.stdout.write(JSON.stringify(values));"
        result = subprocess.run(['node', '--input-type=module', '-e', code], cwd=ROOT, text=True, check=True, capture_output=True)
        self.assertEqual(json.loads(result.stdout), contract.ACTOR_INITIAL_OBSERVATION_VALUES)

    def test_actor_observations_reject_private_unknown_duplicate_and_reordered_rows(self):
        secret = 'private@example.invalid?token=never-publish'
        good = self.actor_observations()
        malformed = [None, secret, {}, good[::-1], [good[0], good[0]], good + [good[0]]]
        for key in contract.ACTOR_INITIAL_OBSERVATION_VALUES:
            row = copy.deepcopy(good); row[0][key] = secret; malformed.append(row)
            row = copy.deepcopy(good); del row[0][key]; malformed.append(row)
        row = copy.deepcopy(good); row[0]['raw_header'] = secret; malformed.append(row)
        for raw in malformed:
            valid, rows = contract.project_actor_initial_observations(raw)
            self.assertFalse(valid)
            self.assertNotIn(secret, json.dumps(rows))
            acceptance, _, _ = self.phase('ui14-actor-follow')
            acceptance['actor_follow_initial_observations'] = raw
            projected = contract.project('ui14-actor-follow', acceptance)
            self.assertFalse(projected['record_valid'])
            self.assertNotIn(secret, json.dumps(projected))

    def test_actor_observations_are_required_for_success_and_honest_for_failure(self):
        phase = 'ui14-actor-follow'; acceptance, owned, _ = self.phase(phase)
        baseline = runner.project_phase(phase, 'actor-follow', acceptance, owned)
        for key, values in contract.ACTOR_INITIAL_OBSERVATION_VALUES.items():
            if key == 'kind': continue
            for value in values:
                row = copy.deepcopy(baseline)
                row['journey_diagnostics']['actor_follow_initial_observations'][0][key] = value
                if value == self.actor_observations()[0][key]: continue
                with self.assertRaises(ValueError): validator.phases({'schema': 1, 'phases': [row], 'batches': []})
                row['status'] = 'failed'
                validator.phases({'schema': 1, 'phases': [row], 'batches': []})
        for raw in [[], self.actor_observations()[:1]]:
            row = copy.deepcopy(baseline); row['journey_diagnostics']['actor_follow_initial_observations'] = raw
            with self.assertRaises(ValueError): validator.phases({'schema': 1, 'phases': [row], 'batches': []})
            row['status'] = 'failed'; validator.phases({'schema': 1, 'phases': [row], 'batches': []})
        row = copy.deepcopy(baseline); del row['journey_diagnostics']['actor_follow_initial_observations']
        with self.assertRaises(ValueError): validator.phases({'schema': 1, 'phases': [row], 'batches': []})

    def test_actor_follow_nested_failure_is_closed_and_keeps_unreached_checks_pending(self):
        phase = 'ui14-actor-follow'
        self.assertEqual(len(contract.CHECKS[phase]), 79)
        self.assertEqual(len(set(contract.CHECKS[phase])), 79)
        acceptance, owned, key = self.phase(phase)
        ids = contract.CHECKS[phase]
        failed = ids.index('actor-follow-group-initial-readiness-status-pending')
        acceptance[key] = [{'check_id': value, 'status': 'failed' if value in [
            'actor-follow-group-initial-readiness', ids[failed]] else 'passed'}
            for value in ids[:failed + 1]]
        acceptance['passed'] = False
        acceptance['steps'][0]['status'] = 'failed'
        owned['returncode'] = 1
        row = runner.project_phase(phase, 'actor-follow', acceptance, owned)
        diagnostics = row['journey_diagnostics']
        self.assertTrue(diagnostics['record_valid'])
        self.assertEqual(diagnostics['check_results'][failed]['status'], 'failed')
        self.assertTrue(all(value['status'] == 'pending' for value in diagnostics['check_results'][failed + 1:]))
        validator.phases({'schema': 1, 'phases': [row], 'batches': []})
        acceptance[key][-1]['error'] = 'private@example.invalid?token=never-publish'
        projection = contract.project(phase, acceptance)
        self.assertFalse(projection['record_valid'])
        self.assertNotIn('never-publish', json.dumps(projection))

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

    def test_compact_outcome_matches_actual_frozen_js_enum(self):
        code = "import {compactFeedHttpOutcomes as outcomes} from './scripts/compact-feed-http-outcome.mjs';process.stdout.write(JSON.stringify(outcomes));"
        result = subprocess.run(['node','--input-type=module','-e',code],cwd=ROOT,text=True,check=True,capture_output=True)
        self.assertEqual(json.loads(result.stdout), contract.COMPACT_HTTP_OUTCOMES)

    def test_compact_outcome_is_finite_and_required_for_pass(self):
        phase = 'ui12-compact-feed'; acceptance, owned, _ = self.phase(phase)
        baseline = runner.project_phase(phase, 'ui-reader', acceptance, owned)
        secret = 'private@example.invalid?token=never-publish'
        for outcome in [None, secret, {}, [], False, 200]:
            adverse = dict(acceptance, compact_feed_http_outcome=outcome)
            row = runner.project_phase(phase, 'ui-reader', adverse, owned)
            self.assertFalse(row['journey_diagnostics']['record_valid'])
            self.assertNotIn(secret, json.dumps(row))
            with self.assertRaises(ValueError): validator.phases({'schema':1,'phases':[row],'batches':[]})
        for outcome in contract.COMPACT_HTTP_OUTCOMES[1:] + ['unavailable']:
            row = copy.deepcopy(baseline); row['journey_diagnostics']['compact_feed_http_outcome'] = outcome
            with self.assertRaises(ValueError): validator.phases({'schema':1,'phases':[row],'batches':[]})
        for key in ['compact_feed_http_outcome', 'journey_diagnostics']:
            row = copy.deepcopy(baseline)
            if key == 'journey_diagnostics': row.pop(key)
            else: row['journey_diagnostics'].pop(key)
            with self.assertRaises(ValueError): validator.phases({'schema':1,'phases':[row],'batches':[]})
        acceptance.pop('compact_feed_http_outcome'); acceptance['passed'] = False
        acceptance['steps'][0]['status'] = 'failed'; owned['returncode'] = 1
        row = runner.project_phase(phase, 'ui-reader', acceptance, owned)
        self.assertEqual(row['journey_diagnostics']['compact_feed_http_outcome'], 'unavailable')
        validator.phases({'schema':1,'phases':[row],'batches':[]})
        for outcome in contract.COMPACT_HTTP_OUTCOMES:
            row = runner.project_phase(phase, 'ui-reader', dict(acceptance, compact_feed_http_outcome=outcome), owned)
            self.assertEqual(row['journey_diagnostics']['compact_feed_http_outcome'], outcome)
            validator.phases({'schema':1,'phases':[row],'batches':[]})

    def test_compact_database_failure_is_closed_and_required_for_pass(self):
        code = "import {compactFeedDatabaseFailures as values} from './scripts/compact-feed-http-outcome.mjs';process.stdout.write(JSON.stringify(values));"
        result = subprocess.run(['node','--input-type=module','-e',code],cwd=ROOT,text=True,check=True,capture_output=True)
        self.assertEqual(json.loads(result.stdout), contract.COMPACT_DATABASE_FAILURES)
        phase = 'ui12-compact-feed'; acceptance, owned, _ = self.phase(phase)
        baseline = runner.project_phase(phase, 'ui-reader', acceptance, owned)
        secret = 'private SQL URL and server detail'
        for failure in [None, secret, {}, [], False, 503]:
            row = runner.project_phase(phase, 'ui-reader', dict(acceptance, compact_feed_database_failure=failure), owned)
            self.assertFalse(row['journey_diagnostics']['record_valid'])
            self.assertNotIn(secret, json.dumps(row))
            with self.assertRaises(ValueError): validator.phases({'schema':1,'phases':[row],'batches':[]})
        for failure in contract.COMPACT_DATABASE_FAILURES[1:]:
            row = copy.deepcopy(baseline); row['journey_diagnostics']['compact_feed_database_failure'] = failure
            with self.assertRaises(ValueError): validator.phases({'schema':1,'phases':[row],'batches':[]})
        missing = dict(acceptance); missing.pop('compact_feed_database_failure')
        row = runner.project_phase(phase, 'ui-reader', missing, owned)
        self.assertEqual(row['journey_diagnostics']['compact_feed_database_failure'], 'unavailable')
        with self.assertRaises(ValueError): validator.phases({'schema':1,'phases':[row],'batches':[]})
        for failure in contract.COMPACT_DATABASE_FAILURES[1:]:
            failed = dict(acceptance, passed=False, compact_feed_http_outcome='unavailable503', compact_feed_database_failure=failure, steps=[{'status':'failed'}])
            row = runner.project_phase(phase, 'ui-reader', failed, dict(owned, returncode=1))
            self.assertEqual(row['journey_diagnostics']['compact_feed_database_failure'], failure)
            validator.phases({'schema':1,'phases':[row],'batches':[]})
        row = copy.deepcopy(baseline); row['journey_diagnostics'].pop('compact_feed_database_failure')
        with self.assertRaises(ValueError): validator.phases({'schema':1,'phases':[row],'batches':[]})

    def test_all_instrumented_phases_reject_missing_duplicate_reversed_or_private_check_rows(self):
        for phase in contract.CHECKS:
            acceptance, owned, key = self.phase(phase)
            original = acceptance[key]
            for rows in [[], original[1:], original[::-1], original + [original[0]],
                         [dict(original[0], raw_value='private')] + original[1:]]:
                row = runner.project_phase(phase, 'ordinary-a', dict(acceptance, **{key:rows}), owned)
                with self.assertRaises(ValueError): validator.phases({'schema':1,'phases':[row],'batches':[]})
            row = runner.project_phase(phase, 'ordinary-a', acceptance, owned); row.pop('journey_diagnostics')
            with self.assertRaises(ValueError): validator.phases({'schema':1,'phases':[row],'batches':[]})

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
