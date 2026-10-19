"""Closed title-command gate and private compiled-artifact binding helpers."""
import hashlib
import json
import os
from pathlib import Path
import re

UNIT_PREFIX = 'public_api::title_disposition::'
UNIT_TESTS = (
    UNIT_PREFIX + 'raw_command_tests::signed_disposition_keeps_closed_object_and_scalar_contracts',
    UNIT_PREFIX + 'raw_command_tests::signed_disposition_rejects_duplicate_authority_and_effect_keys',
    UNIT_PREFIX + 'tests::disposition_requires_exact_version_account_and_removal_effect',
)
HTTP_TEST = 'signed_title_create_edit_submit_publish_and_overlay_preserve_original_authority'
CORE_HASHES = {
    'backend/src/public_api/title_disposition.rs': '859adf8f7eea76ab3f6f71686c83c17a1bf34e52afc0c73e5878d1392eb334e0',
    'backend/tests/public_manga_writes.rs': 'c78673bfbe0008231f9d82a72a82f9d271f875b6319329bd05732b6e3acb6022',
    'tests/runtime/auth_fixture_guard.py': '17ff9c2eb04a4bdefaa3b61e975e77567f873208a83454afcdd6ca19c5cfc33e',
    'scripts/run-auth-tests.sh': '119f49f47d217c3650cd8f34f526b120cab41a585e7f5003759df062edd26ec5',
}
TOOL_PATHS = {'strfry': '../manga-protocol-lab/vendor/strfry/strfry',
              **{name: '../manga-protocol-lab/vendor/pgsql/bin/' + name
                 for name in ['postgres', 'initdb', 'createdb', 'pg_isready']}}
STAGE_FIELDS = {'process_started', 'process_exit_code', 'deadline_exceeded', 'owned_terminal',
                'cleanup_verified', 'exact_tests_verified', 'passed_tests', 'failed_tests',
                'ignored_tests', 'fixture_cleanup_verified'}
RESULT_FIELDS = {'schema', 'status', 'source_verified', 'artifacts_verified', 'budget_exhausted',
                 'units', 'http', 'negative_requests_verified', 'executable_sha256'}


def insist(value):
    if not value:
        raise ValueError('Native title gate failed its closed contract')


def unique_object(pairs):
    value = {}
    for key, item in pairs:
        insist(key not in value)
        value[key] = item
    return value


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def verify_core(root):
    for relative, digest in CORE_HASHES.items():
        path = root / relative
        insist(not path.is_symlink() and path.is_file() and sha(path) == digest)


def executable_path(value, root, kind):
    insist(type(value) is str)
    path = Path(value)
    target = root / '.build/hosted-native/debug/deps'
    prefix = 'weebdex_platform' if kind == 'library' else 'public_manga_writes'
    insist(path.is_absolute() and path.parent == target and path.resolve() == path)
    insist(re.fullmatch(prefix + r'-[0-9a-f]{16}', path.name))
    insist(not path.is_symlink() and path.is_file() and os.access(path, os.X_OK))
    return path


def artifacts(text, root, source_hash):
    """Select actual Cargo executables, never a guessed target/cache filename."""
    verify_core(root)
    selected, finished = {}, []
    for line in text.splitlines():
        if not line.startswith('{'):
            continue  # Cargo diagnostics stay private; only typed JSON is used.
        row = json.loads(line, object_pairs_hook=unique_object)
        if row.get('reason') == 'build-finished':
            finished.append(row.get('success'))
        if row.get('reason') != 'compiler-artifact' or row.get('profile', {}).get('test') is not True or row.get('executable') is None:
            continue
        target = row.get('target', {})
        kind = None
        if target.get('name') == 'weebdex_platform' and target.get('kind') == ['lib']:
            kind = 'library'
        elif target.get('name') == 'public_manga_writes' and target.get('kind') == ['test']:
            kind = 'http'
        if kind is not None:
            insist(kind not in selected)
            path = executable_path(row['executable'], root, kind)
            selected[kind] = {'path': str(path), 'sha256': sha(path)}
    insist(finished == [True] and set(selected) == {'library', 'http'})
    tools = {}
    for name, relative in TOOL_PATHS.items():
        path = (root / relative).resolve()
        insist(not path.is_symlink() and path.is_file() and os.access(path, os.X_OK))
        tools[name] = sha(path)
    value = {'schema': 1, 'source_snapshot_sha256': source_hash, 'executables': selected, 'tools': tools}
    verify_artifacts(value, root, source_hash)
    return value


def verify_artifacts(value, root, source_hash):
    insist(type(value) is dict and set(value) == {'schema', 'source_snapshot_sha256', 'executables', 'tools'})
    insist(type(value['schema']) is int and value['schema'] == 1)
    insist(type(source_hash) is str and re.fullmatch('[0-9a-f]{64}', source_hash))
    insist(value['source_snapshot_sha256'] == source_hash)
    insist(type(value['executables']) is dict and set(value['executables']) == {'library', 'http'})
    verify_core(root)
    output = {}
    for kind, entry in value['executables'].items():
        insist(type(entry) is dict and set(entry) == {'path', 'sha256'})
        path = executable_path(entry['path'], root, kind)
        insist(type(entry['sha256']) is str and sha(path) == entry['sha256'])
        output[kind] = path
    insist(type(value['tools']) is dict and set(value['tools']) == set(TOOL_PATHS))
    for name, relative in TOOL_PATHS.items():
        candidate = root / relative
        insist(not candidate.is_symlink() and candidate.is_file() and candidate.resolve() == Path(os.path.abspath(candidate)))
        insist(os.access(candidate, os.X_OK) and sha(candidate) == value['tools'][name])
    return output


def empty_stage():
    return {'process_started': False, 'process_exit_code': None, 'deadline_exceeded': False,
            'owned_terminal': False, 'cleanup_verified': False, 'exact_tests_verified': False,
            'passed_tests': 0, 'failed_tests': 0, 'ignored_tests': 0, 'fixture_cleanup_verified': False}


def not_run():
    return {'schema': 1, 'status': 'not-run', 'source_verified': False, 'artifacts_verified': False,
            'budget_exhausted': False, 'units': empty_stage(), 'http': empty_stage(),
            'negative_requests_verified': 0, 'executable_sha256': {'library': None, 'http': None}}


def parse_tests(text, expected):
    """Require the exact real Rust harness methods and one terminal summary."""
    running, rows, summaries = [], [], []
    for line in text.splitlines():
        if match := re.fullmatch(r'running ([0-9]+) tests?', line.strip()):
            running.append(int(match[1]))
        if match := re.fullmatch(r'test ([A-Za-z0-9_:]+) \.\.\. (ok|FAILED|ignored(?:, .*)?)', line.strip()):
            rows.append((match[1], match[2]))
        if match := re.fullmatch(r'test result: (ok|FAILED)\. ([0-9]+) passed; ([0-9]+) failed; ([0-9]+) ignored; 0 measured; [0-9]+ filtered out; finished in [0-9.]+s', line.strip()):
            summaries.append((match[1], *(int(match[index]) for index in [2, 3, 4])))
    insist(running == [len(expected)] and len(summaries) == 1)
    insist(sorted(name for name, _ in rows) == sorted(expected))
    summary = summaries[0]
    insist(summary[1:] == (sum(status == 'ok' for _, status in rows),
                           sum(status == 'FAILED' for _, status in rows),
                           sum(status.startswith('ignored') for _, status in rows)))
    insist((summary[0] == 'ok') == (summary[2] == 0))
    return {'passed_tests': summary[1], 'failed_tests': summary[2], 'ignored_tests': summary[3],
            'exact_tests_verified': True}


def project_stage(owned, observed, fixture_verified=False):
    value = empty_stage()
    if type(owned) is dict:
        insist(type(owned.get('deadline_exceeded')) is bool)
        value['process_started'] = True
        code = owned.get('process_exit_code')
        if type(code) is int and -128 <= code <= 255:
            value['process_exit_code'] = code
        value['deadline_exceeded'] = owned['deadline_exceeded']
        value['owned_terminal'] = owned.get('terminal_receipt') is True
        value['cleanup_verified'] = (owned.get('cleanup_verified') is True
            and owned.get('surviving_owned_process_ids') == []
            and owned.get('cleanup_signals') == []
            and owned.get('prior_or_unrelated_processes_touched') is False)
    if observed is not None:
        insist(type(observed) is dict and set(observed) == {'passed_tests', 'failed_tests', 'ignored_tests', 'exact_tests_verified'})
        value.update(observed)
    value['fixture_cleanup_verified'] = fixture_verified is True
    return value


def stage_passed(value, count, fixture=False):
    return (value['process_started'] and value['process_exit_code'] == 0
            and not value['deadline_exceeded'] and value['owned_terminal'] and value['cleanup_verified']
            and value['exact_tests_verified'] and value['passed_tests'] == count
            and value['failed_tests'] == value['ignored_tests'] == 0
            and (value['fixture_cleanup_verified'] if fixture else not value['fixture_cleanup_verified']))


def passed(value):
    return (value['source_verified'] and value['artifacts_verified'] and not value['budget_exhausted']
            and stage_passed(value['units'], 3) and stage_passed(value['http'], 1, True)
            and value['negative_requests_verified'] == 7)


def validate(value):
    insist(type(value) is dict and set(value) == RESULT_FIELDS)
    insist(type(value['schema']) is int and value['schema'] == 1)
    insist(value['status'] in ['not-run', 'failed', 'passed'])
    for key in ['source_verified', 'artifacts_verified', 'budget_exhausted']:
        insist(type(value[key]) is bool)
    hashes = value['executable_sha256']
    insist(type(hashes) is dict and set(hashes) == {'library', 'http'})
    if value['artifacts_verified']:
        insist(all(type(item) is str and re.fullmatch('[0-9a-f]{64}', item) for item in hashes.values()))
    else:
        insist(all(item is None for item in hashes.values()))
    insist(type(value['negative_requests_verified']) is int and value['negative_requests_verified'] in [0, 7])
    for name, maximum in [('units', 3), ('http', 1)]:
        stage = value[name]
        insist(type(stage) is dict and set(stage) == STAGE_FIELDS)
        for key in STAGE_FIELDS - {'process_exit_code', 'passed_tests', 'failed_tests', 'ignored_tests'}:
            insist(type(stage[key]) is bool)
        insist(stage['process_exit_code'] is None or type(stage['process_exit_code']) is int and -128 <= stage['process_exit_code'] <= 255)
        for key in ['passed_tests', 'failed_tests', 'ignored_tests']:
            insist(type(stage[key]) is int and 0 <= stage[key] <= maximum)
        insist(sum(stage[key] for key in ['passed_tests', 'failed_tests', 'ignored_tests']) <= maximum)
        if not stage['process_started']:
            insist(stage == empty_stage())
        if stage['exact_tests_verified']:
            insist(sum(stage[key] for key in ['passed_tests', 'failed_tests', 'ignored_tests']) == maximum)
    insist(value['negative_requests_verified'] == 0 or value['source_verified'] and value['artifacts_verified'] and stage_passed(value['http'], 1, True))
    if value['status'] == 'not-run':
        insist(value == not_run())
    insist((value['status'] == 'passed') == passed(value))
