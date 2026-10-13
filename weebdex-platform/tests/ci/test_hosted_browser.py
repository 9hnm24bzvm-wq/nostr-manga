"""Controller/schema checks and short owned OS-process seams; no browser/fixture."""
import importlib.util
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
    def test_exact_original_38_phases(self):
        handoff = json.loads((ROOT / 'test-evidence/frontend/browser-startup-plan-build19.json').read_text())['phases']
        expected = {'startup'} | {phase for group in handoff.values() for phase in group.get('names', [])}
        self.assertEqual(set(runner.ALL_PHASES), expected)
        self.assertEqual(len(expected), 38)

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
        acceptance = {'phase': 'account', 'passed': True, 'steps': [{'status': 'passed'}], 'page_errors': []}
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

    def test_summary_cannot_promote_partial_scope_to_full_suite(self):
        journal = {'phases': [{'phase': 'startup', 'status': 'passed'}], 'batches': [{'passed': True, 'supervisor_terminal': True, 'supervisor_exit_zero': True, 'cleanup': {'verified_stopped': True, 'lifecycle_clean': True}}]}
        summary = runner.summarize(journal, 'startup', ['startup'])
        self.assertTrue(summary['selected_scope_passed'])
        self.assertFalse(summary['full_suite_passed'])
        self.assertEqual(len(summary['pending_unique_phases']), 37)
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
        self.assertIn('branches: [ci/hosted-browser-e2e]', workflow)
        self.assertIn("github.ref == 'refs/heads/ci/hosted-browser-e2e'", workflow)
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
