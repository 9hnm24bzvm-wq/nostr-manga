"""Finite receipts/source stand-ins only; no Rust, native DB, service or browser."""
import copy
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import shutil
import tempfile
from types import SimpleNamespace
import unittest
from unittest import mock

ROOT = Path(__file__).resolve().parents[2]


def load(name, relative):
    spec = importlib.util.spec_from_file_location(name, ROOT / relative)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


contract = load('title_finite_contract', 'scripts/ci/native-title-diagnostics.py')
adapter = load('title_finite_adapter', 'scripts/ci/run-hosted-title-regression.py')
feed = load('title_finite_feed', 'scripts/ci/native-feed-diagnostics.py')


def owned():
    return {'process_exit_code': 0, 'deadline_exceeded': False, 'terminal_receipt': True,
            'cleanup_verified': True, 'surviving_owned_process_ids': [], 'cleanup_signals': [],
            'prior_or_unrelated_processes_touched': False}


def test_text(names):
    return '\n'.join([f'running {len(names)} tests',
        *[f'test {name} ... ok' for name in names],
        f'test result: ok. {len(names)} passed; 0 failed; 0 ignored; 0 measured; 123 filtered out; finished in 0.01s']) + '\n'


def title_pass():
    value = contract.not_run()
    value.update(status='passed', source_verified=True, artifacts_verified=True, negative_requests_verified=7,
                 executable_sha256={'library': '1' * 64, 'http': '2' * 64},
                 units=contract.project_stage(owned(), contract.parse_tests(test_text(contract.UNIT_TESTS), contract.UNIT_TESTS)),
                 http=contract.project_stage(owned(), contract.parse_tests(test_text((contract.HTTP_TEST,)), (contract.HTTP_TEST,)), True))
    contract.validate(value)
    return value


def fixture_receipt():
    data = (ROOT / 'test-evidence/native-account-profile-20261018/public_native_profile/cleanup.json').read_bytes()
    assert hashlib.sha256(data).hexdigest() == '70676f2f5a325eeae2c1a12c25adc3de1da1bb8634831a10ce9ad71b153b31cb'
    return json.loads(data, object_pairs_hook=contract.unique_object)


class NativeTitleGate(unittest.TestCase):
    def fake_artifacts(self, temporary):
        root = Path(temporary) / 'weebdex-platform'
        root.mkdir()
        for relative in contract.CORE_HASHES:
            destination = root / relative
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(ROOT / relative, destination)
        for relative in contract.TOOL_PATHS.values():
            destination = root / relative
            destination.parent.mkdir(parents=True, exist_ok=True)
            destination.write_text('explicit finite native-tool stand-in; never executed\n')
            destination.chmod(0o700)
        target = root / '.build/hosted-native/debug/deps'
        target.mkdir(parents=True)
        rows = []
        for name, kind in [('weebdex_platform', 'lib'), ('public_manga_writes', 'test')]:
            path = target / (name + '-0123456789abcdef')
            path.write_text('explicit finite compiled-test stand-in; never executed\n')
            path.chmod(0o700)
            rows.append({'reason': 'compiler-artifact', 'target': {'name': name, 'kind': [kind]},
                         'profile': {'test': True}, 'executable': str(path)})
        rows.append({'reason': 'build-finished', 'success': True})
        encoded = '\n'.join(json.dumps(row) for row in rows)
        binding = contract.artifacts(encoded, root, '1' * 64)
        return root, rows, binding

    def test_exact_three_real_unit_names_and_seven_request_source_are_pinned(self):
        contract.verify_core(ROOT)
        source = (ROOT / 'backend/src/public_api/title_disposition.rs').read_text()
        self.assertEqual(len(contract.UNIT_TESTS), 3)
        for name in contract.UNIT_TESTS:
            self.assertIn('fn ' + name.split('::')[-1] + '(', source)
        self.assertIn('tests::disposition_requires_exact_version_account_and_removal_effect', contract.UNIT_TESTS[-1])
        scenario = (ROOT / 'backend/tests/public_manga_writes.rs').read_text()
        self.assertIn('async fn ' + contract.HTTP_TEST + '()', scenario)
        self.assertIn('title_disposition_lifecycle(&f,&native,event).await;', scenario)
        loop = scenario.split('for (method,target,body) in [', 1)[1].split('] {', 1)[0]
        self.assertEqual(loop.count('("POST",'), 4)
        self.assertEqual(loop.count('("DELETE",'), 3)
        self.assertIn('Some(authorization),400).await;', scenario)

    def test_harness_requires_exact_names_counts_and_one_terminal_record(self):
        text = test_text(contract.UNIT_TESTS)
        self.assertEqual(contract.parse_tests(text, contract.UNIT_TESTS)['passed_tests'], 3)
        bad = ['', text + text, text.replace('running 3 tests', 'running 0 tests'),
               text.replace(contract.UNIT_TESTS[0], 'other::test'),
               text.replace(contract.UNIT_TESTS[0], contract.UNIT_TESTS[1]),
               text.replace('3 passed', '2 passed'),
               text.replace('0 measured', '1 measured'),
               text.replace(' ... ok', ' ... ignored', 1)]
        for raw in bad:
            with self.subTest(raw=raw), self.assertRaises(ValueError):
                contract.parse_tests(raw, contract.UNIT_TESTS)
        ignored = text.replace(' ... ok', ' ... ignored', 1).replace('3 passed; 0 failed; 0 ignored', '2 passed; 0 failed; 1 ignored')
        stage = contract.project_stage(owned(), contract.parse_tests(ignored, contract.UNIT_TESTS))
        self.assertFalse(contract.stage_passed(stage, 3))

    def test_closed_title_result_requires_all_units_http_fixture_and_source(self):
        baseline = title_pass()
        for key in contract.RESULT_FIELDS:
            broken = copy.deepcopy(baseline)
            del broken[key]
            with self.subTest(missing=key), self.assertRaises(ValueError):
                contract.validate(broken)
        changes = [('source_verified', False), ('artifacts_verified', False), ('budget_exhausted', True),
                   ('negative_requests_verified', 0), ('negative_requests_verified', True), ('private', 'secret')]
        for field, replacement in changes:
            with self.subTest(field=field), self.assertRaises(ValueError):
                contract.validate(dict(baseline, **{field: replacement}))
        for name in ['units', 'http']:
            for field, replacement in [('process_exit_code', 1), ('process_exit_code', False),
                    ('deadline_exceeded', True), ('owned_terminal', False), ('cleanup_verified', False),
                    ('exact_tests_verified', False), ('passed_tests', 0), ('passed_tests', True),
                    ('ignored_tests', 1), ('private', 'secret')]:
                value = copy.deepcopy(baseline)
                value[name][field] = replacement
                with self.subTest(stage=name, field=field), self.assertRaises(ValueError):
                    contract.validate(value)
        value = copy.deepcopy(baseline)
        value['http']['fixture_cleanup_verified'] = False
        with self.assertRaises(ValueError):
            contract.validate(value)
        contract.validate(contract.not_run())
        failed = contract.not_run()
        failed['status'] = 'failed'
        contract.validate(failed)
        self.assertNotIn('secret', json.dumps(title_pass()))

    def test_owned_projection_rejects_missing_false_like_or_dirty_receipts(self):
        observation = contract.parse_tests(test_text(contract.UNIT_TESTS), contract.UNIT_TESTS)
        for changes in [{'process_exit_code': 1}, {'terminal_receipt': 1}, {'cleanup_verified': 'true'},
                        {'surviving_owned_process_ids': [42]}, {'cleanup_signals': ['SIGKILL']},
                        {'prior_or_unrelated_processes_touched': True}, {'deadline_exceeded': True}]:
            self.assertFalse(contract.stage_passed(contract.project_stage(dict(owned(), **changes), observation), 3))
        for deadline in [None, 0, '', 'false']:
            with self.assertRaises(ValueError):
                contract.project_stage(dict(owned(), deadline_exceeded=deadline), observation)
        private = dict(owned(), command=['secret'], cwd='secret', raw='secret')
        self.assertNotIn('secret', json.dumps(contract.project_stage(private, observation)))

    def test_cargo_selects_exact_actual_artifacts_and_all_native_tools(self):
        with tempfile.TemporaryDirectory() as temporary:
            root, rows, binding = self.fake_artifacts(temporary)
            executables = contract.verify_artifacts(binding, root, '1' * 64)
            self.assertEqual(set(executables), {'library', 'http'})
            self.assertEqual(set(binding['tools']), set(contract.TOOL_PATHS))
            for malformed in [rows[:-1], rows + [rows[0]], rows + [rows[-1]],
                              [*rows[:-1], {'reason': 'build-finished', 'success': False}],
                              [rows[0], rows[-1]]]:
                with self.assertRaises(ValueError):
                    contract.artifacts('\n'.join(json.dumps(row) for row in malformed), root, '1' * 64)
            for field in ['source_snapshot_sha256', 'executables', 'tools']:
                changed = copy.deepcopy(binding)
                del changed[field]
                with self.assertRaises(ValueError):
                    contract.verify_artifacts(changed, root, '1' * 64)
            with self.assertRaises(ValueError):
                contract.verify_artifacts(binding, root, '2' * 64)
            executables['library'].write_text('changed executable')
            with self.assertRaises(ValueError):
                contract.verify_artifacts(binding, root, '1' * 64)

    def test_artifacts_reject_symlinks_outside_targets_and_changed_source_or_tools(self):
        for mode in ['link', 'outside', 'source', 'tool', 'nonexecutable']:
            with self.subTest(mode=mode), tempfile.TemporaryDirectory() as temporary:
                root, rows, binding = self.fake_artifacts(temporary)
                path = Path(binding['executables']['library']['path'])
                if mode == 'link':
                    other = path.with_name('finite-neighbor')
                    path.rename(other)
                    path.symlink_to(other)
                elif mode == 'outside':
                    binding['executables']['library']['path'] = str(root / 'outside')
                elif mode == 'source':
                    (root / next(iter(contract.CORE_HASHES))).write_text('changed source')
                elif mode == 'tool':
                    (root / contract.TOOL_PATHS['strfry']).write_text('changed tool')
                else:
                    path.chmod(0o600)
                with self.assertRaises(ValueError):
                    contract.verify_artifacts(binding, root, '1' * 64)

    def run_standin(self, temporary, *, deadline=600, mode=None):
        root, _, binding = self.fake_artifacts(temporary)
        stage = root / '.build/hosted-browser'
        stage.mkdir(parents=True)
        (stage / 'plan.json').write_text(json.dumps({'native_title_tests': binding}))
        calls = []
        def run(argv, **kwargs):
            self.assertEqual(kwargs['stdout'], adapter.subprocess.DEVNULL)
            self.assertEqual(kwargs['stderr'], adapter.subprocess.DEVNULL)
            command = argv[argv.index('--') + 1:]
            calls.append(command)
            receipt = Path(argv[argv.index('--receipt') + 1])
            log = Path(argv[argv.index('--log') + 1])
            is_http = command[0] == 'bash'
            names = (contract.HTTP_TEST,) if is_http else contract.UNIT_TESTS
            text = test_text(names)
            if mode == 'zero-units' and not is_http:
                text = 'running 0 tests\ntest result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s\n'
            log.write_text(text)
            receipt.write_text(json.dumps(dict(owned(), **({'cleanup_signals': ['SIGKILL']} if mode == 'forced-unit' and not is_http else {}))))
            if is_http:
                self.assertEqual(command[2], binding['executables']['http']['path'])
                self.assertEqual(command[3:], [contract.HTTP_TEST, '--exact', '--ignored', '--test-threads=1'])
                self.assertTrue(Path(kwargs['env']['STRFRY_BIN']).is_absolute())
                for number in range(2 if mode == 'two-fixtures' else 1):
                    directory = root / 'test-runtime' / ('auth-finite-' + str(number))
                    directory.mkdir(parents=True)
                    fixture = fixture_receipt()
                    if mode == 'dirty-fixture':
                        fixture['unconfirmed_births'] = [fixture['captured_births'][0]]
                    (directory / 'cleanup.json').write_text(json.dumps(fixture))
            if mode == 'source-change' and not is_http:
                (root / next(iter(contract.CORE_HASHES))).write_text('source changed during unit stand-in')
            return SimpleNamespace(returncode=0)
        def verify():
            contract.verify_core(root)
        with mock.patch.object(adapter.subprocess, 'run', side_effect=run), mock.patch.object(adapter.time, 'monotonic', return_value=0):
            value = adapter.run(root, stage, {'RUSTUP_TOOLCHAIN': '1.90.0'}, deadline, '1' * 64, verify, feed.fixture_cleanup_verified)
        return value, calls, root

    def test_adapter_runs_only_exact_three_units_and_one_existing_guarded_scenario(self):
        with tempfile.TemporaryDirectory() as temporary:
            value, calls, _ = self.run_standin(temporary)
            self.assertEqual(value['status'], 'passed')
            self.assertEqual(value['negative_requests_verified'], 7)
            self.assertEqual(len(calls), 2)
            self.assertEqual(calls[0][1:], [contract.UNIT_PREFIX, '--test-threads=1'])
            self.assertEqual(calls[1][0], 'bash')
            self.assertNotIn('cargo', calls[1])

    def test_adapter_refuses_zero_units_unclean_cleanup_changed_source_and_ambiguous_fixture(self):
        for mode in ['zero-units', 'forced-unit', 'source-change', 'two-fixtures', 'dirty-fixture']:
            with self.subTest(mode=mode), tempfile.TemporaryDirectory() as temporary:
                value, calls, _ = self.run_standin(temporary, mode=mode)
                self.assertEqual(value['status'], 'failed')
                self.assertEqual(value['negative_requests_verified'], 0)
                if mode in ['zero-units', 'forced-unit', 'source-change']:
                    self.assertEqual(len(calls), 1)

    def test_budget_refusal_precedes_unit_or_new_database_launch(self):
        for seconds, calls_expected in [(29, 0), (100, 1)]:
            with self.subTest(seconds=seconds), tempfile.TemporaryDirectory() as temporary:
                value, calls, _ = self.run_standin(temporary, deadline=seconds)
                self.assertEqual(value['status'], 'failed')
                self.assertTrue(value['budget_exhausted'])
                self.assertEqual(len(calls), calls_expected)
                self.assertFalse(value['http']['process_started'])

    def test_existing_native_contract_requires_title_pass_without_new_report_or_import_plumbing(self):
        measurement = {'scenario': 'compact-feed505-plus10000-metadata', 'baseline_outcome': 'query-canceled',
                       'baseline_ms': 5001, 'batched_first_page_ms': 42, **{key: True for key in feed.FLAGS}}
        self.assertEqual(feed.project(owned(), measurement, fixture_receipt())['status'], 'failed')
        value = feed.project(owned(), measurement, fixture_receipt(), title_pass())
        self.assertEqual(value['status'], 'passed')
        del value['title_disposition']
        with self.assertRaises(ValueError):
            feed.validate(value)
        validator = load('title_finite_schema', 'scripts/ci/validate-ci-evidence.py')
        self.assertIn('native_feed.validate', (ROOT / 'scripts/ci/validate-ci-evidence.py').read_text())
        self.assertIn('native_feed.validate', (ROOT / 'scripts/ci/run-hosted-browser.py').read_text())

    def test_public_hashes_must_match_actual_bound_executables_before_upload(self):
        validator = load('title_bound_public_schema', 'scripts/ci/validate-ci-evidence.py')
        with tempfile.TemporaryDirectory() as temporary:
            root, _, binding = self.fake_artifacts(temporary)
            title = title_pass()
            title['executable_sha256'] = {kind: entry['sha256'] for kind, entry in binding['executables'].items()}
            prep = {'source_snapshot_sha256': '1' * 64,
                    'native_feed_regression': {'title_disposition': title}}
            source = SimpleNamespace(sha=contract.sha)
            with mock.patch.object(validator, 'ROOT', root), mock.patch.object(validator.runner, 'read', return_value={'native_title_tests': binding}):
                validator.title_artifact_hashes(prep, source)
                prep['native_feed_regression']['title_disposition']['executable_sha256']['library'] = 'f' * 64
                with self.assertRaises(ValueError):
                    validator.title_artifact_hashes(prep, source)
        for hashes in [{'library': '1' * 64}, {'library': '1' * 64, 'http': '2' * 64, 'private': 'secret'},
                       {'library': 'secret', 'http': '2' * 64}, {'library': None, 'http': None}]:
            with self.assertRaises(ValueError):
                contract.validate(dict(title_pass(), executable_sha256=hashes))

    def test_precompile_shares_existing_native_build_and_entire_gate_shares_600_seconds(self):
        prep = (ROOT / 'scripts/ci/prepare-hosted-browser.py').read_text()
        self.assertIn("remaining = seconds - (time.monotonic() - started)", prep)
        self.assertIn("'--lib', '--test',", prep)
        self.assertIn("'public_manga_writes', '--no-run', '--message-format=json', '-j2'", prep)
        self.assertIn("'native-title-test-build', ['cargo', 'test', '--offline', '--locked'", prep)
        self.assertIn("'.github/workflows/hosted-browser-e2e.yml'", prep)
        runner = (ROOT / 'scripts/ci/run-hosted-feed-regression.py').read_text()
        self.assertIn('SHARED_SECONDS = 600', runner)
        self.assertIn('time.monotonic() + 305 > deadline', runner)
        self.assertLess(runner.index('title_runner.run('), runner.index("'bash', str(ROOT / 'scripts/run-feed-projection-tests.sh')"))
        workflow = (ROOT.parent / '.github/workflows/hosted-browser-e2e.yml').read_text()
        for minutes in [140, 90, 12, 30]:
            self.assertIn('timeout-minutes: ' + str(minutes), workflow)
        self.assertEqual(workflow.count('ci-sanitized-evidence/'), 3)


if __name__ == '__main__':
    unittest.main()
