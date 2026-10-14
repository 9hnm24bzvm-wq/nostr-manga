"""Closed native-feed result contract; no raw messages, DSNs, paths or SQL."""
import json
import re


FLAGS = {'all_505_ids_reached', 'bilingual506_preserved', 'canonical_sample_parity',
         'visibility_withdrawals_verified', 'statement_timeout_unchanged'}
MEASUREMENT_FIELDS = FLAGS | {'scenario', 'baseline_outcome', 'baseline_ms', 'batched_first_page_ms'}
RESULT_FIELDS = {'schema', 'status', 'process_started', 'process_exit_code', 'deadline_exceeded',
                 'owned_terminal', 'cleanup_verified', 'fixture_cleanup_verified', 'measurement'}


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
            'fixture_cleanup_verified': False, 'measurement': None}


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
    if value['status'] == 'not-run':
        insist(value == not_run())
    passed = (value['process_started'] and code == 0 and value['owned_terminal']
              and not value['deadline_exceeded'] and value['cleanup_verified']
              and value['fixture_cleanup_verified'] and value['measurement'] is not None)
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


def project(owned, observed, fixture):
    value = not_run()
    value['status'] = 'failed'
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
    value['fixture_cleanup_verified'] = (type(fixture) is dict
        and set(fixture) == {'postgres_cleanup', 'exit'}
        and fixture['postgres_cleanup'] == 'verified_stopped'
        and type(fixture['exit']) is int and fixture['exit'] == 0)
    if observed is not None:
        measurement(observed)
        value['measurement'] = observed
    if (value['process_started'] and value['process_exit_code'] == 0 and value['owned_terminal']
            and not value['deadline_exceeded'] and value['cleanup_verified']
            and value['fixture_cleanup_verified'] and value['measurement'] is not None):
        value['status'] = 'passed'
    validate(value)
    return value
