"""Controller/schema checks and short owned OS-process seams; no browser/fixture."""
import importlib.util
import copy
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location('hosted_browser', ROOT / 'scripts/ci/run-hosted-browser.py')
runner = importlib.util.module_from_spec(spec)
spec.loader.exec_module(runner)


class HostedBrowserSeams(unittest.TestCase):
    def diagnostic_inputs(self):
        spec = importlib.util.spec_from_file_location('startup_seam', ROOT / 'scripts/ci/startup-diagnostics.py')
        diagnostics = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(diagnostics)
        private = 'never-export-this@example.invalid'
        origin = 'http://127.0.0.1:32123'
        acceptance = {
            'phase': 'startup', 'origin': origin, 'passed': False,
            'steps': [{'name': name, 'status': 'failed', 'error': 'Deep link has JavaScript page errors ' + private} for name in diagnostics.CHECK_NAMES],
            'page_errors': [
                {'url': origin + '/?token=' + private, 'error': 'process is not defined ' + private, 'error_class': 'ReferenceError'},
                {'url': origin + '/search?token=' + private, 'error': private + ' is not defined', 'error_class': private},
                {'url': origin + '/api/' + private, 'error': 'Failed to fetch dynamically imported module https://private.invalid/' + private, 'error_class': 'TypeError'},
            ],
            'console_errors': [{'url': origin + '/users/' + private, 'error': 'Refused to execute due to Content Security Policy ' + private}],
            'csp_violations': [{'page': origin + '/search?token=' + private, 'directive': 'script-src-elem', 'blocked_uri': 'https://private.invalid/' + private, 'disposition': 'enforce'}],
            'startup_network': [
                {'url': origin + '/_app/immutable/entry/' + private + '.js?token=' + private, 'resource': 'script', 'status': 200, 'content_type': 'text/html; private=' + private},
                {'url': origin + '/api/' + private + '?token=' + private, 'resource': 'fetch', 'status': 0, 'failure': 'net::ERR_CONNECTION_REFUSED'},
            ],
        }
        owned = {'fixture_only': True, 'phase': 'startup', 'verified_stopped': True, 'forced': False, 'returncode': 1}
        spec = importlib.util.spec_from_file_location('startup_validator', ROOT / 'scripts/ci/validate-ci-evidence.py')
        validator = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(validator)
        return diagnostics, validator, acceptance, owned, private

    def test_startup_check_ids_match_actual_harness_and_fixed_fixture_routes(self):
        import ast
        import re
        diagnostics, _, _, _, _ = self.diagnostic_inputs()
        source = (ROOT / 'scripts/browser-acceptance.mjs').read_text()
        routes = json.loads(re.search(r'const routes = (\[[^;]+\]);', source)[1])
        fixture = ast.parse((ROOT / 'tests/runtime/browser_fixture.py').read_text())
        ids = next(ast.literal_eval(row.value) for row in fixture.body if isinstance(row, ast.Assign) and any(isinstance(target, ast.Name) and target.id == 'IDS' for target in row.targets))
        routes += [prefix + ids[kind] for kind, prefix in [('manga', '/title/'), ('group', '/group/'), ('chapter', '/chapter/')]]
        self.assertEqual(diagnostics.ROUTES, ['/', *routes])
        self.assertEqual(diagnostics.CHECK_NAMES, ['homepage bootstraps under exact generated CSP', *['fresh deep link ' + route for route in routes], 'missing built/static assets produce real 404'])
        self.assertIn("inputs.batch || 'all'", (ROOT.parent / '.github/workflows/hosted-browser-e2e.yml').read_text())

    def test_startup_projection_serializes_only_closed_diagnostics_and_passes_real_gate(self):
        diagnostics, validator, acceptance, owned, private = self.diagnostic_inputs()
        row = runner.project_phase('startup', 'startup', acceptance, owned)
        value = row['startup_diagnostics']
        self.assertEqual(row['status'], 'failed')
        self.assertTrue(all(check['check_identity_verified'] for check in value['check_results']))
        self.assertEqual(value['page_errors'][0]['category'], 'missing-global')
        self.assertTrue(any(error['category'] == 'module-fetch' for error in value['page_errors']))
        self.assertTrue(any(error['category'] == 'reference-error' and error['missing_global'] == 'none' for error in value['page_errors']))
        self.assertEqual(value['network'][0]['mime'], 'html')
        self.assertEqual(value['csp_violations'][0]['blocked'], 'other-origin')
        with tempfile.TemporaryDirectory() as root:
            directory = Path(root) / 'input'
            directory.mkdir()
            canonical = Path(root) / 'canonical'
            runner.write(directory / 'phases.json', {'schema': 1, 'phases': [row], 'batches': []})
            self.assertEqual(validator.validate(directory, canonical), 1)
            encoded = (canonical / 'phases.json').read_text()
            for token in [private, 'private.invalid', '32123', '?token=', ' is not defined', 'Content Security Policy']:
                self.assertNotIn(token, encoded)
            self.assertEqual(json.loads(encoded)['phases'][0], row)
        self.assertEqual(diagnostics.public_path('/search', ''), '<unavailable>')
        self.assertEqual(diagnostics.public_path('http://alice:password@127.0.0.1:32123/search', acceptance['origin']), '<other-origin>')
        self.assertEqual(diagnostics.error_kind('private', ['unexpected'])['error_class'], 'Unknown')

    def test_startup_adverse_serialized_diagnostic_values_fail_closed(self):
        _, validator, acceptance, owned, private = self.diagnostic_inputs()
        baseline = runner.project_phase('startup', 'startup', acceptance, owned)
        with tempfile.TemporaryDirectory() as directory:
            directory = Path(directory)
            cases = [
                ('check_results', 'check_id', private), ('check_results', 'status', 'passed'),
                ('page_errors', 'error_class', private), ('page_errors', 'category', private),
                ('page_errors', 'missing_global', 'a' * 64), ('page_errors', 'path', '/api/' + private),
                ('page_errors', 'count', 10000), ('network', 'status', True),
                ('network', 'mime', private), ('network', 'failure', private),
                ('csp_violations', 'directive', private), ('csp_violations', 'blocked', private),
                ('page_errors', 'raw_error', private),
            ]
            for field, key, value in cases:
                row = copy.deepcopy(baseline)
                row['startup_diagnostics'][field][0][key] = value
                runner.write(directory / 'phases.json', {'schema': 1, 'phases': [row], 'batches': []})
                with self.subTest(field=field, key=key), self.assertRaises(ValueError):
                    validator.validate(directory)
            row = copy.deepcopy(baseline)
            row['phase'] = 'account'
            runner.write(directory / 'phases.json', {'schema': 1, 'phases': [row], 'batches': []})
            with self.assertRaises(ValueError):
                validator.validate(directory)
            runner.write(directory / 'phases.json', {'schema': 1, 'phases': [baseline], 'batches': []})
            path = directory / 'phases.json'
            path.write_text(path.read_text().replace('"category": "missing-global"', '"category": "' + private + '", "category": "missing-global"', 1))
            with self.assertRaises(ValueError):
                validator.validate(directory)

    def test_startup_observation_caps_and_check_identity_are_honest(self):
        diagnostics, validator, acceptance, owned, _ = self.diagnostic_inputs()
        acceptance['steps'][0]['name'] = 'unexpected private check name'
        acceptance['page_errors'] = acceptance['page_errors'][:1] * (diagnostics.LIMIT + 1)
        acceptance['startup_network'] *= diagnostics.LIMIT
        row = runner.project_phase('startup', 'startup', acceptance, owned)
        value = row['startup_diagnostics']
        self.assertTrue(value['observations_truncated'])
        self.assertFalse(value['check_results'][0]['check_identity_verified'])
        self.assertEqual(sum(error['count'] for error in value['page_errors']), diagnostics.LIMIT)
        self.assertEqual(sum(request['count'] for request in value['network']), diagnostics.LIMIT)
        validator.phases({'schema': 1, 'phases': [row], 'batches': []})

    def test_startup_error_and_truncation_consistency_uses_actual_serialized_facts(self):
        diagnostics, validator, acceptance, owned, _ = self.diagnostic_inputs()
        # Missing exception text still produces Unknown/unknown, never a no-error
        # claim. This seam does not require free-form exception text to exist.
        acceptance['steps'][0].pop('error')
        acceptance['page_errors'] = acceptance['page_errors'][:1] * (diagnostics.LIMIT + 1)
        row = runner.project_phase('startup', 'startup', acceptance, owned)
        self.assertEqual(row['startup_diagnostics']['check_results'][0]['category'], 'unknown')
        with tempfile.TemporaryDirectory() as directory:
            directory = Path(directory)
            runner.write(directory / 'phases.json', {'schema': 1, 'phases': [row], 'batches': []})
            self.assertEqual(validator.validate(directory), 1)
            cases = []
            bad = copy.deepcopy(row)
            bad['startup_diagnostics']['observations_truncated'] = False
            cases.append(bad)
            for field in ['check_results', 'page_errors']:
                bad = copy.deepcopy(row)
                bad['startup_diagnostics'][field][0].update(error_class='None', category='none', missing_global='none')
                cases.append(bad)
            for bad in cases:
                runner.write(directory / 'phases.json', {'schema': 1, 'phases': [bad], 'batches': []})
                with self.assertRaises(ValueError):
                    validator.validate(directory)

    def test_startup_long_identifier_diagnostics_use_whole_bounded_tokens(self):
        diagnostics, _, _, _, _ = self.diagnostic_inputs()
        token = 'a' * 9000
        self.assertEqual(diagnostics.error_kind(token), {'error_class': 'Unknown', 'category': 'unknown', 'missing_global': 'none'})
        self.assertEqual(diagnostics.error_kind(token + ' is not defined'), {'error_class': 'ReferenceError', 'category': 'reference-error', 'missing_global': 'none'})
        self.assertEqual(diagnostics.error_kind(token + 'process is not defined')['missing_global'], 'none')
        self.assertEqual(diagnostics.error_kind(token + '; process is not defined')['missing_global'], 'process')
        self.assertEqual(diagnostics.error_kind('$state is not defined')['missing_global'], '$state')

    def test_exact_original_38_phases(self):
        handoff = json.loads((ROOT / 'test-evidence/frontend/browser-startup-plan-build19.json').read_text())['phases']
        expected = {'startup'} | {phase for group in handoff.values() for phase in group.get('names', [])}
        self.assertEqual(set(runner.ALL_PHASES) - {'ui14-actor-follow'}, expected)
        self.assertEqual(runner.BATCHES['actor-follow'], ['ui14-actor-follow'])
        self.assertEqual(len(runner.ALL_PHASES), 39)
        self.assertEqual(len(expected), 38)

    def test_supplemental_actor_follow_scope_and_strict_counts_are_consistent(self):
        _, validator, _, _, _ = self.diagnostic_inputs()
        plan = json.loads((ROOT / 'test-evidence/frontend/browser-actor-follow-plan.json').read_text())
        self.assertEqual(plan['supplemental_phases'], runner.BATCHES['actor-follow'])
        self.assertEqual(plan['total_required_unique_phases'], len(runner.ALL_PHASES))
        self.assertEqual(len(runner.BATCHES), 11)
        self.assertEqual(runner.MAX_PHASE_RUNS, 51)
        historical = [phase for phase in runner.ALL_PHASES if phase != 'ui14-actor-follow']
        journal = {'phases': [{'phase': phase, 'status': 'passed'} for phase in historical], 'batches': [{'passed': True, 'supervisor_terminal': True, 'supervisor_exit_zero': True, 'cleanup': {'verified_stopped': True, 'lifecycle_clean': True}}]}
        summary = runner.summarize(journal, 'all', runner.ALL_PHASES)
        self.assertFalse(summary['selected_scope_passed'])
        self.assertFalse(summary['full_suite_passed'])
        self.assertEqual(summary['pending_unique_phases'], ['ui14-actor-follow'])
        validator.summary(summary)
        for key, value in [('required_unique_phases', 38), ('actual_phase_runs', 52)]:
            with self.assertRaises(ValueError):
                validator.summary(dict(summary, **{key: value}))

    def test_original_frontend_and_only_three_pinned_additions_are_accepted(self):
        spec = importlib.util.spec_from_file_location('hosted_preparation', ROOT / 'scripts/ci/prepare-hosted-browser.py')
        preparation = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(preparation)
        preparation.verify_frontend()  # Actual checked-in 679-file tree.
        with tempfile.TemporaryDirectory() as directory:
            frontend = Path(directory) / 'frontend'
            shutil.copytree(ROOT / 'frontend', frontend)
            preparation.verify_frontend(frontend)
            extra = frontend / 'unreviewed.txt'
            extra.write_text('unexpected')
            with self.assertRaises(RuntimeError):
                preparation.verify_frontend(frontend)
            extra.unlink()
            for target in [frontend / 'package.json', frontend / 'src/routes/(app)/announcements/+page.svelte']:
                original = target.read_bytes()
                target.write_bytes(original + b'\n')
                with self.assertRaises(RuntimeError):
                    preparation.verify_frontend(frontend)
                target.write_bytes(original)

    def test_every_batch_fits_a_fresh_fixture(self):
        for phases in runner.BATCHES.values():
            phases = ['startup', *phases]
            self.assertEqual(len(phases), len(set(phases)))
            self.assertLess(sum(runner.maximum(phase) + 35 for phase in phases) + 300, 3600)
            for phase in phases:
                if phase in runner.SIGNED:
                    self.assertEqual(runner.maximum(phase), 600)
                    self.assertGreaterEqual(runner.maximum(phase), 430)

    def test_reader_seed_and_destructive_order(self):
        phases = runner.BATCHES['ui-reader']
        self.assertLess(phases.index('ui12-compact-feed'), phases.index('ui15-chapter-filter'))
        self.assertLess(phases.index('ui15-chapter-filter'), phases.index('ui15-reader-geometry'))
        self.assertLess(phases.index('reader'), phases.index('ui15-16-empty-tabs'))

    def passed_phase_inputs(self):
        # Explicit test doubles for validator checks, never browser evidence.
        acceptance = {'phase': 'account', 'passed': True, 'completed_utc': 'synthetic-finalized', 'proxy_lifecycle': {'connect_denials': 0, 'expected_connect_resets': 0, 'remaining_socket_count': 0, 'observations_truncated': False, 'shutdown_started': True, 'closed': True}, 'runner_lifecycle': {'stage': 'complete', 'fatal_kind': 'none', 'fatal_code': 'none', 'fatal_stage': 'none', 'exit_observed': True, 'exit_code': 0}, 'steps': [{'status': 'passed'}], 'page_errors': []}
        owned = {'fixture_only': True, 'phase': 'account', 'verified_stopped': True, 'forced': False, 'returncode': 0}
        return acceptance, owned

    def test_runner_pass_requires_actual_steps_and_owned_terminal(self):
        acceptance, owned = self.passed_phase_inputs()
        self.assertEqual(runner.project_phase('account', 'test', acceptance, owned)['status'], 'passed')
        for field, value in [('returncode', 1), ('verified_stopped', False), ('forced', True), ('phase', 'reader')]:
            bad = dict(owned, **{field: value})
            self.assertEqual(runner.project_phase('account', 'test', acceptance, bad)['status'], 'failed')
        for field, value in [('steps', []), ('page_errors', ['error']), ('harness_error', 'synthetic adverse error'), ('passed', False), ('phase', 'reader')]:
            bad = dict(acceptance, **{field: value})
            self.assertEqual(runner.project_phase('account', 'test', bad, owned)['status'], 'failed')
        self.assertEqual(runner.project_phase('account', 'test', {}, owned)['status'], 'failed')

    def test_harness_error_producer_and_actual_schema_gate_agree(self):
        spec = importlib.util.spec_from_file_location('evidence_validator_harness_error', ROOT / 'scripts/ci/validate-ci-evidence.py')
        validator = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(validator)
        acceptance, owned = self.passed_phase_inputs()
        acceptance['harness_error'] = 'synthetic adverse error, never public'
        row = runner.project_phase('account', 'ordinary-a', acceptance, owned)
        self.assertEqual(row['status'], 'failed')
        self.assertTrue(row['harness_error_observed'])
        with tempfile.TemporaryDirectory() as directory:
            directory = Path(directory)
            journal = {'schema': 1, 'phases': [row], 'batches': []}
            runner.write(directory / 'phases.json', journal)
            self.assertEqual(validator.validate(directory), 1)
            row['status'] = 'passed'  # Explicit adversarial serialized input.
            runner.write(directory / 'phases.json', journal)
            with self.assertRaises(ValueError):
                validator.validate(directory)

    def cleanup_input(self):
        return {'fixture_only': True, 'status': 'verified_stopped', 'synthetic_server_keys_absent': True, 'loopback_ports_closed': {name: True for name in ['platform', 'mail', 'postgres', 'original', 'recipient']}, 'remaining_owned_processes': [], 'process_tracking_errors': [], 'final_teardown_clean': True, 'lifecycle_clean': True}

    def test_cleanup_requires_all_closed_ports_and_keys(self):
        good = self.cleanup_input()
        self.assertTrue(runner.project_cleanup(good)['verified_stopped'])
        for field, value in [('loopback_ports_closed', {}), ('synthetic_server_keys_absent', False), ('remaining_owned_processes', [{}]), ('process_tracking_errors', [{}]), ('final_teardown_clean', False)]:
            self.assertFalse(runner.project_cleanup(dict(good, **{field: value}))['verified_stopped'])

    def test_native_publication_batches_have_explicit_isolated_mode(self):
        plan = {key: key for key in ['normal_binary', 'normal_sha256', 'frontend_index_sha256', 'frontend_manifest', 'frontend_manifest_sha256', 'provisioner', 'provisioner_sha256']}
        for name in runner.BATCHES:
            args = runner.fixture_arguments(name, plan)
            native = name in {'ordinary-a', 'ordinary-b'}
            self.assertEqual('--native-only' in args, native)
            self.assertEqual('--protocol-provisioner' in args, not native)
            self.assertEqual('--second-protocol-recipient' in args, not native)
            self.assertEqual('--ui-regression-controls' in args, not native)
        self.assertEqual(runner.fixture_mode('startup'), 'combined')

    def test_native_only_cli_rejects_combined_authority_before_constructing_fixture(self):
        fixture = ROOT / 'tests/runtime/browser_fixture.py'
        runtime = ROOT / 'test-runtime'
        before = set(runtime.iterdir()) if runtime.exists() else set()
        conflicts = [['--protocol-provisioner', '/not-an-owned-provisioner'], ['--provisioner-sha256', '0' * 64], ['--second-protocol-recipient'], ['--ui-regression-controls'], ['--pg-home-fault-controls'], ['--ubunchu-noncommercial']]
        if '--protocol-self-enrollment' in fixture.read_text(): conflicts.append(['--protocol-self-enrollment'])
        for conflict in conflicts:
            with self.subTest(flag=conflict[0]):
                result = subprocess.run([sys.executable, str(fixture), '--binary', '/not-an-owned-binary', '--frontend-index-sha256', '0' * 64, '--frontend-artifact-manifest-sha256', '0' * 64, '--native-only', *conflict], capture_output=True, text=True, timeout=10)
                self.assertEqual(result.returncode, 2)
                self.assertIn('native-only mode excludes protocol authorities, provisioner and combined controls', result.stderr)
                self.assertNotIn('FIXTURE_ERROR', result.stdout)
        self.assertEqual(set(runtime.iterdir()) if runtime.exists() else set(), before)

    def test_native_only_readiness_binds_exact_mode_and_no_protocol_authority(self):
        plan = {'normal_sha256': 'binary', 'frontend_manifest_sha256': 'frontend'}
        good = {'status': 'ready', 'run_directory': '/owned', 'binary': {'sha256': 'binary'}, 'frontend': {'artifact_manifest_sha256': 'frontend'}, 'fixture_mode': 'native-only', 'seed': {'native_only': True, 'protocol_signed_publications': False}}
        self.assertTrue(runner.ready_matches(good, Path('/owned'), plan, 'native-only'))
        self.assertFalse(runner.ready_matches(good, Path('/owned'), plan))
        for key in ['protocol_origin', 'relay_origin', 'protocol_recipients', 'protocol_provisioner', 'protocol_seed', 'service_key']:
            self.assertFalse(runner.ready_matches(dict(good, **{key: {}}), Path('/owned'), plan, 'native-only'))
        for key, value in [('fixture_mode', 'combined'), ('seed', {}), ('binary', {}), ('frontend', {}), ('status', 'starting'), ('run_directory', '/other')]:
            self.assertFalse(runner.ready_matches(dict(good, **{key: value}), Path('/owned'), plan, 'native-only'))

    def test_native_cleanup_requires_exact_owned_three_ports(self):
        good = self.cleanup_input()
        good['loopback_ports_closed'] = {name: True for name in ['platform', 'mail', 'postgres']}
        self.assertTrue(runner.project_cleanup(good, 'native-only')['verified_stopped'])
        self.assertFalse(runner.project_cleanup(good)['verified_stopped'])
        self.assertFalse(runner.project_cleanup(self.cleanup_input(), 'native-only')['verified_stopped'])
        for name in ['platform', 'mail', 'postgres']:
            adverse = copy.deepcopy(good); adverse['loopback_ports_closed'][name] = False
            self.assertFalse(runner.project_cleanup(adverse, 'native-only')['verified_stopped'])
        with self.assertRaises(ValueError): runner.project_cleanup(good, 'unknown')

    def test_summary_cannot_promote_partial_scope_to_full_suite(self):
        journal = {'phases': [{'phase': 'startup', 'status': 'passed'}], 'batches': [{'passed': True, 'supervisor_terminal': True, 'supervisor_exit_zero': True, 'cleanup': {'verified_stopped': True, 'lifecycle_clean': True}}]}
        summary = runner.summarize(journal, 'startup', ['startup'])
        self.assertTrue(summary['selected_scope_passed'])
        self.assertFalse(summary['full_suite_passed'])
        self.assertEqual(len(summary['pending_unique_phases']), 38)
        self.assertFalse(runner.summarize(journal, 'all', runner.ALL_PHASES)['selected_scope_passed'])

    def test_failure_is_not_hidden_by_duplicate_success(self):
        journal = {'phases': [{'phase': 'startup', 'status': 'passed'}, {'phase': 'startup', 'status': 'failed'}], 'batches': [{'passed': True, 'supervisor_terminal': True, 'supervisor_exit_zero': True, 'cleanup': {'verified_stopped': True, 'lifecycle_clean': True}}]}
        summary = runner.summarize(journal, 'startup', ['startup'])
        self.assertFalse(summary['selected_scope_passed'])
        self.assertEqual(summary['passed_unique_phases'], [])
        self.assertEqual(summary['failed_unique_phases'], ['startup'])

    def test_real_nonzero_child_exit_and_cleanup_failure_cannot_pass_summary(self):
        # A real waited process supplies the terminal exit fact. The positive
        # step bit is an explicitly adversarial validator input, not a browser
        # report, and no mocked runner/summary implementation is substituted.
        child = subprocess.Popen([sys.executable, '-c', 'raise SystemExit(7)'])
        child.wait(timeout=5)
        journal = {'phases': [{'phase': 'startup', 'status': 'passed'}], 'batches': [{'passed': True, 'supervisor_terminal': child.poll() is not None, 'supervisor_exit_zero': child.returncode == 0, 'cleanup': {'verified_stopped': True, 'lifecycle_clean': True}}]}
        with tempfile.TemporaryDirectory() as directory:
            receipt = Path(directory) / 'summary.json'
            runner.write(receipt, runner.summarize(journal, 'startup', ['startup']))
            value = runner.read(receipt)
            self.assertFalse(value['selected_scope_passed'])
            self.assertFalse(value['all_fixture_lifecycles_clean'])
            self.assertTrue(value['all_fixture_teardowns_verified'])
            for field in ['verified_stopped', 'lifecycle_clean']:
                journal['batches'][0]['supervisor_exit_zero'] = True
                journal['batches'][0]['cleanup'][field] = False
                runner.write(receipt, runner.summarize(journal, 'startup', ['startup']))
                self.assertFalse(runner.read(receipt)['selected_scope_passed'])
                journal['batches'][0]['cleanup'][field] = True

    def test_public_projection_omits_secrets_and_response_bodies(self):
        acceptance, owned = self.passed_phase_inputs()
        acceptance.update(passed=False, observations={'password': 'never-export-this'}, harness_error='never-export-this')
        acceptance['steps'] = [{'status': 'failed', 'name': 'test https://example.invalid/?token=never-export-this user@example.invalid ' + 'a' * 64, 'error': 'never-export-this', 'observation': {'cookie': 'never-export-this'}}]
        encoded = json.dumps(runner.project_phase('account', 'test', acceptance, owned))
        self.assertNotIn('never-export-this', encoded)
        self.assertNotIn('@example.invalid', encoded)
        self.assertNotIn('a' * 64, encoded)

    def test_workflow_has_one_readonly_standard_job(self):
        workflow = (ROOT.parent / '.github/workflows/hosted-browser-e2e.yml').read_text()
        self.assertIn('branches: [ci/hosted-browser-e2e, ci/browser-failure-diagnostics]', workflow)
        self.assertIn("github.ref == 'refs/heads/ci/hosted-browser-e2e'", workflow)
        self.assertIn("github.ref == 'refs/heads/ci/browser-failure-diagnostics'", workflow)
        self.assertIn('contents: read', workflow)
        self.assertIn('runs-on: ubuntu-24.04', workflow)
        self.assertIn('persist-credentials: false', workflow)
        self.assertNotIn('pull_request_target', workflow)
        self.assertNotIn('secrets.', workflow)
        self.assertNotIn('self-hosted', workflow.replace('private/paid/self-hosted', ''))
        self.assertIn('retention-days: 1', workflow)
        self.assertNotIn('test-runtime/', workflow)
        self.assertIn("steps.evidence-validation.outcome == 'success'", workflow)
        self.assertIn('ci-sanitized-evidence/preparation.json', workflow)
        self.assertNotIn('ci-evidence/preparation.json', workflow)

    def test_closed_artifact_gate_rejects_unknown_keys_and_personal_strings(self):
        spec = importlib.util.spec_from_file_location('evidence_validator', ROOT / 'scripts/ci/validate-ci-evidence.py')
        validator = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(validator)
        prep = {'schema': 1, 'status': 'preparation_failed', 'browser_phases_started': False, 'stage': 'source-preservation', 'source_files_checked': 0, 'frontend_preserved': False, 'artifacts_uploaded': False}
        journal = {'schema': 1, 'phases': [], 'batches': []}
        summary = runner.summarize(journal, 'all', runner.ALL_PHASES)
        with tempfile.TemporaryDirectory() as directory:
            directory = Path(directory)
            for name, value in [('preparation.json', prep), ('phases.json', journal), ('summary.json', summary)]:
                runner.write(directory / name, value)
            self.assertEqual(validator.validate(directory), 3)
            for field, value in [('private_key', 'a' * 64), ('raw_log', 'alice@example.com'), ('error_class', 'alice@example.com'), ('normal_binary_sha256', '0123456789abcdef' * 4)]:
                runner.write(directory / 'preparation.json', dict(prep, **{field: value}))
                with self.assertRaises(ValueError):
                    validator.validate(directory)
            runner.write(directory / 'preparation.json', prep)
            bad = dict(summary, pending_unique_phases=['alice@example.com', *summary['pending_unique_phases'][1:]])
            runner.write(directory / 'summary.json', bad)
            with self.assertRaises(ValueError):
                validator.validate(directory)
            runner.write(directory / 'summary.json', summary)
            (directory / 'credentials.txt').write_text('never-export-this')
            with self.assertRaises(ValueError):
                validator.validate(directory)
            (directory / 'credentials.txt').unlink()
            journal['batches'] = [{'batch': 'startup', 'readiness': False, 'passed': False, 'cleanup': {'verified_stopped': 0}}]
            runner.write(directory / 'phases.json', journal)
            with self.assertRaises(ValueError):
                validator.validate(directory)
            link = directory.parent / (directory.name + '-link')
            try:
                link.symlink_to(directory, target_is_directory=True)
                with self.assertRaises(ValueError):
                    validator.validate(link)
            finally:
                link.unlink()

    def test_nested_duplicate_keys_rejected_and_only_canonical_values_published(self):
        spec = importlib.util.spec_from_file_location('evidence_validator_duplicates', ROOT / 'scripts/ci/validate-ci-evidence.py')
        validator = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(validator)
        prep = {'schema': 1, 'status': 'preparation_failed', 'browser_phases_started': False, 'stage': 'source-preservation', 'source_files_checked': 0, 'frontend_preserved': False, 'artifacts_uploaded': False}
        with tempfile.TemporaryDirectory() as root:
            directory = Path(root) / 'input'
            directory.mkdir()
            canonical = Path(root) / 'canonical'
            private = 'never-export-this@example.invalid'
            original = json.dumps(prep, indent=4)
            duplicate = original.replace('"stage": "source-preservation"', f'"stage": "{private}", "stage": "source-preservation"')
            (directory / 'preparation.json').write_text(duplicate)
            with self.assertRaises(ValueError):
                validator.validate(directory, canonical)
            self.assertFalse(canonical.exists())
            (directory / 'preparation.json').write_text(original)
            nested = '{"schema":1,"phases":[],"batches":[{"batch":"startup","readiness":false,"passed":false,"cleanup":{"verified_stopped":"' + private + '","verified_stopped":false}}]}'
            (directory / 'phases.json').write_text(nested)
            with self.assertRaises(ValueError):
                validator.validate(directory, canonical)
            self.assertFalse(canonical.exists())
            (directory / 'phases.json').unlink()
            self.assertEqual(validator.validate(directory, canonical), 1)
            output = (canonical / 'preparation.json').read_text()
            self.assertEqual(output, json.dumps(prep, sort_keys=True, separators=(',', ':'), ensure_ascii=True) + '\n')
            self.assertNotIn(private, output)
            self.assertEqual({p.name for p in canonical.iterdir()}, {'preparation.json'})

    def test_real_finalizer_cleans_after_stop_failure_and_preserves_receipt_failure(self):
        # Filesystem failures are real boundary errors. The dedicated process
        # owns its real child; no cleanup method or terminal state is mocked.
        helper = ROOT / 'scripts/ci/owned-fixture-cleanup.py'
        script = ROOT / 'scripts/ci/run-hosted-browser.py'
        driver = f'''import importlib.util,json,pathlib,subprocess,sys,tempfile
def load(name,path):
 spec=importlib.util.spec_from_file_location(name,path); module=importlib.util.module_from_spec(spec); spec.loader.exec_module(module); return module
owner=load("owner",{str(helper)!r}).OwnedFixtureProcesses(); runner=load("runner",{str(script)!r}); results=[]
with tempfile.TemporaryDirectory() as root:
 for mode in ["stop", "receipt"]:
  fixture=pathlib.Path(root)/mode; fixture.mkdir(); batch={{"cleanup":{{"verified_stopped":False}}}}
  with owner.process_ownership_lock:
   child=subprocess.Popen([sys.executable,"-c","import time; time.sleep(10)"],start_new_session=True); owner.register_process("supervisor",child)
  try:
   if mode=="stop": (fixture/"stop.request").symlink_to(fixture/"missing-parent"/"stop")
   else:
    child.terminate(); child.wait(timeout=5); (fixture/"cleanup.json").write_text("invalid-json")
   runner.finish_supervisor(owner,child,fixture,batch,seconds=.1); batch["remaining_actual_members"]=len(owner.process_members(None)); results.append(batch)
  finally: owner.fallback_cleanup()
print(json.dumps(results))
'''
        result = subprocess.run([sys.executable, '-c', driver], capture_output=True, text=True, timeout=25)
        self.assertEqual(result.returncode, 0, result.stderr)
        stop, receipt = json.loads(result.stdout)
        self.assertEqual(stop['cleanup_error_class'], 'FileNotFoundError')
        self.assertTrue(stop['outer_cleanup']['forced'])
        self.assertTrue(stop['outer_cleanup']['verified_stopped'])
        self.assertEqual(receipt['cleanup_error_class'], 'JSONDecodeError')
        for value in [stop, receipt]:
            self.assertTrue(value['supervisor_terminal'])
            self.assertFalse(value['cleanup']['verified_stopped'])
            self.assertEqual(value['remaining_actual_members'], 0)

    def test_real_detached_descendant_is_cleaned_after_adverse_observation(self):
        helper = ROOT / 'scripts/ci/owned-fixture-cleanup.py'
        script = ROOT / 'scripts/ci/run-hosted-browser.py'
        leader = 'import subprocess,sys,time; child=subprocess.Popen([sys.executable,"-c","import time; time.sleep(8)"],start_new_session=True); print(child.pid,flush=True); time.sleep(.1)'
        driver = f'''import importlib.util,json,pathlib,subprocess,sys,tempfile
def load(name,path):
 spec=importlib.util.spec_from_file_location(name,path); module=importlib.util.module_from_spec(spec); spec.loader.exec_module(module); return module
module=load("owner",{str(helper)!r}); runner=load("runner",{str(script)!r})
class AdverseObservation(module.OwnedFixtureProcesses):
 def __init__(self): super().__init__(); self.fail_once=True
 def process_members(self,process):
  if self.fail_once: self.fail_once=False; raise PermissionError("injected OS observation failure")
  return super().process_members(process)
owner=AdverseObservation()
with owner.process_ownership_lock:
 child=subprocess.Popen([sys.executable,"-c",{leader!r}],stdout=subprocess.PIPE,text=True,start_new_session=True); owner.register_process("supervisor",child)
try:
 descendant=int(child.stdout.readline()); owner.owned_members(); child.wait(timeout=5)
 with tempfile.TemporaryDirectory() as root:
  batch={{"cleanup":{{"verified_stopped":False}}}}; runner.finish_supervisor(owner,child,pathlib.Path(root),batch,seconds=.1); batch["remaining_actual_members"]=len(owner.process_members(None)); print(json.dumps(batch))
finally: owner.fallback_cleanup()
'''
        result = subprocess.run([sys.executable, '-c', driver], capture_output=True, text=True, timeout=25)
        self.assertEqual(result.returncode, 0, result.stderr)
        value = json.loads(result.stdout)
        self.assertEqual(value['cleanup_error_class'], 'PermissionError')
        self.assertTrue(value['supervisor_exit_zero'])
        self.assertTrue(value['outer_cleanup']['forced'])
        self.assertTrue(value['outer_cleanup']['verified_stopped'])
        self.assertFalse(value['cleanup']['verified_stopped'])
        self.assertEqual(value['remaining_actual_members'], 0)

    def test_real_outer_fallback_reaps_detached_tree_and_preserves_unrelated_sibling(self):
        # A dedicated owner process is used, just like the production controller.
        # No browser, database, fixture constructor or network listener is used.
        helper = ROOT / 'scripts/ci/owned-fixture-cleanup.py'
        leader = 'import signal,subprocess,sys,time; signal.signal(signal.SIGTERM,signal.SIG_IGN); child=subprocess.Popen([sys.executable,"-c","import time; time.sleep(8)"],start_new_session=True); print(child.pid,flush=True); time.sleep(8)'
        driver = f'''import importlib.util,json,subprocess,sys
spec=importlib.util.spec_from_file_location("owner",{str(helper)!r}); module=importlib.util.module_from_spec(spec); spec.loader.exec_module(module)
owner=module.OwnedFixtureProcesses()
with owner.process_ownership_lock:
 child=subprocess.Popen([sys.executable,"-c",{leader!r}],stdout=subprocess.PIPE,text=True,start_new_session=True); owner.register_process("supervisor",child)
try:
 descendant=int(child.stdout.readline()); rows=owner.owned_members(); detached=any(row["pid"]==descendant and row["session"]!=child.pid for row in rows)
 terminal_before=owner.wait_terminal(child,.05); result=owner.fallback_cleanup(); result.update(detached_descendant_seen=detached,terminal_before=terminal_before,leader_joined=child.poll() is not None); print(json.dumps(result))
finally:
 owner.fallback_cleanup()
'''
        unrelated = subprocess.Popen([sys.executable, '-c', 'import time; time.sleep(30)'], start_new_session=True)
        try:
            result = subprocess.run([sys.executable, '-c', driver], capture_output=True, text=True, timeout=25)
            self.assertEqual(result.returncode, 0, result.stderr)
            value = json.loads(result.stdout)
            self.assertTrue(value['detached_descendant_seen'])
            self.assertFalse(value['terminal_before'])
            self.assertTrue(value['forced'])
            self.assertTrue(value['verified_stopped'])
            self.assertTrue(value['leader_joined'])
            self.assertEqual(value['remaining_owned_process_count'], 0)
            self.assertIsNone(unrelated.poll())
        finally:
            unrelated.terminate()
            unrelated.wait(timeout=5)


if __name__ == '__main__':
    unittest.main()
