#!/usr/bin/env python3
"""Rebuild this checkout, pin the actual generation, and publish no private logs."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[2]
REPO = ROOT.parent
STAGE = ROOT / '.build/hosted-browser'
PUBLIC = ROOT / 'ci-evidence'


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def write(path, value):
    path.write_text(json.dumps(value, indent=2) + '\n')


def source_snapshot():
    # Build outputs, historical evidence and runtime state are not frozen inputs.
    directories = [ROOT / name for name in ['frontend', 'platform-integrations', 'scripts', 'backend/src', 'backend/examples', 'backend/migrations', 'backend/tests']]
    directories += [REPO / 'manga-protocol-lab/server/src', REPO / 'manga-protocol-lab/server/migrations', ROOT / 'tests/runtime']
    paths = [path for directory in directories for path in directory.rglob('*') if path.is_file()]
    paths += [ROOT / 'backend/Cargo.toml', ROOT / 'backend/Cargo.lock', REPO / 'manga-protocol-lab/server/Cargo.toml', REPO / 'manga-protocol-lab/server/Cargo.lock', REPO / 'manga-protocol-lab/browser-client/package-lock.json', REPO / '.ci/browser/package.json', REPO / '.ci/browser/package-lock.json']
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
    if result.returncode or value.get('process_exit_code') != 0 or value.get('deadline_exceeded') is not False or value.get('cleanup_verified') is not True or value.get('surviving_owned_process_ids') != []:
        raise RuntimeError('Owned preparation stage failed: ' + label)
    return value


def main():
    STAGE.mkdir(parents=True, exist_ok=True)
    PUBLIC.mkdir(exist_ok=True)
    public = {'schema': 1, 'status': 'preparation_failed', 'browser_phases_started': False, 'stage': 'source-preservation', 'source_files_checked': 0, 'frontend_preserved': False, 'artifacts_uploaded': False}
    write(PUBLIC / 'preparation.json', public)
    try:
        verify_frontend()
        before = source_snapshot()
        public.update(source_files_checked=676, frontend_preserved=True)
        env = dict(os.environ)
        env.update(RUSTUP_TOOLCHAIN='1.90.0', CARGO_TARGET_DIR=str(ROOT / '.build/hosted-native'), WEEBDEX_FRONTEND_ROOT=str(ROOT / '.build/frontend-hosted'))
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
        for label, argv, cwd, seconds in stages:
            public['stage'] = label
            write(PUBLIC / 'preparation.json', public)
            receipt = run(label, argv, cwd, seconds)
            if source_snapshot() != before:
                raise RuntimeError('Frozen source changed during preparation')
            if label == 'frontend-build':
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
        verify_frontend()
        if source_snapshot() != before:
            raise RuntimeError('Frozen source changed at the final boundary')
        manifest = ROOT / 'test-evidence/frontend/artifact-manifest.json'
        normal = ROOT / '.build/hosted-native/debug/weebdex-platform'
        provisioner = ROOT / '.build/hosted-native/debug/examples/browser_fixture_provisioner'
        runtime = json.loads((STAGE / 'runtime.json').read_text())
        plan = {'schema': 1, 'normal_binary': str(normal), 'normal_sha256': sha(normal), 'provisioner': str(provisioner), 'provisioner_sha256': sha(provisioner), 'frontend_manifest': str(manifest), 'frontend_manifest_sha256': sha(manifest), 'frontend_index_sha256': sha(ROOT / 'artifacts/frontend/index.html'), 'node': subprocess.check_output(['node', '-p', 'process.execPath'], text=True).strip(), 'runtime_sha256': sha(STAGE / 'runtime.json')}
        write(STAGE / 'plan.json', plan)
        import importlib.util
        spec = importlib.util.spec_from_file_location('prepared_native_feed_contract', ROOT / 'scripts/ci/native-feed-diagnostics.py')
        native_feed = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(native_feed)
        public.update(status='prepared', stage='complete', source_snapshot_sha256=hashlib.sha256(json.dumps(before, sort_keys=True).encode()).hexdigest(), frontend_manifest_sha256=plan['frontend_manifest_sha256'], frontend_index_sha256=plan['frontend_index_sha256'], normal_binary_sha256=plan['normal_sha256'], provisioner_sha256=plan['provisioner_sha256'], playwright='1.62.1', chromium_revision='1234', chromium_binary_sha256=runtime['executable_sha256'], fresh_build=True, historical_binaries_reused=False, browser_phases_started=False, ipc_success_established=False, native_feed_regression=native_feed.not_run())
        write(PUBLIC / 'preparation.json', public)
        return 0
    except Exception as error:
        public['error_class'] = type(error).__name__
        write(PUBLIC / 'preparation.json', public)
        return 1


if __name__ == '__main__':
    sys.exit(main())
