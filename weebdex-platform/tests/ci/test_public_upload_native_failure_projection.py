"""Closed actual-native-failure facts, with explicit observer stand-ins only."""
import copy
import json
import subprocess
import unittest

from test_public_upload_hosted_profile import ROOT, accepted_report, diagnostics, runner, validator

ID = '11111111-1111-8111-8111-111111111111'


def failed(method='POST', path='/api/v1/upload/' + ID, status=None, error='net::ERR_ABORTED'):
    return {'method': method, 'path': path, 'status': status, 'native': 'failed', 'native_error': error}


class PublicUploadNativeFailureProjection(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        result = subprocess.run(['node', str(ROOT / 'tests/ci/public-upload-native-failure-fixture.mjs')],
                                capture_output=True, text=True, timeout=10, check=True)
        assert result.stderr == ''
        cls.actual = json.loads(result.stdout)

    def phase(self, acceptance):
        owned = {'fixture_only': True, 'phase': runner.PUBLIC_UPLOAD_PHASE,
                 'verified_stopped': True, 'forced': False, 'returncode': 0}
        result = runner.project_phase(runner.PUBLIC_UPLOAD_PHASE, 'public-upload', acceptance, owned)
        validator.phases({'schema': 1, 'phases': [result], 'batches': []})
        return result

    def test_every_known_operation_is_method_and_canonical_route_bound(self):
        cases = [
            ('GET', '/api/v1/nostr/account-context', 'account-context'),
            ('GET', '/api/v1/upload', 'current-upload'),
            ('POST', '/api/v1/upload/check-approval-required', 'approval-check'),
            ('POST', '/api/v1/upload/begin', 'begin'),
            ('POST', '/api/v1/upload/begin/' + ID, 'begin-edit'),
            ('GET', '/api/v1/nostr/uploads/' + ID, 'session-read'),
            ('PATCH', '/api/v1/nostr/uploads/' + ID, 'session-save'),
            ('POST', '/api/v1/upload/' + ID, 'file-batch'),
            ('DELETE', '/api/v1/upload/' + ID, 'cancel'),
            ('DELETE', '/api/v1/upload/' + ID + '/batch', 'file-batch-remove'),
            ('DELETE', '/api/v1/upload/' + ID + '/' + ID, 'file-remove'),
            ('POST', '/api/v1/upload/' + ID + '/commit', 'commit'),
            ('GET', '/api/v1/nostr/uploads/' + ID + '/files/' + ID + '/preview', 'preview-read'),
            ('POST', '/api/v1/nostr/uploads/' + ID + '/files/' + ID + '/replace', 'file-replace'),
        ]
        for method, path, operation in cases:
            value = diagnostics.project_native_failures([failed(method, path)])
            self.assertTrue(value['record_valid'])
            self.assertEqual(value['requests'], [{'operation': operation, 'method': method, 'status': None, 'error_family': 'aborted'}])
            self.assertNotIn(ID, json.dumps(value))

    def test_mismatched_method_noncanonical_id_or_extra_segment_is_not_misclassified(self):
        for row in [failed('GET', '/api/v1/upload/begin'),
                    failed(path='/api/v1/upload/' + 'abcdefab-cdef-8abc-9abc-abcdefabcdef'.upper()),
                    failed(path='/api/v1/upload/' + ID + '/commit/extra')]:
            value = diagnostics.project_native_failures([row])
            self.assertTrue(value['record_valid'])
            self.assertEqual(value['requests'][0]['operation'], 'other-watched')

    def test_unknown_watched_path_exports_only_fixed_fallback(self):
        private = '/api/v1/upload/reserved-private-diagnostic-value'
        value = diagnostics.project_native_failures([failed(path=private)])
        self.assertEqual(value['requests'][0]['operation'], 'other-watched')
        self.assertNotIn(private, json.dumps(value))
        self.assertNotIn('reserved-private', json.dumps(value))

    def test_actual_observer_keeps_status_and_native_terminal_as_separate_axes(self):
        for name, operation, status, error in [
            ('aborted_before_headers', 'file-batch', None, 'aborted'),
            ('aborted_after_200_headers', 'file-batch', 200, 'aborted'),
            ('other_failure_after_503_headers', 'preview-read', 503, 'other')]:
            browser = self.actual[name]
            self.assertEqual(browser['failures'], ['signed_native_failed'])
            value = diagnostics.project_native_failures(browser['rows'])
            self.assertEqual(value['requests'], [{'operation': operation, 'method': browser['rows'][0]['method'], 'status': status, 'error_family': error}])
            self.assertTrue(value['record_valid'])
            self.assertNotIn('reserved-example', json.dumps(value))
            acceptance = accepted_report()
            acceptance['public_upload']['browser_http'] = browser
            projected = diagnostics.project(acceptance)
            diagnostics.validate(projected)
            self.assertFalse(projected['browser_observer_closed'])
            self.assertFalse(diagnostics.passed(projected))
            phase = self.phase(acceptance)
            self.assertEqual(phase['status'], 'failed')
            phase['status'] = 'passed'
            with self.assertRaises(ValueError):
                validator.phases({'schema': 1, 'phases': [phase], 'batches': []})

    def test_fully_finished_http_refusal_is_not_reported_as_native_failure(self):
        browser = self.actual['finished_write_503']
        self.assertEqual(browser['failures'], [])
        self.assertEqual(browser['rows'][0]['native'], 'finished')
        self.assertEqual(diagnostics.project_native_failures(browser['rows']),
                         {'record_valid': True, 'truncated': False, 'requests': []})
        acceptance = accepted_report()
        acceptance['public_upload']['browser_http']['rows'][1]['status'] = 503
        value = diagnostics.project(acceptance)
        diagnostics.validate(value)
        self.assertFalse(value['exact_mutation_count'])
        self.assertFalse(diagnostics.passed(value))
        self.assertEqual(self.phase(acceptance)['status'], 'failed')

    def test_finished_discovery_404_is_not_invented_as_a_failure(self):
        browser = self.actual['finished_discovery_404']
        acceptance = accepted_report()
        acceptance['public_upload']['browser_http']['rows'].extend(browser['rows'])
        value = diagnostics.project(acceptance)
        diagnostics.validate(value)
        self.assertEqual(value['native_failure_diagnostics'], {'record_valid': True, 'truncated': False, 'requests': []})
        self.assertTrue(diagnostics.passed(value))
        self.assertEqual(self.phase(acceptance)['status'], 'passed')

    def test_pending_request_cannot_be_promoted_even_without_native_failure(self):
        acceptance = accepted_report()
        acceptance['public_upload']['browser_http']['rows'].extend(self.actual['pending_write']['rows'])
        value = diagnostics.project(acceptance)
        diagnostics.validate(value)
        self.assertTrue(value['native_failure_diagnostics']['record_valid'])
        self.assertEqual(value['native_failure_diagnostics']['requests'], [])
        self.assertFalse(value['browser_observer_closed'])
        self.assertFalse(diagnostics.passed(value))

    def test_only_first_sixteen_failed_rows_in_request_order_are_retained(self):
        for count in [0, 16, 17, 128]:
            rows = [failed(status=200 + index) for index in range(count)]
            value = diagnostics.project_native_failures(rows)
            self.assertTrue(value['record_valid'])
            self.assertEqual(len(value['requests']), min(count, 16))
            self.assertEqual(value['truncated'], count > 16)
            self.assertEqual([row['status'] for row in value['requests']], list(range(200, 200 + min(count, 16))))
        actual = diagnostics.project_native_failures(self.actual['seventeen_failures']['rows'])
        self.assertEqual(len(actual['requests']), 16)
        self.assertTrue(actual['truncated'])

    def test_missing_malformed_or_private_tokens_become_unverified_without_echo(self):
        private = 'reserved-example.invalid/private-native-value'
        invalid = [None, {}, [None], [failed()] * 129]
        for field, value in [('native', {}), ('native', []), ('native', private), ('method', private),
                             ('path', private), ('path', '/api/v1/upload?private=' + private),
                             ('native_error', private), ('status', True), ('status', 99), ('status', 600), ('status', private)]:
            row = failed(); row[field] = value; invalid.append([row])
        for field in ['status', 'native_error', 'method', 'path']:
            row = failed(); del row[field]; invalid.append([row])
        for raw in invalid:
            value = diagnostics.project_native_failures(raw)
            self.assertEqual(value, {'record_valid': False, 'truncated': False, 'requests': []})
            self.assertNotIn(private, json.dumps(value))

    def test_early_incomplete_journey_preserves_only_safe_failure_details(self):
        acceptance = accepted_report()
        acceptance['steps'] = acceptance['steps'][:3]
        acceptance['public_upload']['replacement'] = None
        acceptance['public_upload']['browser_http'] = self.actual['aborted_after_200_headers']
        value = diagnostics.project(acceptance)
        diagnostics.validate(value)
        self.assertFalse(value['record_valid'])
        self.assertEqual(value['native_failure_diagnostics']['requests'][0]['status'], 200)
        self.assertFalse(diagnostics.passed(value))
        self.assertEqual(self.phase(acceptance)['status'], 'failed')

    def test_exact_public_schema_types_method_pairs_cap_and_closure_refuse_forgery(self):
        baseline = diagnostics.project(accepted_report())
        details = diagnostics.project_native_failures([failed()])
        changes = [lambda x: x.update(extra='reserved-example.invalid'),
                   lambda x: x.update(record_valid=1),
                   lambda x: x.update(truncated=1),
                   lambda x: x.update(truncated=True),
                   lambda x: x['requests'][0].update(extra='reserved-example.invalid'),
                   lambda x: x['requests'][0].update(operation='unknown-private'),
                   lambda x: x['requests'][0].update(method='GET'),
                   lambda x: x['requests'][0].update(status=True),
                   lambda x: x['requests'][0].update(error_family='none'),
                   lambda x: x.update(requests=x['requests'] * 17),
                   lambda x: x.update(record_valid=False)]
        for change in changes:
            value = copy.deepcopy(baseline)
            value['browser_observer_closed'] = False
            value['native_failure_diagnostics'] = copy.deepcopy(details)
            change(value['native_failure_diagnostics'])
            with self.assertRaises(ValueError):
                diagnostics.validate(value)
        forged = copy.deepcopy(baseline)
        forged['native_failure_diagnostics'] = details
        with self.assertRaises(ValueError):
            diagnostics.validate(forged)


if __name__ == '__main__':
    unittest.main()
