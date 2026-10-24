"""Finite retained-original integrity controls. No Git, build, service or network."""
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location('retained_combined_runner', Path(__file__).with_name('run_combined_qualification.py'))
runner = importlib.util.module_from_spec(spec)
spec.loader.exec_module(runner)


class RetainedQualificationBaselines(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.work = Path(self.temporary.name)
        self.manifest = runner.RETAINED / 'manifest.json'
        self.encoded = self.manifest.read_bytes()
        self.entries = json.loads(self.encoded)['entries']
        self.name = 'WEEBDEX_KEY_IMPORT_LEDGER_BASELINE_SOURCE'
        self.source = runner.RETAINED / self.entries[self.name]['file']

    def test_all_nineteen_exact_originals_match_independent_git_blob_and_sha_witnesses(self):
        with patch.object(runner, 'git', side_effect=AssertionError('Git must not be invoked')):
            retained = runner.retained_baselines()
        self.assertEqual(len(retained), 19)
        self.assertEqual(sum(len(data) for _, data in retained.values()), 277491)
        self.assertEqual(sum(len(data) for name, (_, data) in retained.items() if name.startswith('device-restore/')), 131165)

    def test_materialization_requires_no_git_and_wires_every_counterfactual(self):
        with patch.object(runner, 'git', side_effect=AssertionError('Git must not be invoked')):
            env = runner.materialize_baselines(self.work)
        self.assertEqual(set(env), set(runner.BASELINES) | {'WEEBDEX_DEVICE_RESTORE_BASELINE_ROOT'})
        for name, (_, _, sha) in runner.BASELINES.items():
            self.assertEqual(hashlib.sha256(Path(env[name]).read_bytes()).hexdigest(), sha)
        restore = Path(env['WEEBDEX_DEVICE_RESTORE_BASELINE_ROOT'])
        self.assertEqual(sorted(str(path.relative_to(restore)) for path in restore.rglob('*') if path.is_file()),
                         sorted('platform-integrations/' + name for name in runner.RESTORE_FILES))
        provenance = json.loads((self.work / 'baseline-provenance.json').read_text())
        self.assertEqual(provenance['retained_manifest_sha256'], runner.RETAINED_MANIFEST_SHA256)

    def test_existing_materialization_is_never_overwritten(self):
        runner.materialize_baselines(self.work)
        with self.assertRaises(FileExistsError):
            runner.materialize_baselines(self.work)

    def test_manifest_whitespace_change_is_refused(self):
        with self.assertRaisesRegex(ValueError, 'manifest SHA-256 changed'):
            runner.retained_baselines(lambda path: self.encoded + b' ' if path == self.manifest else path.read_bytes())

    def test_same_size_original_mutation_is_refused(self):
        mutated = bytearray(self.source.read_bytes()); mutated[0] ^= 1
        with self.assertRaisesRegex(ValueError, 'Retained source bytes'):
            runner.retained_baselines(lambda path: bytes(mutated) if path == self.source else path.read_bytes())

    def test_coordinated_source_and_manifest_substitution_is_refused(self):
        mutated = self.source.read_bytes() + b'\n'
        manifest = json.loads(self.encoded)
        manifest['entries'][self.name].update(bytes=len(mutated), sha256=hashlib.sha256(mutated).hexdigest())
        changed = json.dumps(manifest).encode()
        with self.assertRaisesRegex(ValueError, 'manifest SHA-256 changed'):
            runner.retained_baselines(lambda path: changed if path == self.manifest else mutated if path == self.source else path.read_bytes())

    def test_missing_original_fails_before_materialization(self):
        def read(path):
            if path == self.source:
                raise FileNotFoundError('Exact original absent')
            return path.read_bytes()
        with patch.object(runner, 'retained_baselines', side_effect=FileNotFoundError('Exact original absent')):
            with self.assertRaises(FileNotFoundError):
                runner.materialize_baselines(self.work)
        self.assertFalse((self.work / 'baselines').exists())
        with self.assertRaises(FileNotFoundError):
            runner.retained_baselines(lambda path: read(path))

    def test_ledger_counterfactual_is_registered_and_missing_configuration_refuses(self):
        import test_public_upload_key_import_projection as ledger
        self.assertIn('test_source_extracted_aca_producer_has_thirteen_invalid_inter_step_saved_rows',
                      unittest.defaultTestLoader.getTestCaseNames(ledger.KeyImportProjection))
        with patch.dict(os.environ, {}, clear=True):
            with self.assertRaisesRegex(ValueError, 'Required exact key-import ledger baseline configuration is missing'):
                ledger.KeyImportProjection.setUpClass()

    def test_ledger_counterfactual_rejects_substituted_original(self):
        import test_public_upload_key_import_projection as ledger
        wrong = self.work / 'wrong.mjs'
        wrong.write_text('export const substituted = true;\n')
        with patch.dict(os.environ, {self.name: str(wrong)}):
            with self.assertRaisesRegex(ValueError, 'baseline SHA-256 differs'):
                ledger.KeyImportProjection.setUpClass()
