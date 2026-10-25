"""Finite driver controls; no dependencies, application, browser, or network."""
import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location('combined_runner', Path(__file__).with_name('run_combined_qualification.py'))
runner = importlib.util.module_from_spec(spec)
spec.loader.exec_module(runner)


class QualificationDriverTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)

    def test_discovery_includes_both_node_naming_patterns_and_separates_semantic_consumers(self):
        names = ['test_source.mjs', 'device.test.mjs', 'test_unit.py', *runner.CONSUMERS, runner.ARTIFACT,
                 'helper-fixture.mjs', 'probe_live.mjs']
        for name in names:
            (self.root / name).write_text('// finite fixture\n')
        with patch.object(runner, 'CI', self.root), patch.object(runner, 'REPO', self.root):
            rows = runner.discover()
        observed = {row['path']: row['phase'] for row in rows}
        self.assertEqual(observed, {'test_source.mjs': 'node', 'device.test.mjs': 'node', 'test_unit.py': 'python',
                                   **{name: 'strict-consumer' for name in runner.CONSUMERS}, runner.ARTIFACT: 'artifact'})
        self.assertTrue(all(len(row['sha256']) == 64 for row in rows))

    def test_owned_alias_is_idempotent_but_existing_paths_are_not_replaced(self):
        source = self.root / 'source'
        source.mkdir()
        alias = self.root / 'owned' / 'alias'
        runner.link(alias, source)
        runner.link(alias, source)
        runner.link(source, source)  # Main checkout may already hold the actual restored stage.
        other = self.root / 'other'
        other.mkdir()
        with self.assertRaisesRegex(ValueError, 'Refusing to replace'):
            runner.link(alias, other)
        regular = self.root / 'regular'
        regular.mkdir()
        with self.assertRaisesRegex(ValueError, 'Refusing to replace'):
            runner.link(regular, source)

    def test_discovery_follows_package_aliases_to_actual_physical_node_modules(self):
        container = self.root / 'container'
        container.mkdir()
        package = self.root / 'physical/node_modules/package'
        package.mkdir(parents=True)
        (package / 'package.json').write_text('{}')
        (container / 'package').symlink_to(package, target_is_directory=True)
        roots = runner.physical_roots([container])
        self.assertEqual(roots, sorted([str(container), str(package.parent)]))

    def test_uninventoried_non_package_symlink_fails_closed(self):
        container = self.root / 'container'
        container.mkdir()
        outside = self.root / 'outside'
        outside.mkdir()
        (container / 'escape').symlink_to(outside, target_is_directory=True)
        with self.assertRaisesRegex(ValueError, 'outside a package root'):
            runner.physical_roots([container])

    def test_exact_snapshot_rejects_byte_and_nanosecond_metadata_changes(self):
        cache = self.root / 'cache'
        cache.mkdir()
        file = cache / 'input'
        file.write_bytes(b'original')
        runner.snapshot([str(cache)], self.root, 'before')
        runner.snapshot([str(cache)], self.root, 'after')
        file.write_bytes(b'mutated!')
        with self.assertRaisesRegex(ValueError, 'bytes, mode, mtime or symlink changed'):
            runner.snapshot([str(cache)], self.root, 'after-mutation')

    def test_required_runtime_configuration_never_silently_uses_historical_defaults(self):
        with self.assertRaises(KeyError):
            runner.setup({}, self.root)

    def test_cli_refuses_to_overwrite_any_previous_evidence(self):
        output = self.root / 'already-exists'
        output.mkdir()
        sentinel = output / 'keep.json'
        sentinel.write_text('unchanged')
        result = subprocess.run([sys.executable, str(Path(runner.__file__)), '--config', str(self.root / 'missing.json'),
                                 '--output', str(output)], capture_output=True, text=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('FileExistsError', result.stderr)
        self.assertEqual(sentinel.read_text(), 'unchanged')

    def test_source_pins_and_phase_selection_remain_explicit(self):
        parser = subprocess.run([sys.executable, str(Path(runner.__file__)), '--help'], capture_output=True, text=True)
        self.assertEqual(parser.returncode, 0)
        self.assertIn('--phase {finite,strict,artifact,all}', parser.stdout)
        self.assertEqual(set(runner.BASELINES), {'WEEBDEX_UPLOAD_REFUSAL_BASELINE_SOURCE',
            'WEEBDEX_UPLOAD_OBSERVER_BASELINE_SOURCE', 'WEEBDEX_UPLOAD_GROUP_BASELINE_SOURCE',
            'WEEBDEX_UPLOAD_SECOND_BEGIN_BASELINE_SOURCE', 'WEEBDEX_KEY_IMPORT_DRIVER_BASELINE_SOURCE',
            'WEEBDEX_PUBLIC_UPLOAD_BASELINE_SOURCE', 'WEEBDEX_KEY_IMPORT_LEDGER_BASELINE_SOURCE'})
