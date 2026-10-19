"""Closed fixed observer labels only; no browser, HTTP, DB or native fixture."""
import copy
import json
import shutil
import subprocess
import unittest

from test_public_upload_hosted_profile import ROOT, accepted_report, diagnostics, runner, validator


class PublicUploadObserverProjection(unittest.TestCase):
    def test_runner_binds_verified_fixture_user_once_before_first_navigation(self):
        source = (ROOT / 'scripts/public-upload-recovery-browser.mjs').read_text()
        binding = 'observer.bindAccount(account.user.id);'
        self.assertEqual(source.count(binding), 1)
        provision = source.index("await run('provision real bound synthetic upload publisher'")
        bound = source.index(binding)
        navigate = source.index('await page.goto(origin + pathname()')
        self.assertLess(provision, bound)
        self.assertLess(bound, navigate)
        self.assertIn("for (const kind of ['user','group','manga']) assert.match(account[kind].id, uuid); " + binding,
                      source[provision:navigate])

    def project(self, failures):
        acceptance = accepted_report()
        acceptance['public_upload']['browser_http']['failures'] = failures
        value = diagnostics.project(acceptance)
        diagnostics.validate(value)
        return acceptance, value

    def assert_failed_phase(self, acceptance):
        owned = {'fixture_only': True, 'phase': runner.PUBLIC_UPLOAD_PHASE,
                 'verified_stopped': True, 'forced': False, 'returncode': 0}
        phase = runner.project_phase(runner.PUBLIC_UPLOAD_PHASE, 'public-upload', acceptance, owned)
        self.assertEqual(phase['status'], 'failed')
        validator.phases({'schema': 1, 'phases': [phase], 'batches': []})
        forged = copy.deepcopy(phase)
        forged['status'] = 'passed'
        with self.assertRaises(ValueError):
            validator.phases({'schema': 1, 'phases': [forged], 'batches': []})

    def test_each_fixed_family_and_mixed_repeats_stay_closed(self):
        for failures in [[name] for name in diagnostics.OBSERVER_FAILURES] + [
                ['signed_request_invalid', 'signed_native_failed', 'signed_request_invalid']]:
            with self.subTest(failures=failures):
                acceptance, value = self.project(failures)
                expected = {'record_valid': True, **{name: failures.count(name) for name in diagnostics.OBSERVER_FAILURES}}
                self.assertEqual(value['browser_observer_failures'], expected)
                self.assertFalse(value['browser_observer_closed'])
                self.assertFalse(diagnostics.passed(value))
                self.assert_failed_phase(acceptance)

    def test_retained_label_cap_is_sixteen_not_a_total_failure_claim(self):
        _, value = self.project(['signed_request_invalid'] * 16)
        self.assertEqual(value['browser_observer_failures']['signed_request_invalid'], 16)
        _, value = self.project(['signed_request_invalid'] * 17)
        self.assertEqual(value['browser_observer_failures'],
                         {'record_valid': False, **{name: 0 for name in diagnostics.OBSERVER_FAILURES}})

    def test_unknown_private_malformed_or_missing_values_are_never_echoed(self):
        sentinel = 'reserved-example.invalid/private-diagnostic-value'
        for raw in [None, {}, sentinel, [sentinel], [False], [1], [['signed_request_invalid']]]:
            acceptance, value = self.project(raw)
            self.assertFalse(value['browser_observer_failures']['record_valid'])
            self.assertNotIn(sentinel, json.dumps(value))
            self.assertFalse(diagnostics.passed(value))
            self.assert_failed_phase(acceptance)
        acceptance = accepted_report()
        del acceptance['public_upload']['browser_http']['failures']
        value = diagnostics.project(acceptance)
        diagnostics.validate(value)
        self.assertFalse(value['browser_observer_failures']['record_valid'])

    def test_incomplete_import_keeps_fixed_failure_labels_without_replacement_receipt(self):
        acceptance = accepted_report()
        acceptance['steps'] = acceptance['steps'][:2]
        acceptance['public_upload']['replacement'] = None
        acceptance['public_upload']['browser_http']['failures'] = ['signed_request_invalid']
        value = diagnostics.project(acceptance)
        diagnostics.validate(value)
        self.assertEqual(value['browser_observer_failures']['signed_request_invalid'], 1)
        self.assertFalse(value['record_valid'])
        self.assertFalse(diagnostics.passed(value))
        self.assert_failed_phase(acceptance)

    def test_exact_public_shape_types_cap_and_closure_contradictions_refuse(self):
        _, baseline = self.project([])
        changes = [lambda x: x['browser_observer_failures'].update(extra='reserved-example.invalid'),
                   lambda x: x['browser_observer_failures'].update(record_valid=1),
                   lambda x: x['browser_observer_failures'].update(signed_request_invalid=True),
                   lambda x: x['browser_observer_failures'].update(signed_request_invalid=-1),
                   lambda x: x['browser_observer_failures'].update(signed_request_invalid=17),
                   lambda x: x['browser_observer_failures'].update(signed_request_invalid=9, signed_native_failed=8),
                   lambda x: x['browser_observer_failures'].update(record_valid=False, signed_request_invalid=1),
                   lambda x: x['browser_observer_failures'].update(record_valid=False),
                   lambda x: x['browser_observer_failures'].update(signed_request_invalid=1)]
        for change in changes:
            altered = copy.deepcopy(baseline)
            change(altered)
            with self.assertRaises(ValueError):
                diagnostics.validate(altered)

    def test_natural_success_requires_valid_zero_retained_labels(self):
        _, value = self.project([])
        self.assertTrue(diagnostics.passed(value))
        self.assertEqual(value['browser_observer_failures'],
                         {'record_valid': True, **{name: 0 for name in diagnostics.OBSERVER_FAILURES}})

    def test_actual_node_observer_snapshots_reach_python_and_outer_validation(self):
        node = shutil.which('node')
        self.assertIsNotNone(node, 'Existing Node runtime is required for this finite seam')
        result = subprocess.run([node, str(ROOT / 'tests/ci/public-upload-observer-query-fixture.mjs'), '--json'],
                                capture_output=True, text=True, timeout=10, check=True)
        self.assertEqual(result.stderr, '')
        actual = json.loads(result.stdout)
        self.assertEqual(actual, {'valid_discovery': [], 'malformed_query': ['signed_request_invalid'],
                                 'duplicate_native_terminal': ['duplicate_native_terminal'],
                                 'native_abort': ['signed_native_failed']})
        for name, failures in actual.items():
            acceptance, value = self.project(failures)
            self.assertEqual(value['browser_observer_failures']['record_valid'], True)
            if name == 'valid_discovery':
                self.assertTrue(diagnostics.passed(value))
            else:
                self.assertFalse(diagnostics.passed(value))
                self.assert_failed_phase(acceptance)


if __name__ == '__main__':
    unittest.main()
