#!/usr/bin/env python3
"""Closed-schema gate; publish only newly canonicalized validated JSON."""
import importlib.util
import json
from pathlib import Path
import re
import sys

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location('hosted_evidence_contract', ROOT / 'scripts/ci/run-hosted-browser.py')
runner = importlib.util.module_from_spec(spec)
spec.loader.exec_module(runner)
spec = importlib.util.spec_from_file_location('hosted_diagnostic_contract', ROOT / 'scripts/ci/startup-diagnostics.py')
diagnostics = importlib.util.module_from_spec(spec)
spec.loader.exec_module(diagnostics)
spec = importlib.util.spec_from_file_location('hosted_journey_contract', ROOT / 'scripts/ci/journey-diagnostics.py')
journey_diagnostics = importlib.util.module_from_spec(spec)
spec.loader.exec_module(journey_diagnostics)
spec = importlib.util.spec_from_file_location('hosted_runner_contract', ROOT / 'scripts/ci/runner-diagnostics.py')
runner_diagnostics = importlib.util.module_from_spec(spec)
spec.loader.exec_module(runner_diagnostics)
ERRORS = {'RuntimeError', 'TimeoutError', 'InterruptedError', 'FileNotFoundError', 'PermissionError', 'IsADirectoryError', 'NotADirectoryError', 'OSError', 'JSONDecodeError', 'KeyError', 'TypeError', 'ValueError', 'ProcessLookupError', 'ChildProcessError'}
SCOPE = 'actual browser controls and persisted-result assertions; pixel parity remains unverified'


def insist(value):
    if not value:
        raise ValueError('Evidence failed its closed public contract')


def unique_object(pairs):
    value = {}
    for key, item in pairs:
        insist(key not in value)
        value[key] = item
    return value


def fields(value, required, optional=()):
    insist(type(value) is dict and set(required) <= set(value) and set(value) <= set(required) | set(optional))


def integer(value, low=0, high=10000):
    insist(type(value) is int and low <= value <= high)


def boolean(value):
    insist(type(value) is bool)


def digest(value):
    insist(type(value) is str and re.fullmatch('[0-9a-f]{64}', value))


def phase_list(value):
    insist(type(value) is list and len(value) <= 38 and len(set(value)) == len(value) and all(type(item) is str and item in runner.ALL_PHASES for item in value))


def error_fields(value):
    for key in ['error_class', 'cleanup_error_class']:
        if key in value:
            insist(value[key] in ERRORS)


def preparation(value):
    required = {'schema', 'status', 'browser_phases_started', 'stage', 'source_files_checked', 'frontend_preserved', 'artifacts_uploaded'}
    hashes = {'source_snapshot_sha256', 'frontend_manifest_sha256', 'frontend_index_sha256', 'normal_binary_sha256', 'provisioner_sha256', 'chromium_binary_sha256'}
    optional = hashes | {'error_class', 'playwright', 'chromium_revision', 'fresh_build', 'historical_binaries_reused', 'ipc_success_established'}
    fields(value, required, optional)
    insist(value['schema'] == 1 and type(value['schema']) is int)
    insist(value['status'] in ['prepared', 'preparation_failed'])
    insist(value['stage'] in ['source-preservation', 'native-dependencies', 'native-build', 'signer-dependencies', 'frontend-dependencies', 'frontend-build', 'artifact-record', 'browser-runtime-record', 'complete'])
    insist(type(value['source_files_checked']) is int and value['source_files_checked'] in [0, 676])
    for key in ['browser_phases_started', 'frontend_preserved', 'artifacts_uploaded', 'fresh_build', 'historical_binaries_reused', 'ipc_success_established']:
        if key in value:
            boolean(value[key])
    insist(value['browser_phases_started'] is False and value['artifacts_uploaded'] is False and value.get('ipc_success_established', False) is False)
    for key in hashes & set(value):
        digest(value[key])
    if value['status'] == 'prepared':
        insist(hashes <= set(value) and value['source_files_checked'] == 676 and value['frontend_preserved'] is True and value.get('fresh_build') is True and value.get('historical_binaries_reused') is False and value.get('playwright') == '1.62.1' and value.get('chromium_revision') == '1234' and value['stage'] == 'complete')
    else:
        # Failed preparation has no unbound hash-shaped or success-only text.
        # Such fields could otherwise carry private 64-hex signer material.
        insist(set(value) <= required | {'error_class'})
    error_fields(value)


def startup_diagnostics(value, phase):
    fields(value, {'schema', 'check_results', 'private_values_omitted', 'raw_messages_exported', 'observations_truncated', 'page_errors', 'console_errors', 'csp_violations', 'network'}, {'seed_title_readiness', 'seed_title_network'})
    insist(type(value['schema']) is int and value['schema'] == 1)
    insist(value['private_values_omitted'] is True and value['raw_messages_exported'] is False)
    boolean(value['observations_truncated'])
    insist(value['observations_truncated'] or phase['actual_steps'] <= len(diagnostics.CHECKS) and phase['page_error_count'] <= diagnostics.LIMIT)
    checks = value['check_results']
    insist(type(checks) is list and len(checks) == min(phase['actual_steps'], len(diagnostics.CHECKS)))
    for index, row in enumerate(checks):
        fields(row, {'index', 'check_id', 'check_identity_verified', 'status', 'error_class', 'category', 'missing_global'})
        integer(row['index'], index, index)
        insist(row['check_id'] == diagnostics.CHECKS[index] and row['status'] in ['passed', 'failed', 'unresolved'])
        boolean(row['check_identity_verified'])
        diagnostic_error(row)
        if row['status'] == 'passed':
            insist(row['error_class'] == 'None' and row['category'] == 'none' and row['missing_global'] == 'none')
        else:
            insist(row['error_class'] != 'None' and row['category'] != 'none')
    insist([{'index': row['index'], 'status': row['status']} for row in checks if row['status'] != 'passed'] == [row for row in phase['failed_steps'] if row['index'] < len(checks)])
    if len(checks) > 15:
        insist({'seed_title_readiness', 'seed_title_network'} <= set(value))
        if phase['status'] == 'passed':
            insist(value['seed_title_readiness']['record_valid'] is True)
            insist(any(row.get('path') == '/title/SeedManga1' and row.get('resource') == 'document' and row.get('status') == 200 for row in value['seed_title_network']))
    if 'seed_title_readiness' in value:
        probe = value['seed_title_readiness']
        fields(probe, {'schema', 'record_valid', 'original_body_meaningful', 'wait_outcome', 'initial', 'late'})
        insist(type(probe['schema']) is int and probe['schema'] == 1)
        boolean(probe['record_valid'])
        boolean(probe['original_body_meaningful'])
        insist(type(probe['wait_outcome']) is str and probe['wait_outcome'] in diagnostics.READINESS_OUTCOMES)
        for key in ['initial', 'late']:
            fields(probe[key], diagnostics.READINESS_FLAGS)
            for flag in diagnostics.READINESS_FLAGS:
                boolean(probe[key][flag])
        if probe['record_valid']:
            insist(len(checks) > 15 and checks[15]['check_identity_verified'])
            if checks[15]['status'] == 'passed':
                insist(probe['original_body_meaningful'])
            if checks[15]['category'] == 'no-rendered-content':
                insist(probe['original_body_meaningful'] is False)
            insist(probe['wait_outcome'] != 'ready' or probe['original_body_meaningful'] is False and all(probe['late'][key] for key in diagnostics.READINESS_FLAGS))
            insist(probe['wait_outcome'] != 'not-needed' or probe['original_body_meaningful'] is True)
    contracts = {
        'page_errors': {'path', 'error_class', 'category', 'missing_global', 'count'},
        'console_errors': {'path', 'error_class', 'category', 'missing_global', 'count'},
        'csp_violations': {'path', 'directive', 'disposition', 'blocked', 'count'},
        'network': {'path', 'resource', 'status', 'mime', 'failure', 'count'},
    }
    if 'seed_title_network' in value:
        contracts['seed_title_network'] = contracts['network']
    for key, required in contracts.items():
        rows = value[key]
        limit = 64 if key == 'seed_title_network' else diagnostics.LIMIT
        insist(type(rows) is list and len(rows) <= limit)
        for row in rows:
            fields(row, required)
            insist(type(row['path']) is str and row['path'] in diagnostics.PATHS)
            integer(row['count'], 1, limit)
            if key in ['page_errors', 'console_errors']:
                diagnostic_error(row)
                insist(row['error_class'] != 'None' and row['category'] != 'none')
            elif key == 'csp_violations':
                insist(type(row['directive']) is str and row['directive'] in diagnostics.DIRECTIVES and row['disposition'] in ['enforce', 'report', 'unknown'] and row['blocked'] in ['inline', 'eval', 'same-origin', 'other-origin'])
            else:
                insist(type(row['resource']) is str and row['resource'] in diagnostics.RESOURCES and type(row['mime']) is str and row['mime'] in diagnostics.MIMES and type(row['failure']) is str and row['failure'] in diagnostics.FAILURES)
                integer(row['status'], 0, 599)
                insist(row['status'] == 0 or row['status'] >= 100)
        count = sum(row['count'] for row in rows)
        insist(count <= limit)
        if key == 'page_errors':
            insist(count == min(phase['page_error_count'], diagnostics.LIMIT))


def diagnostic_error(row):
    insist(type(row['error_class']) is str and row['error_class'] in diagnostics.CLASSES and type(row['category']) is str and row['category'] in diagnostics.CATEGORIES and type(row['missing_global']) is str and row['missing_global'] in diagnostics.GLOBALS | {'none'})
    insist(row['missing_global'] == 'none' or row['category'] == 'missing-global' and row['error_class'] == 'ReferenceError')


def journey_checks(value, phase):
    fields(value, {'schema', 'record_valid', 'private_values_omitted', 'raw_messages_exported', 'check_results'})
    insist(type(value['schema']) is int and value['schema'] == 1)
    boolean(value['record_valid'])
    insist(value['private_values_omitted'] is True and value['raw_messages_exported'] is False)
    expected = journey_diagnostics.CHECKS[phase['phase']]
    rows = value['check_results']
    insist(type(rows) is list and len(rows) == len(expected))
    for check_id, row in zip(expected, rows):
        fields(row, {'check_id', 'status'})
        insist(type(row['check_id']) is str and row['check_id'] == check_id and type(row['status']) is str and row['status'] in journey_diagnostics.STATUSES)
    if phase['status'] == 'passed':
        insist(value['record_valid'] and all(row['status'] == 'passed' for row in rows))


def runner_checks(value, phase):
    fields(value, {'schema', 'record_valid', 'stage', 'fatal_kind', 'fatal_stage', 'exit_observed', 'exit_code', 'owned_exit_code', 'report_finalized', 'report_passed', 'private_values_omitted', 'raw_messages_exported'})
    insist(type(value['schema']) is int and value['schema'] == 1)
    for key in ['record_valid', 'exit_observed', 'report_finalized', 'report_passed']:
        boolean(value[key])
    insist(value['private_values_omitted'] is True and value['raw_messages_exported'] is False)
    insist(type(value['stage']) is str and value['stage'] in runner_diagnostics.STAGES | {'unavailable'})
    insist(type(value['fatal_kind']) is str and value['fatal_kind'] in runner_diagnostics.FATAL_KINDS | {'unavailable'})
    insist(type(value['fatal_stage']) is str and value['fatal_stage'] in runner_diagnostics.STAGES | {'none', 'unavailable'})
    for key, low in [('exit_code', 0), ('owned_exit_code', -64)]:
        if value[key] is not None:
            integer(value[key], low, 255)
    insist(phase['runner_exit_zero'] is (value['owned_exit_code'] == 0))
    if value['record_valid']:
        insist(value['stage'] != 'unavailable' and value['fatal_kind'] != 'unavailable' and value['fatal_stage'] != 'unavailable')
        insist(value['exit_observed'] or value['exit_code'] is None)
        insist((value['fatal_kind'] == 'none') is (value['fatal_stage'] == 'none'))
    if phase['status'] == 'passed':
        insist(runner_diagnostics.passed(value))


def phases(value):
    fields(value, {'schema', 'phases', 'batches'}, {'error_class', 'budget_exhausted'})
    insist(value['schema'] == 1 and type(value['schema']) is int)
    error_fields(value)
    if 'budget_exhausted' in value:
        boolean(value['budget_exhausted'])
    insist(type(value['phases']) is list and len(value['phases']) <= 49 and type(value['batches']) is list and len(value['batches']) <= 10)
    for row in value['phases']:
        fields(row, {'batch', 'phase', 'status', 'actual_steps', 'failed_steps', 'page_error_count', 'harness_error_observed', 'runner_report_present', 'owned_runner_terminal', 'runner_exit_zero', 'evidence_scope', 'runner_diagnostics'}, {'startup_diagnostics', 'journey_diagnostics'})
        insist(row['batch'] in ['startup', *runner.BATCHES] and row['phase'] in runner.ALL_PHASES and row['status'] in ['passed', 'failed'] and row['evidence_scope'] == SCOPE)
        integer(row['actual_steps'])
        integer(row['page_error_count'])
        for key in ['harness_error_observed', 'runner_report_present', 'owned_runner_terminal', 'runner_exit_zero']:
            boolean(row[key])
        runner_checks(row['runner_diagnostics'], row)
        insist(type(row['failed_steps']) is list and len(row['failed_steps']) <= row['actual_steps'])
        for step in row['failed_steps']:
            fields(step, {'index', 'status'})
            integer(step['index'], 0, max(0, row['actual_steps'] - 1))
            insist(step['status'] in ['failed', 'unresolved'])
        if 'startup_diagnostics' in row:
            insist(row['phase'] == 'startup')
            startup_diagnostics(row['startup_diagnostics'], row)
        insist(row['phase'] != 'startup' or 'startup_diagnostics' in row)
        if 'journey_diagnostics' in row:
            insist(row['phase'] in journey_diagnostics.CHECKS)
            journey_checks(row['journey_diagnostics'], row)
        insist(row['phase'] not in journey_diagnostics.CHECKS or 'journey_diagnostics' in row)
        if row['status'] == 'passed':
            insist(row['actual_steps'] > 0 and row['failed_steps'] == [] and row['page_error_count'] == 0 and row['harness_error_observed'] is False and all(row[key] is True for key in ['runner_report_present', 'owned_runner_terminal', 'runner_exit_zero']))
    for row in value['batches']:
        fields(row, {'batch', 'readiness', 'cleanup', 'passed'}, {'supervisor_terminal', 'supervisor_exit_zero', 'outer_cleanup', 'error_class', 'cleanup_error_class'})
        insist(row['batch'] in ['startup', *runner.BATCHES])
        for key in ['readiness', 'passed', 'supervisor_terminal', 'supervisor_exit_zero']:
            if key in row:
                boolean(row[key])
        error_fields(row)
        cleanup = row['cleanup']
        if type(cleanup) is dict and set(cleanup) == {'verified_stopped'} and cleanup['verified_stopped'] is False:
            pass  # Missing terminal receipt remains explicitly unresolved.
        else:
            fields(cleanup, {'verified_stopped', 'ports_closed', 'keys_removed', 'remaining_owned_process_count', 'final_teardown_clean', 'lifecycle_clean', 'pg_fault_activated', 'pg_fault_terminal_clean'})
            integer(cleanup['remaining_owned_process_count'])
            for key in set(cleanup) - {'remaining_owned_process_count'}:
                boolean(cleanup[key])
            if cleanup['verified_stopped']:
                insist(cleanup['ports_closed'] and cleanup['keys_removed'] and cleanup['final_teardown_clean'] and cleanup['remaining_owned_process_count'] == 0)
        if 'outer_cleanup' in row:
            outer = row['outer_cleanup']
            fields(outer, {'forced', 'verified_stopped', 'remaining_owned_process_count'})
            boolean(outer['forced'])
            boolean(outer['verified_stopped'])
            integer(outer['remaining_owned_process_count'])
            insist(not outer['verified_stopped'] or outer['remaining_owned_process_count'] == 0)
        if row['passed']:
            insist(row['readiness'] is True and row.get('supervisor_terminal') is True and row.get('supervisor_exit_zero') is True and cleanup.get('verified_stopped') is True and cleanup.get('lifecycle_clean') is True and row.get('outer_cleanup', {}).get('verified_stopped') is True and row.get('outer_cleanup', {}).get('forced') is False and 'error_class' not in row and 'cleanup_error_class' not in row)


def summary(value):
    fields(value, {'schema', 'selection', 'selected_scope_passed', 'full_suite_passed', 'required_unique_phases', 'passed_unique_phases', 'failed_unique_phases', 'pending_unique_phases', 'actual_phase_runs', 'all_fixture_teardowns_verified', 'all_fixture_lifecycles_clean', 'budget_exhausted', 'interrupted', 'screenshots_or_raw_logs_uploaded', 'private_runtime_artifacts_uploaded', 'pixel_parity_established'})
    insist(type(value['schema']) is int and value['schema'] == 1 and value['selection'] in ['all', 'startup', *runner.BATCHES])
    insist(type(value['required_unique_phases']) is int and value['required_unique_phases'] == 38)
    integer(value['actual_phase_runs'], 0, 49)
    for key in ['passed_unique_phases', 'failed_unique_phases', 'pending_unique_phases']:
        phase_list(value[key])
    sets = [set(value[key]) for key in ['passed_unique_phases', 'failed_unique_phases', 'pending_unique_phases']]
    insist(set.union(*sets) == set(runner.ALL_PHASES) and sum(map(len, sets)) == 38)
    for key in set(value) - {'schema', 'selection', 'required_unique_phases', 'actual_phase_runs', 'passed_unique_phases', 'failed_unique_phases', 'pending_unique_phases'}:
        boolean(value[key])
    insist(value['screenshots_or_raw_logs_uploaded'] is False and value['private_runtime_artifacts_uploaded'] is False and value['pixel_parity_established'] is False)
    insist(not value['full_suite_passed'] or value['selection'] == 'all' and value['selected_scope_passed'] and not value['failed_unique_phases'] and not value['pending_unique_phases'] and value['all_fixture_lifecycles_clean'])


def validate(directory, canonical=None):
    allowed = {'preparation.json': preparation, 'phases.json': phases, 'summary.json': summary}
    insist(not directory.is_symlink() and directory.is_dir())
    files = {entry.name for entry in directory.iterdir()}
    insist(bool(files) and files <= set(allowed))
    values = {}
    for name in files:
        path = directory / name
        insist(not path.is_symlink() and path.is_file() and path.stat().st_size <= 2 * 1024 * 1024)
        # Reject duplicate decoded keys at every depth, before last-key-wins
        # parsing could hide earlier private text in the uploaded original.
        value = json.loads(path.read_text(), object_pairs_hook=unique_object)
        allowed[name](value)
        values[name] = value
    if 'summary.json' in values:
        insist('phases.json' in values)
        item = values['summary.json']
        batches = {'startup': []} if item['selection'] == 'startup' else runner.BATCHES if item['selection'] == 'all' else {item['selection']: runner.BATCHES[item['selection']]}
        requested = ['startup', *dict.fromkeys(phase for phases in batches.values() for phase in phases)]
        runner.STOP = item['interrupted']
        insist(runner.summarize(values['phases.json'], item['selection'], requested) == item)
    if values.get('preparation.json', {}).get('status') == 'prepared':
        # Hash-shaped strings must be hashes of the actual approved generation,
        # not arbitrary 64-hex text that might contain private signer material.
        spec = importlib.util.spec_from_file_location('hosted_frozen_source', ROOT / 'scripts/ci/prepare-hosted-browser.py')
        source = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(source)
        source.verify_frontend()
        import hashlib
        before = source.source_snapshot()
        prep = values['preparation.json']
        insist(prep['source_snapshot_sha256'] == hashlib.sha256(json.dumps(before, sort_keys=True).encode()).hexdigest())
        expected = {'normal_binary_sha256': ROOT / '.build/hosted-native/debug/weebdex-platform', 'provisioner_sha256': ROOT / '.build/hosted-native/debug/examples/browser_fixture_provisioner', 'frontend_manifest_sha256': ROOT / 'test-evidence/frontend/artifact-manifest.json', 'frontend_index_sha256': ROOT / 'artifacts/frontend/index.html'}
        for key, path in expected.items():
            insist(not path.is_symlink() and path.is_file() and prep[key] == source.sha(path))
        runtime = runner.read(ROOT / '.build/hosted-browser/runtime.json')
        insist(prep['chromium_binary_sha256'] == source.sha(Path(runtime['executable'])))
    if canonical is not None:
        # Never upload the original representation. All documents must pass
        # first, then a fresh directory receives only canonical schema values.
        insist(not canonical.exists() and not canonical.is_symlink())
        canonical.mkdir(mode=0o700)
        for name, value in sorted(values.items()):
            with (canonical / name).open('x') as output:
                output.write(json.dumps(value, sort_keys=True, separators=(',', ':'), ensure_ascii=True) + '\n')
    return len(files)


if __name__ == '__main__':
    try:
        count = validate(ROOT / 'ci-evidence', ROOT / 'ci-sanitized-evidence')
        print(json.dumps({'sanitized_evidence_valid': True, 'files': count}))
    except Exception:
        print('Sanitized evidence validation failed; no artifacts may be uploaded.')
        sys.exit(1)
