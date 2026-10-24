"""Finite no-Git provenance/tamper controls. Synthetic trees, never a full run."""
import copy
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location('source_export', Path(__file__).with_name('qualification_source_export.py'))
exports = importlib.util.module_from_spec(spec)
spec.loader.exec_module(exports)


class SourceExportTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.base = Path(self.temporary.name)
        self.root = self.base / 'export'
        self.root.mkdir()
        self.path = self.base / 'manifest.json'
        self.files = {'config.json': b'{"finite":true}\n', 'source/run.py': b'print("synthetic")\n', 'tests/check.py': b'assert True\n'}
        for name, data in self.files.items():
            path = self.root / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(data)
            path.chmod(0o755 if name == 'source/run.py' else 0o644)
        self.manifest = self.build()

    def build(self, *, selected=False):
        rows = [{'path': name, 'mode': '100755' if name == 'source/run.py' else '100644',
                 'bytes': len(data), 'sha256': exports.sha256(data), 'git_blob': exports.git_object('blob', data)}
                for name, data in sorted(self.files.items())]
        tree = exports.tree_oid(rows)
        selection = [{'path': '', 'kind': 'tree', 'git_oid': tree}]
        if selected:
            selection = [{'path': 'config.json', 'kind': 'blob', 'git_oid': rows[0]['git_blob']}]
            for name in ['source', 'tests']:
                children = [dict(row, path=row['path'].removeprefix(name + '/')) for row in rows if row['path'].startswith(name + '/')]
                selection.append({'path': name, 'kind': 'tree', 'git_oid': exports.tree_oid(children)})
        return {'schema': exports.SCHEMA, 'scope': 'selected-input-closure' if selected else 'repository-tree',
                'origin': {'commit': 'a' * 40, 'tree': tree}, 'selection': selection, 'files': rows}

    def save(self, manifest=None, encoded=None):
        data = encoded if encoded is not None else (json.dumps(manifest or self.manifest, sort_keys=True) + '\n').encode()
        self.path.write_bytes(data)
        return exports.sha256(data)

    def verifier(self, manifest=None):
        return exports.ExportSource(self.root, self.path, self.save(manifest))

    def test_complete_tree_and_regular_executable_modes_verify_without_fabricated_ancestry(self):
        source = self.verifier()
        self.assertEqual(set(source.verify()), set(self.files))
        self.assertTrue(source.provenance()['complete_origin_repository_tree_verified'])
        self.assertFalse(source.provenance()['origin_commit_ancestry_verified'])
        self.assertNotIn('sourceCommit', source.provenance())

    def test_selected_closure_is_explicitly_not_the_full_repository(self):
        with patch.object(exports, 'REQUIRED_INPUTS', ('config.json', 'source', 'tests')):
            source = self.verifier(self.build(selected=True))
            source.verify()
            self.assertFalse(source.provenance()['complete_origin_repository_tree_verified'])

    def test_oversized_manifest_total_refuses_before_reading_source_without_large_allocation(self):
        changed = copy.deepcopy(self.manifest)
        changed['files'] = [dict(changed['files'][0], path=f'source/{index}.py',
                                 bytes=exports.MAX_FILE_BYTES) for index in range(5)]
        with self.assertRaisesRegex(ValueError, 'Aggregate source byte limit exceeded'):
            self.verifier(changed)

    def test_preparer_checks_counts_types_and_aggregate_sizes_before_any_content_capture(self):
        rows = [(f'{index + 1:040x}', f'source/{index}.py') for index in range(5)]
        listing = b''.join(f'100644 blob {oid}\t{name}\0'.encode() for oid, name in rows)
        def git_output(args, **kwargs):
            if args[1] == 'rev-parse':
                return ('a' * 40 + '\n').encode()
            if args[1] == 'cat-file':
                return b'tree\n'
            if args[1] == 'ls-tree':
                return listing
            self.fail('Unexpected Git request: ' + repr(args))
        cases = {
            'total': ([f'{oid} blob {exports.MAX_FILE_BYTES}' for oid, _ in rows], 'Aggregate source byte limit'),
            'individual': ([f'{oid} blob {exports.MAX_FILE_BYTES + 1}' for oid, _ in rows], 'preflight identity, type or size'),
            'type': ([f'{oid} tree 1' for oid, _ in rows], 'preflight identity, type or size'),
            'identity': ([f'{"b" * 40} blob 1' for _ in rows], 'preflight identity, type or size'),
            'missing': ([f'{oid} missing' for oid, _ in rows], 'preflight record'),
            'count': ([f'{rows[0][0]} blob 1'], 'preflight count differs'),
        }
        for name, (metadata, expected) in cases.items():
            def git_run(args, **kwargs):
                self.assertEqual(args, ['git', 'cat-file', '--batch-check'])
                return subprocess.CompletedProcess(args, 0, ('\n'.join(metadata) + '\n').encode())
            with self.subTest(name=name), patch('subprocess.check_output', side_effect=git_output), \
                    patch('subprocess.run', side_effect=git_run) as run:
                with self.assertRaisesRegex(ValueError, expected):
                    exports.prepare_manifest(self.root, 'a' * 40, 'repository-tree')
                self.assertEqual(run.call_count, 1)
        with patch('subprocess.check_output', side_effect=git_output), \
                patch('subprocess.run') as run, patch.object(exports, 'MAX_SOURCE_FILES', 4):
            with self.assertRaisesRegex(ValueError, 'source-file count'):
                exports.prepare_manifest(self.root, 'a' * 40, 'repository-tree')
            run.assert_not_called()

    def test_preparer_reads_exact_preflighted_blobs_only_after_metadata_validation(self):
        data = b'synthetic-original\n'; oid = exports.git_object('blob', data)
        row = {'path': 'source.txt', 'mode': '100644', 'git_blob': oid}
        tree = exports.tree_oid([row])
        def git_output(args, **kwargs):
            if args[1] == 'rev-parse':
                return ((tree if args[-1].endswith('^{tree}') else 'a' * 40) + '\n').encode()
            if args[1] == 'cat-file':
                return b'tree\n'
            return f'100644 blob {oid}\tsource.txt\0'.encode()
        calls = []
        def git_run(args, **kwargs):
            calls.append(args[-1]); self.assertEqual(kwargs['input'], (oid + '\n').encode())
            header = f'{oid} blob {len(data)}\n'.encode()
            payload = header if args[-1] == '--batch-check' else header + data + b'\n'
            return subprocess.CompletedProcess(args, 0, payload)
        with patch('subprocess.check_output', side_effect=git_output), patch('subprocess.run', side_effect=git_run):
            manifest = exports.prepare_manifest(self.root, 'a' * 40, 'repository-tree')
        self.assertEqual(calls, ['--batch-check', '--batch'])
        self.assertEqual(manifest['files'], [dict(row, bytes=len(data), sha256=exports.sha256(data))])

    def test_exact_old_source_accepts_oversized_manifest_and_requests_content_without_preflight(self):
        import types
        fixture = Path(__file__).parents[1] / 'fixtures/qualification-source-export/before-aggregate-preflight.py.txt'
        data = fixture.read_bytes()
        self.assertEqual(exports.sha256(data), 'e3158b7bc1c5ea025a1833266a2affa1e3f7d003477d6e71143da634a9c476c7')
        legacy = types.ModuleType('source_export_before_aggregate_preflight')
        exec(compile(data, str(fixture), 'exec'), legacy.__dict__)
        changed = copy.deepcopy(self.manifest)
        changed['files'] = [dict(changed['files'][0], path=f'source/{index}.py',
                                 bytes=exports.MAX_FILE_BYTES) for index in range(5)]
        tree = exports.tree_oid(changed['files'])
        changed['selection'][0]['git_oid'] = tree; changed['origin']['tree'] = tree
        expected = self.save(changed)
        legacy.ExportSource(self.root, self.path, expected)  # Old constructor accepts 320 MiB metadata.
        with self.assertRaisesRegex(ValueError, 'Aggregate source byte limit exceeded'):
            exports.ExportSource(self.root, self.path, expected)
        rows = changed['files']
        def git_output(args, **kwargs):
            if args[1] == 'rev-parse':
                return ('a' * 40 + '\n').encode()
            if args[1] == 'cat-file':
                return b'tree\n'
            return b''.join(f"100644 blob {row['git_blob']}\t{row['path']}\0".encode() for row in rows)
        def git_run(args, **kwargs):
            if args[-1] == '--batch':
                raise AssertionError('content was requested before oversized metadata preflight')
            payload = ''.join(f"{row['git_blob']} blob {row['bytes']}\n" for row in rows).encode()
            return subprocess.CompletedProcess(args, 0, payload)
        with patch('subprocess.check_output', side_effect=git_output), patch('subprocess.run', side_effect=git_run):
            with self.assertRaisesRegex(AssertionError, 'content was requested before oversized metadata preflight'):
                legacy.prepare_manifest(self.root, 'a' * 40, 'repository-tree')
            with self.assertRaisesRegex(ValueError, 'Aggregate source byte limit exceeded'):
                exports.prepare_manifest(self.root, 'a' * 40, 'repository-tree')

    def test_independent_expected_digest_is_required_and_must_match(self):
        expected = self.save()
        for value in [None, '', '0' * 64, expected.upper()]:
            with self.assertRaises(ValueError):
                exports.ExportSource(self.root, self.path, value)

    def test_manifest_cannot_supply_its_own_expected_digest(self):
        old = self.save()
        changed = copy.deepcopy(self.manifest)
        changed['origin']['commit'] = 'b' * 40
        self.save(changed)
        with self.assertRaisesRegex(ValueError, 'externally supplied witness'):
            exports.ExportSource(self.root, self.path, old)

    def test_manifest_inside_source_tree_is_refused(self):
        digest = self.save()
        inside = self.root / 'manifest.json'
        inside.write_bytes(self.path.read_bytes())
        with self.assertRaisesRegex(ValueError, 'outside'):
            exports.ExportSource(self.root, inside, digest)

    def test_duplicate_json_fields_are_refused_before_semantic_validation(self):
        digest = self.save(encoded=b'{"schema":"one","schema":"two"}')
        with self.assertRaisesRegex(ValueError, 'Duplicate JSON field'):
            exports.ExportSource(self.root, self.path, digest)

    def test_missing_source_file_fails(self):
        source = self.verifier()
        (self.root / 'tests/check.py').unlink()
        with self.assertRaisesRegex(ValueError, 'missing'):
            source.verify()

    def test_same_size_source_mutation_fails_after_initial_validation(self):
        source = self.verifier(); source.verify()
        path = self.root / 'tests/check.py'
        path.write_bytes(b'assert None\n')
        with self.assertRaisesRegex(ValueError, 'bytes changed'):
            source.verify()

    def test_extra_executable_or_nonexecutable_source_is_never_ignored(self):
        source = self.verifier()
        for name in ['tests/unlisted.py', 'extra.json', '.build/not-owned/generated.js']:
            path = self.root / name; path.parent.mkdir(parents=True, exist_ok=True); path.write_text('extra')
            with self.assertRaisesRegex(ValueError, 'Unlisted source file'):
                source.verify()
            path.unlink()

    def test_executable_bit_drift_fails(self):
        source = self.verifier()
        (self.root / 'source/run.py').chmod(0o644)
        with self.assertRaisesRegex(ValueError, 'mode changed'):
            source.verify()

    def test_noncanonical_paths_duplicates_case_aliases_and_unsafe_modes_fail(self):
        for path in ['/absolute', '../escape', 'source/../escape', 'source//run.py', 'source\\run.py', 'C:/escape', '.git/config', 'source/name.', 'source/name ', 'source/e\u0301.py']:
            changed = copy.deepcopy(self.manifest); changed['files'][0]['path'] = path
            with self.subTest(path=path), self.assertRaises(ValueError):
                self.verifier(changed)
        for mutate in [lambda m: m['files'].append(m['files'][0]),
                       lambda m: m['files'].append(dict(m['files'][1], path='SOURCE/other.py')),
                       lambda m: m['files'][0].update(mode='120000'),
                       lambda m: m['files'][0].update(mode='160000')]:
            changed = copy.deepcopy(self.manifest); mutate(changed)
            with self.assertRaises(ValueError):
                self.verifier(changed)

    def test_source_symlink_and_symlinked_source_directory_are_refused(self):
        source = self.verifier()
        file = self.root / 'tests/check.py'; outside = self.base / 'outside.py'
        outside.write_bytes(file.read_bytes()); file.unlink(); file.symlink_to(outside)
        with self.assertRaisesRegex(ValueError, 'symlinks are unsupported'):
            source.verify()
        file.unlink(); (self.root / 'tests').rmdir(); external = self.base / 'outside-dir'; external.mkdir()
        (external / 'check.py').write_bytes(self.files['tests/check.py']); (self.root / 'tests').symlink_to(external, target_is_directory=True)
        with self.assertRaisesRegex(ValueError, 'symlinks are unsupported'):
            source.verify()

    def test_changed_selected_git_identity_or_origin_tree_fails(self):
        for mutate in [lambda m: m['selection'][0].update(git_oid='b' * 40),
                       lambda m: m['origin'].update(tree='b' * 40)]:
            changed = copy.deepcopy(self.manifest); mutate(changed)
            with self.assertRaises(ValueError):
                self.verifier(changed)

    def test_selected_scope_requires_all_declared_finite_inputs(self):
        with patch.object(exports, 'REQUIRED_INPUTS', ('config.json', 'source', 'tests', 'required-fixtures')):
            with self.assertRaisesRegex(ValueError, 'omits a required finite'):
                self.verifier(self.build(selected=True))

    def test_overlapping_or_missing_selection_is_refused(self):
        changed = self.build(selected=True); changed['selection'].append({'path': 'tests/check.py', 'kind': 'blob', 'git_oid': changed['files'][-1]['git_blob']})
        with self.assertRaisesRegex(ValueError, 'Overlapping'):
            self.verifier(changed)
        changed = self.build(selected=True); changed['selection'].pop()
        with self.assertRaisesRegex(ValueError, 'Incomplete'):
            self.verifier(changed)

    def test_manifest_drift_is_checked_again_after_initial_validation(self):
        source = self.verifier(); source.verify(); self.path.write_bytes(self.path.read_bytes() + b' ')
        with self.assertRaisesRegex(ValueError, 'manifest changed during'):
            source.verify()

    def test_exact_runtime_outputs_and_aliases_are_snapshotted_and_must_not_change(self):
        source = self.verifier(); source.verify()
        work = self.root / '.build/owned'; work.mkdir(parents=True)
        baseline = work / 'baseline.txt'; baseline.write_text('original')
        external = self.base / 'dependencies'; external.mkdir()
        alias = self.root / '.build/dependency'; alias.symlink_to(external, target_is_directory=True)
        source.bind_runtime([work, alias]); source.verify()
        baseline.write_text('modified')
        with self.assertRaisesRegex(ValueError, 'runtime files or dependency aliases changed'):
            source.verify()
        baseline.write_text('original'); alias.unlink(); alias.symlink_to(self.base, target_is_directory=True)
        with self.assertRaisesRegex(ValueError, 'runtime files or dependency aliases changed'):
            source.verify()

    def test_runtime_exclusions_cannot_hide_any_source(self):
        source = self.verifier()
        with self.assertRaisesRegex(ValueError, 'overlaps source'):
            source.bind_runtime([self.root / 'tests'])

    def test_validator_actually_runs_with_git_absent_from_path(self):
        expected = self.save()
        script = "import importlib.util,pathlib,sys;spec=importlib.util.spec_from_file_location('exports',sys.argv[1]);m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m);m.ExportSource(sys.argv[2],sys.argv[3],sys.argv[4]).verify()"
        result = subprocess.run([sys.executable, '-B', '-c', script, exports.__file__, str(self.root), str(self.path), expected],
                                env={**os.environ, 'PATH': '', 'PYTHONDONTWRITEBYTECODE': '1'}, capture_output=True, text=True, timeout=10)
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_runner_refuses_unsupported_export_phases_before_creating_output(self):
        runner = Path(__file__).with_name('run_combined_qualification.py')
        for phase in ['strict', 'artifact', 'all']:
            output = self.base / ('output-' + phase)
            result = subprocess.run([sys.executable, '-B', str(runner), '--config', str(self.base / 'absent.json'),
                '--output', str(output), '--phase', phase, '--export-manifest', str(self.path), '--export-manifest-sha256', 'a' * 64],
                env={**os.environ, 'PATH': ''}, capture_output=True, text=True, timeout=10)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn('supports only --phase finite', result.stderr)
            self.assertFalse(output.exists())

    def test_runner_requires_both_explicit_manifest_arguments(self):
        runner = Path(__file__).with_name('run_combined_qualification.py')
        for extra in [['--export-manifest', str(self.path)], ['--export-manifest-sha256', 'a' * 64]]:
            output = self.base / 'output'
            result = subprocess.run([sys.executable, '-B', str(runner), '--config', str(self.base / 'absent.json'),
                '--output', str(output), '--phase', 'finite', *extra], capture_output=True, text=True, timeout=10)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn('Both export manifest', result.stderr)
            self.assertFalse(output.exists())

    def test_empty_git_tree_hash_matches_standard_independent_identity(self):
        self.assertEqual(exports.tree_oid([]), '4b825dc642cb6eb9a060e54bf8d69288fbee4904')


class ExportRunnerMainSeams(unittest.TestCase):
    """Actual runner main/inventories; explicit setup/process stand-ins only."""
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.base = Path(self.temporary.name)
        self.root = self.base / 'export'
        self.platform = self.root / 'weebdex-platform'
        self.ci = self.platform / 'tests/ci'
        self.ci.mkdir(parents=True)
        self.source = self.platform / 'frontend/input.ts'
        self.source.parent.mkdir()
        self.source.write_text('export const synthetic = true;\n')
        (self.source.parent / 'package.json').write_text('{}\n')
        (self.source.parent / 'yarn.lock').write_text('synthetic lockfile\n')
        helper = self.ci / 'qualification_source_export.py'
        helper.write_bytes(Path(exports.__file__).read_bytes())
        (self.ci / 'test_synthetic.mjs').write_text('// Explicit source inventory fixture, not executed.\n')
        (self.ci / 'test_synthetic.py').write_text('# Explicit source inventory fixture, not executed.\n')
        rows = []
        for path in sorted(self.root.rglob('*')):
            if not path.is_file():
                continue
            path.chmod(0o644)
            data = path.read_bytes()
            rows.append({'path': str(path.relative_to(self.root)), 'mode': '100644', 'bytes': len(data),
                         'sha256': exports.sha256(data), 'git_blob': exports.git_object('blob', data)})
        self.manifest = self.base / 'manifest.json'
        tree = exports.tree_oid(rows)
        self.manifest.write_text(json.dumps({'schema': exports.SCHEMA, 'scope': 'repository-tree',
            'origin': {'commit': 'a' * 40, 'tree': tree}, 'selection': [{'path': '', 'kind': 'tree', 'git_oid': tree}], 'files': rows}))
        self.config = self.base / 'config.json'
        self.bits = self.base / 'bits'; self.bits.mkdir(); (self.bits / 'pinned').write_text('synthetic pinned source')
        self.frontend_controls = self.base / 'frontend-dependency-controls'; self.frontend_controls.mkdir()
        for name in ['package.json', 'yarn.lock']:
            (self.frontend_controls / name).write_bytes((self.source.parent / name).read_bytes())
        self.config.write_text(json.dumps({'frontend': str(self.frontend_controls), 'bits_dialog_sources': str(self.bits)}))
        self.output = self.base / 'result'
        spec = importlib.util.spec_from_file_location('export_main_runner', Path(__file__).with_name('run_combined_qualification.py'))
        self.runner = importlib.util.module_from_spec(spec); spec.loader.exec_module(self.runner)
        self.cache = self.base / 'node_modules'; self.cache.mkdir(); (self.cache / 'fixed').write_text('pinned stand-in')
        self.commands = []

    def setup_standin(self, config, work):
        for name in ['local-library-noble-2.0.1', 'local-signing-base-2.0.0']:
            (self.platform / '.build' / name).symlink_to(self.cache, target_is_directory=True)
        (self.root / 'manga-protocol-lab/browser-client/node_modules').mkdir(parents=True)
        return dict(os.environ), [self.cache]

    def run_main(self, *, drift=None):
        def process_standin(command, cwd, env, out, name, **kwargs):
            self.commands.append((name, command))
            self.assertEqual(env['WEEBDEX_FINITE_EXPORT_MANIFEST'], str(self.manifest))
            self.assertEqual(env['WEEBDEX_FINITE_EXPORT_MANIFEST_SHA256'], exports.sha256(self.manifest.read_bytes()))
            (out / (name + '.log')).write_text('Explicit process stand-in; no application or test process executed.\n')
            if name == 'python':
                (out / 'python-results.json').write_text('{"testsRun":0,"failures":0,"errors":0,"skipped":[]}')
                if drift == 'source':
                    self.source.write_text('export const synthetic = null;\n')
                elif drift == 'dependency-control':
                    (self.frontend_controls / 'package.json').write_text('{"changed":true}\n')
                elif drift == 'dependency-metadata':
                    path = self.frontend_controls / 'package.json'; status = path.stat()
                    os.utime(path, ns=(status.st_atime_ns, status.st_mtime_ns + 1))
                elif drift == 'runtime':
                    next((self.platform / '.build').glob('combined-*')).joinpath('unexpected.mjs').write_text('unexpected output')
            return {'status': 'passed'}
        argv = ['runner', '--config', str(self.config), '--output', str(self.output), '--phase', 'finite',
                '--export-manifest', str(self.manifest), '--export-manifest-sha256', exports.sha256(self.manifest.read_bytes())]
        with patch.object(self.runner, 'REPO', self.root), patch.object(self.runner, 'PLATFORM', self.platform), \
             patch.object(self.runner, 'CI', self.ci), patch.object(self.runner, 'setup', side_effect=self.setup_standin), \
             patch.object(self.runner, 'bounded', side_effect=process_standin), \
             patch.object(self.runner, 'git', side_effect=AssertionError('No Git call permitted in export mode')), \
             patch.object(sys, 'argv', argv), patch('builtins.print'):
            return self.runner.main()

    def test_actual_main_uses_export_provenance_and_both_finite_discovery_paths(self):
        self.assertEqual(self.run_main(), 0)
        state = json.loads((self.output / 'status.json').read_text())
        self.assertNotIn('sourceCommit', state)
        self.assertEqual(state['sourceProvenance']['declared_origin_commit'], 'a' * 40)
        self.assertFalse(state['sourceProvenance']['origin_commit_ancestry_verified'])
        self.assertEqual([name for name, _ in self.commands], ['node', 'python'])
        self.assertIn('weebdex-platform/tests/ci/test_synthetic.mjs', self.commands[0][1])
        self.assertEqual((self.output / 'export-runtime-pins-before.json').read_bytes(),
                         (self.output / 'export-runtime-pins-after.json').read_bytes())
        self.assertEqual((self.output / 'execution-source-pins-before.json').read_bytes(),
                         (self.output / 'execution-source-pins-after.json').read_bytes())

    def test_actual_main_keeps_failed_source_drift_even_when_process_standins_report_success(self):
        with self.assertRaisesRegex(ValueError, 'Source bytes changed'):
            self.run_main(drift='source')
        self.assertEqual(json.loads((self.output / 'status.json').read_text())['status'], 'failed')

    def test_actual_main_keeps_failed_runtime_drift_even_when_process_standins_report_success(self):
        with self.assertRaisesRegex(ValueError, 'runtime files or dependency aliases changed'):
            self.run_main(drift='runtime')
        self.assertEqual(json.loads((self.output / 'status.json').read_text())['status'], 'failed')

    def test_actual_main_refuses_external_dependency_control_byte_or_metadata_drift(self):
        for kind, message in [('dependency-control', 'differs from exported canonical source'),
                              ('dependency-metadata', 'dependency control metadata changed')]:
            if kind != 'dependency-control':
                self.temporary.cleanup()
                self.setUp()
            with self.subTest(kind=kind), self.assertRaisesRegex(ValueError, message):
                self.run_main(drift=kind)
            self.assertEqual(json.loads((self.output / 'status.json').read_text())['status'], 'failed')
