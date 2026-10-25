#!/usr/bin/env python3
"""Rebuild this checkout, pin the actual generation, and publish no private logs."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import time
import importlib.util
import re
import traceback

ROOT = Path(__file__).resolve().parents[2]
REPO = ROOT.parent
STAGE = ROOT / '.build/hosted-browser'
PUBLIC = ROOT / 'ci-evidence'
QA_FAILURE_CHILDREN = ('public-upload-frontend-build', 'public-upload-artifact-record')
QA_TRACEBACK_LIMIT = 64 * 1024
QA_FAILURE_STAGES = (
    'public-upload-profile', 'public-upload-profile-preflight',
    'public-upload-profile-native-off', 'public-upload-profile-native-evidence',
    'public-upload-qa-build', 'public-upload-qa-source', 'public-upload-qa-receipt',
    'public-upload-qa-graph', 'public-upload-qa-record', 'public-upload-qa-final-source',
    'public-upload-qa-final-graph', 'public-upload-qa-seal', 'public-upload-profile-restore',
)


def qa_encoded(value):
    return (json.dumps(value, sort_keys=True, separators=(',', ':'), ensure_ascii=True) + '\n').encode()


def qa_private_write(path, data):
    """Replace one fixed private receipt without following an existing link."""
    if path.parent.is_symlink() or path.is_symlink():
        raise RuntimeError('QA failure receipt links are refused')
    pending = path.with_name(path.name + '.pending')
    descriptor = os.open(pending, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    with os.fdopen(descriptor, 'wb') as output:
        output.write(data)
    pending.replace(path)


def qa_boundaries(capture):
    return {'schema': 1, 'children': capture['children']}


def begin_qa_failure_capture(before):
    """Create a fresh private pre-child boundary; never read broad stage logs."""
    context = dict(repository=os.environ.get('GITHUB_REPOSITORY'),
                   run_id=os.environ.get('GITHUB_RUN_ID'),
                   run_attempt=os.environ.get('GITHUB_RUN_ATTEMPT'),
                   source_commit=os.environ.get('GITHUB_SHA'))
    if (context['repository'] != '9hnm24bzvm-wq/nostr-manga' or
            any(type(context[key]) is not str or re.fullmatch(r'[1-9][0-9]{0,19}', context[key]) is None
                for key in ['run_id', 'run_attempt']) or
            type(context['source_commit']) is not str or re.fullmatch(r'[0-9a-f]{40}', context['source_commit']) is None):
        raise RuntimeError('Current QA failure capture context is unavailable')
    for directory in [ROOT, ROOT / '.build', STAGE]:
        if directory.is_symlink() or not directory.is_dir():
            raise RuntimeError('QA failure capture directory is unavailable')
    directory = STAGE / 'qa-preparation-failure'
    directory.mkdir(mode=0o700)  # A prior capture must never be rebound.
    capture = {'directory': directory, 'context': context,
               'source_snapshot_sha256': hashlib.sha256(json.dumps(before, sort_keys=True).encode()).hexdigest(),
               'children': {label: {'entered': False} for label in QA_FAILURE_CHILDREN}}
    qa_private_write(directory / 'boundaries.json', qa_encoded(qa_boundaries(capture)))
    return capture


def run_qa_failure_child(capture, label, argv, cwd, seconds):
    if label not in QA_FAILURE_CHILDREN or capture['children'][label]['entered']:
        raise RuntimeError('QA failure child boundary is unavailable')
    if label == QA_FAILURE_CHILDREN[1] and not capture['children'][QA_FAILURE_CHILDREN[0]]['entered']:
        raise RuntimeError('QA failure child order changed')
    # Persist BEFORE calling even the owned runner. A crash or refusal after
    # this point without its terminal receipt is uncertainty, never "not run".
    capture['children'][label]['entered'] = True
    qa_private_write(capture['directory'] / 'boundaries.json', qa_encoded(qa_boundaries(capture)))
    return run(label, argv, cwd, seconds)


def capture_qa_failure(capture, public, error):
    """Private traceback tail only; the separate sealer admits ownership."""
    if public.get('status') != 'preparation_failed' or public.get('stage') not in QA_FAILURE_STAGES:
        raise RuntimeError('Only QA profile failures can be captured')
    directory = capture['directory']
    formatted = ''.join(traceback.TracebackException.from_exception(
        error, limit=32, capture_locals=False, max_group_width=4, max_group_depth=4).format()).encode()
    retained = formatted[-QA_TRACEBACK_LIMIT:]
    qa_private_write(directory / 'traceback.txt', retained)
    value = {'schema': 1, 'status': 'qa_preparation_failed', 'stage': public['stage'],
             **capture['context'], 'preparation_sha256': sha(PUBLIC / 'preparation.json'),
             'source_snapshot_sha256': capture['source_snapshot_sha256'],
             'boundaries_sha256': sha(directory / 'boundaries.json'),
             'traceback': {'total_bytes': len(formatted), 'retained_bytes': len(retained),
                           'truncated': len(retained) != len(formatted),
                           'sha256': hashlib.sha256(retained).hexdigest()}}
    qa_private_write(directory / 'capture.json', qa_encoded(value))


def qa_build_contract():
    spec = importlib.util.spec_from_file_location('hosted_upload_qa_build', ROOT / 'scripts/ci/public-upload-qa-build.py')
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def write(path, value):
    path.write_text(json.dumps(value, indent=2) + '\n')


def source_snapshot():
    # Build outputs, historical evidence and runtime state are not frozen inputs.
    directories = [ROOT / name for name in ['frontend', 'platform-integrations', 'scripts', 'backend/src', 'backend/examples', 'backend/migrations', 'backend/tests']]
    directories += [REPO / 'manga-protocol-lab/server/src', REPO / 'manga-protocol-lab/server/migrations', ROOT / 'tests/runtime']
    paths = [path for directory in directories for path in directory.rglob('*') if path.is_file()]
    paths += [ROOT / 'backend/Cargo.toml', ROOT / 'backend/Cargo.lock', REPO / 'manga-protocol-lab/server/Cargo.toml', REPO / 'manga-protocol-lab/server/Cargo.lock', REPO / 'manga-protocol-lab/browser-client/package-lock.json', REPO / '.ci/browser/package.json', REPO / '.ci/browser/package-lock.json', REPO / '.github/workflows/hosted-browser-e2e.yml']
    if any(path.is_symlink() for path in paths):
        raise RuntimeError('Source input links are not allowed')
    return {str(path.relative_to(REPO)): sha(path) for path in sorted(set(paths))}


def verify_frontend(frontend=None):
    frontend = ROOT / 'frontend' if frontend is None else frontend
    manifest = json.loads((ROOT / 'test-evidence/frontend/source-parity.json').read_text())
    rows = manifest['files']
    additions = json.loads((ROOT / 'scripts/ci/frontend-additions.json').read_text())['files']
    expected = {row['path'] for row in [*rows, *additions]}
    actual = {str(path.relative_to(frontend)) for path in frontend.rglob('*') if path.is_file()}
    if len(rows) != 676 or len(additions) != 3 or len(expected) != 679 or actual != expected:
        raise RuntimeError('Original frontend or explicitly reviewed addition membership changed')
    for row in [*rows, *additions]:
        path = frontend / row['path']
        if path.is_symlink() or path.stat().st_size != row['bytes'] or sha(path) != row['source_sha256']:
            raise RuntimeError('Original frontend bytes changed')


def run(label, argv, cwd, seconds):
    receipt = STAGE / (label + '-owned.json')
    command = [sys.executable, str(ROOT / 'scripts/run-bounded.py'), '--cwd', str(cwd), '--deadline', str(seconds), '--receipt', str(receipt), '--log', str(STAGE / (label + '.log')), '--', *map(str, argv)]
    result = subprocess.run(command)
    value = json.loads(receipt.read_text()) if receipt.is_file() else {}
    if result.returncode or value.get('process_exit_code') != 0 or value.get('deadline_exceeded') is not False or value.get('terminal_receipt') is not True or value.get('cleanup_verified') is not True or value.get('surviving_owned_process_ids') != [] or value.get('cleanup_signals') != [] or value.get('prior_or_unrelated_processes_touched') is not False:
        raise RuntimeError('Owned preparation stage failed: ' + label)
    return value


def main():
    STAGE.mkdir(parents=True, exist_ok=True)
    PUBLIC.mkdir(exist_ok=True)
    public = {'schema': 1, 'status': 'preparation_failed', 'browser_phases_started': False, 'stage': 'source-preservation', 'source_files_checked': 0, 'frontend_preserved': False, 'artifacts_uploaded': False}
    write(PUBLIC / 'preparation.json', public)
    qa_capture = None
    qa_profile_active = False
    try:
        verify_frontend()
        before = source_snapshot()
        public.update(source_files_checked=676, frontend_preserved=True)
        env = qa_build_contract().native_environment(dict(os.environ))
        # Only the separate, explicit public-upload QA build may turn recording
        # on. A caller environment cannot enable it in the native generation.
        env.update(RUSTUP_TOOLCHAIN='1.90.0', CARGO_TARGET_DIR=str(ROOT / '.build/hosted-native'), WEEBDEX_FRONTEND_ROOT=str(ROOT / '.build/frontend-hosted'), PUBLIC_MANGADEX_CATALOGUE='0', WEEBDEX_QA_UPLOAD_REFUSALS='0')
        os.environ.update(env)
        frontend_stage = ROOT / '.build/frontend-hosted'
        if frontend_stage.exists():
            raise RuntimeError('Refusing a previously populated frontend stage')
        import shutil
        shutil.copytree(ROOT / 'frontend', frontend_stage)
        stages = [
            ('native-dependencies', ['bash', ROOT / 'scripts/ci/bootstrap-native.sh'], REPO, 2700),
            ('native-build', ['cargo', 'build', '--locked', '--manifest-path', ROOT / 'backend/Cargo.toml', '--bin', 'weebdex-platform', '--example', 'browser_fixture_provisioner', '-j2'], REPO, 2100),
            ('signer-dependencies', [sys.executable, ROOT / 'scripts/restore-local-library-noble.py'], ROOT, 90),
            ('frontend-dependencies', ['node', REPO / '.ci/browser/node_modules/yarn/bin/yarn.js', 'install', '--frozen-lockfile', '--ignore-scripts', '--non-interactive', '--cache-folder', ROOT / '.cache/hosted-yarn'], frontend_stage, 600),
            ('frontend-build', ['node', ROOT / 'scripts/build-frontend.mjs'], ROOT, 900),
        ]
        native_title_tests = None
        for label, argv, cwd, seconds in stages:
            public['stage'] = label
            write(PUBLIC / 'preparation.json', public)
            started = time.monotonic()
            receipt = run(label, argv, cwd, seconds)
            if label == 'native-build':
                # Reuse the same target cache and the existing 2100s native
                # build envelope. Offline failure never triggers dependency
                # fetching or an extra preparation/run allowance.
                remaining = seconds - (time.monotonic() - started)
                if remaining <= 0:
                    raise RuntimeError('Native test build has no remaining preparation budget')
                run('native-title-test-build', ['cargo', 'test', '--offline', '--locked',
                    '--manifest-path', ROOT / 'backend/Cargo.toml', '--lib', '--test',
                    'public_manga_writes', '--no-run', '--message-format=json', '-j2'], REPO, remaining)
                import importlib.util
                spec = importlib.util.spec_from_file_location('prepared_native_title_contract', ROOT / 'scripts/ci/native-title-diagnostics.py')
                native_title = importlib.util.module_from_spec(spec)
                spec.loader.exec_module(native_title)
                log = STAGE / 'native-title-test-build.log'
                if log.is_symlink() or not log.is_file() or log.stat().st_size > 16 * 1024 * 1024:
                    raise RuntimeError('Unavailable native test artifact output')
                source_hash = hashlib.sha256(json.dumps(before, sort_keys=True).encode()).hexdigest()
                native_title_tests = native_title.artifacts(log.read_text(), ROOT, source_hash)
            if source_snapshot() != before:
                raise RuntimeError('Frozen source changed during preparation')
            if label == 'frontend-build':
                qa_build_contract().verify(ROOT, ROOT / 'artifacts/frontend', False)
                # These facts are derived from the just-completed owned command,
                # never from historical successful receipts or guessed binaries.
                result = {'status': 'passed', 'command_exit': receipt['process_exit_code'], 'remaining_owned_members': receipt['surviving_owned_process_ids'], 'changed_inputs': [], 'owned_terminal_receipt': str((STAGE / (label + '-owned.json')).relative_to(ROOT)), 'browser_tests_executed': False}
                destination = ROOT / 'test-evidence/frontend/hosted-build'
                destination.mkdir(exist_ok=True)
                write(destination / 'build-result.json', result)
        for label, argv in [('artifact-record', [sys.executable, ROOT / 'scripts/record-frontend-artifacts.py', 'hosted-build/build-result.json']), ('browser-runtime-record', ['node', ROOT / 'scripts/ci/browser-runtime.mjs', '--record-hosted'])]:
            public['stage'] = label
            write(PUBLIC / 'preparation.json', public)
            run(label, argv, ROOT, 60)
        import importlib.util
        public_upload_profile = None
        if os.environ.get('SELECTED_BATCH', 'all') in ['all', 'public-upload', 'public-signed-features']:
            public['stage'] = 'public-upload-profile'
            write(PUBLIC / 'preparation.json', public)
            qa_profile_active = True
            qa_capture = begin_qa_failure_capture(before)
            spec = importlib.util.spec_from_file_location('hosted_public_upload_profile', ROOT / 'scripts/ci/hosted-public-upload-profile.py')
            profile = importlib.util.module_from_spec(spec)
            spec.loader.exec_module(profile)
            def verify_source():
                if source_snapshot() != before:
                    raise RuntimeError('Frozen source changed during public upload preparation')
            def profile_progress(stage):
                if stage not in profile.PUBLIC_STAGES:
                    raise RuntimeError('Unknown public upload preparation boundary')
                public['stage'] = stage
                write(PUBLIC / 'preparation.json', public)
            def profile_run(label, argv, cwd, seconds):
                return run_qa_failure_child(qa_capture, label, argv, cwd, seconds)
            public_upload_profile = profile.prepare(ROOT, STAGE, profile_run, verify_source, profile_progress)
            qa_profile_active = False
        verify_frontend()
        if source_snapshot() != before:
            raise RuntimeError('Frozen source changed at the final boundary')
        manifest = ROOT / 'test-evidence/frontend/artifact-manifest.json'
        normal = ROOT / '.build/hosted-native/debug/weebdex-platform'
        provisioner = ROOT / '.build/hosted-native/debug/examples/browser_fixture_provisioner'
        runtime = json.loads((STAGE / 'runtime.json').read_text())
        plan = {'schema': 1, 'normal_binary': str(normal), 'normal_sha256': sha(normal), 'provisioner': str(provisioner), 'provisioner_sha256': sha(provisioner), 'frontend_manifest': str(manifest), 'frontend_manifest_sha256': sha(manifest), 'frontend_index_sha256': sha(ROOT / 'artifacts/frontend/index.html'), 'node': subprocess.check_output(['node', '-p', 'process.execPath'], text=True).strip(), 'runtime_sha256': sha(STAGE / 'runtime.json')}
        if native_title_tests is None:
            raise RuntimeError('Fresh native title test artifacts are required')
        plan['native_title_tests'] = native_title_tests
        if public_upload_profile is not None:
            plan['public_upload_recovery'] = public_upload_profile
        write(STAGE / 'plan.json', plan)
        import importlib.util
        spec = importlib.util.spec_from_file_location('prepared_native_feed_contract', ROOT / 'scripts/ci/native-feed-diagnostics.py')
        native_feed = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(native_feed)
        public.update(status='prepared', stage='complete', source_snapshot_sha256=hashlib.sha256(json.dumps(before, sort_keys=True).encode()).hexdigest(), frontend_manifest_sha256=plan['frontend_manifest_sha256'], frontend_index_sha256=plan['frontend_index_sha256'], normal_binary_sha256=plan['normal_sha256'], provisioner_sha256=plan['provisioner_sha256'], playwright=runtime['playwright'], chromium_revision=runtime['chromium_revision'], chromium_binary_sha256=runtime['executable_sha256'], fresh_build=True, historical_binaries_reused=False, browser_phases_started=False, ipc_success_established=False, native_feed_regression=native_feed.not_run())
        write(PUBLIC / 'preparation.json', public)
        return 0
    except Exception as error:
        public['error_class'] = type(error).__name__
        write(PUBLIC / 'preparation.json', public)
        if qa_profile_active and qa_capture is not None:
            try:
                capture_qa_failure(qa_capture, public, error)
            except Exception:
                pass  # Diagnostic capture must not replace the preparation failure.
        return 1


if __name__ == '__main__':
    sys.exit(main())
