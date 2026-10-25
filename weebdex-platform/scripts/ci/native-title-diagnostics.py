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
    'tests/runtime/auth_fixture_guard.py': '8bd64b731f4e608989dd12b129be0323055dfa71c01076e0d0cc71b968d52bec',
    'scripts/run-auth-tests.sh': '119f49f47d217c3650cd8f34f526b120cab41a585e7f5003759df062edd26ec5',
}
# Historical qualification remains bound to its original source, not this
# successor. Only the reviewed officialLinks HTTP assertions changed; this
# association is not a new execution or compiled-artifact receipt.
HISTORICAL_CORE_HASHES = dict(CORE_HASHES)
CORE_SOURCE_SUCCESSOR = {
    'path': 'backend/tests/public_manga_writes.rs',
    'prior_sha256': 'c78673bfbe0008231f9d82a72a82f9d271f875b6319329bd05732b6e3acb6022',
    'current_sha256': 'a4db6be2152bae12c7a65d78006a4c805f51f703e7aeb00ab71c17c9f4d8c5e7',
    'reviewed_commit': '4563d7dff0e31d9e9e2ce836434da4fc7559b093',
    'reason': 'Reviewed officialLinks dictionary permission, validation, isolation and audit assertions extend the existing owned title HTTP scenario; disposition, auth guard and launcher bytes are unchanged.',
}
OFFICIAL_LINKS_CORE_HASHES = {**HISTORICAL_CORE_HASHES,
               CORE_SOURCE_SUCCESSOR['path']: CORE_SOURCE_SUCCESSOR['current_sha256']}
# Later source-only association: preserve the earlier exact four-file pinset,
# and protect both newly imported cover helpers plus their shared fixture bytes.
CORE_COVER_SUCCESSOR = {
    'path': 'backend/tests/public_manga_writes.rs',
    'prior_sha256': 'a4db6be2152bae12c7a65d78006a4c805f51f703e7aeb00ab71c17c9f4d8c5e7',
    'current_sha256': '1afe8d6e2f5c1439207cac8f5e65cc53e5d4f79fdf76978980b21c82854e8851',
    'reviewed_commit': '55a55d6578d991285caaef1eff362ad2f9fc6f1c',
    'reason': 'Reviewed primary-cover and bounded signed-cover-pagination test modules plus query-aware private-cache assertion; original title-disposition scenario and three other core files remain unchanged. No prior binary or execution receipt is relabelled.',
    'added_sources': {
        'backend/tests/support/primary_cover_contract.rs': 'd3e4a1f3a7745c5468ba053edfcf98438bcd2bf28da9f74f6cb3da1fefd1f188',
        'backend/tests/support/cover_pagination.rs': '1d200c822ab6cff4888b9964bfb7e6706802e031657f065f6a7f9c7ae549071f',
        '../manga-protocol-lab/server/tests/support/mod.rs': 'bf78e30a51e798537eb04366a7c39761b85638445746a528635dc5385b9496ca',
    },
}
COVER_CORE_HASHES = {**OFFICIAL_LINKS_CORE_HASHES,
              CORE_COVER_SUCCESSOR['path']: CORE_COVER_SUCCESSOR['current_sha256'],
              **CORE_COVER_SUCCESSOR['added_sources']}
# Public-language coverage extends, never replaces, the immutable cover pinset.
CORE_LANGUAGE_SUCCESSOR = {
    'path': 'backend/tests/public_manga_writes.rs',
    'prior_sha256': '1afe8d6e2f5c1439207cac8f5e65cc53e5d4f79fdf76978980b21c82854e8851',
    'current_sha256': '968d201297b0f86f78ff781d598f775790f96b5aa882869da061cbfea36ee7c1',
    'reviewed_commit': '99483383cf6f50802945a731a5f1665c64038c42',
    'reason': 'Reviewed public-language HTTP/restart modules and public-language negative input correction; query-aware cache assertion and both cover modules remain intact. The other six protected core sources are byte-identical. No prior execution is relabelled.',
    'added_sources': {
        'backend/tests/support/public_language_contract.rs': '8f3acba1248f6641ec61aebfe59e1e0dd78a84f0beab5c845bcf253563eb1a41',
        'backend/tests/support/public_language_restart.rs': '5474f85a2d3573abfd401243914fa65c4d1a18f3f1dafa018b23746ba26ea1f8',
    },
}
LANGUAGE_CORE_HASHES = {**COVER_CORE_HASHES,
              CORE_LANGUAGE_SUCCESSOR['path']: CORE_LANGUAGE_SUCCESSOR['current_sha256'],
              **CORE_LANGUAGE_SUCCESSOR['added_sources']}
# The reviewed Unicode fixture adds exactly one module; it is not an old HTTP pass.
CORE_COVER_UNICODE_SUCCESSOR = {
    'path': 'backend/tests/public_manga_writes.rs',
    'prior_sha256': '968d201297b0f86f78ff781d598f775790f96b5aa882869da061cbfea36ee7c1',
    'current_sha256': '6180cd02a63bde2c8d0d511471dc47d3e28c8fdf17a2f41963a268a22f90b0f4',
    'reviewed_commits': ['72ade3a90862b7f580e37880004a18e547229616', '73929bb8e674b73eeae8bab8d824e5431d956a3a'],
    'source_freeze': '216ba56ad48e1379dc56b13a611a229695a10edc',
    'status': 'source-association-only-no-execution',
    'added_sources': {
        'backend/tests/support/cover_unicode.rs': '81fbf0eb961b1421631ea0a9eeb2ee26e273595641e7833004f165c1c04ef3db',
    },
}
COVER_UNICODE_APPEND = b'\n#[path = "support/cover_unicode.rs"]\nmod cover_unicode;\n'
COVER_UNICODE_CORE_HASHES = {**LANGUAGE_CORE_HASHES,
              CORE_COVER_UNICODE_SUCCESSOR['path']: CORE_COVER_UNICODE_SUCCESSOR['current_sha256'],
              **CORE_COVER_UNICODE_SUCCESSOR['added_sources']}
# Account-disable admission now requires binding each fixture identity while its
# user is enabled, then applying the existing disabled fixture state. This exact
# source association neither changes the scenario nor renews historical proof.
CORE_ACCOUNT_DISABLE_SUCCESSOR = {
    'path': 'backend/tests/public_manga_writes.rs',
    'prior_sha256': '6180cd02a63bde2c8d0d511471dc47d3e28c8fdf17a2f41963a268a22f90b0f4',
    'current_sha256': 'f499311565a0ee327528de678502088b7278b09b7d98753c3e67b5a7086147c4',
    'reviewed_commit': 'dfa6135752d9e01c5bb5afa56cfc48b2bfa79bda',
    'source_freeze': '11ea3ea9f593c134f2ba478bb84fa683f4b14149',
    'status': 'source-association-only-no-execution',
}
ACCOUNT_DISABLE_PRIOR = (
    b'            &[&user,&format!("upload-{key}@fixture.invalid"),&(key!=5),&(key==6),&roles,&permissions]).await.unwrap();\n'
    b'        db.execute("INSERT INTO platform.protocol_identity_bindings(publisher,user_id) VALUES($1,$2)",&[&public(key),&user]).await.unwrap();\n'
)
ACCOUNT_DISABLE_CURRENT = (
    b'            &[&user,&format!("upload-{key}@fixture.invalid"),&(key!=5),&false,&roles,&permissions]).await.unwrap();\n'
    b'        db.execute("INSERT INTO platform.protocol_identity_bindings(publisher,user_id) VALUES($1,$2)",&[&public(key),&user]).await.unwrap();\n'
    b'        db.execute("UPDATE platform.users SET disabled=$2 WHERE id=$1",&[&user,&(key==6)]).await.unwrap();\n'
)
CORE_HASHES = {**COVER_UNICODE_CORE_HASHES,
              CORE_ACCOUNT_DISABLE_SUCCESSOR['path']: CORE_ACCOUNT_DISABLE_SUCCESSOR['current_sha256']}
TOOL_PATHS = {'strfry': '../manga-protocol-lab/vendor/strfry/strfry',
              **{name: '../manga-protocol-lab/vendor/pgsql/bin/' + name
                 for name in ['postgres', 'initdb', 'createdb', 'pg_isready']}}
STAGE_FIELDS = {'process_started', 'process_exit_code', 'deadline_exceeded', 'owned_terminal',
                'cleanup_verified', 'exact_tests_verified', 'passed_tests', 'failed_tests',
                'ignored_tests', 'fixture_cleanup_verified'}
RESULT_FIELDS = {'schema', 'status', 'source_verified', 'artifacts_verified', 'budget_exhausted',
                 'units', 'http', 'negative_requests_verified', 'executable_sha256', 'fixture_diagnostics'}
FIXTURE_FIELDS = {'postgres_cleanup', 'exit', 'forced', 'closed_port', 'remaining_owned_processes',
                  'cleanup_errors', 'error', 'test_timeout_seconds', 'captured_births',
                  'unconfirmed_births', 'unconfirmed_child_pids', 'test_returncode',
                  'postgres_returncode', 'startup_timeout_seconds', 'cleanup_phase_seconds'}
FIXTURE_DIAGNOSTIC_FIELDS = {'schema', 'receipt_state', 'error_family', 'error_reason',
    'physical_stopped_reported', 'forced', 'test_exit', 'database_exit', 'normal_run_verified'}
ERROR_REASONS = {
    'RuntimeError: owned child birth/session unavailable': 'owned-birth',
    'RuntimeError: isolated initdb failed': 'initdb',
    'RuntimeError: isolated postgres exited before readiness': 'postgres-exit',
    'RuntimeError: owned postgres startup deadline': 'startup-deadline',
    'RuntimeError: postgres socket ownership changed before createdb': 'pre-create-ownership',
    'RuntimeError: isolated createdb failed': 'createdb',
    'RuntimeError: postgres socket ownership changed before tests': 'pre-test-ownership',
    'postgres exited before cleanup': 'postgres-exit',
    'interrupted': 'interrupted', 'owned command deadline': 'deadline',
}


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


def restore_cover_unicode_append(source, successor=None):
    value = CORE_COVER_UNICODE_SUCCESSOR if successor is None else successor
    insist(hashlib.sha256(json.dumps(value, separators=(',', ':')).encode()).hexdigest() == '3eab371db1c437dc4f953b3b14f327e0e4cc78bb2cc40e97982b19d2434844af')
    insist(type(source) is bytes and hashlib.sha256(source).hexdigest() == value['current_sha256'])
    insist(source.endswith(COVER_UNICODE_APPEND) and source.count(COVER_UNICODE_APPEND) == 1)
    previous = source[:-len(COVER_UNICODE_APPEND)]
    insist(hashlib.sha256(previous).hexdigest() == value['prior_sha256'])
    return previous


def restore_account_disable_fixture(source, successor=None):
    """Return predecessor bytes for association checking only, never execution."""
    value = CORE_ACCOUNT_DISABLE_SUCCESSOR if successor is None else successor
    insist(hashlib.sha256(json.dumps(value, separators=(',', ':')).encode()).hexdigest() == '125978668f6bd542d8470bbd38b21012ee22d5a0c522e9ce1bedbbfc4079ca5d')
    insist(type(source) is bytes and hashlib.sha256(source).hexdigest() == value['current_sha256'])
    insist(source.count(ACCOUNT_DISABLE_CURRENT) == 1 and source.count(ACCOUNT_DISABLE_PRIOR) == 0)
    previous = source.replace(ACCOUNT_DISABLE_CURRENT, ACCOUNT_DISABLE_PRIOR, 1)
    insist(hashlib.sha256(previous).hexdigest() == value['prior_sha256'])
    return previous


def verify_core(root):
    for relative, digest in CORE_HASHES.items():
        path = root / relative
        insist(not path.is_symlink() and path.is_file() and sha(path) == digest)
    # Validate both predecessor seams in memory; builds and runtime bindings use
    # only the current on-disk sources pinned by CORE_HASHES above.
    previous = restore_account_disable_fixture((root / CORE_ACCOUNT_DISABLE_SUCCESSOR['path']).read_bytes())
    restore_cover_unicode_append(previous)


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


def empty_fixture_diagnostics(state='not-run'):
    return {'schema': 1, 'receipt_state': state, 'error_family': 'unavailable',
            'error_reason': 'unavailable', 'physical_stopped_reported': False,
            'forced': False, 'test_exit': None, 'database_exit': None,
            'normal_run_verified': False}


def fixture_diagnostics(raw, normal=False):
    """Fixed facts from the existing private receipt, never raw errors or IDs.

    Reported physical stop and the unchanged strict normal-run validator are
    different facts. A nullable child exit does not establish whether it ran.
    """
    invalid = empty_fixture_diagnostics('invalid')
    if type(raw) is not dict or set(raw) != FIXTURE_FIELDS or type(normal) is not bool:
        return invalid
    if raw['postgres_cleanup'] not in ['verified_stopped', 'cleanup_unconfirmed']:
        return invalid
    if any(type(raw[key]) is not bool for key in ['forced', 'closed_port']):
        return invalid
    for key in ['exit', 'test_returncode', 'postgres_returncode']:
        code = raw[key]
        if code is None and key != 'exit':
            continue
        if type(code) is not int or not -128 <= code <= 255:
            return invalid
    error = raw['error']
    if error is not None and (type(error) is not str or len(error) > 65536):
        return invalid
    for key in ['captured_births', 'remaining_owned_processes', 'unconfirmed_births']:
        rows, seen = raw[key], set()
        if type(rows) is not list or len(rows) > 4096:
            return invalid
        for row in rows:
            if type(row) is not dict or set(row) != {'pid', 'start_ticks', 'group', 'session'}:
                return invalid
            for name, value in row.items():
                maximum = (1 << 63) - 1 if name == 'start_ticks' else (1 << 31) - 1
                if type(value) is not int or not 0 < value <= maximum:
                    return invalid
            identity = (row['pid'], row['start_ticks'])
            if identity in seen:
                return invalid
            seen.add(identity)
    pids, errors = raw['unconfirmed_child_pids'], raw['cleanup_errors']
    if (type(pids) is not list or len(pids) > 4096 or
            any(type(pid) is not int or not 0 < pid < (1 << 31) for pid in pids) or len(set(pids)) != len(pids)):
        return invalid
    if (type(errors) is not list or len(errors) > 4096 or
            any(type(item) is not str or len(item) > 256 for item in errors)):
        return invalid
    for key in ['test_timeout_seconds', 'startup_timeout_seconds']:
        if type(raw[key]) is not int or not 0 <= raw[key] <= 86400:
            return invalid
    phases = raw['cleanup_phase_seconds']
    if type(phases) is not list or len(phases) != 3 or any(type(n) is not int or not 0 <= n <= 86400 for n in phases):
        return invalid
    if normal and (raw['exit'] != 0 or not raw['captured_births'] or
                   raw['test_timeout_seconds'] != 180 or raw['startup_timeout_seconds'] != 10 or phases != [15, 5, 5]):
        return invalid
    family = 'none' if error is None else 'other'
    if error == 'interrupted':
        family = 'interrupted'
    elif error == 'owned command deadline':
        family = 'deadline'
    elif error is not None:
        for prefix, name in [('FileNotFoundError:', 'file-not-found'), ('PermissionError:', 'permission'),
                             ('RuntimeError:', 'runtime'), ('OSError:', 'os')]:
            if error.startswith(prefix):
                family = name
                break
    physical = (raw['postgres_cleanup'] == 'verified_stopped' and raw['closed_port'] is True
                and all(raw[key] == [] for key in ['remaining_owned_processes', 'cleanup_errors',
                                                 'unconfirmed_births', 'unconfirmed_child_pids']))
    value = {'schema': 1, 'receipt_state': 'available', 'error_family': family,
             'error_reason': 'none' if error is None else ERROR_REASONS.get(error, 'other'),
             'physical_stopped_reported': physical, 'forced': raw['forced'],
             'test_exit': raw['test_returncode'], 'database_exit': raw['postgres_returncode'],
             'normal_run_verified': normal}
    try:
        validate_fixture_diagnostics(value)
    except ValueError:
        return invalid
    return value


def validate_fixture_diagnostics(value):
    insist(type(value) is dict and set(value) == FIXTURE_DIAGNOSTIC_FIELDS)
    insist(type(value['schema']) is int and value['schema'] == 1)
    state = value['receipt_state']
    insist(state in ['not-run', 'unavailable', 'ambiguous', 'invalid', 'available'])
    for key in ['physical_stopped_reported', 'forced', 'normal_run_verified']:
        insist(type(value[key]) is bool)
    for key in ['test_exit', 'database_exit']:
        insist(value[key] is None or type(value[key]) is int and -128 <= value[key] <= 255)
    if state != 'available':
        insist(value == empty_fixture_diagnostics(state))
        return
    insist(value['error_family'] in ['none', 'interrupted', 'deadline', 'file-not-found', 'permission', 'runtime', 'os', 'other'])
    insist(value['error_reason'] in set(ERROR_REASONS.values()) | {'none', 'other'})
    insist((value['error_family'] == 'none') == (value['error_reason'] == 'none'))
    if value['error_reason'] in ['owned-birth', 'initdb', 'startup-deadline', 'pre-create-ownership', 'createdb', 'pre-test-ownership']:
        insist(value['error_family'] == 'runtime')
    elif value['error_reason'] == 'postgres-exit':
        insist(value['error_family'] in ['runtime', 'other'])
    elif value['error_reason'] in ['interrupted', 'deadline']:
        insist(value['error_family'] == value['error_reason'])
    if value['normal_run_verified']:
        insist(value['physical_stopped_reported'] and not value['forced'] and value['error_family'] == 'none'
               and value['test_exit'] == value['database_exit'] == 0)


def not_run():
    return {'schema': 1, 'status': 'not-run', 'source_verified': False, 'artifacts_verified': False,
            'budget_exhausted': False, 'units': empty_stage(), 'http': empty_stage(),
            'negative_requests_verified': 0, 'executable_sha256': {'library': None, 'http': None},
            'fixture_diagnostics': empty_fixture_diagnostics()}


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
            and value['negative_requests_verified'] == 7 and value['fixture_diagnostics']['normal_run_verified'])


def validate(value):
    insist(type(value) is dict and set(value) == RESULT_FIELDS)
    insist(type(value['schema']) is int and value['schema'] == 1)
    insist(value['status'] in ['not-run', 'failed', 'passed'])
    validate_fixture_diagnostics(value['fixture_diagnostics'])
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
    insist(value['http']['fixture_cleanup_verified'] == value['fixture_diagnostics']['normal_run_verified'])
    if value['status'] == 'not-run':
        insist(value == not_run())
    insist((value['status'] == 'passed') == passed(value))
