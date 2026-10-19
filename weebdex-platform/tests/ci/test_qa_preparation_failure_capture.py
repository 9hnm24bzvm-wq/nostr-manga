"""Finite capture/workflow and actual Node sealer seams, existing public key only.

No build, browser/service, key generation, private key, decryption or upload.
"""
from contextlib import contextmanager
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import subprocess
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


prepare = load('qa_failure_capture_prepare', ROOT / 'scripts/ci/prepare-hosted-browser.py')
profile_fixture = load('qa_failure_capture_profile_fixture', ROOT / 'tests/ci/test_public_upload_hosted_profile.py')
CONTEXT = {'GITHUB_REPOSITORY': '9hnm24bzvm-wq/nostr-manga', 'GITHUB_RUN_ID': '123',
           'GITHUB_RUN_ATTEMPT': '2', 'GITHUB_SHA': 'a' * 40}
SNAPSHOT = {'synthetic/frozen-source': 'b' * 64}


def digest(value):
    return hashlib.sha256(value).hexdigest()


def read(path):
    return json.loads(path.read_bytes())


@contextmanager
def capture_fixture():
    with tempfile.TemporaryDirectory() as temporary, patch.dict(os.environ, CONTEXT):
        root = Path(temporary)
        stage = root / '.build/hosted-browser'
        stage.mkdir(parents=True)
        public = root / 'ci-evidence'
        public.mkdir()
        with patch.multiple(prepare, ROOT=root, STAGE=stage, PUBLIC=public):
            capture = prepare.begin_qa_failure_capture(SNAPSHOT)
            value = {'schema': 1, 'status': 'preparation_failed', 'browser_phases_started': False,
                     'stage': 'public-upload-qa-build', 'source_files_checked': 676,
                     'frontend_preserved': True, 'artifacts_uploaded': False, 'error_class': 'RuntimeError'}
            prepare.write(public / 'preparation.json', value)
            yield root, stage, public, capture, value


class QaPreparationFailureCapture(unittest.TestCase):
    @contextmanager
    def seal_fixture(self, entered=False, missing_receipt=False):
        with tempfile.TemporaryDirectory() as temporary, patch.dict(os.environ, CONTEXT):
            repo = Path(temporary) / 'synthetic-repository'
            root = repo / 'weebdex-platform'
            stage = root / '.build/hosted-browser'
            stage.mkdir(parents=True)
            public = root / 'ci-evidence'
            public.mkdir()
            (repo / '.ci').mkdir()
            (repo / '.ci/qa-preparation-recipient-public.pem').write_bytes((ROOT.parent / '.ci/qa-preparation-recipient-public.pem').read_bytes())
            value = {'schema': 1, 'status': 'preparation_failed', 'browser_phases_started': False,
                     'stage': 'public-upload-qa-build' if entered else 'public-upload-profile',
                     'source_files_checked': 676, 'frontend_preserved': True,
                     'artifacts_uploaded': False, 'error_class': 'RuntimeError'}
            with patch.multiple(prepare, ROOT=root, STAGE=stage, PUBLIC=public):
                capture = prepare.begin_qa_failure_capture(SNAPSHOT)
                prepare.write(public / 'preparation.json', value)
                if entered:
                    def owned(label, argv, cwd, seconds):
                        self.assertTrue(read(capture['directory'] / 'boundaries.json')['children'][label]['entered'])
                        (stage / (label + '.log')).write_bytes(b'finite nonzero compilation output; no environment or dependencies')
                        # Explicit terminal stand-in with the exact current
                        # run-bounded.py field order and Python float format.
                        receipt = {'command': argv, 'cwd': str(cwd), 'deadline_seconds': float(seconds),
                                   'elapsed_seconds': 0.001, 'process_exit_code': 2, 'deadline_exceeded': False,
                                   'terminal_receipt': True, 'owned_process_group': 1234, 'cleanup_signals': [],
                                   'surviving_owned_process_ids': [], 'cleanup_verified': True,
                                   'prior_or_unrelated_processes_touched': False}
                        if not missing_receipt:
                            prepare.write(stage / (label + '-owned.json'), receipt)
                        raise RuntimeError('finite QA compilation refusal')
                    try:
                        with patch.object(prepare, 'run', owned):
                            prepare.run_qa_failure_child(capture, prepare.QA_FAILURE_CHILDREN[0],
                                                         ['node', str(root / 'scripts/build-frontend.mjs')], root, 900)
                    except RuntimeError as error:
                        prepare.capture_qa_failure(capture, value, error)
                else:
                    prepare.capture_qa_failure(capture, value, RuntimeError('finite QA pre-child refusal'))
                canonical = root / 'ci-sanitized-evidence'
                self.assertEqual(profile_fixture.validator.validate(public, canonical), 1)
                yield root, digest((canonical / 'preparation.json').read_bytes())

    def node_seal(self, root, expected, refused=False):
        driver = """
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';
const api = await import(pathToFileURL(process.argv[2]));
const options = {root:process.argv[3],env:process.env,expectedPreparationSha256:process.argv[4]};
const output = path.join(options.root,'ci-encrypted-evidence/qa-preparation-failure.json');
if (process.argv[5] === 'refuse') {
  assert.throws(() => api.sealRecordedQaFailure(options), api.SealedLogRefused);
  assert.equal(fs.existsSync(output), false);
  console.log(JSON.stringify({refused:true,artifact_created:false}));
} else {
  const result = api.sealRecordedQaFailure(options);
  assert.equal(api.validateRecordedQaEnvelope({...options,expectedSealedSha256:result.sha256}),true);
  assert.throws(() => api.validateRecordedQaEnvelope({...options,expectedSealedSha256:'f'.repeat(64)}),api.SealedLogRefused);
  assert.ok(result.bytes <= api.MAX_ARTIFACT);
  console.log(JSON.stringify({...result,validated:true,wrong_marker_refused:true}));
}
"""
        result = subprocess.run(['node', '--input-type=module', '-e', driver, 'qa-capture-seam',
                                 str(ROOT / 'scripts/ci/seal-qa-preparation.mjs'), str(root), expected,
                                 'refuse' if refused else 'seal'], capture_output=True, text=True,
                                env=dict(CONTEXT, PATH=os.environ.get('PATH', '')), timeout=15)
        self.assertEqual(result.returncode, 0, result.stderr)
        return json.loads(result.stdout)

    def test_actual_python_prechild_capture_reaches_node_sealer_and_fresh_envelope_validation(self):
        with self.seal_fixture() as (root, expected):
            result = self.node_seal(root, expected)
            output = root / 'ci-encrypted-evidence/qa-preparation-failure.json'
            self.assertEqual(result['sha256'], digest(output.read_bytes()))
            self.assertEqual(result['bytes'], output.stat().st_size)
            self.assertTrue(result['validated'] and result['wrong_marker_refused'])
            self.assertEqual(output.stat().st_mode & 0o777, 0o600)
            self.assertNotIn(b'finite QA pre-child refusal', output.read_bytes())

    def test_actual_python_entered_child_accepts_known_stopped_nonzero_but_refuses_missing_terminal(self):
        with self.seal_fixture(entered=True) as (root, expected):
            self.assertTrue(self.node_seal(root, expected)['validated'])
            self.assertNotIn(b'finite nonzero compilation output', (root / 'ci-encrypted-evidence/qa-preparation-failure.json').read_bytes())
        with self.seal_fixture(entered=True, missing_receipt=True) as (root, expected):
            self.assertEqual(self.node_seal(root, expected, refused=True), {'refused': True, 'artifact_created': False})

    def test_fixed_stages_match_the_actual_profile_and_existing_import_boundary(self):
        self.assertEqual(prepare.QA_FAILURE_STAGES, ('public-upload-profile', *profile_fixture.builder.PUBLIC_STAGES))
        self.assertEqual(prepare.QA_FAILURE_CHILDREN, ('public-upload-frontend-build', 'public-upload-artifact-record'))

    def test_fresh_boundary_is_canonical_private_and_has_no_child_entered(self):
        with capture_fixture() as (_, _, _, capture, _):
            path = capture['directory'] / 'boundaries.json'
            value = read(path)
            self.assertEqual(value, {'schema': 1, 'children': {label: {'entered': False} for label in prepare.QA_FAILURE_CHILDREN}})
            self.assertEqual(path.read_bytes(), prepare.qa_encoded(value))
            self.assertEqual(path.stat().st_mode & 0o777, 0o600)
            self.assertEqual(capture['directory'].stat().st_mode & 0o777, 0o700)
            self.assertEqual({path.name for path in capture['directory'].iterdir()}, {'boundaries.json'})
            with self.assertRaises(FileExistsError):
                prepare.begin_qa_failure_capture(SNAPSHOT)

    def test_boundary_is_on_disk_before_each_exact_child_even_when_it_raises(self):
        with capture_fixture() as (_, _, _, capture, _):
            calls = []
            primary = RuntimeError('synthetic owned failure')
            def owned(label, argv, cwd, seconds):
                value = read(capture['directory'] / 'boundaries.json')
                self.assertTrue(value['children'][label]['entered'])
                calls.append((label, argv, cwd, seconds))
                if len(calls) == 2:
                    raise primary
                return {'synthetic': True}
            with patch.object(prepare, 'run', owned):
                self.assertEqual(prepare.run_qa_failure_child(capture, prepare.QA_FAILURE_CHILDREN[0], ['finite'], ROOT, 1), {'synthetic': True})
                with self.assertRaises(RuntimeError) as caught:
                    prepare.run_qa_failure_child(capture, prepare.QA_FAILURE_CHILDREN[1], ['finite'], ROOT, 1)
            self.assertIs(caught.exception, primary)
            self.assertEqual(len(calls), 2)
            self.assertTrue(all(row['entered'] for row in read(capture['directory'] / 'boundaries.json')['children'].values()))

    def test_unknown_repeated_or_out_of_order_child_never_calls_the_runner(self):
        for kind in ['unknown', 'out-of-order', 'repeated']:
            with self.subTest(kind=kind), capture_fixture() as (_, _, _, capture, _), patch.object(prepare, 'run') as owned:
                label = 'private-selector' if kind == 'unknown' else prepare.QA_FAILURE_CHILDREN[1] if kind == 'out-of-order' else prepare.QA_FAILURE_CHILDREN[0]
                if kind == 'repeated':
                    prepare.run_qa_failure_child(capture, label, [], ROOT, 1)
                    owned.reset_mock()
                with self.assertRaises(RuntimeError):
                    prepare.run_qa_failure_child(capture, label, [], ROOT, 1)
                owned.assert_not_called()

    def test_failure_capture_binds_current_public_bytes_context_boundaries_and_no_locals(self):
        with capture_fixture() as (_, _, public, capture, value), patch.dict(os.environ, {'PRIVATE_ENV_SENTINEL': 'never-copy-environment'}):
            def refuse():
                local_secret = 'never-copy-this-local-value'
                raise RuntimeError('synthetic exception message allowed only inside encryption')
            try:
                refuse()
            except RuntimeError as error:
                prepare.capture_qa_failure(capture, value, error)
            encoded = (capture['directory'] / 'capture.json').read_bytes()
            receipt = json.loads(encoded)
            self.assertEqual(encoded, prepare.qa_encoded(receipt))
            self.assertEqual(set(receipt), {'schema', 'status', 'stage', 'repository', 'run_id', 'run_attempt', 'source_commit',
                                          'preparation_sha256', 'source_snapshot_sha256', 'boundaries_sha256', 'traceback'})
            self.assertEqual(receipt['preparation_sha256'], digest((public / 'preparation.json').read_bytes()))
            self.assertEqual(receipt['boundaries_sha256'], digest((capture['directory'] / 'boundaries.json').read_bytes()))
            self.assertEqual(receipt['source_snapshot_sha256'], digest(json.dumps(SNAPSHOT, sort_keys=True).encode()))
            self.assertEqual([receipt[key] for key in ['repository', 'run_id', 'run_attempt', 'source_commit']], list(CONTEXT.values()))
            trace = (capture['directory'] / 'traceback.txt').read_bytes()
            self.assertIn(b'synthetic exception message allowed only inside encryption', trace)
            self.assertNotIn(b'never-copy-this-local-value', trace)
            self.assertNotIn(b'never-copy-environment', trace)
            self.assertNotIn(b'synthetic exception message', (public / 'preparation.json').read_bytes())
            self.assertEqual(receipt['traceback'], {'total_bytes': len(trace), 'retained_bytes': len(trace), 'truncated': False, 'sha256': digest(trace)})
            self.assertEqual({path.name for path in capture['directory'].iterdir()}, {'capture.json', 'boundaries.json', 'traceback.txt'})
            self.assertTrue(all(path.stat().st_mode & 0o777 == 0o600 for path in capture['directory'].iterdir()))

    def test_large_traceback_is_an_explicitly_truncated_byte_tail_under_the_fixed_cap(self):
        with capture_fixture() as (_, _, _, capture, value):
            error = RuntimeError('finite unicode \u2603' * 10000)
            prepare.capture_qa_failure(capture, value, error)
            receipt = read(capture['directory'] / 'capture.json')['traceback']
            trace = (capture['directory'] / 'traceback.txt').read_bytes()
            self.assertEqual(len(trace), 64 * 1024)
            self.assertTrue(receipt['truncated'])
            self.assertGreater(receipt['total_bytes'], receipt['retained_bytes'])
            self.assertEqual(receipt['retained_bytes'], len(trace))
            self.assertEqual(receipt['sha256'], digest(trace))

    def test_unknown_stage_or_success_cannot_produce_a_capture(self):
        for change in [{'status': 'prepared'}, {'stage': 'frontend-build'}, {'stage': 'public-upload-private-value'}]:
            with self.subTest(change=change), capture_fixture() as (_, _, _, capture, value):
                with self.assertRaises(RuntimeError):
                    prepare.capture_qa_failure(capture, dict(value, **change), RuntimeError('finite'))
                self.assertFalse((capture['directory'] / 'capture.json').exists())

    def test_fresh_context_is_fixed_at_initialization_and_bad_context_is_refused(self):
        with capture_fixture() as (_, _, _, capture, value), patch.dict(os.environ, {'GITHUB_RUN_ID': '999'}):
            prepare.capture_qa_failure(capture, value, RuntimeError('finite'))
            self.assertEqual(read(capture['directory'] / 'capture.json')['run_id'], '123')
        for key, value in [('GITHUB_REPOSITORY', 'foreign/repository'), ('GITHUB_RUN_ID', '0'),
                           ('GITHUB_RUN_ATTEMPT', '../file'), ('GITHUB_SHA', 'A' * 40)]:
            with self.subTest(key=key), capture_fixture() as (_, stage, _, _, _), patch.dict(os.environ, {key: value}):
                with self.assertRaises(RuntimeError):
                    prepare.begin_qa_failure_capture(SNAPSHOT)
                self.assertEqual(len(list(stage.iterdir())), 1)

    def test_existing_links_and_pending_writes_are_never_followed_or_reused(self):
        with capture_fixture() as (root, _, _, capture, value):
            target = root / 'unrelated-private-file'
            target.write_bytes(b'untouched')
            (capture['directory'] / 'traceback.txt').symlink_to(target)
            with self.assertRaises(RuntimeError):
                prepare.capture_qa_failure(capture, value, RuntimeError('finite'))
            self.assertEqual(target.read_bytes(), b'untouched')
            self.assertFalse((capture['directory'] / 'capture.json').exists())
            pending = capture['directory'] / 'boundaries.json.pending'
            pending.write_bytes(b'uncertain previous write')
            with patch.object(prepare, 'run') as owned, self.assertRaises(FileExistsError):
                prepare.run_qa_failure_child(capture, prepare.QA_FAILURE_CHILDREN[0], [], root, 1)
            owned.assert_not_called()

    def main_fixture(self, root, kind):
        stage, evidence, _, _, _, profile_run = profile_fixture.PublicUploadHostedProfile().profile_fixture(root)
        (root / 'frontend').mkdir()
        native_module = root / 'synthetic-native-artifacts.py'
        native_module.write_text('def artifacts(*args):\n    return {"synthetic": True}\n')
        original_spec = importlib.util.spec_from_file_location
        def module_spec(name, location, *args, **kwargs):
            if Path(location).name == 'native-title-diagnostics.py':
                location = native_module
            elif Path(location).name == 'hosted-public-upload-profile.py':
                if kind == 'import':
                    raise RuntimeError('synthetic profile import refusal')
                location = ROOT / 'scripts/ci/hosted-public-upload-profile.py'
            return original_spec(name, location, *args, **kwargs)
        def owned(label, argv, cwd, seconds):
            if label == 'native-title-test-build':
                (stage / (label + '.log')).write_text('finite artifact stand-in')
            if label in prepare.QA_FAILURE_CHILDREN:
                boundaries = read(stage / 'qa-preparation-failure/boundaries.json')
                self.assertTrue(boundaries['children'][label]['entered'])
                (stage / (label + '.log')).write_bytes(b'finite owned QA output')
                prepare.write(stage / (label + '-owned.json'), {'synthetic_terminal_standin': True})
                if kind == 'compile' and label == prepare.QA_FAILURE_CHILDREN[0] or kind == 'record' and label == prepare.QA_FAILURE_CHILDREN[1]:
                    raise RuntimeError('synthetic owned refusal')
                receipt = profile_run(label, argv, cwd, seconds)
                if kind == 'graph' and label == prepare.QA_FAILURE_CHILDREN[0]:
                    (root / 'artifacts/frontend' / profile_fixture.qa_fixture.qa.GRAPH).unlink()
                return receipt
            return {'process_exit_code': 0, 'surviving_owned_process_ids': []}
        return stage, module_spec, owned

    def test_actual_main_captures_only_profile_import_compile_graph_and_record_failure(self):
        expected = {'import': ('public-upload-profile', [False, False]), 'compile': ('public-upload-qa-build', [True, False]),
                    'graph': ('public-upload-qa-graph', [True, False]), 'record': ('public-upload-qa-record', [True, True])}
        for kind, (failed_stage, entered) in expected.items():
            with self.subTest(kind=kind), tempfile.TemporaryDirectory() as temporary, patch.dict(os.environ, dict(CONTEXT, SELECTED_BATCH='public-upload')):
                root = Path(temporary)
                stage, module_spec, owned = self.main_fixture(root, kind)
                public = root / 'ci-evidence'
                with patch.multiple(prepare, ROOT=root, REPO=root, STAGE=stage, PUBLIC=public), \
                        patch.object(prepare, 'verify_frontend'), patch.object(prepare, 'source_snapshot', return_value=SNAPSHOT), \
                        patch.object(prepare, 'qa_build_contract', return_value=profile_fixture.qa_fixture.qa), \
                        patch.object(prepare, 'run', owned), patch.object(importlib.util, 'spec_from_file_location', module_spec):
                    self.assertEqual(prepare.main(), 1)
                receipt = read(stage / 'qa-preparation-failure/capture.json')
                self.assertEqual(receipt['stage'], failed_stage)
                self.assertEqual(read(public / 'preparation.json')['stage'], failed_stage)
                self.assertEqual([read(stage / 'qa-preparation-failure/boundaries.json')['children'][label]['entered'] for label in prepare.QA_FAILURE_CHILDREN], entered)

    def test_actual_main_never_captures_native_failure_or_failure_after_profile_success(self):
        for kind in ['native', 'after-profile', 'non-qa-batch']:
            with self.subTest(kind=kind), tempfile.TemporaryDirectory() as temporary, patch.dict(os.environ, dict(CONTEXT, SELECTED_BATCH='startup' if kind == 'non-qa-batch' else 'public-upload')):
                root = Path(temporary)
                stage, module_spec, owned = self.main_fixture(root, kind)
                public = root / 'ci-evidence'
                refusal = RuntimeError('synthetic non-profile refusal')
                with patch.multiple(prepare, ROOT=root, REPO=root, STAGE=stage, PUBLIC=public), \
                        patch.object(prepare, 'verify_frontend', side_effect=[refusal] if kind == 'native' else [None, refusal]), \
                        patch.object(prepare, 'source_snapshot', return_value=SNAPSHOT), \
                        patch.object(prepare, 'qa_build_contract', return_value=profile_fixture.qa_fixture.qa), \
                        patch.object(prepare, 'run', owned), patch.object(importlib.util, 'spec_from_file_location', module_spec):
                    self.assertEqual(prepare.main(), 1)
                self.assertFalse((stage / 'qa-preparation-failure/capture.json').exists())

    def test_capture_write_failure_cannot_replace_original_public_error_or_success_status(self):
        with tempfile.TemporaryDirectory() as temporary, patch.dict(os.environ, dict(CONTEXT, SELECTED_BATCH='public-upload')):
            root = Path(temporary)
            stage, module_spec, owned = self.main_fixture(root, 'compile')
            public = root / 'ci-evidence'
            with patch.multiple(prepare, ROOT=root, REPO=root, STAGE=stage, PUBLIC=public), \
                    patch.object(prepare, 'verify_frontend'), patch.object(prepare, 'source_snapshot', return_value=SNAPSHOT), \
                    patch.object(prepare, 'qa_build_contract', return_value=profile_fixture.qa_fixture.qa), \
                    patch.object(prepare, 'run', owned), patch.object(importlib.util, 'spec_from_file_location', module_spec), \
                    patch.object(prepare, 'capture_qa_failure', side_effect=OSError('secondary capture failure')):
                self.assertEqual(prepare.main(), 1)
            value = read(public / 'preparation.json')
            self.assertEqual((value['status'], value['stage'], value['error_class']), ('preparation_failed', 'public-upload-qa-build', 'RuntimeError'))
            self.assertFalse((stage / 'qa-preparation-failure/capture.json').exists())

    def test_workflow_requires_failed_preparation_validated_public_hash_and_fresh_sealed_validation(self):
        workflow = (ROOT.parent / '.github/workflows/hosted-browser-e2e.yml').read_text()
        seal = workflow.split('      - name: Seal current QA preparation failure diagnostics\n', 1)[1].split('      - name:', 1)[0]
        self.assertIn("steps.preparation.outcome == 'failure'", seal)
        self.assertIn("steps.evidence-validation.outcome == 'success'", seal)
        self.assertIn("hashFiles('weebdex-platform/.build/hosted-browser/qa-preparation-failure/capture.json') != ''", seal)
        self.assertIn('--expected-preparation-sha256 "$PREP_SHA"', seal)
        self.assertIn('--validate --expected-preparation-sha256 "$PREP_SHA" --expected-sealed-sha256 "$SEALED_SHA"', seal)
        self.assertLess(workflow.index('id: evidence-validation'), workflow.index('id: qa-preparation-seal'))
        self.assertIn('timeout-minutes: 140', workflow)
        blocks = re.findall(r'      - name: ([^\n]+)\n(.*?)(?=      - name:|\Z)', workflow, re.S)
        uploads = [(name, block) for name, block in blocks if 'uses: actions/upload-artifact@' in block]
        self.assertEqual(len(uploads), 2)
        encrypted = next(block for name, block in uploads if name == 'Upload only the validated encrypted QA preparation failure')
        self.assertIn("steps.qa-preparation-seal.outcome == 'success'", encrypted)
        self.assertIn("steps.qa-preparation-seal.outputs.sealed_sha256 != ''", encrypted)
        self.assertIn('path: weebdex-platform/ci-encrypted-evidence/qa-preparation-failure.json\n', encrypted)
        self.assertIn('if-no-files-found: error', encrypted)
        self.assertIn('retention-days: 1', encrypted)
        public = next(block for name, block in uploads if name == 'Upload allowlisted sanitized evidence only')
        self.assertEqual(re.findall(r'weebdex-platform/ci-sanitized-evidence/[a-z]+\.json', public),
                         ['weebdex-platform/ci-sanitized-evidence/preparation.json', 'weebdex-platform/ci-sanitized-evidence/summary.json', 'weebdex-platform/ci-sanitized-evidence/phases.json'])
        for _, block in uploads:
            for forbidden in ['.log', 'traceback', 'capture.json', 'boundaries', '.build/', 'public.pem']:
                self.assertNotIn(forbidden, block)


if __name__ == '__main__':
    unittest.main()
