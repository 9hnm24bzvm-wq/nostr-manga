"""Pure provenance controls: no mounted fixture, service or benchmark launched."""
import importlib.util
import json
import os
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

import qualification_source_export as exports


class MountedExportProvenanceTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.base = Path(self.temporary.name)
        self.root = self.base / 'export'
        self.platform = self.root / 'weebdex-platform'
        self.platform.mkdir(parents=True)
        actual = Path(__file__).resolve().parents[2]
        with patch.object(sys, 'path', [str(actual / 'scripts'), *sys.path]):
            spec = importlib.util.spec_from_file_location('mounted_export_control', actual / 'scripts/integrated_load_scenarios.py')
            self.subject = importlib.util.module_from_spec(spec)
            spec.loader.exec_module(self.subject)
        self.subject.ROOT = self.platform
        files = {
            'scripts/integrated_load_scenarios.py': b'# synthetic driver\n',
            'scripts/integrated_load_budget.py': b'# synthetic budget\n',
            'scripts/integrated-load/mounted-reader.mjs': b'// synthetic mounted source\n',
            self.subject.EXPORT_HELPER: (actual / self.subject.EXPORT_HELPER).read_bytes(),
        }
        self.driver_hashes = {name: exports.sha256(data) for name, data in files.items() if name != self.subject.EXPORT_HELPER}
        rows = []
        for name, data in sorted(files.items()):
            path = self.platform / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(data); path.chmod(0o644)
            rows.append({'path': 'weebdex-platform/' + name, 'mode': '100644', 'bytes': len(data),
                         'sha256': exports.sha256(data), 'git_blob': exports.git_object('blob', data)})
        tree = exports.tree_oid(rows)
        self.manifest = {'schema': exports.SCHEMA, 'scope': 'repository-tree',
                         'origin': {'commit': 'a' * 40, 'tree': tree},
                         'selection': [{'path': '', 'kind': 'tree', 'git_oid': tree}], 'files': rows}
        self.path = self.base / 'manifest.json'
        self.save()

    def save(self):
        self.path.write_text(json.dumps(self.manifest))
        self.env = dict(zip(self.subject.EXPORT_ENV, [str(self.path), exports.sha256(self.path.read_bytes())]))

    def context(self):
        with patch.dict(os.environ, self.env, clear=True):
            return self.subject.export_source(self.driver_hashes)

    def test_valid_subset_never_calls_git_or_claims_complete_tree_bytes(self):
        with patch.object(self.subject.subprocess, 'check_output', side_effect=AssertionError('Git forbidden')):
            source = self.context()
            result = self.subject.source_identity(self.driver_hashes, source)
        self.assertNotIn('source_commit', result)
        provenance = result['source_provenance']
        self.assertEqual(provenance['declared_origin_commit'], 'a' * 40)
        self.assertFalse(provenance['origin_commit_ancestry_verified'])
        self.assertFalse(provenance['complete_export_bytes_verified_by_consumer'])
        self.assertTrue(provenance['outer_full_before_after_inventory_required'])
        self.assertEqual(len(provenance['consumer_source_files_verified']), 4)

    def test_default_context_retains_real_git_contract(self):
        with patch.dict(os.environ, {}, clear=True):
            self.assertIsNone(self.subject.export_source(self.driver_hashes))
        with patch.object(self.subject.subprocess, 'check_output', return_value='b' * 40 + '\n') as git:
            self.assertEqual(self.subject.source_identity(self.driver_hashes, None), {'source_commit': 'b' * 40})
        git.assert_called_once_with(['git', 'rev-parse', 'HEAD'], cwd=self.platform, text=True)

    def test_half_empty_wrong_digest_contexts_fail(self):
        original = dict(self.env)
        for env in [{key: value} for key, value in original.items()] + [
                {**original, self.subject.EXPORT_ENV[0]: ''},
                {**original, self.subject.EXPORT_ENV[1]: '0' * 64}]:
            with self.subTest(env=env), patch.dict(os.environ, env, clear=True), self.assertRaises(ValueError):
                self.subject.export_source(self.driver_hashes)

    def test_changed_driver_bytes_mode_or_missing_file_fail(self):
        path = self.platform / next(iter(self.driver_hashes))
        original = path.read_bytes()
        for change in ['bytes', 'mode', 'missing']:
            if change == 'bytes': path.write_bytes(b'changed')
            elif change == 'mode': path.chmod(0o755)
            else: path.unlink()
            with self.subTest(change=change), self.assertRaises((ValueError, FileNotFoundError)):
                self.context()
            path.write_bytes(original); path.chmod(0o644)

    def test_missing_driver_membership_fails_even_with_valid_new_witness(self):
        self.manifest['files'] = self.manifest['files'][1:]
        tree = exports.tree_oid(self.manifest['files'])
        self.manifest['origin']['tree'] = tree
        self.manifest['selection'][0]['git_oid'] = tree
        self.save()
        with self.assertRaisesRegex(ValueError, 'missing from export manifest'):
            self.context()

    def test_helper_tamper_refused_before_import(self):
        (self.platform / self.subject.EXPORT_HELPER).write_text('raise AssertionError("must not execute")')
        with self.assertRaisesRegex(ValueError, 'helper differs'):
            self.context()

    def test_manifest_or_driver_drift_rejected_after_initial_verification(self):
        source = self.context()
        encoded = self.path.read_bytes()
        self.path.write_bytes(encoded + b' ')
        with self.assertRaisesRegex(ValueError, 'manifest changed'):
            self.subject.source_identity(self.driver_hashes, source)
        self.path.write_bytes(encoded)
        (self.platform / next(iter(self.driver_hashes))).write_bytes(b'changed')
        with self.assertRaisesRegex(ValueError, 'source byte count changed'):
            self.subject.source_identity(self.driver_hashes, source)

    def test_subset_does_not_claim_unrelated_source_and_refuses_symlinks(self):
        source = self.context()
        (self.root / 'unrelated-extra').write_text('Outer full verification must reject this.')
        self.subject.source_identity(self.driver_hashes, source)
        with self.assertRaisesRegex(ValueError, 'Unlisted source file'):
            source.verify()
        path = self.platform / next(iter(self.driver_hashes))
        data = path.read_bytes(); path.unlink()
        target = self.base / 'outside'; target.write_bytes(data); path.symlink_to(target)
        with self.assertRaisesRegex(ValueError, 'symlink'):
            self.subject.source_identity(self.driver_hashes, source)

    def test_half_context_rejected_before_fixture_or_child(self):
        import asyncio
        with patch.object(self.subject, 'check_runtime'), patch.object(self.subject.tempfile, 'TemporaryDirectory', side_effect=AssertionError('Fixture launched')), \
             patch.dict(os.environ, {self.subject.EXPORT_ENV[0]: str(self.path)}, clear=True), self.assertRaisesRegex(ValueError, 'Both explicit'):
            asyncio.run(self.subject.run({}))
