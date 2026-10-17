#!/usr/bin/env python3
"""Run real browser phases serially; emit only explicit non-sensitive projections."""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import signal
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[2]
STAGE = ROOT / '.build/hosted-browser'
PUBLIC = ROOT / 'ci-evidence'
SIGNED = {'signed-library', 'signed-profile', 'signed-social', 'signed-context', 'signed-source-controls', 'signed-namespace', 'signed-recipients', 'signed-restart'}
PUBLIC_UPLOAD_PHASE = 'public-upload-recovery'
BATCHES = {
    'ordinary-a': ['account', 'account-recovery', 'upload', 'reader', 'announcements'],
    'ordinary-b': ['local-library', 'filters', 'editorial', 'community', 'pwa'],
    'ui10': ['ui10-api-client', 'ui10-auth-lifetimes', 'ui10-recovery-lifetimes', 'ui10-preferences', 'ui10-reader-transition'],
    'signed-a': ['signed-library', 'signed-profile'],
    'signed-b': ['signed-social', 'signed-context'],
    'signed-c': ['signed-source-controls', 'signed-namespace'],
    'signed-d': ['signed-recipients', 'signed-restart'],
    'ui-feed': ['ui11-device-navigation', 'ui11-device-progress', 'ui12-compact-feed', 'ui13-terminal-feed', 'ui13-feed-authority', 'ui14-entity-pagination'],
    # Compact is deliberately repeated on the new fixture before reader seeding.
    # Baseline actual reader/cover assertions precede last-cover deletion.
    'ui-reader': ['ui12-compact-feed', 'reader', 'ui15-chapter-filter', 'ui15-reader-geometry', 'ui15-reader-feedback', 'ui15-16-empty-tabs'],
    'ui-home': ['ui14-account-failures', 'ui14-home-failures', 'ui16-home-preferences', 'ui16-search-cap'],
    # Supplemental Group/User controller coverage has its own fresh fixture.
    'actor-follow': ['ui14-actor-follow'],
    # Gate-on UUID catalogue profile, with its own operationally ready fixture.
    # Native Startup's SeedManga1 routes belong to the separate gate-off profile.
    'public-upload': [PUBLIC_UPLOAD_PHASE],
}
ALL_PHASES = ['startup', *dict.fromkeys(phase for phases in BATCHES.values() for phase in phases)]
assert len(ALL_PHASES) == 40
MAX_PHASE_RUNS = sum(len(phases) + (name != 'public-upload') for name, phases in BATCHES.items())
STOP = False


def batch_phases(name, phases):
    return list(phases) if name == 'public-upload' else ['startup', *phases]


def requested_phases(batches):
    return list(dict.fromkeys(phase for name, phases in batches.items() for phase in batch_phases(name, phases)))


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def read(path, limit=2 * 1024 * 1024):
    if path.is_symlink() or not path.is_file() or path.stat().st_size > limit:
        raise RuntimeError('Unsafe or unavailable private receipt')
    return json.loads(path.read_text())


def write(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_suffix('.new')
    if temporary.exists():
        raise RuntimeError('Uncertain output staging file')
    temporary.write_text(json.dumps(value, indent=2) + '\n')
    temporary.replace(path)


def maximum(phase):
    return 150 if phase == 'startup' else 600 if phase in SIGNED or phase == PUBLIC_UPLOAD_PHASE else 300


def public_upload_profile(plan):
    profile = plan.get('public_upload_recovery')
    expected = {'enabled', 'ui_dir', 'frontend_manifest', 'frontend_manifest_sha256',
                'frontend_index_sha256', 'build_receipt', 'build_receipt_sha256'}
    if type(profile) is not dict or set(profile) != expected or profile['enabled'] is not True:
        raise RuntimeError('A separate prepared gate-on public upload profile is required')
    paths = {'ui_dir': STAGE / 'public-upload-frontend', 'frontend_manifest': STAGE / 'public-upload-manifest.json', 'build_receipt': STAGE / 'public-upload-build.json'}
    if any(profile[key] != str(path) or path.is_symlink() for key, path in paths.items()):
        raise RuntimeError('Unexpected public upload artifact layout')
    for key in ['frontend_manifest', 'build_receipt']:
        if sha(paths[key]) != profile[key + '_sha256']:
            raise RuntimeError('Prepared public upload receipt changed')
    build = read(paths['build_receipt'])
    if build.get('public_api_catalogue_enabled') is not True or build.get('public_signed_upload_enabled') is not True or build.get('spa_fallback_sha256') != profile['frontend_index_sha256']:
        raise RuntimeError('Public upload artifact is not the exact gate-on generation')
    manifest = read(paths['frontend_manifest'])
    rows = manifest.get('files')
    if type(rows) is not list or not rows or len(rows) > 4096:
        raise RuntimeError('Missing bounded public upload artifact membership')
    expected_files = {}
    for row in rows:
        relative = row.get('path') if type(row) is dict else None
        if type(relative) is not str or not relative or Path(relative).is_absolute() or '..' in Path(relative).parts or str(Path(relative)) != relative or relative in expected_files:
            raise RuntimeError('Unsafe or duplicate public upload artifact member')
        expected_files[relative] = row
    ui = paths['ui_dir']
    if not ui.is_dir() or any(path.is_symlink() for path in ui.rglob('*')):
        raise RuntimeError('Public upload artifact must contain regular files only')
    actual = {str(path.relative_to(ui)): path for path in ui.rglob('*') if not path.is_dir()}
    if set(actual) != set(expected_files) or any(not path.is_file() or path.stat().st_size != expected_files[name].get('bytes') or sha(path) != expected_files[name].get('sha256') for name, path in actual.items()) or sha(ui / 'index.html') != profile['frontend_index_sha256']:
        raise RuntimeError('Public upload artifact content changed')
    return dict(plan, **{key: profile[key] for key in ['ui_dir', 'frontend_manifest', 'frontend_manifest_sha256', 'frontend_index_sha256']})


def verify_public_upload_ready(state, fixture, plan):
    declaration = state.get('public_upload', {})
    if declaration.get('enabled') is not True or declaration.get('fixture_only') is not True or declaration.get('scenario') != 'public-upload-v1':
        raise RuntimeError('Public upload UUID fixture is not opted in')
    reference = state.get('operational_http', {})
    path = ROOT / 'test-evidence/security/live-native-http.json'
    if set(reference) != {'path', 'sha256'} or reference.get('path') != str(path) or sha(path) != reference.get('sha256'):
        raise RuntimeError('Current-fixture operational receipt is missing or changed')
    receipt = read(path)
    checks = receipt.get('checks')
    if receipt.get('run_directory') != str(fixture) or receipt.get('fixture_only') is not True or receipt.get('production') is not False or receipt.get('binary', {}).get('sha256') != plan['normal_sha256'] or receipt.get('frontend_index_sha256') != plan['frontend_index_sha256'] or type(checks) is not list or not 1 <= len(checks) <= 64 or receipt.get('passed') != len(checks) or receipt.get('failed') != 0 or any(check.get('passed') is not True for check in checks):
        raise RuntimeError('Operational readiness belongs to another or failed generation')
    rows = [check for check in checks if check.get('name') == 'bounded oversized JSON']
    if len(rows) != 1:
        raise RuntimeError('Current early-response check is missing')
    check = rows[0]
    transport = check.get('transport', {})
    sent = transport.get('completed_body_send_bytes')
    error = transport.get('write_error_class')
    if check.get('method') != 'POST' or check.get('path') != '/api/manga' or check.get('status') != 413 or check.get('expected_status') != 413 or check.get('request_body_bytes') != 1048577 or transport.get('kind') != 'same-connection-early-response' or transport.get('declared_body_bytes') != 1048577 or transport.get('observed_status') != 413 or type(sent) is not int or not 0 <= sent <= 1048577 or error not in [None, 'BrokenPipeError', 'ConnectionResetError'] or (error is None and sent != 1048577) or transport.get('response_body_complete') is not True or transport.get('deadline_expired') is not False or transport.get('deadline_thread_settled') is not True:
        raise RuntimeError('Current HTTP413 transport readiness is incomplete')


def wait_for(process, predicate, seconds, owner=None):
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        if owner:
            owner.owned_members()
        if STOP:
            raise InterruptedError('Owned run interrupted')
        value = predicate()
        if value:
            return value
        if process.poll() is not None:
            raise RuntimeError('Owned fixture exited before the requested receipt')
        time.sleep(.25)
    raise TimeoutError('Bounded owned observation expired')


def project_phase(phase, batch, acceptance, owned):
    spec = importlib.util.spec_from_file_location('runner_diagnostics', ROOT / 'scripts/ci/runner-diagnostics.py')
    diagnostics = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(diagnostics)
    runner_diagnostics = diagnostics.project(acceptance, owned)
    steps = acceptance.get('steps', [])
    clean = owned.get('fixture_only') is True and owned.get('phase') == phase and owned.get('verified_stopped') is True and owned.get('forced') is False
    passed = clean and owned.get('returncode') == 0 and acceptance.get('phase') == phase and acceptance.get('passed') is True and not acceptance.get('harness_error') and bool(steps) and all(row.get('status') == 'passed' for row in steps) and acceptance.get('page_errors') == [] and diagnostics.passed(runner_diagnostics)
    value = {'batch': batch, 'phase': phase, 'status': 'passed' if passed else 'failed', 'actual_steps': len(steps), 'failed_steps': [{'index': index, 'status': row.get('status') if row.get('status') in ['passed', 'failed'] else 'unresolved'} for index, row in enumerate(steps) if row.get('status') != 'passed'], 'page_error_count': len(acceptance.get('page_errors', [])), 'harness_error_observed': bool(acceptance.get('harness_error')), 'runner_report_present': bool(acceptance), 'owned_runner_terminal': clean, 'runner_exit_zero': type(owned.get('returncode')) is int and owned['returncode'] == 0, 'evidence_scope': 'actual browser controls and persisted-result assertions; pixel parity remains unverified'}
    value['runner_diagnostics'] = runner_diagnostics
    if phase == 'startup':
        spec = importlib.util.spec_from_file_location('startup_diagnostics', ROOT / 'scripts/ci/startup-diagnostics.py')
        diagnostics = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(diagnostics)
        value['startup_diagnostics'] = diagnostics.project(acceptance)
    spec = importlib.util.spec_from_file_location('journey_diagnostics', ROOT / 'scripts/ci/journey-diagnostics.py')
    diagnostics = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(diagnostics)
    if phase in diagnostics.CHECKS:
        value['journey_diagnostics'] = diagnostics.project(phase, acceptance)
    if phase == PUBLIC_UPLOAD_PHASE:
        spec = importlib.util.spec_from_file_location('public_upload_diagnostics', ROOT / 'scripts/ci/public-upload-diagnostics.py')
        diagnostics = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(diagnostics)
        value['public_upload_diagnostics'] = diagnostics.project(acceptance)
        if not diagnostics.passed(value['public_upload_diagnostics']):
            value['status'] = 'failed'
    return value


def project_cleanup(value):
    ports = value.get('loopback_ports_closed', {})
    ports_closed = set(ports) == {'platform', 'mail', 'postgres', 'original', 'recipient'} and all(item is True for item in ports.values())
    stopped = value.get('fixture_only') is True and value.get('status') == 'verified_stopped' and value.get('synthetic_server_keys_absent') is True and ports_closed and value.get('remaining_owned_processes') == [] and value.get('process_tracking_errors') == [] and value.get('final_teardown_clean') is True
    fault = value.get('pg_home_fault_terminal')
    fault_clean = isinstance(fault, dict) and fault.get('terminal') is True and fault.get('status') == 'verified_stopped' and fault.get('returncode') == 0 and fault.get('watchdog_returncode') == 0 and fault.get('forced') is False and fault.get('remaining_owned_processes') == [] and fault.get('error') is None
    return {'verified_stopped': stopped, 'ports_closed': ports_closed, 'keys_removed': value.get('synthetic_server_keys_absent') is True, 'remaining_owned_process_count': len(value.get('remaining_owned_processes', [])), 'final_teardown_clean': value.get('final_teardown_clean') is True, 'lifecycle_clean': value.get('lifecycle_clean') is True, 'pg_fault_activated': fault is not None, 'pg_fault_terminal_clean': fault_clean}


def finish_supervisor(owner, process, fixture, batch, seconds=600):
    """Always attempt owned terminal cleanup, even if a stop/receipt step fails."""
    terminal = False
    remaining = []
    observed = False
    batch['outer_cleanup'] = {'forced': False, 'verified_stopped': False, 'remaining_owned_process_count': 0}
    try:
        if fixture:
            (fixture / 'stop.request').touch()
        elif process.poll() is None:
            owner.signal_owned(process, signal.SIGTERM, leader_only=True)
        terminal = owner.wait_terminal(process, seconds)
        remaining = owner.process_members(None)
        batch['outer_cleanup'] = {'forced': False, 'verified_stopped': terminal and not remaining and not owner.process_tracking_errors, 'remaining_owned_process_count': len(remaining)}
        observed = True
    except Exception as error:
        batch['cleanup_error_class'] = type(error).__name__
    finally:
        if not observed or not terminal or remaining or owner.process_tracking_errors:
            batch.setdefault('cleanup_error_class', 'TimeoutError')
            # Remain fail-closed even when the owned fallback itself errors.
            batch['outer_cleanup'] = {'forced': True, 'verified_stopped': False, 'remaining_owned_process_count': len(remaining)}
            try:
                batch['outer_cleanup'] = owner.fallback_cleanup()
            except Exception as error:
                batch['cleanup_error_class'] = type(error).__name__
        batch['supervisor_terminal'] = process.poll() is not None
        batch['supervisor_exit_zero'] = process.returncode == 0
    try:
        if fixture and (fixture / 'cleanup.json').is_file():
            batch['cleanup'] = project_cleanup(read(fixture / 'cleanup.json'))
    except Exception as error:
        batch['cleanup_error_class'] = type(error).__name__
    if batch['outer_cleanup']['forced'] or not batch['outer_cleanup']['verified_stopped'] or 'cleanup_error_class' in batch:
        # Forced/uncertain cleanup never permits another fixture or a pass.
        batch['cleanup']['verified_stopped'] = False


def run_batch(name, phases, plan, journal, run_id):
    if name == 'public-upload':
        plan = public_upload_profile(plan)  # Fail before allocating an owner.
    spec = importlib.util.spec_from_file_location('outer_fixture_cleanup', ROOT / 'scripts/ci/owned-fixture-cleanup.py')
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    owner = module.OwnedFixtureProcesses()
    roots = ROOT / 'test-runtime'
    roots.mkdir(exist_ok=True)
    before = set(roots.glob('browser-final-*'))
    fixture = None
    process = None
    batch = {'batch': name, 'readiness': False, 'cleanup': {'verified_stopped': False}}
    log = STAGE / ('fixture-' + name + '.log')
    argv = [sys.executable, str(ROOT / 'tests/runtime/browser_fixture.py'), '--binary', plan['normal_binary'], '--expected-sha256', plan['normal_sha256'], '--protocol-provisioner', plan['provisioner'], '--provisioner-sha256', plan['provisioner_sha256'], '--ui-dir', plan.get('ui_dir', str(ROOT / 'artifacts/frontend')), '--frontend-index-sha256', plan['frontend_index_sha256'], '--frontend-artifact-manifest', plan['frontend_manifest'], '--frontend-artifact-manifest-sha256', plan['frontend_manifest_sha256'], '--second-protocol-recipient', '--ui-regression-controls', '--pg-home-fault-controls', '--lifetime-seconds', '3600']
    if name == 'public-upload':
        argv.append('--public-upload-controls')
    # No Actions token or provider/account secrets reach the fixture process.
    allowed = {'PATH', 'HOME', 'TMPDIR', 'TMP', 'TEMP', 'LANG', 'LC_ALL', 'LC_CTYPE', 'LD_LIBRARY_PATH', 'SSL_CERT_FILE', 'SSL_CERT_DIR'}
    env = {key: value for key, value in os.environ.items() if key in allowed}
    with log.open('x') as output:
        try:
            with owner.process_ownership_lock:
                process = subprocess.Popen(argv, cwd=ROOT, env=env, stdout=output, stderr=subprocess.STDOUT, start_new_session=True)
                owner.register_process('supervisor', process)

            def ready():
                nonlocal fixture
                candidates = set(roots.glob('browser-final-*')) - before
                if len(candidates) > 1:
                    raise RuntimeError('Unexpected competing fixture root')
                if candidates:
                    candidate = candidates.pop()
                    if candidate.is_symlink() or candidate.stat().st_uid != os.getuid() or candidate.stat().st_mode & 0o077:
                        raise RuntimeError('Fixture root is not newly owned/private')
                    fixture = candidate
                    path = fixture / 'supervisor.json'
                    if path.is_file():
                        state = read(path)
                        if state.get('status') == 'ready':
                            if state.get('run_directory') != str(fixture) or state.get('protocol_seed', {}).get('fully_ready') is not True or state.get('binary', {}).get('sha256') != plan['normal_sha256'] or state.get('frontend', {}).get('artifact_manifest_sha256') != plan['frontend_manifest_sha256']:
                                raise RuntimeError('Newly ready fixture differs from the actual build plan')
                            return state
                return None

            state = wait_for(process, ready, 300, owner)
            if name == 'public-upload':
                verify_public_upload_ready(state, fixture, plan)
            batch['readiness'] = True
            for phase in batch_phases(name, phases):
                acceptance_path = ROOT / 'test-evidence/frontend/browser' / (phase + '-' + run_id + '-' + name) / 'acceptance.json'
                if acceptance_path.parent.exists():
                    raise RuntimeError('Refusing prior phase evidence')
                command = fixture / 'browser-command.json'
                if command.exists() or (fixture / 'browser-command.rejected.json').exists():
                    raise RuntimeError('Uncertain prior queue state')
                payload = {'phase': phase, 'argv': [plan['node'], str(ROOT / 'scripts/browser-acceptance.mjs')], 'timeout_seconds': maximum(phase), 'env': {'BROWSER_PHASE': phase, 'BROWSER_RUN_ID': run_id + '-' + name}}
                write(command, payload)
                terminal = fixture / ('browser-phase-' + phase + '.json')
                def completed():
                    if (fixture / 'browser-command.rejected.json').exists():
                        raise RuntimeError('Owned browser command rejected')
                    return read(terminal) if terminal.is_file() else None
                owned = wait_for(process, completed, maximum(phase) + 35, owner)
                acceptance = read(acceptance_path) if acceptance_path.is_file() else {}
                row = project_phase(phase, name, acceptance, owned)
                journal['phases'].append(row)
                write(PUBLIC / 'phases.json', journal)
                print(json.dumps({key: row[key] for key in ['batch', 'phase', 'status', 'actual_steps']}), flush=True)
                if row['status'] != 'passed':
                    break  # Mutation state after a failed phase is not assumed safe.
        except Exception as error:
            batch['error_class'] = type(error).__name__
        finally:
            if process:
                finish_supervisor(owner, process, fixture, batch)
    if any(row['batch'] == name and row['phase'] == 'ui14-home-failures' and row['status'] == 'passed' for row in journal['phases']) and not batch['cleanup'].get('pg_fault_terminal_clean'):
        batch['cleanup']['verified_stopped'] = False
    journal['batches'].append(batch)
    runs = [row for row in journal['phases'] if row['batch'] == name]
    batch['passed'] = batch.get('readiness') is True and batch.get('supervisor_terminal') is True and batch.get('supervisor_exit_zero') is True and batch['cleanup'].get('verified_stopped') is True and batch['cleanup'].get('lifecycle_clean') is True and batch.get('outer_cleanup', {}).get('verified_stopped') is True and batch.get('outer_cleanup', {}).get('forced') is False and len(runs) == len(batch_phases(name, phases)) and all(row['status'] == 'passed' for row in runs) and 'error_class' not in batch and 'cleanup_error_class' not in batch
    write(PUBLIC / 'phases.json', journal)
    return batch


def summarize(journal, selection, requested):
    passed = {row['phase'] for row in journal['phases'] if row['status'] == 'passed'}
    failed = {row['phase'] for row in journal['phases'] if row['status'] != 'passed'}
    clean = bool(journal['batches']) and all(row.get('supervisor_terminal') is True and row['cleanup']['verified_stopped'] is True for row in journal['batches'])
    lifecycles_clean = clean and all(row.get('supervisor_exit_zero') is True and row['cleanup'].get('lifecycle_clean') is True and row.get('passed') is True for row in journal['batches'])
    target_passed = set(requested) <= passed and not failed and lifecycles_clean and not journal.get('error_class')
    return {'schema': 1, 'selection': selection, 'selected_scope_passed': target_passed, 'full_suite_passed': selection == 'all' and target_passed and set(ALL_PHASES) <= passed, 'required_unique_phases': len(ALL_PHASES), 'passed_unique_phases': sorted(passed - failed), 'failed_unique_phases': sorted(failed), 'pending_unique_phases': sorted(set(ALL_PHASES) - passed - failed), 'actual_phase_runs': len(journal['phases']), 'all_fixture_teardowns_verified': clean, 'all_fixture_lifecycles_clean': lifecycles_clean, 'budget_exhausted': journal.get('budget_exhausted', False), 'interrupted': STOP, 'screenshots_or_raw_logs_uploaded': False, 'private_runtime_artifacts_uploaded': False, 'pixel_parity_established': False}


def main():
    global STOP
    parser = argparse.ArgumentParser()
    parser.add_argument('--batch', choices=['all', 'startup', *BATCHES], default='all')
    parser.add_argument('--budget-seconds', type=int, default=14400)
    parser.add_argument('--plan-only', action='store_true', help='Static scheduling inspection; launches nothing')
    args = parser.parse_args()
    if not 3600 <= args.budget_seconds <= 14400:
        parser.error('budget must be 3600..14400 seconds')
    batches = {'startup': []} if args.batch == 'startup' else BATCHES if args.batch == 'all' else {args.batch: BATCHES[args.batch]}
    requested = requested_phases(batches)
    if args.plan_only:
        print(json.dumps({'unique_phase_count': len(requested), 'all_unique_phase_count': len(ALL_PHASES), 'batches': batches, 'browser_or_fixture_started': False}, indent=2))
        return 0
    if os.environ.get('GITHUB_ACTIONS') != 'true' or sys.platform != 'linux':
        raise RuntimeError('Reviewed hosted Linux runner required; no local launcher retry')
    def interrupt(*_):
        global STOP
        STOP = True
    for signum in [signal.SIGINT, signal.SIGTERM]:
        signal.signal(signum, interrupt)
    PUBLIC.mkdir(exist_ok=True)
    journal = {'schema': 1, 'phases': [], 'batches': []}
    deadline = time.monotonic() + args.budget_seconds
    try:
        spec = importlib.util.spec_from_file_location('browser_native_feed_contract', ROOT / 'scripts/ci/native-feed-diagnostics.py')
        native_feed = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(native_feed)
        prepared = read(PUBLIC / 'preparation.json')
        native_feed.validate(prepared.get('native_feed_regression'))
        if prepared.get('status') != 'prepared' or prepared['native_feed_regression']['status'] != 'passed':
            raise RuntimeError('Genuine native-feed regression must pass before any browser fixture')
        plan = read(STAGE / 'plan.json')
        expected_paths = {'normal_binary': str(ROOT / '.build/hosted-native/debug/weebdex-platform'), 'provisioner': str(ROOT / '.build/hosted-native/debug/examples/browser_fixture_provisioner'), 'frontend_manifest': str(ROOT / 'test-evidence/frontend/artifact-manifest.json')}
        if plan.get('schema') != 1 or any(plan.get(key) != value for key, value in expected_paths.items()):
            raise RuntimeError('Unexpected hosted build layout')
        for file, digest in [(plan['normal_binary'], plan['normal_sha256']), (plan['provisioner'], plan['provisioner_sha256']), (plan['frontend_manifest'], plan['frontend_manifest_sha256']), (ROOT / '.build/hosted-browser/runtime.json', plan['runtime_sha256'])]:
            if sha(Path(file)) != digest:
                raise RuntimeError('Prepared generation changed before browser execution')
        run_id = 'gh-' + os.environ.get('GITHUB_RUN_ID', '') + '-' + os.environ.get('GITHUB_RUN_ATTEMPT', '')
        if not re.fullmatch(r'gh-[0-9]{1,20}-[0-9]{1,5}', run_id):
            raise RuntimeError('A current safe GitHub run identity is required')
        for name, phases in batches.items():
            # A complete worst-case phase allowance, startup, readiness and
            # terminal cleanup reserve must fit before starting a fresh fixture.
            reserve = sum(maximum(phase) + 35 for phase in batch_phases(name, phases)) + 300 + 600
            if STOP or time.monotonic() + reserve > deadline:
                journal['budget_exhausted'] = not STOP
                break
            batch = run_batch(name, phases, plan, journal, run_id)
            # Physical teardown allows independent fresh diagnosis after a
            # failed phase. It cannot establish a passing batch or suite.
            if batch.get('supervisor_terminal') is not True or batch['cleanup']['verified_stopped'] is not True:
                break  # Unresolved cleanup blocks every later fixture.
            write(PUBLIC / 'summary.json', summarize(journal, args.batch, requested))
    except Exception as error:
        journal['error_class'] = type(error).__name__
    write(PUBLIC / 'phases.json', journal)
    summary = summarize(journal, args.batch, requested)
    write(PUBLIC / 'summary.json', summary)
    print(json.dumps(summary), flush=True)
    return 0 if summary['selected_scope_passed'] else 1


if __name__ == '__main__':
    sys.exit(main())
