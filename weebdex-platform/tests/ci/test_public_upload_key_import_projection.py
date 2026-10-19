"""Actual Node trace -> Python projection -> outer evidence validator, finite only."""
import copy
import json
import hashlib
import os
from pathlib import Path
import subprocess
import unittest
from test_public_upload_hosted_profile import accepted_report, diagnostics, runner, validator

ROOT = Path(__file__).resolve().parents[2]


class KeyImportProjection(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        fixture = (ROOT / 'tests/ci/public-upload-key-import-fixture.mjs').as_uri()
        helper = (ROOT / 'scripts/public-upload-key-import.mjs').as_uri()
        if os.environ.get('WEEBDEX_KEY_IMPORT_LEDGER_BASELINE_SOURCE'):
            assert hashlib.sha256(Path(os.environ['WEEBDEX_KEY_IMPORT_LEDGER_BASELINE_SOURCE']).read_bytes()).hexdigest() == 'd0db904c9961e41e66e708ddf9d8672073d90b965dbe81c1a930f8d9808a694b'
        source = f"""
          import {{importFixture,serializedImportTrace,serializedImportSnapshots}} from {json.dumps(fixture)};
          import {{keyImportSubsteps,keyImportCategories}} from {json.dumps(helper)};
          import {{pathToFileURL}} from 'node:url';
          const failed=[];
          for(const substep of keyImportSubsteps){{const f=importFixture({{failAt:substep}});await f.run().catch(()=>{{}});failed.push(f.attempts);}}
          const review=importFixture({{backupSessionControl:true,reviewFails:true}});await review.run().catch(()=>{{}});
          const saved=[],attempts=[];for(let i=0;i<2;i++)await importFixture({{attemptRows:attempts,snapshotSink:saved}}).run();
          const saveFailures=[];for(const failSaveAt of [[1],[2],[28],[2,3]])saveFailures.push(await serializedImportSnapshots({{failSaveAt}}));
          saveFailures.push(await serializedImportSnapshots({{failAt:'import-form',failSaveAt:[12]}}));
          let baselineSnapshots=null;
          if(process.env.WEEBDEX_KEY_IMPORT_LEDGER_BASELINE_SOURCE){{
            const old=await import(pathToFileURL(process.env.WEEBDEX_KEY_IMPORT_LEDGER_BASELINE_SOURCE).href);
            const f=importFixture();f.attempts.length=0;baselineSnapshots=[];
            f.args.importStep=old.createKeyImportStep(f.attempts,()=>baselineSnapshots.push(structuredClone(f.attempts)));
            await old.connectPublicUploadKey(f.args);
          }}
          process.stdout.write(JSON.stringify({{passed:await serializedImportTrace(),failed,reviewFailed:review.attempts,saved,saveFailures,baselineSnapshots,substeps:keyImportSubsteps,categories:keyImportCategories}}));
        """
        result = subprocess.run(['node', '--input-type=module', '-e', source], capture_output=True, text=True, timeout=10, check=True)
        cls.serialized = json.loads(result.stdout)

    def test_node_and_python_closed_contracts_match_exactly(self):
        self.assertEqual(self.serialized['substeps'], diagnostics.KEY_IMPORT_SUBSTEPS)
        self.assertEqual(set(self.serialized['categories']), diagnostics.KEY_IMPORT_CATEGORIES)

    def test_two_node_imports_survive_outer_projection_and_exact_validator(self):
        acceptance = accepted_report()
        acceptance['public_upload']['key_import'] = self.serialized['passed']
        owned = {'fixture_only': True, 'phase': runner.PUBLIC_UPLOAD_PHASE, 'verified_stopped': True, 'forced': False, 'returncode': 0}
        projected = runner.project_phase(runner.PUBLIC_UPLOAD_PHASE, 'public-upload', acceptance, owned)
        self.assertEqual(projected['public_upload_diagnostics']['key_import_diagnostics']['attempts'], self.serialized['passed'])
        self.assertEqual(projected['status'], 'passed')
        validator.phases({'schema': 1, 'phases': [projected], 'batches': []})
        for private in ['sentinel', 'synthetic', 'secretHex', 'Nostr ', 'nsec', 'http:']:
            self.assertNotIn(private, json.dumps(projected))

    def test_every_node_failure_boundary_survives_serializer_and_blocks_promotion(self):
        for index, rows in enumerate(self.serialized['failed']):
            with self.subTest(substep=diagnostics.KEY_IMPORT_SUBSTEPS[index]):
                acceptance = accepted_report()
                acceptance['public_upload']['key_import'] = rows
                value = diagnostics.project(acceptance)
                diagnostics.validate(value)
                imported = value['key_import_diagnostics']
                self.assertTrue(imported['record_valid'])
                self.assertEqual(imported['attempts'], rows)
                self.assertEqual(rows[0]['completed_substeps'], index)
                self.assertEqual(rows[0]['caught_category'], 'unexpected')
                self.assertFalse(diagnostics.passed(value))

    def test_every_actual_saved_pre_action_inter_step_and_final_snapshot_validates(self):
        snapshots = self.serialized['saved']
        self.assertEqual(len(snapshots), 56)
        for index, rows in enumerate(snapshots):
            with self.subTest(snapshot=index):
                acceptance = accepted_report()
                acceptance['public_upload']['key_import'] = rows
                owned = {'fixture_only': True, 'phase': runner.PUBLIC_UPLOAD_PHASE, 'verified_stopped': True, 'forced': False, 'returncode': 0}
                projected = runner.project_phase(runner.PUBLIC_UPLOAD_PHASE, 'public-upload', acceptance, owned)
                imported = projected['public_upload_diagnostics']['key_import_diagnostics']
                self.assertTrue(imported['record_valid'])
                self.assertEqual(imported['attempts'], rows)
                self.assertEqual(projected['status'] == 'passed', index == len(snapshots) - 1)
                validator.phases({'schema': 1, 'phases': [projected], 'batches': []})

    def test_each_snapshot_remains_valid_across_save_failures_without_losing_terminal_failure(self):
        for case in self.serialized['saveFailures']:
            self.assertTrue(case['failed'])
            self.assertEqual(case['attempts'][-1]['status'], 'failed')
            for rows in case['snapshots']:
                acceptance = accepted_report()
                acceptance['public_upload']['key_import'] = rows
                value = diagnostics.project(acceptance)
                self.assertTrue(value['key_import_diagnostics']['record_valid'])
                self.assertEqual(value['key_import_diagnostics']['attempts'], rows)
                self.assertFalse(diagnostics.passed(value))
                diagnostics.validate(value)
                owned = {'fixture_only': True, 'phase': runner.PUBLIC_UPLOAD_PHASE, 'verified_stopped': True, 'forced': False, 'returncode': 0}
                phase = runner.project_phase(runner.PUBLIC_UPLOAD_PHASE, 'public-upload', acceptance, owned)
                self.assertEqual(phase['status'], 'failed')
                validator.phases({'schema': 1, 'phases': [phase], 'batches': []})

    if os.environ.get('WEEBDEX_KEY_IMPORT_LEDGER_BASELINE_SOURCE'):
        def test_source_extracted_aca_producer_has_thirteen_invalid_inter_step_saved_rows(self):
            snapshots = self.serialized['baselineSnapshots']
            self.assertEqual(len(snapshots), 28)
            valid = [diagnostics.project_key_import(rows)['record_valid'] for rows in snapshots]
            self.assertEqual(valid.count(False), 13)
            self.assertEqual(valid.count(True), 15)
            self.assertEqual(snapshots[1][0]['substep'], 'open-settings')
            self.assertEqual(snapshots[1][0]['completed_substeps'], 1)
            self.assertFalse(valid[1])

    def test_actual_failed_review_with_backup_connection_survives_projection_without_success(self):
        acceptance = accepted_report()
        acceptance['public_upload']['key_import'] = self.serialized['reviewFailed']
        owned = {'fixture_only': True, 'phase': runner.PUBLIC_UPLOAD_PHASE, 'verified_stopped': True, 'forced': False, 'returncode': 0}
        projected = runner.project_phase(runner.PUBLIC_UPLOAD_PHASE, 'public-upload', acceptance, owned)
        imported = projected['public_upload_diagnostics']['key_import_diagnostics']
        self.assertTrue(imported['record_valid'])
        self.assertEqual(imported['attempts'][0]['substep'], 'recipient-review')
        self.assertEqual(imported['attempts'][0]['caught_category'], 'assertion')
        self.assertEqual(imported['attempts'][0]['completed_substeps'], 8)
        self.assertEqual(projected['status'], 'failed')
        validator.phases({'schema': 1, 'phases': [projected], 'batches': []})

    def test_missing_pending_incomplete_or_invalid_rows_never_satisfy_positive_gate(self):
        for raw in [None, [], self.serialized['passed'][:1], [{'attempt': 1, 'substep': 'open-settings', 'completed_substeps': 0, 'status': 'running', 'caught_category': 'none'}],
                    [dict(self.serialized['passed'][0], attempt=True)], self.serialized['passed'] * 2,
                    [dict(self.serialized['passed'][0], substep='unknown-private-value')]]:
            with self.subTest(raw=raw):
                acceptance = accepted_report()
                acceptance['public_upload']['key_import'] = raw
                value = diagnostics.project(acceptance)
                diagnostics.validate(value)
                self.assertFalse(diagnostics.passed(value))
                self.assertNotIn('unknown-private-value', json.dumps(value))

    def test_extra_raw_fields_and_unclosed_strings_are_omitted_not_exported(self):
        secret = 'PRIVATE-SENTINEL-http://private.invalid/nsec-value'
        for field in ['substep', 'status', 'caught_category', 'unexpected-field']:
            acceptance = accepted_report()
            acceptance['public_upload']['key_import'] = copy.deepcopy(self.serialized['passed'])
            acceptance['public_upload']['key_import'][0][field] = secret
            value = diagnostics.project(acceptance)
            self.assertFalse(value['key_import_diagnostics']['record_valid'])
            self.assertEqual(value['key_import_diagnostics']['attempts'], [])
            self.assertNotIn(secret, json.dumps(value))
            diagnostics.validate(value)

    def test_nested_exact_validator_rejects_injected_fields_types_and_inconsistent_progress(self):
        value = diagnostics.project(accepted_report())
        changes = [lambda x: x['key_import_diagnostics'].update(extra='private'),
                   lambda x: x['key_import_diagnostics'].update(schema=True),
                   lambda x: x['key_import_diagnostics'].update(record_valid=False),
                   lambda x: x['key_import_diagnostics']['attempts'][0].update(extra='private'),
                   lambda x: x['key_import_diagnostics']['attempts'][0].update(completed_substeps=True),
                   lambda x: x['key_import_diagnostics']['attempts'][0].update(status='failed'),
                   lambda x: x['key_import_diagnostics']['attempts'][0].update(caught_category='none-private')]
        for change in changes:
            altered = copy.deepcopy(value)
            change(altered)
            with self.assertRaises(ValueError):
                diagnostics.validate(altered)


if __name__ == '__main__':
    unittest.main()
