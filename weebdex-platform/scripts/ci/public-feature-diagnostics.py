"""Fixed feature-check outcomes only; no exception text, IDs or private values."""
PHASE = 'public-feature-recovery'
SCOPE = 'reviewed-user-features-v1'
CHECKS = (
    'backup-v2-merge-roundtrip',
    'backup-v2-foreign-replacement-cancel',
    'public-search-complete-tag-options',
    'public-search-default-sort-ascending',
    'public-search-empty-rating-selection',
    'chapter-edit-entry-signed',
)

BACKUP_STAGES = (
    frozenset(('preconditions', 'open-settings', 'initialize-device', 'resolve-fixture',
               'seed-device', 'verify-seed', 'reload-seed', 'export-backup', 'preview-merge',
               'review-merge', 'verify-review-unchanged', 'download-pre-restore',
               'confirm-merge', 'verify-merge', 'reload-merge', 'unavailable')),
    frozenset(('preconditions', 'merge-prerequisite', 'verify-owned-state',
               'preview-replacement', 'acknowledge-foreign', 'review-replacement',
               'verify-review-unchanged', 'download-pre-restore', 'cancel-replacement',
               'verify-cancel', 'reload-cancel', 'preview-replacement-again',
               'acknowledge-foreign-again', 'review-replacement-again',
               'confirm-replacement', 'verify-replacement', 'reload-replacement',
               'export-replacement', 'unavailable')),
)
BACKUP_CATEGORIES = frozenset(('check', 'timeout', 'operation', 'prerequisite'))


def check_row(row, index):
    required = {'id', 'status'} | ({'stage', 'category'} if index < 2 else set())
    if (type(row) is not dict or set(row) != required or
            type(row['id']) is not str or row['id'] != CHECKS[index] or
            type(row['status']) is not str or row['status'] not in {'passed', 'failed'}):
        return False
    if index >= 2:
        return True
    stage, category = row['stage'], row['category']
    if type(stage) is not str or type(category) is not str:
        return False
    if row['status'] == 'passed':
        return stage == 'complete' and category == 'none'
    if stage not in BACKUP_STAGES[index] or category not in BACKUP_CATEGORIES:
        return False
    # A missing successful merge is a precise dependency refusal, not a second
    # independently exercised replacement defect. No other case can claim it.
    if stage == 'merge-prerequisite' or category == 'prerequisite':
        return index == 1 and stage == 'merge-prerequisite' and category == 'prerequisite'
    return True


def empty():
    return {'schema': 1, 'record_valid': False, 'completed': False,
            'actual_context_closed': False, 'checks': [],
            'private_values_omitted': True, 'raw_messages_exported': False}


def check_rows(rows):
    return (type(rows) is list and len(rows) <= len(CHECKS) and
            all(check_row(row, index) for index, row in enumerate(rows)))


def project(acceptance):
    invalid = empty()
    if type(acceptance) is not dict or acceptance.get('phase') != PHASE:
        return invalid
    raw, steps, cleanup = acceptance.get('public_features'), acceptance.get('steps'), acceptance.get('cleanup')
    if (type(raw) is not dict or set(raw) != {'schema', 'scope', 'checks', 'completed', 'actual_context_closed'} or
            type(raw['schema']) is not int or raw['schema'] != 1 or raw['scope'] != SCOPE or
            type(raw['completed']) is not bool or type(raw['actual_context_closed']) is not bool or
            not check_rows(raw['checks']) or type(steps) is not list or
            not len(raw['checks']) <= len(steps) <= min(len(CHECKS), len(raw['checks']) + 1) or
            type(cleanup) is not dict or type(cleanup.get('context_closed')) is not bool or
            raw['actual_context_closed'] is not cleanup['context_closed']):
        return invalid
    # Only settled checks are exported. One interrupted next step may have been
    # persisted before its callback/check row completed; it supplies no success.
    for index, step in enumerate(steps):
        if type(step) is not dict or step.get('name') != 'Feature ' + CHECKS[index]:
            return invalid
        if index < len(raw['checks']):
            if step.get('status') != raw['checks'][index]['status']:
                return invalid
        elif step.get('status') not in (None, 'passed', 'failed'):
            return invalid
    if raw['completed'] and (len(raw['checks']) != len(CHECKS) or len(steps) != len(CHECKS)):
        return invalid
    return {'schema': 1, 'record_valid': True, 'completed': raw['completed'],
            'actual_context_closed': raw['actual_context_closed'],
            'checks': [dict(row) for row in raw['checks']],
            'private_values_omitted': True, 'raw_messages_exported': False}


def passed(value):
    return (value['record_valid'] is True and value['completed'] is True and
            value['actual_context_closed'] is True and len(value['checks']) == len(CHECKS) and
            all(row['status'] == 'passed' for row in value['checks']))


def validate(value):
    if (type(value) is not dict or set(value) != set(empty()) or
            type(value['schema']) is not int or value['schema'] != 1 or
            any(type(value[key]) is not bool for key in ['record_valid', 'completed', 'actual_context_closed']) or
            value['private_values_omitted'] is not True or value['raw_messages_exported'] is not False or
            not check_rows(value['checks']) or
            value['completed'] and len(value['checks']) != len(CHECKS) or
            not value['record_valid'] and (value['checks'] or value['completed'] or value['actual_context_closed'])):
        raise ValueError('Invalid fixed public feature diagnostics')
