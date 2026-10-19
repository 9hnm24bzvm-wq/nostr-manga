"""Finite build/receipt stand-ins. No browser, build, socket or database starts."""
import copy
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]


def load(name, relative):
    spec = importlib.util.spec_from_file_location(name, ROOT / relative)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


runner = load('upload_hosted_runner', 'scripts/ci/run-hosted-browser.py')
builder = load('upload_profile_builder', 'scripts/ci/hosted-public-upload-profile.py')
diagnostics = load('upload_public_diagnostics', 'scripts/ci/public-upload-diagnostics.py')
validator = load('upload_hosted_validator', 'scripts/ci/validate-ci-evidence.py')


def digest(data):
    return hashlib.sha256(data).hexdigest()


def accepted_report():
    session, file, other = 'synthetic-session', 'synthetic-file', 'synthetic-neighbour'
    old, new, neighbour, event = 'a' * 64, 'b' * 64, 'c' * 64, 'd' * 64
    rows = [{'method': 'POST', 'path': '/api/v1/upload/synthetic', 'status': 200, 'native': 'finished', 'event_id': f'{index:064x}'} for index in range(21)]
    rows[0].update(path=f'/api/v1/nostr/uploads/{session}/files/{file}/replace')
    previews = []
    for index, (identity, file_hash) in enumerate([(other, neighbour), (file, new)], 21):
        event_id = f'{index:064x}'
        rows.append({'method': 'GET', 'path': f'/api/v1/nostr/uploads/{session}/files/{identity}/preview', 'status': 200, 'native': 'finished', 'event_id': event_id})
        previews.append({'file_id': identity, 'file_hash': file_hash, 'event_ids': [event_id]})
    report = {'phase': runner.PUBLIC_UPLOAD_PHASE, 'passed': True, 'completed_utc': 'synthetic-finalized', 'steps': [{'name': 'synthetic', 'status': 'passed'} for _ in range(14)], 'page_errors': [],
              'proxy_lifecycle': {'connect_denials': 0, 'expected_connect_resets': 0, 'remaining_socket_count': 0, 'observations_truncated': False, 'shutdown_started': True, 'closed': True},
              'cleanup': {'browser_created': True, 'context_created': True, 'page_created': True}, 'runner_lifecycle': {'stage': 'complete', 'fatal_kind': 'none', 'fatal_code': 'none', 'fatal_stage': 'none', 'caught_stage': 'none', 'caught_category': 'none', 'exit_observed': True, 'exit_code': 0}}
    report['steps'][12] = {'name': 'hard reload, real key reconnect and durable signed private previews', 'status': 'passed', 'observation': {'hard_reload': True, 'explicit_key_reimport': True, 'local_File_thumbnails_absent': True, 'signed_previews': previews}}
    report['public_upload'] = {'scope': 'reconstructed-public-upload-replacement-v1', 'supported_replacement': True, 'completed': True, 'actual_context_closed': True, 'fixture_control_calls': 14,
        'browser_http': {'closed': True, 'failures': [], 'rows': rows},
        'independent_http': {'closed': True, 'pending': 0, 'rows': [{'event_id': 'e' * 64, 'method': 'GET', 'complete': True, 'status': 200}]},
        'replacement': {'session_id': session, 'file_id': file, 'mutation_event_id': rows[0]['event_id'], 'version_before': 9, 'version_after': 10, 'old_sha256': old, 'new_sha256': new, 'stored_original_bytes': 100, 'stable_file_id': True, 'native_position_preserved': True, 'stored_order_preserved': True, 'neighbours_unchanged': True},
        'durable': [{'label': 'committed', 'fixture_only': True, 'scenario': 'public-upload-v1', 'session': {'state': 'committed'},
                     'publication': {'event_id': event, 'original_event': {'id': event, 'kind': 1001, 'verified': True}},
                     'chapter': {'event_id': event, 'pages': [{'file_hash': item, 'storage_sha256': item, 'storage_bytes': 100, 'fresh_original_read': True} for item in [neighbour, new]]}}]}
    return report


class PublicUploadHostedProfile(unittest.TestCase):
    def test_exact_scope_is_forty_current_phases_not_historical_equivalence(self):
        self.assertEqual(len(runner.ALL_PHASES), 40)
        self.assertEqual(len(runner.BATCHES), 12)
        self.assertEqual(runner.MAX_PHASE_RUNS, 52)
        self.assertEqual(runner.batch_phases('public-upload', runner.BATCHES['public-upload']), [runner.PUBLIC_UPLOAD_PHASE])
        self.assertEqual(runner.requested_phases({'public-upload': runner.BATCHES['public-upload']}), [runner.PUBLIC_UPLOAD_PHASE])
        self.assertEqual(runner.requested_phases({'startup': []}), ['startup'])
        self.assertEqual(runner.maximum(runner.PUBLIC_UPLOAD_PHASE), 600)
        self.assertEqual(runner.requested_phases(runner.BATCHES), runner.ALL_PHASES)
        workflow = (ROOT.parent / '.github/workflows/hosted-browser-e2e.yml').read_text()
        self.assertIn('actor-follow, public-upload]', workflow)
        self.assertEqual(workflow.count("SELECTED_BATCH: ${{ inputs.batch || 'public-upload' }}"), 2)

    def profile_fixture(self, root, fail_at=None):
        stage = root / '.build/hosted-browser'
        stage.mkdir(parents=True)
        evidence = root / 'test-evidence/frontend'
        evidence.mkdir(parents=True)
        ui = root / 'artifacts/frontend'
        ui.mkdir(parents=True)
        (ui / 'index.html').write_bytes(b'native artifact')
        saved = {name: ('native receipt ' + name).encode() for name in builder.NATIVE_EVIDENCE}
        for name, data in saved.items():
            (evidence / name).write_bytes(data)
        calls = []
        def run(label, _argv, _cwd, seconds):
            calls.append((label, seconds, os.environ.get('PUBLIC_MANGADEX_CATALOGUE')))
            if fail_at == label:
                raise RuntimeError('synthetic build failure')
            if label == 'public-upload-frontend-build':
                ui.mkdir()
                (ui / 'index.html').write_bytes(b'public artifact')
                (ui / 'app.js').write_bytes(b'explicit synthetic bundle')
                build = {'public_api_catalogue_enabled': True, 'public_signed_upload_enabled': True, 'spa_fallback_sha256': digest(b'public artifact')}
                (evidence / 'build-integration.json').write_text(json.dumps(build))
            else:
                files = [{'path': str(path.relative_to(ui)), 'bytes': path.stat().st_size, 'sha256': digest(path.read_bytes())} for path in sorted(ui.iterdir())]
                (evidence / 'artifact-manifest.json').write_text(json.dumps({'files': files}))
                for name in ['bootstrap-csp.json', 'execution-status.json', 'integration-source-manifest.json']:
                    (evidence / name).write_text('explicit public receipt')
            return {'process_exit_code': 0, 'surviving_owned_process_ids': []}
        return stage, evidence, ui, saved, calls, run

    def test_second_build_is_gate_on_and_native_generation_is_restored_exactly(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            stage, evidence, ui, saved, calls, run = self.profile_fixture(root)
            old = os.environ.get('PUBLIC_MANGADEX_CATALOGUE')
            profile = builder.prepare(root, stage, run, lambda: None)
            self.assertEqual(os.environ.get('PUBLIC_MANGADEX_CATALOGUE'), old)
            self.assertEqual((ui / 'index.html').read_bytes(), b'native artifact')
            self.assertEqual({name: (evidence / name).read_bytes() for name in saved}, saved)
            self.assertEqual(calls, [('public-upload-frontend-build', 900, '1'), ('public-upload-artifact-record', 60, '1')])
            previous = runner.STAGE
            try:
                runner.STAGE = stage
                selected = runner.public_upload_profile({'public_upload_recovery': profile, 'normal_binary': 'synthetic unchanged backend'})
                self.assertEqual(selected['ui_dir'], str(stage / 'public-upload-frontend'))
                self.assertEqual(selected['normal_binary'], 'synthetic unchanged backend')
                (stage / 'public-upload-frontend/app.js').write_bytes(b'changed')
                with self.assertRaises(RuntimeError):
                    runner.public_upload_profile({'public_upload_recovery': profile})
            finally:
                runner.STAGE = previous

    def test_build_and_record_failures_restore_native_artifact_and_receipts(self):
        for label in ['public-upload-frontend-build', 'public-upload-artifact-record']:
            with self.subTest(label=label), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                stage, evidence, ui, saved, _, run = self.profile_fixture(root, label)
                old = os.environ.get('PUBLIC_MANGADEX_CATALOGUE')
                with self.assertRaises(RuntimeError):
                    builder.prepare(root, stage, run, lambda: None)
                self.assertEqual(os.environ.get('PUBLIC_MANGADEX_CATALOGUE'), old)
                self.assertEqual((ui / 'index.html').read_bytes(), b'native artifact')
                self.assertEqual({name: (evidence / name).read_bytes() for name in saved}, saved)

    def test_missing_and_gate_off_profiles_cannot_allocate_a_browser_fixture(self):
        for plan in [{}, {'public_upload_recovery': {}}, {'public_upload_recovery': {'enabled': False}}]:
            with self.assertRaises(RuntimeError):
                runner.run_batch('public-upload', [runner.PUBLIC_UPLOAD_PHASE], plan, {}, 'synthetic')

    def test_profile_rejects_gate_off_receipt_changed_membership_and_unsafe_manifest(self):
        for kind in ['gate-off', 'extra-file', 'duplicate-member', 'traversal', 'linked-member']:
            with self.subTest(kind=kind), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                stage, _, _, _, _, run = self.profile_fixture(root)
                profile = builder.prepare(root, stage, run, lambda: None)
                if kind == 'gate-off':
                    path = Path(profile['build_receipt'])
                    value = json.loads(path.read_text())
                    value['public_signed_upload_enabled'] = False
                    path.write_text(json.dumps(value))
                    profile['build_receipt_sha256'] = digest(path.read_bytes())
                elif kind in ['duplicate-member', 'traversal']:
                    path = Path(profile['frontend_manifest'])
                    value = json.loads(path.read_text())
                    if kind == 'duplicate-member':
                        value['files'].append(copy.deepcopy(value['files'][0]))
                    else:
                        value['files'][0]['path'] = '../outside'
                    path.write_text(json.dumps(value))
                    profile['frontend_manifest_sha256'] = digest(path.read_bytes())
                elif kind == 'extra-file':
                    (Path(profile['ui_dir']) / 'unreviewed.js').write_text('synthetic')
                else:
                    path = Path(profile['ui_dir']) / 'app.js'
                    path.unlink()
                    path.symlink_to(Path(profile['ui_dir']) / 'index.html')
                previous = runner.STAGE
                try:
                    runner.STAGE = stage
                    with self.assertRaises(RuntimeError):
                        runner.public_upload_profile({'public_upload_recovery': profile})
                finally:
                    runner.STAGE = previous

    def test_previous_scope_or_one_profile_cannot_claim_current_full_suite(self):
        clean = {'passed': True, 'supervisor_terminal': True, 'supervisor_exit_zero': True,
                 'cleanup': {'verified_stopped': True, 'lifecycle_clean': True}}
        historical = {'phases': [{'phase': phase, 'status': 'passed'} for phase in runner.ALL_PHASES if phase != runner.PUBLIC_UPLOAD_PHASE], 'batches': [clean]}
        summary = runner.summarize(historical, 'all', runner.ALL_PHASES)
        self.assertFalse(summary['selected_scope_passed'])
        self.assertFalse(summary['full_suite_passed'])
        self.assertEqual(summary['pending_unique_phases'], [runner.PUBLIC_UPLOAD_PHASE])
        validator.summary(summary)
        isolated = {'phases': [{'phase': runner.PUBLIC_UPLOAD_PHASE, 'status': 'passed'}], 'batches': [clean]}
        summary = runner.summarize(isolated, 'public-upload', [runner.PUBLIC_UPLOAD_PHASE])
        self.assertTrue(summary['selected_scope_passed'])
        self.assertFalse(summary['full_suite_passed'])
        self.assertEqual(len(summary['pending_unique_phases']), 39)
        validator.summary(summary)

    def test_closed_projection_passes_only_complete_positive_outcome(self):
        acceptance = accepted_report()
        value = diagnostics.project(acceptance)
        diagnostics.validate(value)
        self.assertTrue(diagnostics.passed(value))
        owned = {'fixture_only': True, 'phase': runner.PUBLIC_UPLOAD_PHASE, 'verified_stopped': True, 'forced': False, 'returncode': 0}
        phase = runner.project_phase(runner.PUBLIC_UPLOAD_PHASE, 'public-upload', acceptance, owned)
        self.assertEqual(phase['status'], 'passed')
        validator.phases({'schema': 1, 'phases': [phase], 'batches': []})
        encoded = json.dumps(phase)
        for private in ['synthetic-session', 'synthetic-file', 'synthetic-neighbour', 'Nostr ', 'original_event', 'storage_sha256']:
            self.assertNotIn(private, encoded)

    def test_adverse_or_missing_receipts_never_promote_report_passed(self):
        changes = [
            lambda value: value.pop('public_upload'),
            lambda value: value['public_upload'].update(completed=False),
            lambda value: value['public_upload'].update(actual_context_closed=False),
            lambda value: value['public_upload']['replacement'].update(version_after=11),
            lambda value: value['public_upload']['replacement'].update(neighbours_unchanged=False),
            lambda value: value['public_upload']['browser_http']['rows'].pop(),
            lambda value: value['public_upload']['browser_http']['rows'][0].update(status=503),
            lambda value: value['public_upload']['browser_http']['rows'][0].update(native='failed'),
            lambda value: value['public_upload']['browser_http'].update(closed=False),
            lambda value: value['public_upload']['independent_http'].update(pending=1),
            lambda value: value['public_upload'].update(fixture_control_calls=29),
            lambda value: value['steps'][12]['observation'].update(hard_reload=False),
            lambda value: value['steps'][12]['observation']['signed_previews'][0].update(event_ids=['f' * 64]),
            lambda value: value['public_upload']['durable'][0]['publication']['original_event'].update(verified=False),
            lambda value: value['public_upload']['durable'][0]['chapter']['pages'].reverse(),
            lambda value: value['public_upload']['durable'][0]['chapter']['pages'][0].update(fresh_original_read=False),
        ]
        owned = {'fixture_only': True, 'phase': runner.PUBLIC_UPLOAD_PHASE, 'verified_stopped': True, 'forced': False, 'returncode': 0}
        for index, change in enumerate(changes):
            with self.subTest(index=index):
                acceptance = accepted_report()
                change(acceptance)
                value = diagnostics.project(acceptance)
                diagnostics.validate(value)
                self.assertFalse(diagnostics.passed(value))
                phase = runner.project_phase(runner.PUBLIC_UPLOAD_PHASE, 'public-upload', acceptance, owned)
                self.assertEqual(phase['status'], 'failed')
                validator.phases({'schema': 1, 'phases': [phase], 'batches': []})
                forged = copy.deepcopy(phase)
                forged['status'] = 'passed'
                with self.assertRaises(ValueError):
                    validator.phases({'schema': 1, 'phases': [forged], 'batches': []})

    def test_public_schema_rejects_private_and_inconsistent_fields(self):
        baseline = diagnostics.project(accepted_report())
        for key, value in [('private_key', 'a' * 64), ('mutations', 20), ('schema', True), ('fixture_controls', True), ('raw_messages_exported', True)]:
            with self.subTest(key=key), self.assertRaises(ValueError):
                diagnostics.validate(dict(baseline, **{key: value}))

    def test_operational_gate_requires_current_complete_early_response_receipt(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            path = root / 'test-evidence/security/live-native-http.json'
            path.parent.mkdir(parents=True)
            fixture = root / 'synthetic-fixture'
            plan = {'normal_sha256': 'a' * 64, 'frontend_index_sha256': 'b' * 64}
            transport = {'kind': 'same-connection-early-response', 'declared_body_bytes': 1048577, 'observed_status': 413, 'completed_body_send_bytes': 1048577, 'write_error_class': None, 'response_body_complete': True, 'deadline_expired': False, 'deadline_thread_settled': True}
            check = {'name': 'bounded oversized JSON', 'method': 'POST', 'path': '/api/manga', 'status': 413, 'expected_status': 413, 'request_body_bytes': 1048577, 'passed': True, 'transport': transport}
            receipt = {'run_directory': str(fixture), 'fixture_only': True, 'production': False, 'binary': {'sha256': plan['normal_sha256']}, 'frontend_index_sha256': plan['frontend_index_sha256'], 'checks': [check], 'passed': 1, 'failed': 0}
            previous = runner.ROOT
            try:
                runner.ROOT = root
                def observe(value):
                    path.write_text(json.dumps(value))
                    state = {'public_upload': {'enabled': True, 'fixture_only': True, 'scenario': 'public-upload-v1'}, 'operational_http': {'path': str(path), 'sha256': digest(path.read_bytes())}}
                    runner.verify_public_upload_ready(state, fixture, plan)
                observe(receipt)
                for change in [lambda value: value.update(run_directory='other'), lambda value: value['checks'][0]['transport'].update(response_body_complete=False), lambda value: value['checks'][0]['transport'].update(completed_body_send_bytes=2), lambda value: value['checks'][0]['transport'].update(deadline_thread_settled=False)]:
                    value = copy.deepcopy(receipt)
                    change(value)
                    with self.assertRaises(RuntimeError):
                        observe(value)
            finally:
                runner.ROOT = previous


if __name__ == '__main__':
    unittest.main()
