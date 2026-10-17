"""Build a separate gate-on artifact; restore every native-profile input."""
import hashlib
import json
import os
from pathlib import Path
import sys


NATIVE_EVIDENCE = ['build-integration.json', 'integration-source-manifest.json',
                   'artifact-manifest.json', 'bootstrap-csp.json', 'execution-status.json']


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def write(path, value):
    path.write_text(json.dumps(value, indent=2) + '\n')


def prepare(root, stage, run, verify_source):
    """The caller's owned runner supplies all execution and deadline handling."""
    evidence = root / 'test-evidence/frontend'
    ui = root / 'artifacts/frontend'
    native = stage / 'public-upload-native-frontend'
    public = stage / 'public-upload-frontend'
    failed = stage / 'public-upload-failed-frontend'
    manifest = stage / 'public-upload-manifest.json'
    build = stage / 'public-upload-build.json'
    for path in [native, public, failed, manifest, build]:
        if path.exists() or path.is_symlink():
            raise RuntimeError('Refusing an existing public-upload profile')
    if ui.is_symlink() or not ui.is_dir():
        raise RuntimeError('A genuine native frontend generation is required')
    saved = {}
    for name in NATIVE_EVIDENCE:
        path = evidence / name
        if path.is_symlink() or not path.is_file():
            raise RuntimeError('Native profile receipt is unavailable')
        saved[name] = path.read_bytes()
    old_gate = os.environ.get('PUBLIC_MANGADEX_CATALOGUE')
    ui.rename(native)
    try:
        os.environ['PUBLIC_MANGADEX_CATALOGUE'] = '1'
        receipt = run('public-upload-frontend-build', ['node', root / 'scripts/build-frontend.mjs'], root, 900)
        verify_source()
        current = json.loads((evidence / 'build-integration.json').read_text())
        if current.get('public_api_catalogue_enabled') is not True or current.get('public_signed_upload_enabled') is not True or current.get('spa_fallback_sha256') != sha(ui / 'index.html'):
            raise RuntimeError('Public upload requires the exact gate-on build receipt')
        result_dir = evidence / 'hosted-public-upload-build'
        result_dir.mkdir()
        write(result_dir / 'build-result.json', {
            'status': 'passed', 'command_exit': receipt['process_exit_code'],
            'remaining_owned_members': receipt['surviving_owned_process_ids'],
            'changed_inputs': [], 'owned_terminal_receipt': str((stage / 'public-upload-frontend-build-owned.json').relative_to(root)),
            'browser_tests_executed': False,
        })
        run('public-upload-artifact-record', [sys.executable, root / 'scripts/record-frontend-artifacts.py', 'hosted-public-upload-build/build-result.json'], root, 60)
        verify_source()
        manifest.write_bytes((evidence / 'artifact-manifest.json').read_bytes())
        build.write_bytes((evidence / 'build-integration.json').read_bytes())
        ui.rename(public)
        return {'enabled': True, 'ui_dir': str(public), 'frontend_manifest': str(manifest),
                'frontend_manifest_sha256': sha(manifest), 'frontend_index_sha256': sha(public / 'index.html'),
                'build_receipt': str(build), 'build_receipt_sha256': sha(build)}
    finally:
        if old_gate is None:
            os.environ.pop('PUBLIC_MANGADEX_CATALOGUE', None)
        else:
            os.environ['PUBLIC_MANGADEX_CATALOGUE'] = old_gate
        if ui.exists() or ui.is_symlink():
            ui.rename(failed)
        native.rename(ui)
        for name, data in saved.items():
            (evidence / name).write_bytes(data)
