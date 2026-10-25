#!/usr/bin/env python3
"""Use existing owned commands/fixture guard for three units and one HTTP case."""
import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location('native_title_gate_contract', ROOT / 'scripts/ci/native-title-diagnostics.py')
contract = importlib.util.module_from_spec(spec)
spec.loader.exec_module(contract)


def read(path, limit=2 * 1024 * 1024):
    if path.is_symlink() or not path.is_file() or path.stat().st_size > limit:
        raise RuntimeError('Unavailable private native-title input')
    return json.loads(path.read_text(), object_pairs_hook=contract.unique_object)


def owned_test(root, stage, label, command, seconds, env, expected):
    receipt, log = stage / (label + '-owned.json'), stage / (label + '.log')
    if receipt.exists() or log.exists():
        raise RuntimeError('Refusing prior native-title execution')
    argv = [sys.executable, str(root / 'scripts/run-bounded.py'), '--cwd', str(root),
            '--deadline', str(seconds), '--receipt', str(receipt), '--log', str(log), '--', *map(str, command)]
    result = subprocess.run(argv, env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    owned, observed = None, None
    try:
        owned = read(receipt)
        if log.is_symlink() or not log.is_file() or log.stat().st_size > 8 * 1024 * 1024:
            raise RuntimeError('Unsafe native-title log')
        observed = contract.parse_tests(log.read_text(), expected)
    except (OSError, ValueError, RuntimeError):
        pass
    try:
        value = contract.project_stage(owned, observed)
    except ValueError:
        value = contract.empty_stage()
    if result.returncode != 0:
        value['exact_tests_verified'] = False
    return value


def run(root, stage, env, deadline, source_hash, verify_source, fixture_validator):
    value = contract.not_run()
    value['status'] = 'failed'
    binding = None
    try:
        plan = read(stage / 'plan.json')
        binding = plan.get('native_title_tests')
        executables = contract.verify_artifacts(binding, root, source_hash)
        verify_source()
        value.update(source_verified=True, artifacts_verified=True)
        value['executable_sha256'] = {kind: entry['sha256'] for kind, entry in binding['executables'].items()}
        # run-bounded may need 2s TERM + 2s KILL + a final ownership probe.
        # Reserve that outer cleanup inside the same shared native deadline.
        if time.monotonic() + 35 > deadline:
            value['budget_exhausted'] = True
            return value
        value['units'] = owned_test(root, stage, 'native-title-units',
            [executables['library'], contract.UNIT_PREFIX, '--test-threads=1'], 30, env, contract.UNIT_TESTS)
        verify_source()
        contract.verify_artifacts(binding, root, source_hash)
        if not contract.stage_passed(value['units'], 3):
            return value
        # Reserve the full fixed guard envelope before starting a new DB. Never
        # shorten its 180-second test or 15/5/5 cleanup policy to fit the budget.
        if time.monotonic() + 305 > deadline:
            value['budget_exhausted'] = True
            return value
        runtime = root / 'test-runtime'
        if runtime.is_symlink():
            raise RuntimeError('Native fixture runtime cannot be a link')
        old = set(runtime.glob('auth-*')) if runtime.exists() else set()
        native_env = dict(env, STRFRY_BIN=str((root / contract.TOOL_PATHS['strfry']).resolve()),
                          PLATFORM_TEST_PG_BIN=str((root / contract.TOOL_PATHS['postgres']).resolve().parent))
        command = ['bash', root / 'scripts/run-auth-tests.sh', executables['http'],
                   contract.HTTP_TEST, '--exact', '--ignored', '--test-threads=1']
        value['http'] = owned_test(root, stage, 'native-title-http', command, 300, native_env, (contract.HTTP_TEST,))
        fresh = set(runtime.glob('auth-*')) - old
        value['fixture_diagnostics'] = contract.empty_fixture_diagnostics('ambiguous' if len(fresh) > 1 else 'unavailable')
        if len(fresh) == 1:
            directory = fresh.pop()
            if not directory.is_symlink() and directory.is_dir():
                try:
                    fixture = read(directory / 'cleanup.json')
                    verified = fixture_validator(fixture)
                    value['fixture_diagnostics'] = contract.fixture_diagnostics(fixture, verified)
                    value['http']['fixture_cleanup_verified'] = value['fixture_diagnostics']['normal_run_verified']
                except (OSError, ValueError, RuntimeError):
                    value['fixture_diagnostics'] = contract.empty_fixture_diagnostics('invalid')
        verify_source()
        contract.verify_artifacts(binding, root, source_hash)
        if contract.stage_passed(value['http'], 1, True):
            value['negative_requests_verified'] = 7
        if contract.passed(value):
            value['status'] = 'passed'
    except (OSError, ValueError, RuntimeError, KeyError, TypeError):
        value.update(source_verified=False, artifacts_verified=False, negative_requests_verified=0)
        value['executable_sha256'] = {'library': None, 'http': None}
    finally:
        contract.validate(value)
    return value
