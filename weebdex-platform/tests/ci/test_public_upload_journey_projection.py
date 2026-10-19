"""Actual fixed JS progress snapshots -> Python -> outer validator, finite only."""
import copy
import json
import subprocess
import unittest

from test_public_upload_hosted_profile import ROOT, accepted_report, diagnostics, runner, validator


class PublicUploadJourneyProjection(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        module = (ROOT / 'scripts/public-upload-journey-progress.mjs').as_uri()
        group_module = (ROOT / 'tests/ci/public-upload-group-controls-fixture.mjs').as_uri()
        source = f"""
          import {{createUploadJourneyProgress,uploadJourneyStages,uploadJourneyCategories}} from {json.dumps(module)};
          import {{groupFixture}} from {json.dumps(group_module)};
          const rows=[],snapshots=[];
          const p=createUploadJourneyProgress(rows,()=>snapshots.push(structuredClone(rows)));
          for(let i=0;i<14;i++){{p.begin(i);p.mark('route');p.passed();}}
          const failed=[];
          for(const stage of uploadJourneyStages.filter(x=>!['start','complete'].includes(x))){{
            const r=[],q=createUploadJourneyProgress(r,()=>{{}});q.begin(0);q.mark(stage);
            q.failed(Object.assign(Error('synthetic'),{{code:'ERR_ASSERTION'}}));failed.push(r);
          }}
          const saveFailures=[];
          for(const failAt of [1,2,3]){{
            const r=[],saved=[];let n=0;const primary=Error('synthetic');
            const q=createUploadJourneyProgress(r,()=>{{saved.push(structuredClone(r));if(++n===failAt||n===failAt+1)throw primary;}});
            let same=false;
            try{{q.begin(0);q.mark('group-query');q.passed();}}catch(e){{same=q.failed(e)===primary;}}
            saveFailures.push({{rows:r,snapshots:saved,primary_preserved:same}});
          }}
          const groups=[];
          for(const options of [{{}},{{triggerDelay:3,resultDelay:3}},{{duplicateTrigger:true}},{{disabledTrigger:true}},{{searchFails:true}},{{duplicateResult:true}},{{disabledResult:true}},{{closeFails:true}}]){{
            const f=groupFixture(options);await f.run().catch(()=>{{}});groups.push({{rows:f.rows,snapshots:f.snapshots}});
          }}
          process.stdout.write(JSON.stringify({{stages:uploadJourneyStages,categories:uploadJourneyCategories,passed:rows,snapshots,failed,saveFailures,groups}}));
        """
        result = subprocess.run(['node', '--input-type=module', '-e', source],
                                capture_output=True, text=True, timeout=10, check=True)
        assert result.stderr == ''
        cls.actual = json.loads(result.stdout)

    def project_rows(self, rows):
        acceptance = accepted_report()
        acceptance['public_upload']['journey_progress'] = rows
        projected = diagnostics.project(acceptance)
        diagnostics.validate(projected)
        return acceptance, projected

    def phase(self, acceptance):
        owned = {'fixture_only': True, 'phase': runner.PUBLIC_UPLOAD_PHASE,
                 'verified_stopped': True, 'forced': False, 'returncode': 0}
        result = runner.project_phase(runner.PUBLIC_UPLOAD_PHASE, 'public-upload', acceptance, owned)
        validator.phases({'schema': 1, 'phases': [result], 'batches': []})
        return result

    def test_fixed_js_and_python_stage_category_contracts_match_exactly(self):
        self.assertEqual(self.actual['stages'], diagnostics.JOURNEY_STAGES)
        self.assertEqual(len(self.actual['stages']), 44)
        self.assertEqual(set(self.actual['categories']), diagnostics.KEY_IMPORT_CATEGORIES)

    def test_all_actual_saved_running_and_terminal_snapshots_validate(self):
        self.assertEqual(len(self.actual['snapshots']), 42)
        for index, rows in enumerate(self.actual['snapshots']):
            acceptance, value = self.project_rows(rows)
            self.assertTrue(value['journey_diagnostics']['record_valid'])
            self.assertEqual(value['journey_diagnostics']['checks'], rows)
            self.assertEqual(diagnostics.passed(value), index == 41)
            self.assertEqual(self.phase(acceptance)['status'] == 'passed', index == 41)

    def test_complete_fourteen_checks_are_required_for_positive_completion(self):
        acceptance, value = self.project_rows(self.actual['passed'])
        self.assertTrue(diagnostics.passed(value))
        self.assertEqual(self.phase(acceptance)['status'], 'passed')
        for size in range(14):
            acceptance, value = self.project_rows(self.actual['passed'][:size])
            self.assertFalse(value['completed'])
            self.assertFalse(diagnostics.passed(value))
            self.assertEqual(self.phase(acceptance)['status'], 'failed')

    def test_each_actual_failed_stage_is_fixed_and_cannot_promote_to_success(self):
        self.assertEqual(len(self.actual['failed']), 42)
        for rows in self.actual['failed']:
            acceptance, value = self.project_rows(rows)
            self.assertEqual(value['journey_diagnostics']['checks'], rows)
            self.assertFalse(diagnostics.passed(value))
            phase = self.phase(acceptance)
            self.assertEqual(phase['status'], 'failed')
            phase['status'] = 'passed'
            with self.assertRaises(ValueError):
                validator.phases({'schema': 1, 'phases': [phase], 'batches': []})

    def test_persistence_failures_restore_valid_failed_stage_and_preserve_primary(self):
        for case in self.actual['saveFailures']:
            self.assertTrue(case['primary_preserved'])
            self.assertEqual(case['rows'][0]['status'], 'failed')
            self.assertNotEqual(case['rows'][0]['substep'], 'complete')
            for rows in [case['rows'], *case['snapshots']]:
                acceptance, value = self.project_rows(rows)
                self.assertTrue(value['journey_diagnostics']['record_valid'])
                self.assertFalse(diagnostics.passed(value))
                self.assertEqual(self.phase(acceptance)['status'], 'failed')

    def test_actual_source_extracted_group_driver_snapshots_survive_closed_pipeline(self):
        self.assertEqual(len(self.actual['groups']), 8)
        for index, case in enumerate(self.actual['groups']):
            self.assertEqual(case['rows'][-1]['check_index'], 2)
            self.assertEqual(case['rows'][-1]['status'], 'passed' if index < 2 else 'failed')
            for rows in [case['rows'], *case['snapshots']]:
                acceptance, value = self.project_rows(rows)
                self.assertTrue(value['journey_diagnostics']['record_valid'])
                self.assertEqual(value['journey_diagnostics']['checks'], rows)
                self.assertFalse(diagnostics.passed(value))
                self.assertEqual(self.phase(acceptance)['status'], 'failed')

    def test_unknown_private_shape_and_types_are_not_echoed(self):
        sentinel = 'reserved-example.invalid/private-progress-value'
        invalid = [None, {}, sentinel, self.actual['passed'] + [self.actual['passed'][0]]]
        for field, value in [('check_index', True), ('check_index', 1), ('substep', sentinel),
                             ('status', sentinel), ('caught_category', sentinel), ('extra', sentinel)]:
            rows = copy.deepcopy(self.actual['passed'])
            rows[0][field] = value
            invalid.append(rows)
        rows = copy.deepcopy(self.actual['passed'])
        rows[0].update(status='failed', substep='route', caught_category='assertion')
        invalid.append(rows)
        for rows in invalid:
            acceptance, value = self.project_rows(rows)
            self.assertFalse(value['journey_diagnostics']['record_valid'])
            self.assertEqual(value['journey_diagnostics']['checks'], [])
            self.assertNotIn(sentinel, json.dumps(value))
            self.assertFalse(diagnostics.passed(value))
            self.assertEqual(self.phase(acceptance)['status'], 'failed')

    def test_public_exact_schema_rejects_contradictions_and_injected_fields(self):
        _, baseline = self.project_rows(self.actual['passed'])
        changes = [lambda x: x['journey_diagnostics'].update(extra='reserved-example.invalid'),
                   lambda x: x['journey_diagnostics'].update(schema=True),
                   lambda x: x['journey_diagnostics'].update(record_valid=False),
                   lambda x: x['journey_diagnostics']['checks'][0].update(check_index=True),
                   lambda x: x['journey_diagnostics']['checks'][0].update(extra='reserved-example.invalid'),
                   lambda x: x['journey_diagnostics']['checks'][0].update(status='running'),
                   lambda x: x['journey_diagnostics']['checks'][0].update(caught_category='assertion'),
                   lambda x: x['journey_diagnostics']['checks'].pop()]
        for change in changes:
            value = copy.deepcopy(baseline)
            change(value)
            with self.assertRaises(ValueError):
                diagnostics.validate(value)


if __name__ == '__main__':
    unittest.main()
