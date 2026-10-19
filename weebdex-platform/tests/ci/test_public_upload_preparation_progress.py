"""Finite profile operations and closed public boundaries; no build or service."""
import importlib.util
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location('progress_profile_fixture', ROOT / 'tests/ci/test_public_upload_hosted_profile.py')
fixture = importlib.util.module_from_spec(spec)
spec.loader.exec_module(fixture)
builder, validator = fixture.builder, fixture.validator


class UploadPreparationProgress(unittest.TestCase):
    def setup_profile(self, directory, fail_at=None):
        return fixture.PublicUploadHostedProfile().profile_fixture(Path(directory), fail_at)

    def test_success_visits_exact_closed_action_boundaries_and_restores_native(self):
        with tempfile.TemporaryDirectory() as directory, patch.dict(os.environ):
            root = Path(directory)
            stage, evidence, ui, saved, _, run = self.setup_profile(directory)
            rows = []
            result = builder.prepare(root, stage, run, lambda: None, rows.append)
            self.assertEqual(rows, list(builder.PUBLIC_STAGES[:-1]))
            self.assertTrue(result['enabled'])
            self.assertEqual((ui / 'index.html').read_bytes(), b'native artifact')
            self.assertEqual({name: (evidence / name).read_bytes() for name in saved}, saved)

    def test_owned_compile_and_record_failures_retain_distinct_boundaries(self):
        for label, expected in [('public-upload-frontend-build', 'public-upload-qa-build'),
                                ('public-upload-artifact-record', 'public-upload-qa-record')]:
            with self.subTest(label=label), tempfile.TemporaryDirectory() as directory, patch.dict(os.environ):
                root = Path(directory)
                stage, _, ui, _, _, run = self.setup_profile(directory, label)
                rows = []
                with self.assertRaisesRegex(RuntimeError, '^synthetic build failure$'):
                    builder.prepare(root, stage, run, lambda: None, rows.append)
                self.assertEqual(rows[-1], expected)
                self.assertEqual((ui / 'index.html').read_bytes(), b'native artifact')

    def test_receipt_and_graph_refusals_have_distinct_boundaries(self):
        for kind, expected in [('receipt', 'public-upload-qa-receipt'), ('graph', 'public-upload-qa-graph')]:
            with self.subTest(kind=kind), tempfile.TemporaryDirectory() as directory, patch.dict(os.environ):
                root = Path(directory)
                stage, evidence, ui, _, _, run = self.setup_profile(directory)
                def owned(label, *args):
                    result = run(label, *args)
                    if label == 'public-upload-frontend-build':
                        if kind == 'receipt':
                            (evidence / 'build-integration.json').write_text('{}')
                        else:
                            (ui / fixture.qa_fixture.qa.GRAPH).unlink()
                    return result
                rows = []
                with self.assertRaises((RuntimeError, FileNotFoundError)):
                    builder.prepare(root, stage, owned, lambda: None, rows.append)
                self.assertEqual(rows[-1], expected)
                self.assertEqual((ui / 'index.html').read_bytes(), b'native artifact')

    def test_source_refusal_preserves_primary_exception_and_restores(self):
        with tempfile.TemporaryDirectory() as directory, patch.dict(os.environ):
            root = Path(directory)
            stage, _, ui, _, _, run = self.setup_profile(directory)
            primary = RuntimeError('private source failure sentinel')
            def refuse():
                raise primary
            rows = []
            with self.assertRaises(RuntimeError) as caught:
                builder.prepare(root, stage, run, refuse, rows.append)
            self.assertIs(caught.exception, primary)
            self.assertEqual(rows[-1], 'public-upload-qa-source')
            self.assertEqual((ui / 'index.html').read_bytes(), b'native artifact')

    def test_native_off_and_missing_evidence_refuse_before_compile(self):
        for kind, expected in [('off', 'public-upload-profile-native-off'),
                               ('evidence', 'public-upload-profile-native-evidence')]:
            with self.subTest(kind=kind), tempfile.TemporaryDirectory() as directory, patch.dict(os.environ):
                root = Path(directory)
                stage, evidence, ui, _, calls, run = self.setup_profile(directory)
                if kind == 'off':
                    (ui / 'leaked.js').write_text('readUploadRequestRefusals')
                else:
                    (evidence / builder.NATIVE_EVIDENCE[0]).unlink()
                rows = []
                with self.assertRaises(RuntimeError):
                    builder.prepare(root, stage, run, lambda: None, rows.append)
                self.assertEqual(rows[-1], expected)
                self.assertEqual(calls, [])

    def test_restore_failure_is_reported_without_replacing_its_exception(self):
        with tempfile.TemporaryDirectory() as directory, patch.dict(os.environ):
            root = Path(directory)
            stage, _, _, _, _, run = self.setup_profile(directory)
            primary = OSError('private restore sentinel')
            original = Path.rename
            def rename(path, target):
                if path == stage / 'public-upload-native-frontend':
                    raise primary
                return original(path, target)
            rows = []
            def progress(value):
                rows.append(value)
                if value == 'public-upload-profile-restore':
                    raise RuntimeError('secondary diagnostic failure')
            with patch.object(Path, 'rename', rename), self.assertRaises(OSError) as caught:
                builder.prepare(root, stage, run, lambda: None, progress)
            self.assertIs(caught.exception, primary)
            self.assertEqual(rows[-1], 'public-upload-profile-restore')

    def test_every_fixed_boundary_is_accepted_but_no_private_text_or_success_promotion(self):
        template = {'schema': 1, 'status': 'preparation_failed', 'browser_phases_started': False,
                    'source_files_checked': 676, 'frontend_preserved': True,
                    'artifacts_uploaded': False, 'error_class': 'RuntimeError'}
        for stage in builder.PUBLIC_STAGES:
            with self.subTest(stage=stage):
                validator.preparation(dict(template, stage=stage))
                with self.assertRaises(ValueError):
                    validator.preparation(dict(template, stage=stage, status='prepared'))
        for stage in ['private URL sentinel', 'public-upload-qa-build/private', None, True]:
            with self.subTest(stage=stage), self.assertRaises(ValueError):
                validator.preparation(dict(template, stage=stage))


if __name__ == '__main__':
    unittest.main()
