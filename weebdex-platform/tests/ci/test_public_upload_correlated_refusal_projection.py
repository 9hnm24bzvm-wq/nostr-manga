"""Finite actual-callback/observer to closed Python/outer-schema association."""
import copy
import json
import os
import subprocess
import unittest
from test_public_upload_hosted_profile import ROOT, accepted_report, diagnostics, runner, validator

ID = 'a' * 64
PATH = '/api/v1/nostr/uploads/11111111-1111-8111-8111-111111111111/files/22222222-2222-8222-8222-222222222222/preview'


def observed(identity=ID, **extra):
    return {'event_id': identity, 'method': 'GET', 'path': PATH, 'native': 'failed',
            'native_error': 'net::ERR_ABORTED', 'status': 200, **extra}


def record(identity=ID, **extra):
    return {'event_id': identity, 'stage': 'body-read', 'transport_status': 200, **extra}


def capture(records=None, **extra):
    return {'capture_valid': True, 'snapshot': {'schema': 1, 'enabled': True, 'record_valid': True,
            'truncated': False, 'records': [] if records is None else records, **extra}}


class CorrelatedUploadRefusals(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        result = subprocess.run(['node', '--experimental-vm-modules',
            str(ROOT / 'tests/ci/public-upload-correlated-refusal-fixture.mjs'), '--json'],
            capture_output=True, text=True, timeout=5, check=True,
            env={**os.environ, 'NODE_NO_WARNINGS': '1'})
        cls.actual = json.loads(result.stdout)

    def project(self, captures=None, rows=None):
        value = diagnostics.project_transport_refusals([capture([record()])] if captures is None else captures,
                                                       [observed()] if rows is None else rows)
        diagnostics.validate_transport_refusals(value)
        return value

    def test_actual_callback_observer_and_outer_schema_keep_exact_correlated_stage(self):
        acceptance = accepted_report()
        acceptance['public_upload'].update(completed=False, browser_http=self.actual['browser'],
                                           transport_refusal_captures=self.actual['captures'])
        value = diagnostics.project(acceptance)
        diagnostics.validate(value)
        detail = value['transport_refusal_diagnostics']
        self.assertTrue(detail['record_valid'])
        self.assertEqual(detail['requests'], [{'operation': 'preview-read', 'method': 'GET',
            'observed_status': 200, 'transport_status': 200, 'native_terminal': 'failed',
            'refusal_stage': 'body-read'}])
        serialized = json.dumps(detail)
        for private in [self.actual['browser']['rows'][0]['event_id'], PATH, 'Nostr ', 'net::']:
            self.assertNotIn(private, serialized)
        owned = {'fixture_only': True, 'phase': runner.PUBLIC_UPLOAD_PHASE, 'verified_stopped': True,
                 'forced': False, 'returncode': 0}
        phase = runner.project_phase(runner.PUBLIC_UPLOAD_PHASE, 'public-upload', acceptance, owned)
        validator.phases({'schema': 1, 'phases': [phase], 'batches': []})
        self.assertEqual(phase['status'], 'failed')
        phase['status'] = 'passed'
        with self.assertRaises(ValueError):
            validator.phases({'schema': 1, 'phases': [phase], 'batches': []})

    def test_natural_success_requires_two_available_empty_document_captures(self):
        acceptance = accepted_report()
        value = diagnostics.project(acceptance)
        diagnostics.validate(value)
        self.assertTrue(diagnostics.passed(value))
        acceptance['public_upload']['transport_refusal_captures'].pop()
        value = diagnostics.project(acceptance)
        diagnostics.validate(value)
        self.assertFalse(value['transport_refusal_diagnostics']['capture_valid'])
        self.assertFalse(diagnostics.passed(value))

    def test_missing_reader_differs_from_valid_reader_without_matching_catch(self):
        missing = self.project([{'capture_valid': False, 'snapshot': None}])
        absent = self.project([capture()])
        self.assertFalse(missing['capture_valid'])
        self.assertTrue(absent['capture_valid'])
        self.assertFalse(absent['correlation_complete'])
        self.assertEqual(absent['requests'], [])

    def test_full_success_rejects_missing_noninteger_or_single_reader_handshake(self):
        for count in [None, False, 0, 1, 3]:
            acceptance = accepted_report()
            acceptance['public_upload']['transport_refusal_reader_checks'] = count
            value = diagnostics.project(acceptance)
            diagnostics.validate(value)
            self.assertFalse(value['transport_refusal_diagnostics']['capture_valid'])
            self.assertFalse(diagnostics.passed(value))

    def test_identical_cross_snapshot_record_is_one_observation(self):
        value = self.project([capture([record()]), capture([record()])])
        self.assertTrue(value['record_valid'])
        self.assertEqual(len(value['requests']), 1)

    def test_duplicate_within_ring_and_conflicting_repetition_are_unverified(self):
        for captures in [[capture([record(), record()])],
                         [capture([record()]), capture([record(stage='fetch')])]]:
            value = self.project(captures)
            self.assertFalse(value['record_valid'])
            self.assertEqual(value['requests'], [])

    def test_unmatched_record_duplicate_observer_or_missing_failed_record_is_unverified(self):
        for captures, rows in [([capture([record('b' * 64)])], [observed()]),
                               ([capture([record()])], [observed(), observed()]),
                               ([capture()], [observed()])]:
            value = self.project(captures, rows)
            self.assertFalse(value['record_valid'])
            self.assertFalse(value['correlation_complete'])
            self.assertEqual(value['requests'], [])

    def test_exact_sixteen_cap_and_combined_document_overflow(self):
        records = [record(f'{index:064x}') for index in range(16)]
        rows = [observed(f'{index:064x}') for index in range(17)]
        value = self.project([capture(records)], rows[:16])
        self.assertTrue(value['record_valid'])
        self.assertEqual(len(value['requests']), 16)
        overflow = self.project([capture(records), capture([record(f'{16:064x}')])], rows)
        self.assertTrue(overflow['capture_valid'])
        self.assertTrue(overflow['truncated'])
        self.assertFalse(overflow['record_valid'])
        self.assertEqual(overflow['requests'], [])
        sticky = self.project([capture(records, truncated=True)], rows[:16])
        self.assertTrue(sticky['truncated'])
        self.assertFalse(sticky['correlation_complete'])

    def test_enabled_but_unverified_producer_is_not_promoted(self):
        value = self.project([capture(record_valid=False)])
        self.assertTrue(value['capture_valid'])
        self.assertFalse(value['correlation_complete'])

    def test_all_nine_closed_stages_and_status_axes_are_preserved(self):
        for stage in diagnostics.TRANSPORT_REFUSAL_STAGES:
            value = self.project([capture([record(stage=stage)])])
            self.assertEqual(value['requests'][0]['refusal_stage'], stage)
        value = self.project([capture([record(stage='fetch', transport_status=None)])])
        self.assertEqual(value['requests'][0]['observed_status'], 200)
        self.assertIsNone(value['requests'][0]['transport_status'])
        self.assertEqual(value['requests'][0]['native_terminal'], 'failed')
        self.assertFalse(diagnostics.transport_refusals_clear(value))

    def test_known_status_disagreement_and_private_malformed_values_are_never_echoed(self):
        for change in [{'transport_status': 503}, {'transport_status': True}, {'stage': 'PRIVATE SENTINEL'},
                       {'event_id': 'PRIVATE SENTINEL'}, {'extra': 'PRIVATE SENTINEL'}]:
            value = self.project([capture([record(**change)])])
            self.assertFalse(value['record_valid'])
            self.assertNotIn('PRIVATE', json.dumps(value))

    def test_method_bound_classification_falls_back_without_exporting_private_path(self):
        value = self.project(rows=[observed(path='/api/v1/upload/PRIVATE_SENTINEL', method='GET')])
        self.assertEqual(value['requests'][0]['operation'], 'other-watched')
        self.assertNotIn('PRIVATE', json.dumps(value))
        bad = self.project(rows=[observed(method='PUT')])
        self.assertFalse(bad['record_valid'])

    def test_finished_caught_response_remains_a_refusal_and_cannot_clear_success(self):
        value = self.project(rows=[observed(native='finished')])
        self.assertTrue(value['record_valid'])
        self.assertEqual(value['requests'][0]['native_terminal'], 'finished')
        self.assertFalse(diagnostics.transport_refusals_clear(value))

    def test_public_extra_keys_bool_status_contradictions_and_unknown_enums_are_rejected(self):
        base = self.project()
        changes = [
            lambda v: v.update(extra='PRIVATE'),
            lambda v: v.update(record_valid=False),
            lambda v: v.update(capture_valid=False),
            lambda v: v.update(truncated=True),
            lambda v: v['requests'][0].update(observed_status=True),
            lambda v: v['requests'][0].update(refusal_stage='PRIVATE'),
            lambda v: v['requests'][0].update(native_terminal='complete'),
            lambda v: v['requests'][0].update(method='POST'),
            lambda v: v['requests'][0].update(transport_status=503),
        ]
        for mutate in changes:
            value = copy.deepcopy(base)
            mutate(value)
            with self.assertRaises(ValueError):
                diagnostics.validate_transport_refusals(value)
        with self.assertRaises(ValueError):
            diagnostics.validate_transport_refusals({'record_valid': False, 'capture_valid': False,
                'correlation_complete': True, 'truncated': False, 'requests': []})


if __name__ == '__main__':
    unittest.main()

