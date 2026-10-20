"""Actual scheduler/batch control flow with inert owned process seams; no browser."""
import contextlib
import copy
import importlib.machinery
import importlib.util
import io
import json
from pathlib import Path
import tempfile
import threading
from types import SimpleNamespace
import unittest
from unittest import mock

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location('independent_public_runner', ROOT / 'scripts/ci/run-hosted-browser.py')
runner = importlib.util.module_from_spec(spec)
spec.loader.exec_module(runner)


def cleaned(passed=False):
    return {'batch': 'public-upload', 'readiness': True, 'passed': passed,
            'supervisor_terminal': True, 'supervisor_exit_zero': passed,
            'cleanup': {'verified_stopped': True, 'ports_closed': True, 'keys_removed': True,
                        'remaining_owned_process_count': 0, 'final_teardown_clean': True,
                        'lifecycle_clean': passed, 'pg_fault_activated': False, 'pg_fault_terminal_clean': False},
            'outer_cleanup': {'forced': False, 'verified_stopped': True, 'remaining_owned_process_count': 0}}


class IndependentPublicUpload(unittest.TestCase):
    def schedule(self, first=None, stop=False, elapsed=0):
        journal = {'schema': 1, 'phases': [], 'batches': []}
        calls, now = [], [100.0]
        def run(name, phases, plan, output, run_id, **kwargs):
            calls.append((name, list(phases), kwargs))
            output['phases'].append({'batch': name, 'phase': phases[0], 'status': 'failed' if len(calls) == 1 else 'passed'})
            value = copy.deepcopy(first if len(calls) == 1 and first is not None else cleaned(len(calls) != 1))
            output['batches'].append(value)
            if len(calls) == 1:
                now[0] += elapsed
                runner.STOP = stop
            return value
        with mock.patch.object(runner, 'run_batch', side_effect=run), mock.patch.object(runner, 'write'), \
                mock.patch.object(runner.time, 'monotonic', side_effect=lambda: now[0]), mock.patch.object(runner, 'STOP', False):
            runner.run_scheduled_batches({'public-upload': runner.BATCHES['public-upload']}, {}, journal, 'finite', 4000, 'public-upload')
            summary = runner.summarize(journal, 'public-upload', runner.BATCHES['public-upload'])
        return calls, journal, summary

    def test_two_fresh_slots_have_same_total_1750_second_admission(self):
        slots = runner.fixture_schedule('public-upload', runner.BATCHES['public-upload'])
        self.assertEqual([s[0] for s in slots], ['public-upload-features', 'public-upload-journey'])
        self.assertEqual([s[1] for s in slots], [[runner.PUBLIC_FEATURE_PHASE], [runner.PUBLIC_UPLOAD_PHASE]])
        self.assertEqual([s[2:] for s in slots], [(150, 300), (150, 300)])
        self.assertEqual(sum(runner.fixture_reserve('public-upload', p, r, c) for _, p, r, c in slots), 1750)
        self.assertLess(1750, 30 * 60)
        self.assertEqual(runner.maximum(runner.PUBLIC_FEATURE_PHASE), 180)
        self.assertEqual(runner.maximum(runner.PUBLIC_UPLOAD_PHASE), 600)
        self.assertEqual(runner.MAX_PHASE_RUNS, 53)
        self.assertEqual(runner.MAX_BATCH_RUNS, len(runner.BATCHES) + 1)
        self.assertIn('timeout-minutes: 140', (ROOT.parent / '.github/workflows/hosted-browser-e2e.yml').read_text())
        self.assertIn('timeout-minutes: 30', (ROOT.parent / '.github/workflows/hosted-browser-e2e.yml').read_text())

    def test_failed_feature_then_upload_success_preserves_failure_and_both_outcomes(self):
        calls, journal, summary = self.schedule()
        self.assertEqual(len(calls), 2)
        self.assertNotEqual(calls[0][2]['fixture_slot'], calls[1][2]['fixture_slot'])
        self.assertEqual(summary['failed_unique_phases'], [runner.PUBLIC_FEATURE_PHASE])
        self.assertEqual(summary['passed_unique_phases'], [runner.PUBLIC_UPLOAD_PHASE])
        self.assertFalse(summary['selected_scope_passed'])
        self.assertFalse(summary['full_suite_passed'])
        self.assertFalse(summary['all_fixture_lifecycles_clean'])
        self.assertTrue(summary['all_fixture_teardowns_verified'])
        self.assertEqual(summary['actual_phase_runs'], 2)
        self.assertFalse(journal['batches'][0]['passed'])

    def test_each_uncertain_forced_or_incomplete_cleanup_blocks_second_fixture(self):
        patches = [lambda b: b.update(supervisor_terminal=False),
                   lambda b: b['cleanup'].update(verified_stopped=False),
                   lambda b: b['cleanup'].update(final_teardown_clean=False),
                   lambda b: b['cleanup'].update(ports_closed=False),
                   lambda b: b['cleanup'].update(keys_removed=False),
                   lambda b: b['cleanup'].update(remaining_owned_process_count=1),
                   lambda b: b['outer_cleanup'].update(forced=True),
                   lambda b: b['outer_cleanup'].update(verified_stopped=False),
                   lambda b: b['outer_cleanup'].update(remaining_owned_process_count=1),
                   lambda b: b.update(cleanup_error_class='TimeoutError')]
        for patch in patches:
            value = cleaned(); patch(value)
            self.assertEqual(len(self.schedule(value)[0]), 1)

    def test_interrupt_and_spent_pair_budget_do_not_restart_fresh_budget(self):
        self.assertEqual(len(self.schedule(stop=True)[0]), 1)
        calls, journal, _ = self.schedule(elapsed=666)
        self.assertEqual(len(calls), 1)
        self.assertTrue(journal['budget_exhausted'])
        self.assertEqual(len(self.schedule(elapsed=665)[0]), 2)

    def test_total_pair_reserve_is_required_before_first_fixture(self):
        journal = {'phases': [], 'batches': []}
        with mock.patch.object(runner, 'run_batch') as run, mock.patch.object(runner.time, 'monotonic', return_value=100), mock.patch.object(runner, 'STOP', False):
            runner.run_scheduled_batches({'public-upload': runner.BATCHES['public-upload']}, {}, journal, 'finite', 1849, 'public-upload')
        run.assert_not_called(); self.assertTrue(journal['budget_exhausted'])

    def test_nonpublic_schedule_is_unchanged_and_public_subset_is_not_arbitrary(self):
        self.assertEqual(runner.fixture_schedule('ordinary-a', ['account']), [('ordinary-a', ['account'], 300, 600)])
        for phases in [[], [runner.PUBLIC_UPLOAD_PHASE], list(reversed(runner.BATCHES['public-upload']))]:
            with self.assertRaises(RuntimeError): runner.fixture_schedule('public-upload', phases)

    def execute_actual_batches(self, independent=True, cleanup_delay=0, profile_delay=0):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp); stage = root / '.build/hosted-browser'; stage.mkdir(parents=True)
            events, owners, children, fixture_paths, waits, finishes = [], [], [], [], [], []
            clock = [100.0]
            class Loader:
                def create_module(self, spec): return None
                def exec_module(self, module):
                    def owner():
                        obj = SimpleNamespace(process_ownership_lock=threading.RLock(), register_process=lambda *_: None)
                        owners.append(obj); return obj
                    module.OwnedFixtureProcesses = owner
            actual_spec = runner.importlib.util.spec_from_file_location
            def owned_spec(name, path, *args, **kwargs):
                return importlib.machinery.ModuleSpec(name, Loader()) if name == 'outer_fixture_cleanup' else actual_spec(name, path, *args, **kwargs)
            plan = dict(normal_binary='inert', normal_sha256='a'*64, provisioner='inert', provisioner_sha256='b'*64,
                        frontend_manifest='inert', frontend_manifest_sha256='c'*64, frontend_index_sha256='d'*64, node='node')
            def profile(plan):
                clock[0] += profile_delay
                return plan
            def prepare(owner, slot):
                events.append(('temp', slot)); owner.browser_temp = SimpleNamespace(path=root/slot, admitted=True, check=lambda: None)
            def spawn(argv, **kwargs):
                index = len(children); child = SimpleNamespace(returncode=1 if index == 0 else 0, poll=lambda: 0)
                children.append(child); fixture = root/'test-runtime'/('browser-final-'+str(index)); fixture.mkdir(mode=0o700)
                fixture_paths.append(fixture)
                runner.write(fixture/'supervisor.json', {'status':'ready','run_directory':str(fixture),'protocol_seed':{'fully_ready':True},'binary':{'sha256':plan['normal_sha256']},'frontend':{'artifact_manifest_sha256':plan['frontend_manifest_sha256']}})
                events.append(('spawn', kwargs['env']['TMPDIR'])); return child
            def wait(process, predicate, seconds, owner):
                waits.append(seconds); fixture = fixture_paths[-1]; command = fixture/'browser-command.json'
                if command.exists():
                    payload=json.loads(command.read_text()); phase=payload['phase']; rid=payload['env']['BROWSER_RUN_ID']; events.append(('phase',phase,rid))
                    runner.write(fixture/('browser-phase-'+phase+'.json'), {})
                    runner.write(root/'test-evidence/frontend/browser'/(phase+'-'+rid)/'acceptance.json', {'phase':phase})
                return predicate()
            def finish(owner, process, fixture, batch, seconds=600):
                finishes.append(seconds); batch.update(cleaned(process.returncode == 0)); events.append(('cleanup', str(fixture))); clock[0] += cleanup_delay
            def project(phase, batch, *_): return {'batch':batch,'phase':phase,'status':'failed' if phase==runner.PUBLIC_FEATURE_PHASE else 'passed','actual_steps':6}
            journal={'schema':1,'phases':[],'batches':[]}
            with mock.patch.object(runner,'ROOT',root), mock.patch.object(runner,'STAGE',stage), mock.patch.object(runner,'PUBLIC',root/'evidence'), \
                 mock.patch.object(runner.importlib.util,'spec_from_file_location',side_effect=owned_spec), mock.patch.object(runner,'public_upload_profile',side_effect=profile), \
                 mock.patch.object(runner,'verify_public_upload_ready'), mock.patch.object(runner,'prepare_browser_temp',side_effect=prepare), \
                 mock.patch.object(runner.subprocess,'Popen',side_effect=spawn), mock.patch.object(runner,'wait_for',side_effect=wait), \
                 mock.patch.object(runner,'finish_supervisor',side_effect=finish), mock.patch.object(runner,'finish_browser_temp'), \
                 mock.patch.object(runner,'project_phase',side_effect=project), mock.patch.object(runner,'STOP',False), \
                 mock.patch.object(runner.time,'monotonic',side_effect=lambda:clock[0]), contextlib.redirect_stdout(io.StringIO()):
                if independent:
                    runner.run_scheduled_batches({'public-upload':runner.BATCHES['public-upload']},plan,journal,'finite',runner.time.monotonic()+3600,'public-upload')
                else:
                    # Exact retained old call path: both phases in one fixture.
                    runner.run_batch('public-upload',runner.BATCHES['public-upload'],plan,journal,'finite')
            if cleanup_delay > 300:
                self.assertEqual(len(owners),1)
                self.assertEqual(journal['batches'][0]['cleanup_error_class'],'TimeoutError')
                self.assertFalse(journal['batches'][0]['cleanup']['verified_stopped'])
                self.assertEqual([e[1] for e in events if e[0]=='phase'],[runner.PUBLIC_FEATURE_PHASE])
                return
            if not independent:
                self.assertEqual(len(owners),1)
                self.assertEqual([e[1] for e in events if e[0]=='phase'],[runner.PUBLIC_FEATURE_PHASE])
                self.assertEqual([b['passed'] for b in journal['batches']],[False])
                self.assertEqual(finishes,[600])
                return
            self.assertEqual(len(owners),2); self.assertIsNot(owners[0],owners[1])
            self.assertEqual(len(set(fixture_paths)),2)
            self.assertEqual(len(list(stage.glob('fixture-*.log'))),2)
            self.assertEqual([e[1] for e in events if e[0]=='temp'],['public-upload-features','public-upload-journey'])
            self.assertEqual([e[1] for e in events if e[0]=='phase'],runner.BATCHES['public-upload'])
            self.assertEqual(len({e[2] for e in events if e[0]=='phase'}),2)
            self.assertLess(events.index(('cleanup',str(fixture_paths[0]))),events.index(('spawn',str(root/'public-upload-journey'))))
            self.assertEqual([b['passed'] for b in journal['batches']],[False,True])
            if profile_delay:
                self.assertEqual(finishes[0], 665 - profile_delay - 20)
            self.assertTrue(all(0 < n <= 280 for n in finishes))
            self.assertTrue(all(0 < n <= cap for n,cap in zip(waits,[150,215,150,635])))

    def test_actual_profile_hashing_consumes_fixture_deadline(self):
        self.execute_actual_batches(profile_delay=400)

    def test_actual_two_batch_calls_use_distinct_resources_and_per_execution_verdict(self):
        self.execute_actual_batches()

    def test_actual_late_cleanup_overrides_clean_receipt_and_blocks_second_fixture(self):
        self.execute_actual_batches(cleanup_delay=301)

    def test_actual_cleanup_equal_to_reserved_boundary_can_continue(self):
        self.execute_actual_batches(cleanup_delay=300)

    def test_old_combined_fixture_counterfactual_stops_after_failed_feature(self):
        self.execute_actual_batches(independent=False)


if __name__ == '__main__': unittest.main()
