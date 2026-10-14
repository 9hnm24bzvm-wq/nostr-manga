"""Closed result and owned-command seams; no native database or browser run."""
import importlib.util
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
    def measurement(self):
        return {'scenario': 'compact-feed505-plus10000-metadata', 'baseline_outcome': 'query-canceled',
                'baseline_ms': 5001, 'batched_first_page_ms': 42,
                **{key: True for key in contract.FLAGS}}

    def owned(self):
        return {'process_exit_code': 0, 'deadline_exceeded': False, 'terminal_receipt': True,
                'cleanup_verified': True, 'surviving_owned_process_ids': [], 'cleanup_signals': [],
                'prior_or_unrelated_processes_touched': False}

    def fixture(self):
        return {'postgres_cleanup': 'verified_stopped', 'exit': 0}

    def test_not_run_and_failed_are_honest_closed_states(self):
        value = contract.not_run()
        contract.validate(value)
        self.assertEqual(value['status'], 'not-run')
        self.assertIsNone(value['measurement'])
        for owned, observed, fixture in [(None, None, None), (self.owned(), None, self.fixture()),
                                          (self.owned(), self.measurement(), None)]:
            with self.subTest(owned=owned, observed=observed, fixture=fixture):
                value = contract.project(owned, observed, fixture)
                self.assertEqual(value['status'], 'failed')
                contract.validate(value)

    def test_pass_needs_actual_exit_terminal_cleanup_fixture_and_measurement(self):
        value = contract.project(self.owned(), self.measurement(), self.fixture())
        self.assertEqual(value['status'], 'passed')
        for key, replacement in [('process_exit_code', 1), ('deadline_exceeded', True),
                                  ('terminal_receipt', False), ('cleanup_verified', False),
                                  ('surviving_owned_process_ids', [123]), ('cleanup_signals', ['SIGKILL']),
                                  ('prior_or_unrelated_processes_touched', True)]:
            with self.subTest(key=key):
                self.assertEqual(contract.project(dict(self.owned(), **{key: replacement}), self.measurement(), self.fixture())['status'], 'failed')
        for fixture in [{'postgres_cleanup': 'failed', 'exit': 0}, {'postgres_cleanup': 'verified_stopped', 'exit': 1},
                        {'postgres_cleanup': 'verified_stopped', 'exit': False}, dict(self.fixture(), raw='private')]:
            self.assertEqual(contract.project(self.owned(), self.measurement(), fixture)['status'], 'failed')

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
                contract.project(owned, self.measurement(), self.fixture())
        self.assertEqual(contract.project(dict(self.owned(), deadline_exceeded=True), self.measurement(), self.fixture())['status'], 'failed')
        self.assertEqual(contract.project(dict(self.owned(), deadline_exceeded=False), self.measurement(), self.fixture())['status'], 'passed')

    def test_measurement_and_result_reject_unbounded_or_private_values(self):
        for key, value in [('baseline_outcome', 'private'), ('scenario', 'private'), ('baseline_ms', True),
                           ('baseline_ms', -1), ('batched_first_page_ms', 180001), ('all_505_ids_reached', False)]:
            with self.subTest(key=key), self.assertRaises(ValueError):
                contract.measurement(dict(self.measurement(), **{key: value}))
        baseline = contract.project(self.owned(), self.measurement(), self.fixture())
        cases = [dict(baseline, raw_log='never-export'), dict(baseline, process_exit_code=True),
                 dict(baseline, status='not-run'), dict(baseline, status='failed'),
                 dict(baseline, process_started=False), dict(baseline, measurement=None),
                 dict(contract.not_run(), process_started=True)]
        for value in cases:
            with self.subTest(value=value), self.assertRaises(ValueError):
                contract.validate(value)
        private = dict(self.owned(), command=['token=private'], cwd='private', raw='private')
        encoded = json.dumps(contract.project(private, self.measurement(), self.fixture()))
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
            value = contract.project(json.loads(receipt.read_text()), contract.parse_measurement(log.read_text()), self.fixture())
            self.assertEqual(value['status'], 'passed')
            self.assertNotIn('private', json.dumps(value))

    def test_workflow_and_browser_gate_precede_every_fixture(self):
        workflow = (ROOT.parent / '.github/workflows/hosted-browser-e2e.yml').read_text()
        native = workflow.index('run-hosted-feed-regression.py')
        self.assertLess(workflow.index('prepare-hosted-browser.sh'), native)
        self.assertLess(native, workflow.index('Run genuine browser batches'))
        self.assertIn('timeout-minutes: 12', workflow)
        source = (ROOT / 'scripts/ci/run-hosted-feed-regression.py').read_text()
        self.assertIn("'--deadline', '600'", source)
        self.assertIn("'bash', str(ROOT / 'scripts/run-feed-projection-tests.sh')", source)
        self.assertIn("RUSTUP_TOOLCHAIN='1.90.0'", source)
        self.assertIn("os.environ.get('GITHUB_ACTIONS') != 'true'", source)
        self.assertIn('stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL', source)
        runner = (ROOT / 'scripts/ci/run-hosted-browser.py').read_text()
        main = runner[runner.index('def main():'):]
        self.assertLess(main.index('native_feed.validate'), main.index('batch = run_batch('))
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
        validator.preparation(dict(baseline, native_feed_regression=contract.project(self.owned(), self.measurement(), self.fixture())))
        for value in [None, {}, dict(contract.not_run(), raw='private'), dict(contract.not_run(), status='passed')]:
            with self.subTest(value=value), self.assertRaises(ValueError):
                validator.preparation(dict(baseline, native_feed_regression=value))


if __name__ == '__main__':
    unittest.main()
