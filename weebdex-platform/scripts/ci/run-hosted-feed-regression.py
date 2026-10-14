#!/usr/bin/env python3
"""Bound the real disposable-Postgres regression and expose only closed facts."""
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[2]
STAGE = ROOT / '.build/hosted-browser'
PUBLIC = ROOT / 'ci-evidence/preparation.json'


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


contract = load('native_feed_contract', ROOT / 'scripts/ci/native-feed-diagnostics.py')


def read(path, limit=2 * 1024 * 1024):
    if path.is_symlink() or not path.is_file() or path.stat().st_size > limit:
        raise RuntimeError('Unavailable private native-feed input')
    return json.loads(path.read_text(), object_pairs_hook=contract.unique_object)


def write(value):
    temporary = PUBLIC.with_suffix('.native-new')
    if temporary.exists():
        raise RuntimeError('Uncertain native-feed result staging')
    with temporary.open('x') as output:
        output.write(json.dumps(value, indent=2) + '\n')
    temporary.replace(PUBLIC)


def main():
    if os.environ.get('GITHUB_ACTIONS') != 'true' or os.environ.get('RUNNER_OS') != 'Linux' or sys.platform != 'linux':
        raise RuntimeError('Reviewed hosted Linux runner required; no local native retry')
    preparation = read(PUBLIC)
    if preparation.get('status') != 'prepared' or preparation.get('native_feed_regression') != contract.not_run():
        raise RuntimeError('Fresh prepared generation and unrun native gate required')
    source = load('native_feed_frozen_source', ROOT / 'scripts/ci/prepare-hosted-browser.py')
    before = source.source_snapshot()
    if hashlib.sha256(json.dumps(before, sort_keys=True).encode()).hexdigest() != preparation.get('source_snapshot_sha256'):
        raise RuntimeError('Frozen native-feed source changed')
    receipt, log = STAGE / 'native-feed-owned.json', STAGE / 'native-feed.log'
    if receipt.exists() or log.exists():
        raise RuntimeError('Refusing prior native-feed execution')
    runtime = ROOT / 'test-runtime'
    old = set(runtime.glob('auth-*')) if runtime.exists() else set()
    env = dict(os.environ, RUSTUP_TOOLCHAIN='1.90.0', CARGO_TARGET_DIR=str(ROOT / '.build/hosted-native'), CARGO_BUILD_JOBS='2')
    # Existing owned command lifecycle and disposable PostgreSQL runner. Neither
    # raw subprocess output nor its private receipt is printed or uploaded.
    command = [sys.executable, str(ROOT / 'scripts/run-bounded.py'), '--cwd', str(ROOT),
               '--deadline', '600', '--receipt', str(receipt), '--log', str(log), '--',
               'bash', str(ROOT / 'scripts/run-feed-projection-tests.sh')]
    result = subprocess.run(command, env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    owned, observed, fixture = None, None, None
    try:
        owned = read(receipt)
        if log.is_symlink() or not log.is_file() or log.stat().st_size > 8 * 1024 * 1024:
            raise RuntimeError('Unsafe native-feed log')
        observed = contract.parse_measurement(log.read_text())
    except (OSError, ValueError, RuntimeError):
        pass  # Missing/malformed measurements remain an explicit failed gate.
    fresh = set(runtime.glob('auth-*')) - old
    if len(fresh) == 1:
        directory = fresh.pop()
        if directory.is_dir() and not directory.is_symlink():
            try:
                fixture = read(directory / 'cleanup.json')
            except (OSError, ValueError, RuntimeError):
                pass
    value = contract.project(owned, observed, fixture)
    if result.returncode != 0 or source.source_snapshot() != before:
        value['status'] = 'failed'
        value['measurement'] = None
    contract.validate(value)
    preparation['native_feed_regression'] = value
    write(preparation)
    print(json.dumps({'native_feed_regression': value}, sort_keys=True))
    return 0 if value['status'] == 'passed' else 1


if __name__ == '__main__':
    sys.exit(main())
