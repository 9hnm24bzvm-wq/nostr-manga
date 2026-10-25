"""Closed fixed outcomes only. Never export identities, bodies, keys, or paths."""
PHASE = 'public-signed-feature-recovery'
SCOPE = 'actual-signed-report-draft-chapter-v1'
CHECKS = ('signed-feature-identity', 'signed-report-cancel', 'signed-report-submit',
          'signed-chapter-save', 'signed-draft-owner-preview-cancel', 'signed-draft-submit-removal-review')


def empty():
    return {'schema': 1, 'record_valid': False, 'completed': False,
            'actual_context_closed': False, 'observer_closed': False, 'checks': [],
            'private_values_omitted': True, 'raw_messages_exported': False}


def checks(rows):
    return (type(rows) is list and len(rows) <= len(CHECKS) and
            all(type(row) is dict and set(row) == {'id', 'status'} and
                type(row['id']) is str and row['id'] == CHECKS[index] and
                type(row['status']) is str and row['status'] in {'passed', 'failed'}
                for index, row in enumerate(rows)) and
            all(row['status'] == 'passed' for row in rows[:-1]))


def project(acceptance):
    invalid = empty()
    if type(acceptance) is not dict or acceptance.get('phase') != PHASE:
        return invalid
    raw, steps, cleanup = acceptance.get('signed_features'), acceptance.get('steps'), acceptance.get('cleanup')
    if (type(raw) is not dict or set(raw) != {'schema', 'scope', 'checks', 'completed', 'actual_context_closed', 'observer_closed'} or
            type(raw['schema']) is not int or raw['schema'] != 1 or raw['scope'] != SCOPE or
            any(type(raw[key]) is not bool for key in ['completed', 'actual_context_closed', 'observer_closed']) or
            not checks(raw['checks']) or type(steps) is not list or
            not len(raw['checks']) <= len(steps) <= min(len(CHECKS), len(raw['checks']) + 1) or
            type(cleanup) is not dict or type(cleanup.get('context_closed')) is not bool or
            raw['actual_context_closed'] is not cleanup['context_closed']):
        return invalid
    for index, step in enumerate(steps):
        if type(step) is not dict or step.get('name') != 'Signed feature ' + CHECKS[index]:
            return invalid
        if index < len(raw['checks']):
            if step.get('status') != raw['checks'][index]['status']:
                return invalid
        elif step.get('status') not in (None, 'passed', 'failed'):
            return invalid
    if raw['completed'] and (len(steps) != len(CHECKS) or len(raw['checks']) != len(CHECKS) or any(row['status'] != 'passed' for row in raw['checks'])):
        return invalid
    return dict(invalid, record_valid=True, completed=raw['completed'], actual_context_closed=raw['actual_context_closed'], observer_closed=raw['observer_closed'], checks=[dict(row) for row in raw['checks']])


def passed(value):
    return (value['record_valid'] is True and value['completed'] is True and
            value['actual_context_closed'] is True and value['observer_closed'] is True and
            len(value['checks']) == len(CHECKS) and all(row['status'] == 'passed' for row in value['checks']))


def validate(value):
    if (type(value) is not dict or set(value) != set(empty()) or type(value['schema']) is not int or value['schema'] != 1 or
            any(type(value[key]) is not bool for key in ['record_valid', 'completed', 'actual_context_closed', 'observer_closed']) or
            value['private_values_omitted'] is not True or value['raw_messages_exported'] is not False or not checks(value['checks']) or
            value['completed'] and (len(value['checks']) != len(CHECKS) or any(row['status'] != 'passed' for row in value['checks'])) or
            not value['record_valid'] and (value['checks'] or value['completed'] or value['actual_context_closed'] or value['observer_closed'])):
        raise ValueError('Invalid fixed signed feature diagnostics')
