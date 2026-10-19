"""Closed passive preview attributes, using the actual JS collector with stand-ins."""
import copy
import json
import subprocess
import unittest
from test_public_upload_hosted_profile import ROOT, accepted_report, diagnostics, runner, validator

class PublicUploadPreviewRefusalProjection(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        r=subprocess.run(['node',str(ROOT/'tests/ci/public-upload-preview-refusal-fixture.mjs'),'--json'],
                         capture_output=True,text=True,timeout=10,check=True)
        assert not r.stderr
        cls.rows=json.loads(r.stdout)

    def phase(self, report):
        owned={'fixture_only':True,'phase':runner.PUBLIC_UPLOAD_PHASE,'verified_stopped':True,'forced':False,'returncode':0}
        value=runner.project_phase(runner.PUBLIC_UPLOAD_PHASE,'public-upload',report,owned)
        validator.phases({'schema':1,'phases':[value],'batches':[]})
        return value

    def test_every_actual_js_fixed_code_reaches_python_and_refuses_forged_success(self):
        for code in diagnostics.PREVIEW_REFUSAL_CODES:
            report=accepted_report()
            report['public_upload']['preview_transport_refusals']=self.rows[code]
            value=diagnostics.project(report);diagnostics.validate(value)
            self.assertEqual(value['preview_transport_refusals']['codes'],[code])
            self.assertFalse(diagnostics.passed(value))
            phase=self.phase(report);self.assertEqual(phase['status'],'failed')
            phase['status']='passed'
            with self.assertRaises(ValueError):validator.phases({'schema':1,'phases':[phase],'batches':[]})

    def test_actual_empty_ui_snapshot_preserves_all_existing_positive_gates(self):
        report=accepted_report();report['public_upload']['preview_transport_refusals']=self.rows['empty']
        value=diagnostics.project(report);diagnostics.validate(value)
        self.assertTrue(diagnostics.passed(value));self.assertEqual(self.phase(report)['status'],'passed')
        report['public_upload']['browser_http']['rows'][0]['native']='failed'
        report['public_upload']['browser_http']['rows'][0]['native_error']='net::ERR_ABORTED'
        self.assertFalse(diagnostics.passed(diagnostics.project(report)))

    def test_missing_malformed_private_and_truncated_snapshots_never_promote(self):
        for raw in [None,{},self.rows['missing'],self.rows['private'],self.rows['read_error'],self.rows['truncated'],
                    {'record_valid':True,'truncated':False,'codes':['reserved-private']},
                    {'record_valid':True,'truncated':False,'codes':[{}]},
                    {'record_valid':False,'truncated':False,'codes':['fetch']}]:
            report=accepted_report();report['public_upload']['preview_transport_refusals']=raw
            value=diagnostics.project(report);diagnostics.validate(value)
            self.assertFalse(diagnostics.passed(value))
            self.assertNotIn('reserved-private',json.dumps(value))
            self.assertEqual(self.phase(report)['status'],'failed')

    def test_early_incomplete_replacement_retains_fixed_ui_fact(self):
        report=accepted_report()
        report['public_upload']={'preview_transport_refusals':self.rows['fetch']}
        value=diagnostics.project(report);diagnostics.validate(value)
        self.assertEqual(value['preview_transport_refusals'],self.rows['fetch'])
        self.assertFalse(value['record_valid']);self.assertFalse(diagnostics.passed(value))

    def test_public_schema_refuses_types_extra_keys_cap_and_inconsistent_invalidity(self):
        base=diagnostics.project(accepted_report())
        for raw in [[],None,{}, {'record_valid':1,'truncated':False,'codes':[]},
                    {'record_valid':True,'truncated':0,'codes':[]},
                    {'record_valid':False,'truncated':False,'codes':['fetch']},
                    {'record_valid':False,'truncated':True,'codes':[]},
                    {'record_valid':True,'truncated':True,'codes':['fetch']},
                    {'record_valid':True,'truncated':False,'codes':['fetch']*17},
                    {'record_valid':True,'truncated':False,'codes':['reserved-private']},
                    {'record_valid':True,'truncated':False,'codes':[],'raw':'reserved-private'}]:
            value=copy.deepcopy(base);value['preview_transport_refusals']=raw
            with self.assertRaises(ValueError):diagnostics.validate(value)
