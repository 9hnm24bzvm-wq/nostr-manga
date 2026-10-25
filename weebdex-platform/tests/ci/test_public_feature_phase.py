"""Finite phase scheduling/projection controls; no browser, native or DB starts."""
import copy
import ast
import importlib.util
import json
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[2]


def load(name, relative):
    spec = importlib.util.spec_from_file_location(name, ROOT / relative)
    value = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(value)
    return value


runner = load('feature_phase_runner', 'scripts/ci/run-hosted-browser.py')
features = load('feature_fixed_projection', 'scripts/ci/public-feature-diagnostics.py')
validator = load('feature_outer_contract', 'scripts/ci/validate-ci-evidence.py')


def accepted_report(count=6, failed=()):
    checks = [{'id': identity, 'status': 'failed' if index in failed else 'passed'}
              for index, identity in enumerate(features.CHECKS[:count])]
    for index, row in enumerate(checks[:2]):
        row.update(stage='open-settings' if index in failed and index == 0 else 'verify-owned-state' if index in failed else 'complete', category='check' if index in failed else 'none')
    return {'phase': features.PHASE, 'passed': count == 6 and not failed,
            'completed_utc': 'finite-finalized',
            'steps': [{'name': 'Feature ' + row['id'], 'status': row['status']} for row in checks],
            'page_errors': [],
            'cleanup': {'browser_created': True, 'context_created': True, 'page_created': True, 'context_closed': True},
            'runner_lifecycle': {'stage': 'complete', 'fatal_kind': 'none', 'fatal_code': 'none',
                'fatal_stage': 'none', 'caught_stage': 'none', 'caught_category': 'none',
                'exit_observed': True, 'exit_code': 0},
            'proxy_lifecycle': {'connect_denials': 0, 'expected_connect_resets': 0,
                'remaining_socket_count': 0, 'observations_truncated': False,
                'shutdown_started': True, 'closed': True},
            'public_features': {'schema': 1, 'scope': features.SCOPE, 'checks': checks,
                'completed': count == 6, 'actual_context_closed': True}}


def owned():
    return {'fixture_only': True, 'phase': features.PHASE, 'verified_stopped': True,
            'forced': False, 'returncode': 0}


def projected(report, terminal=None):
    return runner.project_phase(features.PHASE, 'public-upload', report, terminal or owned())


def command_admission():
    # Execute only the exact pure admission function. Importing the fixture or
    # instantiating its supervisor would expand this finite test unnecessarily.
    source = (ROOT / 'tests/runtime/browser_fixture.py').read_text()
    tree = ast.parse(source)
    node = next(row for row in tree.body if isinstance(row, ast.FunctionDef) and row.name == 'public_feature_command')
    namespace = {'ROOT': ROOT, 'PUBLIC_FEATURE_PHASE': features.PHASE}
    exec(compile(ast.Module(body=[node], type_ignores=[]), '<actual-pure-feature-admission>', 'exec'), namespace)
    return namespace['public_feature_command'], source


class PublicFeaturePhase(unittest.TestCase):
    def test_feature_precedes_upload_within_unchanged_total_budget(self):
        self.assertEqual(runner.BATCHES['public-upload'],
                         ['public-feature-recovery', 'public-upload-recovery'])
        self.assertEqual(len(runner.ALL_PHASES), 42)
        self.assertEqual(runner.MAX_PHASE_RUNS, 54)
        self.assertEqual(runner.maximum('public-feature-recovery'), 180)
        reserve = sum(runner.maximum(phase) + 35 for phase in runner.BATCHES['public-upload']) + 300 + 600
        self.assertEqual(reserve, 1750)
        self.assertLess(reserve, 1800)

    def test_exact_full_set_binds_names_statuses_and_actual_context_closure(self):
        report = accepted_report()
        value = features.project(report)
        features.validate(value)
        self.assertTrue(features.passed(value))
        phase = projected(report)
        self.assertEqual(phase['status'], 'passed')
        validator.phases({'schema': 1, 'phases': [phase], 'batches': []})
        self.assertEqual([row['id'] for row in value['checks']], list(features.CHECKS))

    def test_every_completed_prefix_is_visible_but_cannot_claim_phase_success(self):
        for count in range(6):
            with self.subTest(count=count):
                report = accepted_report(count)
                # Even an incorrect generic passed flag cannot promote a prefix.
                report['passed'] = True
                value = features.project(report)
                self.assertTrue(value['record_valid'])
                self.assertEqual(len(value['checks']), count)
                self.assertFalse(features.passed(value))
                phase = projected(report)
                self.assertEqual(phase['status'], 'failed')
                validator.phases({'schema': 1, 'phases': [phase], 'batches': []})

    def test_an_interrupted_next_step_preserves_only_its_settled_prefix(self):
        for count in range(6):
            report = accepted_report(count, failed=(0,) if count else ())
            report['steps'].append({'name': 'Feature ' + features.CHECKS[count]})
            value = features.project(report)
            self.assertTrue(value['record_valid'])
            self.assertEqual(len(value['checks']), count)
            phase = projected(report)
            self.assertEqual(phase['failed_steps'][-1], {'index': count, 'status': 'unresolved'})
            validator.phases({'schema': 1, 'phases': [phase], 'batches': []})

    def test_independent_failures_keep_later_results_without_becoming_a_pass(self):
        for failed in [(0,), (3,), (5,), (0, 2, 5), tuple(range(6))]:
            report = accepted_report(failed=failed)
            value = features.project(report)
            self.assertTrue(value['record_valid'])
            self.assertTrue(value['completed'])
            self.assertEqual(len(value['checks']), 6)
            self.assertFalse(features.passed(value))
            phase = projected(report)
            self.assertEqual(phase['status'], 'failed')
            validator.phases({'schema': 1, 'phases': [phase], 'batches': []})

    def test_raw_unknown_malformed_promoted_or_reordered_facts_are_unverified(self):
        changes = [
            lambda r: r.pop('public_features'),
            lambda r: r.update(phase=runner.PUBLIC_UPLOAD_PHASE),
            lambda r: r['public_features'].update(schema=True),
            lambda r: r['public_features'].update(scope='private-user-text'),
            lambda r: r['public_features'].update(secret='private-user-text'),
            lambda r: r['public_features'].update(completed=1),
            lambda r: r['public_features'].update(actual_context_closed=True) or r['cleanup'].update(context_closed=False),
            lambda r: r['public_features']['checks'][0].update(id='unknown-private-user-text'),
            lambda r: r['public_features']['checks'][0].update(status='running'),
            lambda r: r['public_features']['checks'][0].update(error='private-user-text'),
            lambda r: r['public_features']['checks'].reverse(),
            lambda r: r['public_features']['checks'].append(copy.deepcopy(r['public_features']['checks'][0])),
            lambda r: r['steps'][0].update(name='Other journey'),
            lambda r: r['steps'][0].update(status='failed'),
            lambda r: r['steps'].pop(),
            lambda r: r['steps'].append({'name': 'Feature extra', 'status': 'passed'}),
            lambda r: r['cleanup'].pop('context_closed'),
        ]
        for change in changes:
            report = accepted_report()
            change(report)
            value = features.project(report)
            self.assertEqual(value, features.empty())
            self.assertNotIn('private-user-text', json.dumps(value))
            features.validate(value)
        report = accepted_report(5)
        report['public_features']['completed'] = True
        self.assertEqual(features.project(report), features.empty())

    def test_generic_health_and_owned_cleanup_gates_remain_required(self):
        report_changes = [lambda r: r.update(passed=False), lambda r: r.update(harness_error='private'),
            lambda r: r.update(page_errors=[{'error': 'private'}]),
            lambda r: r['runner_lifecycle'].update(exit_code=1),
            lambda r: r['proxy_lifecycle'].update(closed=False),
            lambda r: r['public_features'].update(actual_context_closed=False) or r['cleanup'].update(context_closed=False)]
        for change in report_changes:
            report = accepted_report()
            change(report)
            self.assertEqual(projected(report)['status'], 'failed')
        for patch in [{'returncode': 1}, {'verified_stopped': False}, {'forced': True}, {'phase': runner.PUBLIC_UPLOAD_PHASE}]:
            self.assertEqual(projected(accepted_report(), dict(owned(), **patch))['status'], 'failed')

    def test_outer_contract_rejects_missing_foreign_promoted_or_status_mismatched_diagnostics(self):
        good = projected(accepted_report())
        changes = [lambda p: p.pop('public_feature_diagnostics'),
            lambda p: p.update(batch='ordinary-a'),
            lambda p: p['public_feature_diagnostics'].update(completed=False),
            lambda p: p['public_feature_diagnostics'].update(record_valid=False),
            lambda p: p['public_feature_diagnostics']['checks'][0].update(status='failed'),
            lambda p: p['public_feature_diagnostics'].update(private_values_omitted=False),
            lambda p: p['public_feature_diagnostics'].update(raw_messages_exported=True),
            lambda p: p['public_feature_diagnostics'].update(extra='private')]
        for change in changes:
            phase = copy.deepcopy(good)
            change(phase)
            with self.assertRaises(ValueError):
                validator.phases({'schema': 1, 'phases': [phase], 'batches': []})
        phase = copy.deepcopy(good)
        phase.update(phase=runner.PUBLIC_UPLOAD_PHASE)
        with self.assertRaises(ValueError):
            validator.phases({'schema': 1, 'phases': [phase], 'batches': []})

    def test_upload_failure_cannot_erase_prior_feature_results_or_green_the_batch(self):
        first = projected(accepted_report())
        second = dict(first, phase=runner.PUBLIC_UPLOAD_PHASE, status='failed')
        clean = {'passed': False, 'supervisor_terminal': True, 'supervisor_exit_zero': False,
                 'cleanup': {'verified_stopped': True, 'lifecycle_clean': False}}
        summary = runner.summarize({'phases': [first, second], 'batches': [clean]},
                                   'public-upload', runner.BATCHES['public-upload'])
        self.assertEqual(summary['passed_unique_phases'], [features.PHASE])
        self.assertEqual(summary['failed_unique_phases'], [runner.PUBLIC_UPLOAD_PHASE])
        self.assertFalse(summary['selected_scope_passed'])
        self.assertEqual(len(summary['pending_unique_phases']), 40)
        validator.summary(summary)

    def test_fail_stop_remains_after_each_projected_phase(self):
        source = (ROOT / 'scripts/ci/run-hosted-browser.py').read_text()
        self.assertIn("if row['status'] != 'passed':\n                    break", source)
        self.assertEqual(runner.batch_phases('public-upload', runner.BATCHES['public-upload']),
                         [features.PHASE, runner.PUBLIC_UPLOAD_PHASE])

    def test_backup_stage_category_allowlists_are_case_specific_and_status_bound(self):
        for index, stages in enumerate(features.BACKUP_STAGES):
            for stage in stages:
                report = accepted_report(failed=(index,))
                row = report['public_features']['checks'][index]
                row.update(stage=stage, category='prerequisite' if stage == 'merge-prerequisite' else 'operation')
                if stage in features.REVIEW_STAGES:
                    row['ui_error'] = 'unavailable'
                value = features.project(report)
                self.assertTrue(value['record_valid'])
                self.assertFalse(features.passed(value))
                features.validate(value)
        report = accepted_report(failed=(0, 1))
        report['public_features']['checks'][1].update(stage='merge-prerequisite', category='prerequisite')
        value = features.project(report)
        self.assertEqual(value['checks'][1]['category'], 'prerequisite')
        self.assertNotEqual(value['checks'][0]['category'], 'prerequisite')

    def test_review_ui_observation_requires_fixed_failure_only_state(self):
        for index in (0, 1):
            for stage in features.BACKUP_STAGES[index] & features.REVIEW_STAGES:
                for state in features.UI_ERROR_STATES:
                    row = {'id': features.CHECKS[index], 'status': 'failed', 'stage': stage, 'category': 'timeout', 'ui_error': state}
                    self.assertTrue(features.check_row(row, index))
                    for invalid in ('private error text', True, {}, None):
                        self.assertFalse(features.check_row(dict(row, ui_error=invalid), index))
                    self.assertFalse(features.check_row({key: value for key, value in row.items() if key != 'ui_error'}, index))
                    self.assertFalse(features.check_row(dict(row, status='passed'), index))
        self.assertFalse(features.check_row({'id': features.CHECKS[0], 'status': 'failed', 'stage': 'open-settings', 'category': 'operation', 'ui_error': 'visible'}, 0))

    def test_backup_diagnostics_reject_raw_values_cross_case_stages_and_false_success(self):
        changes = [
            lambda r: r['public_features']['checks'][0].pop('stage'),
            lambda r: r['public_features']['checks'][0].update(stage='https://private.example/key'),
            lambda r: r['public_features']['checks'][0].update(category='secret'),
            lambda r: r['public_features']['checks'][0].update(stage='confirm-replacement'),
            lambda r: r['public_features']['checks'][0].update(stage='merge-prerequisite', category='prerequisite'),
            lambda r: r['public_features']['checks'][0].update(stage='complete', category='none'),
            lambda r: r['public_features']['checks'][0].update(stage='initialize-device', category='none'),
            lambda r: r['public_features']['checks'][0].update(category=True),
            lambda r: r['public_features']['checks'][0].update(error='secret'),
            lambda r: r['public_features']['checks'][2].update(stage='complete', category='none'),
        ]
        for change in changes:
            report = accepted_report(failed=(0,))
            change(report)
            self.assertEqual(features.project(report), features.empty())
        for patch in [{'stage': 'initialize-device'}, {'category': 'check'}]:
            report = accepted_report()
            report['public_features']['checks'][0].update(patch)
            self.assertEqual(features.project(report), features.empty())
        # Old status-only rows cannot silently become new diagnostic evidence.
        report = accepted_report(failed=(0,))
        for row in report['public_features']['checks'][:2]:
            del row['stage']
            del row['category']
        self.assertEqual(features.project(report), features.empty())

    def test_fixture_requires_exact_opt_in_script_phase_and_180_second_cap(self):
        admit, source = command_admission()
        args = [features.PHASE, {'BROWSER_PHASE': features.PHASE}, ROOT / 'scripts/browser-acceptance.mjs', 180, True]
        self.assertTrue(admit(*args))
        self.assertFalse(admit(runner.PUBLIC_UPLOAD_PHASE, {'BROWSER_PHASE': runner.PUBLIC_UPLOAD_PHASE}, args[2], 600, True))
        for index, bad in [(0, 'ordinary'), (1, {'BROWSER_PHASE': 'ordinary'}), (2, ROOT / 'tests/ui/other.mjs'),
                           (3, 181), (3, 14), (3, True), (3, '180'), (4, False)]:
            changed = list(args)
            changed[index] = bad
            with self.assertRaises(ValueError):
                admit(*changed)
        self.assertIn('feature_phase=public_feature_command(phase,options,script,timeout,self.public_upload_controls)', source)
        self.assertIn('(180 if feature_phase else 600 if signed_control_phase else 300)', source)


if __name__ == '__main__':
    unittest.main()
