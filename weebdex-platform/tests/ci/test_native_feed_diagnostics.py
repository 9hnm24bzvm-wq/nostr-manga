"""Closed result and owned-command seams; no native database or browser run."""
import importlib.util
import ast
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


contract = load('native_feed_seam', ROOT / 'scripts/ci/native-feed-diagnostics.py')


class NativeFeedDiagnostics(unittest.TestCase):
    def title(self):
        # Explicit finite title-gate stand-in; no native unit/HTTP execution.
        value = contract.title.not_run()
        observation = lambda count: {'passed_tests': count, 'failed_tests': 0,
            'ignored_tests': 0, 'exact_tests_verified': True}
        value.update(status='passed', source_verified=True, artifacts_verified=True,
            executable_sha256={'library': '1' * 64, 'http': '2' * 64},
            units=contract.title.project_stage(self.owned(), observation(3)),
            http=contract.title.project_stage(self.owned(), observation(1), True),
            negative_requests_verified=7)
        value['fixture_diagnostics'] = contract.title.fixture_diagnostics(self.fixture(), True)
        contract.title.validate(value)
        return value

    def project(self, owned, observed, fixture):
        return contract.project(owned, observed, fixture, self.title())

    def measurement(self):
        return {'scenario': 'compact-feed505-plus10000-metadata', 'baseline_outcome': 'query-canceled',
                'baseline_ms': 5001, 'batched_first_page_ms': 42,
                **{key: True for key in contract.FLAGS}}

    def owned(self):
        return {'process_exit_code': 0, 'deadline_exceeded': False, 'terminal_receipt': True,
                'cleanup_verified': True, 'surviving_owned_process_ids': [], 'cleanup_signals': [],
                'prior_or_unrelated_processes_touched': False}

    def fixture(self):
        # Unchanged retained output of the actual auth_fixture_guard producer,
        # not a reconstructed two-field stand-in or a fresh PostgreSQL run.
        path = ROOT / 'test-evidence/native-account-profile-20261018/public_native_profile/cleanup.json'
        content = path.read_bytes()
        self.assertEqual(hashlib.sha256(content).hexdigest(), '70676f2f5a325eeae2c1a12c25adc3de1da1bb8634831a10ce9ad71b153b31cb')
        return json.loads(content, object_pairs_hook=contract.unique_object)

    def test_actual_retained_producer_receipt_matches_closed_schema_and_stays_private(self):
        fixture = self.fixture()
        self.assertEqual(set(fixture), contract.FIXTURE_FIELDS)
        self.assertTrue(contract.fixture_cleanup_verified(fixture))
        value = self.project(self.owned(), self.measurement(), fixture)
        self.assertEqual(value['status'], 'passed')
        self.assertEqual(set(value), contract.RESULT_FIELDS)
        for private in ['captured_births', 'postgres_returncode', 'cleanup_errors', 'start_ticks']:
            self.assertNotIn(private, json.dumps(value))
        # Bind exact declared producer fields and policy without running its
        # process owner, creating a database or opening a socket.
        source = ast.parse((ROOT / 'tests/runtime/auth_fixture_guard.py').read_text())
        receipts = [node.value for node in ast.walk(source) if isinstance(node, ast.Assign)
                    and any(isinstance(target, ast.Name) and target.id == 'receipt' for target in node.targets)]
        self.assertEqual(len(receipts), 1)
        self.assertIsInstance(receipts[0], ast.Call)
        self.assertEqual(receipts[0].func.id, 'dict')
        self.assertEqual({item.arg for item in receipts[0].keywords}, contract.FIXTURE_FIELDS)
        constants = {target.id: node.value for node in source.body if isinstance(node, ast.Assign)
                     for target in node.targets if isinstance(target, ast.Name)}
        self.assertEqual([ast.literal_eval(item.elts[1]) for item in constants['STOP_PHASES'].elts], [15, 5, 5])
        timeout = next(node for node in source.body if isinstance(node, ast.Assign)
                       and isinstance(node.targets[0], ast.Tuple)
                       and [item.id for item in node.targets[0].elts] == ['START_SECONDS', 'TEST_SECONDS'])
        self.assertEqual(ast.literal_eval(timeout.value), (10, 180))

    def test_fixture_requires_every_field_and_rejects_legacy_or_extra_keys(self):
        fixture = self.fixture()
        for field in contract.FIXTURE_FIELDS:
            with self.subTest(missing=field):
                missing = dict(fixture)
                del missing[field]
                self.assertFalse(contract.fixture_cleanup_verified(missing))
                self.assertEqual(self.project(self.owned(), self.measurement(), missing)['status'], 'failed')
        for malformed in [None, [], '', 0, True, {'postgres_cleanup': 'verified_stopped', 'exit': 0},
                          dict(fixture, raw='private')]:
            with self.subTest(malformed=malformed):
                self.assertFalse(contract.fixture_cleanup_verified(malformed))

    def test_fixture_refuses_forced_open_unknown_error_and_nonzero_lifecycle(self):
        changes = {'postgres_cleanup': ['cleanup_unconfirmed', None, True],
                   'forced': [True, 0, None, 'false'], 'closed_port': [False, 1, None, 'true'],
                   'error': ['fixture error', '', False, []],
                   'remaining_owned_processes': [[{'pid': 42}], None, {}, False],
                   'cleanup_errors': [['error'], None, {}, False],
                   'unconfirmed_births': [[{'pid': 42}], None, {}, False],
                   'unconfirmed_child_pids': [[42], None, {}, False],
                   'exit': [1, -1, False, 0.0, None, '0'],
                   'test_returncode': [1, -15, False, 0.0, None, '0'],
                   'postgres_returncode': [1, -15, False, 0.0, None, '0'],
                   'test_timeout_seconds': [181, 0, True, 180.0, None],
                   'startup_timeout_seconds': [11, 0, True, 10.0, None],
                   'cleanup_phase_seconds': [[15, 5], [15, 5, 6], [15, 5.0, 5], None, '15,5,5']}
        for field, replacements in changes.items():
            for replacement in replacements:
                with self.subTest(field=field, replacement=replacement):
                    fixture = dict(self.fixture(), **{field: replacement})
                    self.assertFalse(contract.fixture_cleanup_verified(fixture))
                    self.assertEqual(self.project(self.owned(), self.measurement(), fixture)['status'], 'failed')

    def test_fixture_requires_bounded_exact_unique_owned_birth_identities(self):
        first = self.fixture()['captured_births'][0]
        malformed = [None, {}, [], [None], [dict(first, raw='private')], [first, first], [first] * 4097]
        for field in contract.BIRTH_FIELDS:
            missing = dict(first)
            del missing[field]
            malformed.append([missing])
            malformed.extend([[dict(first, **{field: value})] for value in [None, True, 0, -1, 1.0, '1', 1 << 64]])
        for births in malformed:
            with self.subTest(births=births):
                self.assertFalse(contract.fixture_cleanup_verified(dict(self.fixture(), captured_births=births)))
        # Reused numeric PID with a genuinely different captured birth is a
        # distinct identity, not an ownership collision.
        reused = dict(first, start_ticks=first['start_ticks'] + 1)
        self.assertTrue(contract.fixture_cleanup_verified(dict(self.fixture(), captured_births=[first, reused])))

    def test_not_run_and_failed_are_honest_closed_states(self):
        value = contract.not_run()
        contract.validate(value)
        self.assertEqual(value['status'], 'not-run')
        self.assertIsNone(value['measurement'])
        for owned, observed, fixture in [(None, None, None), (self.owned(), None, self.fixture()),
                                          (self.owned(), self.measurement(), None)]:
            with self.subTest(owned=owned, observed=observed, fixture=fixture):
                value = self.project(owned, observed, fixture)
                self.assertEqual(value['status'], 'failed')
                contract.validate(value)

    def test_pass_needs_actual_exit_terminal_cleanup_fixture_and_measurement(self):
        value = self.project(self.owned(), self.measurement(), self.fixture())
        self.assertEqual(value['status'], 'passed')
        for key, replacement in [('process_exit_code', 1), ('deadline_exceeded', True),
                                  ('terminal_receipt', False), ('cleanup_verified', False),
                                  ('surviving_owned_process_ids', [123]), ('cleanup_signals', ['SIGKILL']),
                                  ('prior_or_unrelated_processes_touched', True)]:
            with self.subTest(key=key):
                self.assertEqual(self.project(dict(self.owned(), **{key: replacement}), self.measurement(), self.fixture())['status'], 'failed')
        for fixture in [{'postgres_cleanup': 'failed', 'exit': 0}, {'postgres_cleanup': 'verified_stopped', 'exit': 1},
                        {'postgres_cleanup': 'verified_stopped', 'exit': False}, dict(self.fixture(), raw='private')]:
            self.assertEqual(self.project(self.owned(), self.measurement(), fixture)['status'], 'failed')

    def test_parser_admits_one_exact_retained_rust_record_only(self):
        row = self.measurement()
        encoded = json.dumps(row)
        for prefix in ['', 'test compact_feed_postgres_projection_parity_visibility_and_pagination ... ']:
            self.assertEqual(contract.parse_measurement('private log message\n' + prefix + encoded + '\nother private log'), row)
        for text in ['', encoded + '\n' + encoded, 'different-test ... ' + encoded,
                     encoded[:-1] + ', "scenario":"private"}', json.dumps(dict(row, raw_log='secret'))]:
            with self.subTest(text=text), self.assertRaises(ValueError):
                contract.parse_measurement(text)

    def test_missing_and_nonboolean_deadline_receipts_cannot_pass(self):
        missing = self.owned()
        del missing['deadline_exceeded']
        cases = [missing, *[dict(self.owned(), deadline_exceeded=value) for value in [None, 0, 1, '', 'private', [], {}]]]
        for owned in cases:
            with self.subTest(owned=owned), self.assertRaises(ValueError):
                self.project(owned, self.measurement(), self.fixture())
        self.assertEqual(self.project(dict(self.owned(), deadline_exceeded=True), self.measurement(), self.fixture())['status'], 'failed')
        self.assertEqual(self.project(dict(self.owned(), deadline_exceeded=False), self.measurement(), self.fixture())['status'], 'passed')

    def test_measurement_and_result_reject_unbounded_or_private_values(self):
        for key, value in [('baseline_outcome', 'private'), ('scenario', 'private'), ('baseline_ms', True),
                           ('baseline_ms', -1), ('batched_first_page_ms', 180001), ('all_505_ids_reached', False)]:
            with self.subTest(key=key), self.assertRaises(ValueError):
                contract.measurement(dict(self.measurement(), **{key: value}))
        baseline = self.project(self.owned(), self.measurement(), self.fixture())
        cases = [dict(baseline, raw_log='never-export'), dict(baseline, process_exit_code=True),
                 dict(baseline, status='not-run'), dict(baseline, status='failed'),
                 dict(baseline, process_started=False), dict(baseline, measurement=None),
                 dict(contract.not_run(), process_started=True)]
        for value in cases:
            with self.subTest(value=value), self.assertRaises(ValueError):
                contract.validate(value)
        private = dict(self.owned(), command=['token=private'], cwd='private', raw='private')
        encoded = json.dumps(self.project(private, self.measurement(), self.fixture()))
        self.assertNotIn('private', encoded)
        self.assertNotIn('command', encoded)

    def test_real_short_owned_command_receipt_is_projected_without_its_log(self):
        # This is a real harmless Python process, explicitly not the native test.
        with tempfile.TemporaryDirectory() as temporary:
            temporary = Path(temporary)
            log, receipt = temporary / 'private.log', temporary / 'owned.json'
            result = subprocess.run([sys.executable, str(ROOT / 'scripts/run-bounded.py'), '--cwd', str(temporary),
                '--deadline', '5', '--receipt', str(receipt), '--log', str(log), '--', sys.executable, '-c',
                'print("private stand-in log"); print(' + repr(json.dumps(self.measurement())) + ')'],
                capture_output=True, text=True, timeout=10)
            self.assertEqual(result.returncode, 0)
            value = self.project(json.loads(receipt.read_text()), contract.parse_measurement(log.read_text()), self.fixture())
            self.assertEqual(value['status'], 'passed')
            self.assertNotIn('private', json.dumps(value))

    def test_workflow_and_browser_gate_precede_every_fixture(self):
        workflow = (ROOT.parent / '.github/workflows/hosted-browser-e2e.yml').read_text()
        native = workflow.index('run-hosted-feed-regression.py')
        self.assertLess(workflow.index('prepare-hosted-browser.sh'), native)
        self.assertLess(native, workflow.index('Run genuine browser batches'))
        self.assertIn('timeout-minutes: 12', workflow)
        source = (ROOT / 'scripts/ci/run-hosted-feed-regression.py').read_text()
        self.assertIn("SHARED_SECONDS = 600", source)
        self.assertIn("'--deadline', '300'", source)
        self.assertIn("'bash', str(ROOT / 'scripts/run-feed-projection-tests.sh')", source)
        self.assertIn("RUSTUP_TOOLCHAIN='1.90.0'", source)
        self.assertIn("os.environ.get('GITHUB_ACTIONS') != 'true'", source)
        self.assertIn('stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL', source)
        runner = (ROOT / 'scripts/ci/run-hosted-browser.py').read_text()
        main = runner[runner.index('def main():'):]
        self.assertLess(main.index('native_feed.validate'), main.index('run_scheduled_batches('))
        self.assertEqual(main.count('run_scheduled_batches('), 1)
        self.assertNotIn('run_batch(', main)
        scheduler = runner[runner.index('def run_scheduled_batches('):runner.index('def main():')]
        self.assertEqual(scheduler.count('batch = run_batch('), 1)
        self.assertLess(scheduler.index('fixture_schedule('), scheduler.index('batch = run_batch('))
        self.assertIn("prepared['native_feed_regression']['status'] != 'passed'", main)
        prep = (ROOT / 'scripts/ci/prepare-hosted-browser.py').read_text()
        self.assertIn("'backend/tests'", prep)

    def test_actual_public_schema_enforces_nested_contract(self):
        validator = load('native_feed_public_schema', ROOT / 'scripts/ci/validate-ci-evidence.py')
        baseline = {'schema': 1, 'status': 'prepared', 'browser_phases_started': False, 'stage': 'complete',
                    'source_files_checked': 676, 'frontend_preserved': True, 'artifacts_uploaded': False,
                    'fresh_build': True, 'historical_binaries_reused': False, 'playwright': '1.62.1',
                    'chromium_revision': '1234', 'native_feed_regression': contract.not_run(),
                    **{key: '0' * 64 for key in ['source_snapshot_sha256', 'frontend_manifest_sha256',
                        'frontend_index_sha256', 'normal_binary_sha256', 'provisioner_sha256', 'chromium_binary_sha256']}}
        validator.preparation(baseline)
        validator.preparation(dict(baseline, native_feed_regression=self.project(self.owned(), self.measurement(), self.fixture())))
        for value in [None, {}, dict(contract.not_run(), raw='private'), dict(contract.not_run(), status='passed')]:
            with self.subTest(value=value), self.assertRaises(ValueError):
                validator.preparation(dict(baseline, native_feed_regression=value))


if __name__ == '__main__':
    unittest.main()
