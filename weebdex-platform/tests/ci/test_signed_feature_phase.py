"""Finite schedule and evidence-contract checks. No browser/service is started."""
import copy
import importlib.util
import json
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[2]
def load(name, relative):
    spec = importlib.util.spec_from_file_location(name, ROOT / relative)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module
runner = load('signed_features_runner', 'scripts/ci/run-hosted-browser.py')
diag = load('signed_features_diag', 'scripts/ci/signed-feature-diagnostics.py')
validator = load('signed_features_validator', 'scripts/ci/validate-ci-evidence.py')
existing = load('signed_feature_existing_contract', 'tests/ci/test_public_feature_phase.py')
def acceptance(count=6, failed=False):
    report = existing.accepted_report()
    report.pop('public_features')
    rows = [{'id': name, 'status': 'failed' if failed and index == count - 1 else 'passed'} for index, name in enumerate(diag.CHECKS[:count])]
    report.update(phase=diag.PHASE, passed=count == 6 and not failed,
                  steps=[{'name': 'Signed feature ' + row['id'], 'status': row['status']} for row in rows],
                  signed_features={'schema': 1, 'scope': diag.SCOPE, 'checks': rows, 'completed': count == 6 and not failed, 'actual_context_closed': True, 'observer_closed': True})
    return report

def projected(report, **owned):
    return runner.project_phase(diag.PHASE, 'public-signed-features', report,
        dict(fixture_only=True, phase=diag.PHASE, verified_stopped=True, forced=False, returncode=0, **owned))

class SignedFeaturePhase(unittest.TestCase):
    def test_distinct_gate_on_phase_fits_existing_job_and_preserves_upload_pair(self):
        self.assertEqual(runner.BATCHES['public-signed-features'], [diag.PHASE])
        self.assertEqual(runner.batch_phases('public-signed-features', [diag.PHASE]), [diag.PHASE])
        self.assertEqual(runner.maximum(diag.PHASE), 300)
        self.assertEqual(runner.fixture_reserve('public-signed-features', [diag.PHASE], 300, 600), 1235)
        self.assertLess(1235, 1800)
        self.assertEqual(runner.BATCHES['public-upload'], [runner.PUBLIC_FEATURE_PHASE, runner.PUBLIC_UPLOAD_PHASE])
        self.assertEqual(runner.fixture_schedule('public-upload', runner.BATCHES['public-upload']),
            [('public-upload-features', [runner.PUBLIC_FEATURE_PHASE], 150, 300), ('public-upload-journey', [runner.PUBLIC_UPLOAD_PHASE], 150, 300)])
        source = (ROOT / 'scripts/ci/run-hosted-browser.py').read_text()
        self.assertIn("if name in PUBLIC_GATE_BATCHES:\n        plan = public_upload_profile(plan)", source)
        self.assertIn("if name == 'public-signed-features':\n        argv.append('--signed-feature-controls')", source)
        self.assertIn("signed_features=name == 'public-signed-features'", source)
        prep = (ROOT / 'scripts/ci/prepare-hosted-browser.py').read_text()
        self.assertIn("in ['all', 'public-upload', 'public-signed-features']", prep)
        workflow = (ROOT.parent / '.github/workflows/hosted-browser-e2e.yml').read_text()
        self.assertIn('timeout-minutes: 140', workflow)
        self.assertIn('timeout-minutes: 30', workflow)
        self.assertIn('default: public-upload', workflow)
        self.assertEqual(workflow.count('retention-days: 1'), 3)
        self.assertIn("chromiumSandbox: true", (ROOT / 'scripts/browser-acceptance.mjs').read_text())

    def test_full_closed_proof_requires_all_six_checks_cleanup_and_outer_terminal(self):
        report = acceptance()
        value = diag.project(report)
        diag.validate(value)
        self.assertTrue(diag.passed(value))
        row = projected(report)
        self.assertEqual(row['status'], 'passed')
        validator.phases({'schema': 1, 'phases': [row], 'batches': []})
        for key in ['actual_context_closed', 'observer_closed', 'completed']:
            altered = copy.deepcopy(report)
            altered['signed_features'][key] = False
            if key == 'actual_context_closed':
                altered['cleanup']['context_closed'] = False
            self.assertEqual(projected(altered)['status'], 'failed')
        for value in [False, True, 1]:
            altered = copy.deepcopy(report)
            altered['passed'] = value
            if value is not True:
                self.assertNotEqual(projected(altered)['status'], 'passed')

    def test_each_prefix_interruption_and_failure_is_visible_but_never_green(self):
        for count in range(7):
            for failed in [False, True]:
                if failed and count == 0:
                    continue
                report = acceptance(count, failed)
                value = diag.project(report)
                diag.validate(value)
                self.assertTrue(value['record_valid'])
                self.assertEqual(diag.passed(value), count == 6 and not failed)
                if count < 6 and not failed:
                    report['steps'].append({'name': 'Signed feature ' + diag.CHECKS[count]})
                    value = diag.project(report)
                    self.assertTrue(value['record_valid'])
                    self.assertFalse(diag.passed(value))

    def test_strict_projection_drops_untrusted_raw_fields_and_wrong_case_association(self):
        for change in [lambda r: r['signed_features'].update(secret='private'),
                       lambda r: r['signed_features'].update(schema=True),
                       lambda r: r['signed_features'].update(scope='other'),
                       lambda r: r['signed_features']['checks'][0].update(id='signed-report-submit'),
                       lambda r: r['signed_features']['checks'][0].update(status=True),
                       lambda r: r['signed_features']['checks'][0].update(status='failed'),
                       lambda r: r['steps'][0].update(name='Other step'),
                       lambda r: r['steps'][0].update(status='failed'),
                       lambda r: r['cleanup'].update(context_closed=False),
                       lambda r: r.update(phase=runner.PUBLIC_UPLOAD_PHASE)]:
            report = acceptance()
            change(report)
            self.assertEqual(diag.project(report), diag.empty())
        report = acceptance()
        report['steps'][0]['error'] = 'private synthetic body or path'
        self.assertNotIn('private synthetic', json.dumps(diag.project(report)))

    def test_outer_validator_binds_phase_batch_failed_steps_and_fixed_shape(self):
        row = projected(acceptance())
        for change in [lambda r: r.update(batch='public-upload'),
                       lambda r: r.update(phase=runner.PUBLIC_FEATURE_PHASE),
                       lambda r: r.update(actual_steps=5),
                       lambda r: r.update(public_feature_diagnostics={}),
                       lambda r: r['signed_feature_diagnostics'].update(secret='private'),
                       lambda r: r.update(failed_steps=[{'index': 0, 'status': 'failed'}]),
                       lambda r: r.pop('signed_feature_diagnostics')]:
            altered = copy.deepcopy(row)
            change(altered)
            with self.assertRaises(ValueError):
                validator.phases({'schema': 1, 'phases': [altered], 'batches': []})
        failed = projected(acceptance(3, True))
        validator.phases({'schema': 1, 'phases': [failed], 'batches': []})
        self.assertEqual(failed['status'], 'failed')
        self.assertEqual(failed['failed_steps'], [{'index': 2, 'status': 'failed'}])

    def test_all_scope_cannot_omit_new_actual_features(self):
        old = [name for name in runner.ALL_PHASES if name != diag.PHASE]
        journal = {'phases': [{'phase': name, 'status': 'passed'} for name in old],
                   'batches': [{'passed': True, 'supervisor_terminal': True, 'supervisor_exit_zero': True,
                                'cleanup': {'verified_stopped': True, 'lifecycle_clean': True}}]}
        result = runner.summarize(journal, 'all', runner.ALL_PHASES)
        self.assertFalse(result['full_suite_passed'])
        self.assertEqual(result['pending_unique_phases'], [diag.PHASE])
        validator.summary(result)

if __name__ == '__main__':
    unittest.main()
