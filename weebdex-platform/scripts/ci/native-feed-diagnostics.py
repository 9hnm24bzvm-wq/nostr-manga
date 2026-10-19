"""Closed native-feed result contract; no raw messages, DSNs, paths or SQL."""
import json
import re
import importlib.util
from pathlib import Path

spec = importlib.util.spec_from_file_location('native_title_closed_contract', Path(__file__).with_name('native-title-diagnostics.py'))
title = importlib.util.module_from_spec(spec)
spec.loader.exec_module(title)


FLAGS = {'all_505_ids_reached', 'bilingual506_preserved', 'canonical_sample_parity',
         'visibility_withdrawals_verified', 'statement_timeout_unchanged'}
MEASUREMENT_FIELDS = FLAGS | {'scenario', 'baseline_outcome', 'baseline_ms', 'batched_first_page_ms'}
RESULT_FIELDS = {'schema', 'status', 'process_started', 'process_exit_code', 'deadline_exceeded',
                 'owned_terminal', 'cleanup_verified', 'fixture_cleanup_verified', 'measurement', 'title_disposition'}
FIXTURE_FIELDS = {'postgres_cleanup', 'exit', 'forced', 'closed_port', 'remaining_owned_processes',
                  'cleanup_errors', 'error', 'test_timeout_seconds', 'captured_births',
                  'unconfirmed_births', 'unconfirmed_child_pids', 'test_returncode',
                  'postgres_returncode', 'startup_timeout_seconds', 'cleanup_phase_seconds'}
BIRTH_FIELDS = {'pid', 'start_ticks', 'group', 'session'}


def insist(value):
    if not value:
        raise ValueError('Native feed evidence failed its closed contract')


def unique_object(pairs):
    value = {}
    for key, item in pairs:
        insist(key not in value)
        value[key] = item
    return value


def not_run():
    return {'schema': 1, 'status': 'not-run', 'process_started': False, 'process_exit_code': None,
            'deadline_exceeded': False, 'owned_terminal': False, 'cleanup_verified': False,
            'fixture_cleanup_verified': False, 'measurement': None, 'title_disposition': title.not_run()}


def measurement(value):
    insist(type(value) is dict and set(value) == MEASUREMENT_FIELDS)
    insist(value['scenario'] == 'compact-feed505-plus10000-metadata')
    insist(value['baseline_outcome'] in ['success', 'query-canceled'])
    for key in ['baseline_ms', 'batched_first_page_ms']:
        insist(type(value[key]) is int and 0 <= value[key] <= 180000)
    for key in FLAGS:
        insist(value[key] is True)


def validate(value):
    insist(type(value) is dict and set(value) == RESULT_FIELDS)
    insist(type(value['schema']) is int and value['schema'] == 1)
    insist(value['status'] in ['not-run', 'failed', 'passed'])
    for key in ['process_started', 'deadline_exceeded', 'owned_terminal', 'cleanup_verified', 'fixture_cleanup_verified']:
        insist(type(value[key]) is bool)
    code = value['process_exit_code']
    insist(code is None or type(code) is int and -128 <= code <= 255)
    if value['measurement'] is not None:
        measurement(value['measurement'])
    title.validate(value['title_disposition'])
    if value['status'] == 'not-run':
        insist(value == not_run())
    passed = (value['process_started'] and code == 0 and value['owned_terminal']
              and not value['deadline_exceeded'] and value['cleanup_verified']
              and value['fixture_cleanup_verified'] and value['measurement'] is not None
              and value['title_disposition']['status'] == 'passed')
    insist((value['status'] == 'passed') == passed)


def parse_measurement(text):
    """Admit exactly the retained test's JSON record, never surrounding output."""
    rows = []
    prefix = r'(?:test compact_feed_postgres_projection_parity_visibility_and_pagination \.\.\. )?'
    for line in text.splitlines():
        match = re.fullmatch(prefix + r'(\{.*\})', line.strip())
        if not match:
            continue
        try:
            row = json.loads(match[1], object_pairs_hook=unique_object)
        except (ValueError, TypeError):
            raise ValueError('Invalid native measurement record') from None
        measurement(row)
        rows.append(row)
    insist(len(rows) == 1)
    return rows[0]


def fixture_cleanup_verified(value):
    """Consume the full private auth_fixture_guard receipt; export one boolean.

    verified_stopped alone does not exclude forced shutdown, an earlier fixture
    error or a failed test. Keep the producer schema and all terminal witnesses
    closed, including its birth ledger and fixed timeout policy.
    """
    if type(value) is not dict or set(value) != FIXTURE_FIELDS:
        return False
    if (value['postgres_cleanup'] != 'verified_stopped' or value['forced'] is not False
            or value['closed_port'] is not True or value['error'] is not None):
        return False
    for key in ['exit', 'test_returncode', 'postgres_returncode']:
        if type(value[key]) is not int or value[key] != 0:
            return False
    for key in ['remaining_owned_processes', 'cleanup_errors', 'unconfirmed_births', 'unconfirmed_child_pids']:
        if type(value[key]) is not list or value[key] != []:
            return False
    for key, seconds in [('test_timeout_seconds', 180), ('startup_timeout_seconds', 10)]:
        if type(value[key]) is not int or value[key] != seconds:
            return False
    phases = value['cleanup_phase_seconds']
    if type(phases) is not list or phases != [15, 5, 5] or any(type(item) is not int for item in phases):
        return False
    births = value['captured_births']
    if type(births) is not list or not 1 <= len(births) <= 4096:
        return False
    seen = set()
    for birth in births:
        if type(birth) is not dict or set(birth) != BIRTH_FIELDS:
            return False
        for key in BIRTH_FIELDS:
            maximum = (1 << 63) - 1 if key == 'start_ticks' else (1 << 31) - 1
            if type(birth[key]) is not int or not 0 < birth[key] <= maximum:
                return False
        identity = (birth['pid'], birth['start_ticks'])
        if identity in seen:
            return False
        seen.add(identity)
    return True


def project(owned, observed, fixture, title_disposition=None):
    value = not_run()
    value['status'] = 'failed'
    if title_disposition is not None:
        title.validate(title_disposition)
        value['title_disposition'] = title_disposition
    if type(owned) is dict:
        # Absence or malformed false-like text must never become evidence that
        # the owned command stayed inside its deadline.
        insist(type(owned.get('deadline_exceeded')) is bool)
        value['process_started'] = True
        code = owned.get('process_exit_code')
        if type(code) is int and -128 <= code <= 255:
            value['process_exit_code'] = code
        value['deadline_exceeded'] = owned.get('deadline_exceeded') is True
        value['owned_terminal'] = owned.get('terminal_receipt') is True
        value['cleanup_verified'] = (owned.get('cleanup_verified') is True
            and owned.get('surviving_owned_process_ids') == []
            and owned.get('cleanup_signals') == []
            and owned.get('prior_or_unrelated_processes_touched') is False)
    value['fixture_cleanup_verified'] = fixture_cleanup_verified(fixture)
    if observed is not None:
        measurement(observed)
        value['measurement'] = observed
    if (value['process_started'] and value['process_exit_code'] == 0 and value['owned_terminal']
            and not value['deadline_exceeded'] and value['cleanup_verified']
            and value['fixture_cleanup_verified'] and value['measurement'] is not None
            and value['title_disposition']['status'] == 'passed'):
        value['status'] = 'passed'
    validate(value)
    return value
