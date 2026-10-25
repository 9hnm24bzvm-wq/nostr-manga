"""Finite driver/manifest/order seams only. No PostgreSQL, app, sockets or keys."""
import argparse
import copy
import hashlib
import importlib.util
import io
import json
import os
from pathlib import Path
import tempfile
import subprocess
import sys
import threading
import unittest
from unittest.mock import Mock, patch

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / 'scripts/ci/qualify-full-application-restore.py'
spec = importlib.util.spec_from_file_location('restore_qualification', SCRIPT)
m = importlib.util.module_from_spec(spec)
spec.loader.exec_module(m)


class ManifestTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='synthetic-restore-seams-')
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.ui = self.root / 'ui'
        self.ui.mkdir()
        (self.ui / 'index.html').write_bytes(b'synthetic manifest seam, not an app')
        self.binary = self.root / 'binary'
        self.binary.write_bytes(b'not executable')
        self.frontend = self.root / 'frontend.json'
        self.release = self.root / 'release.json'
        self.args = argparse.Namespace(ui_dir=self.ui, release_manifest=self.release,
            frontend_artifact_manifest=self.frontend)
        self.frontend_value = {'files': [{'path': 'index.html', **m.file_row(self.ui / 'index.html')}],
            'file_count': 1, 'total_bytes': (self.ui / 'index.html').stat().st_size}
        self.frontend.write_text(json.dumps(self.frontend_value))
        self.release_value = {'format': 1, 'lockfiles': {name: m.sha(ROOT / name) for name in
            ('backend/Cargo.lock', 'frontend/yarn.lock')}, 'artifacts': [
                {'path': 'artifacts/backend/weebdex-platform', **m.file_row(self.binary)},
                {'path': 'artifacts/frontend/index.html', **m.file_row(self.ui / 'index.html')}]}
        self.release.write_text(json.dumps(self.release_value))
        self.pins = {'tools': {'platform': m.file_row(self.binary)}}

    def check(self):
        return m.validate_release(self.args, self.pins)

    def test_exact_manifest_binding(self):
        self.assertEqual(self.check(), m.sha(self.ui / 'index.html'))

    def test_changed_binary_refused(self):
        self.pins['tools']['platform']['sha256'] = '0' * 64
        with self.assertRaises(m.Refused):
            self.check()

    def test_changed_frontend_bytes_refused(self):
        (self.ui / 'index.html').write_bytes(b'changed')
        with self.assertRaises(m.Refused):
            self.check()

    def test_extra_frontend_member_refused(self):
        (self.ui / 'unrecorded.js').write_bytes(b'not in manifest')
        with self.assertRaises(m.Refused):
            self.check()

    def test_linked_frontend_refused(self):
        (self.ui / 'linked').symlink_to(self.binary)
        with self.assertRaises(m.Refused):
            self.check()

    def test_duplicate_release_artifact_refused(self):
        self.release_value['artifacts'].append(self.release_value['artifacts'][0])
        self.release.write_text(json.dumps(self.release_value))
        with self.assertRaises(m.Refused):
            self.check()

    def test_lockfile_mismatch_refused(self):
        self.release_value['lockfiles']['backend/Cargo.lock'] = '0' * 64
        self.release.write_text(json.dumps(self.release_value))
        with self.assertRaises(m.Refused):
            self.check()

    def test_traversal_and_duplicate_frontend_refused(self):
        for name in ('../binary', '/tmp/elsewhere', './index.html', 'index.html'):
            with self.subTest(name=name):
                value = copy.deepcopy(self.frontend_value)
                value['files'].append({'path': name, **m.file_row(self.binary)})
                self.frontend.write_text(json.dumps(value))
                with self.assertRaises(m.Refused):
                    self.check()


class DriverSeams(unittest.TestCase):
    def test_private_list_fixture_has_only_its_required_permission(self):
        drill = m.Drill.__new__(m.Drill)
        drill.token = 'synthetic-test-token-not-a-user-credential'
        drill.password_auth = Mock(return_value='11111111-1111-1111-1111-111111111111')
        drill.browser_flow = Mock(side_effect=[({'id':'22222222-2222-2222-2222-222222222222'}, 'weebdex_browser=synthetic'),
                                               ({'id':'33333333-3333-3333-3333-333333333333'}, 'weebdex_browser=synthetic')])
        drill.sql = Mock()
        drill.http = Mock(side_effect=[(201, b'{"id":"SyntheticList","visibility":"private"}', {}),
                                      (200, b'{}', {}), (200, b'{}', {}), (200, b'{}', {})])
        drill.seed_account()
        statement = drill.sql.call_args_list[0].args[0]
        self.assertIn('UPDATE platform.users SET verified=TRUE,permissions=', statement)
        self.assertIn("'[\"self.preferences.update\",\"self.library.read\"]'::jsonb", statement)
        self.assertNotIn(drill.token, statement)
        self.assertNotIn('INSERT INTO platform.sessions', '\n'.join(call.args[0] for call in drill.sql.call_args_list))
        drill.password_auth.assert_called_once_with('registration')
        drill.http.assert_any_call('/api/user/lists', auth=True,
            payload={'name': 'Synthetic private restore list', 'manga_ids': ['SeedManga1']})
        self.assertEqual(drill.list_snapshot['visibility'], 'private')

    def test_private_list_fixture_preserves_authorization_refusal(self):
        drill = m.Drill.__new__(m.Drill)
        drill.token = 'synthetic-test-token-not-a-user-credential'
        drill.password_auth = Mock(return_value='11111111-1111-1111-1111-111111111111')
        drill.sql = Mock()
        drill.http = Mock(return_value=(403, b'{}', {}))
        with self.assertRaises(m.Refused):
            drill.seed_account()

    def test_password_login_uses_new_flow_bound_csrf_and_real_session_cookie(self):
        drill = object.__new__(m.Drill)
        drill.password = 'synthetic-password-only'
        flow_id = '11111111-1111-1111-1111-111111111111'
        user_id = '22222222-2222-2222-2222-222222222222'
        drill.browser_flow = Mock(return_value=({'id':flow_id, 'ui':{'nodes':[
            {'attributes':{'name':'csrf_token','value':'synthetic-csrf'}}]}}, 'weebdex_browser=synthetic'))
        drill.http = Mock(return_value=(200, json.dumps({'session':{'active':True,'identity':{'id':user_id}}}).encode(),
            {'set-cookie':'weebdex_session=' + 's'*43 + '; Path=/; HttpOnly'}))
        self.assertEqual(drill.password_auth('login'), user_id)
        self.assertEqual(drill.token, 's'*43)
        drill.http.assert_called_once_with('/auth/self-service/login?flow=' + flow_id,
            headers={'Cookie':'weebdex_browser=synthetic'}, payload={'method':'password',
            'csrf_token':'synthetic-csrf','password':drill.password,'identifier':'restore-fixture-user'})
        drill.http.return_value = (401, b'{}', {})
        with self.assertRaises(m.Refused):
            drill.password_auth('login')

    def test_old_credentials_must_fail_before_fresh_login_can_be_credited(self):
        drill = object.__new__(m.Drill)
        drill.report = {}
        drill.backup_token = 'old-synthetic-cookie'
        drill.api_token = 'old-synthetic-api'
        drill.list_id = 'SyntheticList'
        drill.old_flows = [('recovery','synthetic-flow','weebdex_browser=synthetic','old-synthetic-email')]
        drill.user_id = 'synthetic-user'
        def login(kind):
            drill.token = 'fresh-synthetic-session'
            return drill.user_id
        drill.password_auth = Mock(side_effect=login)
        good = [(401,b'',{}),(404,b'',{}),(410,b'',{}),(400,b'',{}),(200,b'',{})]
        for index in range(4):
            responses = list(good); responses[index] = (200,b'',{})
            drill.http = Mock(side_effect=responses)
            with self.subTest(index=index), self.assertRaises(m.Refused):
                drill.restored_authentication()
            drill.password_auth.assert_not_called()
            self.assertNotIn('fresh_login_verified', drill.report)
        drill.http = Mock(side_effect=good)
        drill.restored_authentication()
        self.assertTrue(drill.report['fresh_login_verified'])
        drill.password_auth.assert_called_once_with('login')

    def test_business_projection_keeps_every_row_and_canonical_policy_column(self):
        self.assertEqual(m.row_projection('platform.users', True), 'to_jsonb(t)')
        self.assertEqual(m.row_projection('platform.protocol_publication_commands', True), 'to_jsonb(t)')
        self.assertEqual(m.row_projection('public.account_permissions', True), 'to_jsonb(t)')
        self.assertEqual(m.row_projection('public.backend_outbox', True), 'to_jsonb(t)')
        self.assertEqual(m.row_projection('platform.sessions', False), 'to_jsonb(t)')
        self.assertEqual(m.row_projection('platform.sessions', True), "to_jsonb(t) - ARRAY['revoked_at']::text[]")
        self.assertNotIn('service_key', m.row_projection('public.node_identity', True))
        self.assertNotIn('payload', m.row_projection('platform.scheduled_uploads', True))
        self.assertNotIn('started_at', m.row_projection('platform.scheduled_uploads', True))
        self.assertNotIn('all_table_rows_equal', m.REQUIRED)
        self.assertIn('all_sequences_equal', m.REQUIRED)

    def test_paused_restore_config_preserves_identity_and_input_config(self):
        config = {'protocol_publication': {'operator_key_file':'/unchanged/operator', 'native_node':{
            'service_key_file':'/unchanged/service', 'admin_pubkey':'synthetic-unchanged',
            'sync_worker_enabled':True, 'public_serving':{'enabled':True, 'root':'/retained'},
            'enrollment':{'enabled':True}, 'discovery_private_overrides':['http://private.invalid']}}}
        original = copy.deepcopy(config)
        result = m.paused_config(config)
        self.assertEqual(config, original)
        m.checkpoint.require_paused_roles(result)
        node = result['protocol_publication']['native_node']
        self.assertEqual(node['service_key_file'], '/unchanged/service')
        self.assertEqual(node['admin_pubkey'], 'synthetic-unchanged')
        self.assertEqual(node['public_serving']['root'], '/retained')

    def valid(self):
        return dict(format='synthetic-full-restore-result-v1', runtime_executed=True,
            stage='complete', error_class=None, budget_failure=None,
            native_pending_upload_qualified=False, production_dr_qualified=False,
            **m.SCOPE, **{name: True for name in m.REQUIRED})

    def test_strict_success_requires_every_terminal_observation(self):
        value = self.valid()
        self.assertTrue(m.clean_success(value))
        for key in m.REQUIRED:
            for replacement in (False, None, 1, 'true', []):
                with self.subTest(key=key, replacement=replacement):
                    self.assertFalse(m.clean_success(dict(value, **{key: replacement})))
            incomplete = dict(value)
            del incomplete[key]
            self.assertFalse(m.clean_success(incomplete))

    def test_harness_cannot_claim_runtime_or_production_success(self):
        baseline = self.valid()
        for mutation in ([{key: not value} for key, value in m.SCOPE.items()] +
                         [{'runtime_executed': False}, {'stage': 'restored-checks'},
                         {'error_class': 'RuntimeError'}, {'budget_failure': 'resource-or-time-bound'},
                         {'native_pending_upload_qualified': True}, {'production_dr_qualified': True}]):
            with self.subTest(mutation=mutation):
                self.assertFalse(m.clean_success(dict(baseline, **mutation)))

    def test_relocation_changes_only_three_roots_and_preserves_separate_keys(self):
        config = {'uploads_dir': '/old/native', 'ui_dir': '/ui', 'bind': '127.0.0.1:10001',
            'protocol_publication': {'operator_key_file': '/keys/operator', 'group_keys_dir': '/keys/groups', 'storage': {'path': '/old/original'},
                'native_node': {'storage': {'path': '/old/original'}, 'backend': {'data_dir': '/old/projection'},
                    'service_key_file': '/keys/service', 'group_keys_dir': '/keys/groups'}}}
        original = copy.deepcopy(config)
        roots = {name: Path('/new') / name for name in ('native', 'originals', 'projection')}
        value = m.relocated(config, roots)
        self.assertEqual(config, original)
        self.assertEqual(value['uploads_dir'], '/new/native')
        self.assertEqual(value['protocol_publication']['storage']['path'], '/new/originals')
        self.assertEqual(value['protocol_publication']['native_node']['storage']['path'], '/new/originals')
        self.assertEqual(value['protocol_publication']['native_node']['backend']['data_dir'], '/new/projection')
        self.assertEqual(value['protocol_publication']['operator_key_file'], '/keys/operator')
        self.assertEqual(value['protocol_publication']['native_node']['service_key_file'], '/keys/service')
        self.assertEqual(value['protocol_publication']['group_keys_dir'], '/keys/groups')
        self.assertEqual(value['protocol_publication']['native_node']['group_keys_dir'], '/keys/groups')

    def test_unstarted_watchdog_can_close_after_prestart_failure(self):
        owner = Mock()
        guard = m.ResourceGuard(owner, Path('/not-read'), 60)
        guard.close()
        owner.assert_not_called()

    def test_durable_pg_launch_reuses_existing_owner(self):
        owner = object.__new__(m.RestoreFixture)
        with patch.object(m.fixture_module.Fixture, 'spawn', return_value='owned') as base:
            self.assertEqual(owner.spawn('postgres', ['postgres', '-c', 'fsync=off', '-c', 'full_page_writes=off'], {}), 'owned')
            self.assertEqual(base.call_args.args[1], ['postgres', '-c', 'fsync=on', '-c', 'full_page_writes=on'])

    def test_checkpoint_runner_uses_fixed_tools_and_passes_failures(self):
        drill = object.__new__(m.Drill)
        drill.guard = Mock(end=m.time.monotonic() + 30)
        drill.fixture = Mock()
        drill.fixture.run_owned.return_value = argparse.Namespace(returncode=1, stdout='', stderr='PRIVATE_SENTINEL')
        budget = Mock(end=m.time.monotonic() + 30)
        with self.assertRaises(m.Refused) as caught:
            drill.pg_tool('pg_restore', ['--single-transaction'], {}, budget)
        self.assertNotIn('PRIVATE_SENTINEL', str(caught.exception))
        with self.assertRaises(m.Refused):
            drill.pg_tool('sh', [], {}, budget)
        self.assertEqual(drill.fixture.run_owned.call_count, 1)

    def test_source_inventory_includes_both_migrations_and_guarded_owners(self):
        names = {str(p.relative_to(m.REPO)) for p in m.source_paths()}
        for name in ('weebdex-platform/backend/migrations/0102_native_reading_status_revisions.sql',
                     'manga-protocol-lab/server/migrations/0052_reader_enrollment.sql',
                     'weebdex-platform/tests/runtime/browser_fixture.py',
                     'weebdex-platform/scripts/private-checkpoint.py',
                     'weebdex-platform/backend/examples/browser_fixture_provisioner.rs',
                     'weebdex-platform/platform-integrations/integration-utility-sources.mjs',
                     'weebdex-platform/scripts/build-frontend.mjs',
                     'weebdex-platform/scripts/prepare-frontend-integration.mjs',
                     'weebdex-platform/scripts/vite.platform.config.mjs',
                     'weebdex-platform/frontend/svelte.config.js'):
            self.assertIn(name, names)

    def test_changed_composed_overlay_or_build_input_invalidates_preflight(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            paths = [root / 'overlay.mjs', root / 'build-frontend.mjs']
            for path in paths:
                path.write_text('reviewed composed input')
            def current(_):
                return {'source': {p.name: m.file_row(p) for p in paths}}
            pins = root / 'pins.json'
            pins.write_text(json.dumps(current(None)))
            args = argparse.Namespace(seconds=600, pins=pins, pins_sha256=m.sha(pins))
            for path in paths:
                original = path.read_text()
                path.write_text('changed after source pin')
                with self.subTest(path=path.name), patch.object(m, 'inventory', side_effect=current), self.assertRaises(m.Refused):
                    m.preflight(args)
                path.write_text(original)

    def test_restore_start_verifies_never_migrates_and_requires_actual_readiness(self):
        drill = object.__new__(m.Drill)
        drill.args = argparse.Namespace(binary=Path('/synthetic/platform'), pins=Path('/synthetic/pins'))
        drill.restore_config = Path('/private/restored.json')
        drill.check_retained_credentials = Mock()
        drill.fixture = Mock(pg_port=12345, generation=0, ui_dir=Path('/ui'))
        child = Mock()
        child.poll.return_value = None
        drill.fixture.children = {'platform': child}
        drill.fixture.environment.return_value = {}
        drill.fixture.run_owned.return_value = argparse.Namespace(returncode=0)
        drill.guard = Mock(end=m.time.monotonic() + 3)
        drill.http = Mock(return_value=(200, b'', {}))
        drill.pending_state = Mock(return_value=['1', '1', '0', 'digest'])
        with patch.object(m, 'inventory', return_value={}), patch.object(m, 'read_json', return_value={}):
            drill.start_restored()
        self.assertEqual([call.args[0][1] for call in drill.fixture.run_owned.call_args_list], ['check-config', 'verify'])
        self.assertEqual(drill.fixture.spawn.call_args.args[1][-1], 'serve')
        self.assertEqual(drill.fixture.platform_env['PLATFORM_DATABASE_URL'], 'host=127.0.0.1 port=12345 dbname=weebdex_restore_test')
        self.assertEqual(drill.pending_state.call_args.args, (m.TARGET_DB,))

    def test_failed_matching_verify_never_starts_listener(self):
        drill = object.__new__(m.Drill)
        drill.args = argparse.Namespace(binary=Path('/synthetic/platform'), pins=Path('/synthetic/pins'))
        drill.restore_config = Path('/private/restored.json')
        drill.check_retained_credentials = Mock()
        drill.fixture = Mock(pg_port=12345)
        drill.fixture.environment.return_value = {}
        drill.fixture.run_owned.return_value = argparse.Namespace(returncode=1)
        with patch.object(m, 'inventory', return_value={}), patch.object(m, 'read_json', return_value={}):
            with self.assertRaises(m.Refused):
                drill.start_restored()
        drill.fixture.spawn.assert_not_called()

    def test_private_stage_note_uses_real_account_api_without_outbox_sql_mutation(self):
        source = (ROOT / 'backend/examples/browser_fixture_provisioner.rs').read_text()
        body = source[source.index('fn restore_qualification_enabled'):source.index('async fn source_revoke')]
        for required in ('full_restore_qualification', 'listeners_offline(root)', 'repo.verify_schema()',
                         'repo.set_account_admission', 'repo.account_permit', 'repo.stage_personal_note',
                         'pending == 1', 'native_upload_intent_covered'):
            self.assertIn(required, body)
        self.assertNotIn('INSERT INTO backend_outbox', body)
        self.assertNotIn('UPDATE backend_outbox', body)

    def test_no_browser_or_install_and_required_order_is_explicit(self):
        source = SCRIPT.read_text()
        body = source[source.index('    def run(self):'):source.index('\ndef main():')]
        stages = ['baseline', 'account', 'quiesce', 'pending-outbox', 'handoff', 'backup-plan', 'backup',
                  'verify-bundle', 'restore-plan', 'nonempty-refusal', 'restore', 'compare']
        offsets = [body.index("self.stage('" + stage + "')") for stage in stages]
        self.assertEqual(offsets, sorted(offsets))
        self.assertIn("for label in ('restored', 'restart')", body)
        self.assertIn("str(error) == 'Target name mismatch or nonempty application database'", body)
        for forbidden in ('browser_commands(', 'operational_checks(', 'cargo ', 'npm ', 'pip ', 'curl '):
            self.assertNotIn(forbidden, body)


class CredentialSeams(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='synthetic-credential-seam-')
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.groups = self.root / 'original-group-keys'
        self.groups.mkdir(mode=0o700)
        self.keys = self.root / 'fixture-keys'
        self.keys.mkdir(mode=0o700)
        self.operator = self.keys / 'operator.key'
        self.service = self.keys / 'service.key'
        for path in (self.operator, self.service):
            path.write_bytes(b'not-a-real-key: synthetic ' + path.name.encode())
            path.chmod(0o600)
        self.config = {'protocol_publication': {'operator_key_file': str(self.operator),
            'group_keys_dir': str(self.groups), 'native_node': {
                'service_key_file': str(self.service), 'group_keys_dir': str(self.groups)}}}
        self.drill = object.__new__(m.Drill)
        self.drill.report = {}
        self.drill.fixture = Mock(run=self.root)
        self.drill.credential_state = m.retained_credentials(self.config, self.root)
        self.drill.restore_config = self.root / 'config.json'
        self.drill.restore_config.write_text(json.dumps(self.config))
        self.drill.args = argparse.Namespace(pins=self.root / 'pins', binary=Path('/not-executed'))
        self.drill.args.pins.write_text('{}')

    def refused_before_any_child(self):
        with patch.object(m, 'inventory', return_value={}), self.assertRaises(m.Refused) as caught:
            self.drill.start_restored()
        self.drill.fixture.run_owned.assert_not_called()
        self.drill.fixture.spawn.assert_not_called()
        self.assertNotIn('not-a-real-key', str(caught.exception))
        self.assertNotIn('retained_credentials_match', self.drill.report)

    def test_matching_retained_credentials_report_only_boolean(self):
        self.drill.check_retained_credentials()
        self.assertEqual(self.drill.report, {'retained_credentials_match': True})

    def test_missing_service_key_refused_before_verify_or_serve(self):
        self.service.unlink()
        self.refused_before_any_child()
        self.assertFalse(self.service.exists())

    def test_replaced_service_key_refused_before_verify_or_serve(self):
        self.service.write_bytes(b'not-a-real-key: changed service bytes')
        self.refused_before_any_child()

    def test_missing_operator_key_refused_before_verify_or_serve(self):
        self.operator.unlink()
        self.refused_before_any_child()

    def test_replaced_operator_key_refused_before_verify_or_serve(self):
        self.operator.write_bytes(b'not-a-real-key: changed operator bytes')
        self.refused_before_any_child()

    def test_missing_group_directory_refused_before_verify_or_serve(self):
        self.groups.rmdir()
        self.refused_before_any_child()

    def test_group_key_addition_is_not_silently_claimed_or_read(self):
        (self.groups / 'placeholder.key').write_bytes(b'not-a-real-key')
        with patch.object(m, 'file_row') as read_key:
            self.refused_before_any_child()
        read_key.assert_not_called()

    def test_retargeted_key_path_refused_without_reading_alternate(self):
        self.config['protocol_publication']['native_node']['service_key_file'] = '/not-read'
        self.drill.restore_config.write_text(json.dumps(self.config))
        with patch.object(m, 'file_row') as read_key:
            self.refused_before_any_child()
        read_key.assert_not_called()

    def test_retargeted_group_directory_refused_without_reading_alternate(self):
        self.config['protocol_publication']['group_keys_dir'] = '/not-read'
        self.drill.restore_config.write_text(json.dumps(self.config))
        with patch.object(m, 'file_row') as read_key:
            self.refused_before_any_child()
        read_key.assert_not_called()

    def test_linked_key_is_refused(self):
        original = self.service.read_bytes()
        alternate = self.root / 'alternate-placeholder'
        alternate.write_bytes(original)
        self.service.unlink()
        self.service.symlink_to(alternate)
        self.refused_before_any_child()



class WorkflowSeams(unittest.TestCase):
    def model(self, root):
        drill = object.__new__(m.Drill)
        drill.report = {'runtime_executed': False}
        drill.guard = Mock()
        drill.fixture = Mock()
        drill.fixture.sink_thread.is_alive.return_value = False
        drill.fixture.protocol_lock = threading.RLock()
        drill.fixture.provision_protocol.return_value = {'pending_original_outbox_items': 1,
            'native_upload_intent_covered': False, 'event_id': 'a' * 64}
        drill.args = argparse.Namespace(release_manifest=root / 'release', pins=root / 'pins')
        drill.args.release_manifest.write_text('synthetic-release')
        drill.bundle = root / 'bundle'
        drill.source_config = root / 'source.json'
        drill.source_roots = {'native': root / 'source-native'}
        drill.original_roots = {'native': root / 'original-native'}
        drill.source_rows = []
        drill.source_snapshot = {'tables': 200, 'rows_sha256': 'a' * 64, 'schema_sha256': 'b' * 64}
        drill.source_business_snapshot = dict(drill.source_snapshot)
        drill.capture_baseline = Mock()
        drill.seed_account = Mock()
        drill.stop_checked = Mock()
        drill.handoff = Mock()
        drill.revoke_source_credentials = Mock()
        drill.verify_fence = Mock()
        drill.check_retained_credentials = Mock()
        drill.checkpoint_args = lambda config, **kw: argparse.Namespace(config=config, **kw)
        drill.pg_tool = Mock()
        drill.target_config = lambda label: (root / (label + '.json'), {'native': root / label / 'native'})
        drill.sql = Mock(return_value='synthetic-untouched')
        drill.pending_state = Mock(return_value=['1', '1', '1', 'digest'])
        drill.database_snapshot = Mock(return_value=drill.source_snapshot)
        drill.start_restored = Mock()
        drill.restored_checks = Mock()
        return drill

    def test_full_control_sequence_repeats_restored_checks_without_runtime_claim(self):
        with tempfile.TemporaryDirectory() as temporary:
            drill = self.model(Path(temporary))
            manifest = {'releaseManifestSha256': m.sha(drill.args.release_manifest), 'entries': []}
            with patch.object(m.checkpoint, 'backup', side_effect=[{'dryRun': True}, {'complete': True}]) as backup, \
                 patch.object(m.checkpoint, 'verify', return_value=manifest), \
                 patch.object(m.checkpoint, 'restore', side_effect=[{'dryRun': True}, m.checkpoint.Refused('Target name mismatch or nonempty application database'), {'complete': True}]) as restore, \
                 patch.object(m.checkpoint, 'scan', return_value=[]), \
                 patch.object(m, 'inventory', return_value={}), patch.object(m, 'read_json', return_value={}):
                drill.run()
            self.assertEqual(backup.call_count, 2)
            self.assertEqual(restore.call_count, 3)
            self.assertEqual(drill.start_restored.call_count, 2)
            self.assertEqual(drill.restored_checks.call_count, 2)
            self.assertEqual(drill.stop_checked.call_count, 2)
            drill.revoke_source_credentials.assert_called_once_with()
            drill.verify_fence.assert_called_once_with()
            drill.database_snapshot.assert_any_call(m.TARGET_DB, business=True)
            self.assertTrue(drill.report['source_unchanged'])
            self.assertTrue(drill.report['nonempty_sentinel_unchanged'])
            # A mocked sequence lacks real verification, cleanup and complete phase.
            self.assertFalse(m.clean_success(drill.report))

    def test_unrelated_restore_refusal_never_counts_as_nonempty_refusal(self):
        with tempfile.TemporaryDirectory() as temporary:
            drill = self.model(Path(temporary))
            manifest = {'releaseManifestSha256': m.sha(drill.args.release_manifest), 'entries': []}
            with patch.object(m.checkpoint, 'backup', side_effect=[{'dryRun': True}, {'complete': True}]), \
                 patch.object(m.checkpoint, 'verify', return_value=manifest), \
                 patch.object(m.checkpoint, 'restore', side_effect=[{'dryRun': True}, m.checkpoint.Refused('Storage still referenced')]) as restore:
                with self.assertRaises(m.Refused):
                    drill.run()
            self.assertEqual(restore.call_count, 2)
            drill.start_restored.assert_not_called()
            self.assertNotIn('nonempty_sentinel_unchanged', drill.report)

    def test_media_range_and_conditional_are_actual_response_predicates(self):
        drill = object.__new__(m.Drill)
        payload = b'synthetic image bytes'
        digest = hashlib.sha256(payload).hexdigest()
        good = [(200, payload, {'etag': 'fixture'}),
                (206, payload[2:6], {'content-range': f'bytes 2-5/{len(payload)}'}),
                (304, b'', {})]
        drill.http = Mock(side_effect=good)
        drill.media_check('/data/fixture/page.png', digest)
        for index, bad in [(0, (200, b'corrupt', {'etag': 'fixture'})),
                           (1, (200, payload, {})), (2, (200, payload, {}))]:
            values = list(good)
            values[index] = bad
            drill.http = Mock(side_effect=values)
            with self.subTest(index=index), self.assertRaises(m.Refused):
                drill.media_check('/data/fixture/page.png', digest)

    def test_original_endpoint_hash_check_does_not_claim_range_or_etag(self):
        drill = object.__new__(m.Drill)
        payload = b'synthetic original bytes'
        digest = hashlib.sha256(payload).hexdigest()
        drill.http = Mock(return_value=(200, payload, {}))
        drill.media_check('/originals/' + digest, digest, protocol=True, range_required=False)
        drill.http.assert_called_once_with('/originals/' + digest, protocol=True)
        self.assertIs(m.SCOPE['original_endpoint_range_qualified'], False)

    def test_default_plan_never_constructs_fixture_or_output(self):
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary) / 'not-created'
            command = [str(SCRIPT)]
            for argument in ('binary', 'protocol-provisioner', 'ui-dir', 'release-manifest', 'frontend-artifact-manifest', 'pins'):
                command.extend(['--' + argument, '/synthetic/input'])
            command.extend(['--pins-sha256', 'a' * 64, '--output', str(output)])
            stream = io.StringIO()
            previous_umask = os.umask(0o077)
            try:
                with patch.object(sys, 'argv', command), patch.dict(os.environ, clear=False), \
                     patch.object(m, 'preflight', return_value={'release': {'sha256': 'b' * 64}}), \
                     patch.object(m.subprocess, 'run', return_value=argparse.Namespace(returncode=0, stdout='c' * 40 + '\n')), \
                     patch.object(m, 'Drill') as constructor, m.contextlib.redirect_stdout(stream):
                    self.assertEqual(m.main(), 0)
                constructor.assert_not_called()
            finally:
                os.umask(previous_umask)
            self.assertFalse(output.exists())
            result = json.loads(stream.getvalue())
            self.assertEqual(result['status'], 'plan-only')
            self.assertFalse(result['runtime_executed'])

    def test_preflight_failure_emits_only_fixed_sanitized_json(self):
        with tempfile.TemporaryDirectory() as temporary:
            secret_path = str(Path(temporary) / 'PRIVATE_SENTINEL')
            command = [sys.executable, str(SCRIPT)]
            for argument in ('binary', 'protocol-provisioner', 'ui-dir', 'release-manifest', 'frontend-artifact-manifest', 'pins', 'output'):
                command.extend(['--' + argument, secret_path])
            command.extend(['--pins-sha256', 'a' * 64])
            result = subprocess.run(command, capture_output=True, text=True, timeout=10)
            self.assertEqual(result.returncode, 1)
            self.assertNotIn('PRIVATE_SENTINEL', result.stdout + result.stderr)
            value = json.loads(result.stdout)
            self.assertFalse(value['runtime_executed'])
            self.assertFalse(value['production_dr_qualified'])
            self.assertFalse(Path(secret_path).exists())


if __name__ == '__main__':
    unittest.main()
