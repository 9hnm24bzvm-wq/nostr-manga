"""Closed public facts from the private positive-replacement browser report."""
import re


FLAGS = {'record_valid', 'completed', 'context_closed', 'replacement_verified',
         'hard_reload_previews_verified', 'committed_originals_verified',
         'browser_observer_closed', 'independent_reader_closed', 'request_caps_respected',
         'exact_mutation_count'}
COUNTS = {'signed_requests': 128, 'mutations': 32, 'independent_reads': 96, 'fixture_controls': 28}
OBSERVER_FAILURES = ('signed_request_invalid', 'duplicate_native_terminal', 'signed_native_failed')


def project_observer_failures(raw):
    # Only the observer's first sixteen fixed failure labels are retained. These
    # counts are not a total-failure or total-request claim.
    value = {'record_valid': False, **{key: 0 for key in OBSERVER_FAILURES}}
    if (type(raw) is not list or len(raw) > 16 or
            any(type(code) is not str or code not in OBSERVER_FAILURES for code in raw)):
        return value
    return {'record_valid': True, **{key: raw.count(key) for key in OBSERVER_FAILURES}}


def observer_failures_clear(value):
    return value['record_valid'] is True and all(value[key] == 0 for key in OBSERVER_FAILURES)


KEY_IMPORT_SUBSTEPS = ['open-settings', 'settings-route', 'public-identity', 'identity-panel',
    'key-dialog', 'import-form', 'import-key', 'backup-ack', 'recipient-review',
    'session-connect', 'session-ready', 'done', 'upload-return', 'upload-ready']
KEY_IMPORT_CATEGORIES = {'none', 'readiness-timeout', 'duplicate-control', 'browser-timeout', 'assertion', 'unexpected'}
KEY_IMPORT_FIELDS = {'attempt', 'substep', 'completed_substeps', 'status', 'caught_category'}


def key_import_row_valid(row, attempt):
    if (type(row) is not dict or set(row) != KEY_IMPORT_FIELDS or type(row.get('attempt')) is not int or row['attempt'] != attempt or
            type(row.get('substep')) is not str or row['substep'] not in KEY_IMPORT_SUBSTEPS or
            type(row.get('completed_substeps')) is not int or not 0 <= row['completed_substeps'] <= len(KEY_IMPORT_SUBSTEPS) or
            type(row.get('status')) is not str or row['status'] not in {'running', 'passed', 'failed'} or
            type(row.get('caught_category')) is not str or row['caught_category'] not in KEY_IMPORT_CATEGORIES):
        return False
    if row['status'] == 'passed':
        return row['substep'] == KEY_IMPORT_SUBSTEPS[-1] and row['completed_substeps'] == len(KEY_IMPORT_SUBSTEPS) and row['caught_category'] == 'none'
    if row['completed_substeps'] >= len(KEY_IMPORT_SUBSTEPS) or row['substep'] != KEY_IMPORT_SUBSTEPS[row['completed_substeps']]:
        return False
    return (row['caught_category'] == 'none') is (row['status'] == 'running')


def project_key_import(raw):
    valid = type(raw) is list and len(raw) <= 2
    rows = []
    for index, row in enumerate(raw[:2] if type(raw) is list else []):
        if not key_import_row_valid(row, index + 1) or index and rows[-1]['status'] != 'passed':
            valid = False
            break
        rows.append({key: row[key] for key in sorted(KEY_IMPORT_FIELDS)})
    return {'schema': 1, 'record_valid': bool(valid), 'attempts': rows if valid else []}


def key_import_passed(value):
    return (value['record_valid'] is True and len(value['attempts']) == 2 and
            all(row['status'] == 'passed' for row in value['attempts']))


def digest(value):
    return type(value) is str and re.fullmatch('[0-9a-f]{64}', value) is not None


def project(acceptance):
    raw = acceptance.get('public_upload')
    browser = raw.get('browser_http') if type(raw) is dict else None
    value = {'schema': 1, **{key: False for key in FLAGS}, **{key: 0 for key in COUNTS},
             'private_values_omitted': True, 'raw_messages_exported': False,
             'browser_observer_failures': project_observer_failures(browser.get('failures') if type(browser) is dict else None),
             'key_import_diagnostics': project_key_import(acceptance.get('public_upload', {}).get('key_import') if type(acceptance.get('public_upload')) is dict else None)}
    try:
        raw = acceptance['public_upload']
        browser, independent = raw['browser_http'], raw['independent_http']
        rows, reads = browser['rows'], independent['rows']
        mutations = [row for row in rows if row['method'] != 'GET']
        counts = {'signed_requests': len(rows), 'mutations': len(mutations),
                  'independent_reads': len(reads), 'fixture_controls': raw['fixture_control_calls']}
        value.update({key: count if type(count) is int and 0 <= count <= COUNTS[key] else 0 for key, count in counts.items()})
        value['record_valid'] = raw.get('scope') == 'reconstructed-public-upload-replacement-v1' and raw.get('supported_replacement') is True and len(acceptance.get('steps', [])) == 14
        value['completed'] = raw.get('completed') is True
        value['context_closed'] = raw.get('actual_context_closed') is True
        value['browser_observer_closed'] = browser.get('closed') is True and observer_failures_clear(value['browser_observer_failures']) and all(row.get('native') == 'finished' and type(row.get('status')) is int and 200 <= row['status'] <= 599 and digest(row.get('event_id')) for row in rows)
        value['independent_reader_closed'] = independent.get('closed') is True and type(independent.get('pending')) is int and independent['pending'] == 0 and all(row.get('complete') is True and row.get('method') == 'GET' and type(row.get('status')) is int and 200 <= row['status'] <= 599 and digest(row.get('event_id')) for row in reads)
        value['request_caps_respected'] = all(type(count) is int and 0 < count <= COUNTS[key] for key, count in counts.items())
        value['exact_mutation_count'] = len(mutations) == 21 and all(row.get('status') == 200 for row in mutations)
        replacement = raw['replacement']
        before, after = replacement['version_before'], replacement['version_after']
        path = '/api/v1/nostr/uploads/' + replacement['session_id'] + '/files/' + replacement['file_id'] + '/replace'
        observed = [row for row in mutations if row.get('method') == 'POST' and row.get('path') == path and row.get('event_id') == replacement.get('mutation_event_id')]
        value['replacement_verified'] = (all(replacement.get(key) is True for key in ['stable_file_id', 'native_position_preserved', 'stored_order_preserved', 'neighbours_unchanged']) and
            type(before) is int and before > 0 and type(after) is int and after == before + 1 and
            digest(replacement.get('old_sha256')) and digest(replacement.get('new_sha256')) and replacement['old_sha256'] != replacement['new_sha256'] and
            type(replacement.get('stored_original_bytes')) is int and 0 < replacement['stored_original_bytes'] <= 512 and len(observed) == 1)
        reload = [step.get('observation', {}) for step in acceptance['steps'] if step.get('name') == 'hard reload, real key reconnect and durable signed private previews' and step.get('status') == 'passed']
        if len(reload) == 1:
            reload = reload[0]
            previews = reload['signed_previews']
            valid = (all(reload.get(key) is True for key in ['hard_reload', 'explicit_key_reimport', 'local_File_thumbnails_absent']) and
                     type(previews) is list and len(previews) == 2 and len({row['file_id'] for row in previews}) == 2 and
                     replacement['file_id'] in [row['file_id'] for row in previews])
            for preview in previews:
                preview_path = '/api/v1/nostr/uploads/' + replacement['session_id'] + '/files/' + preview['file_id'] + '/preview'
                ids = preview['event_ids']
                valid = valid and digest(preview['file_hash']) and type(ids) is list and bool(ids) and len(ids) == len(set(ids)) and all(digest(event) and any(row.get('event_id') == event and row.get('method') == 'GET' and row.get('path') == preview_path and row.get('status') == 200 and row.get('native') == 'finished' for row in rows) for event in ids)
            value['hard_reload_previews_verified'] = bool(valid)
        committed = [row for row in raw['durable'] if row.get('label') == 'committed']
        if len(committed) == 1:
            committed = committed[0]
            publication, chapter = committed['publication'], committed['chapter']
            event = publication['original_event']
            pages = chapter['pages']
            hashes = [row['file_hash'] for row in reload['signed_previews']] if type(reload) is dict else []
            value['committed_originals_verified'] = (committed.get('fixture_only') is True and committed.get('scenario') == 'public-upload-v1' and committed.get('session', {}).get('state') == 'committed' and
                digest(publication.get('event_id')) and publication['event_id'] == chapter.get('event_id') == event.get('id') and event.get('verified') is True and event.get('kind') == 1001 and
                type(pages) is list and len(pages) == 2 and [row['file_hash'] for row in pages] == hashes and replacement['new_sha256'] in hashes and
                all(row.get('fresh_original_read') is True and row.get('storage_sha256') == row.get('file_hash') and digest(row.get('file_hash')) and type(row.get('storage_bytes')) is int and 0 < row['storage_bytes'] <= 512 for row in pages))
    except (KeyError, TypeError, ValueError, AttributeError):
        value['record_valid'] = False
    return value


def passed(value):
    return (all(value.get(key) is True for key in FLAGS) and key_import_passed(value['key_import_diagnostics']) and
            observer_failures_clear(value['browser_observer_failures']))


def validate(value):
    if type(value) is not dict or set(value) != FLAGS | set(COUNTS) | {'schema', 'private_values_omitted', 'raw_messages_exported', 'key_import_diagnostics', 'browser_observer_failures'} or type(value['schema']) is not int or value['schema'] != 1:
        raise ValueError('Invalid public upload diagnostics schema')
    if any(type(value[key]) is not bool for key in FLAGS) or any(type(value[key]) is not int or not 0 <= value[key] <= cap for key, cap in COUNTS.items()) or value['private_values_omitted'] is not True or value['raw_messages_exported'] is not False:
        raise ValueError('Invalid public upload diagnostic facts')
    if value['exact_mutation_count'] and value['mutations'] != 21:
        raise ValueError('Mutation-count facts disagree')
    failures = value['browser_observer_failures']
    if (type(failures) is not dict or set(failures) != {'record_valid', *OBSERVER_FAILURES} or
            type(failures.get('record_valid')) is not bool or
            any(type(failures.get(key)) is not int or not 0 <= failures[key] <= 16 for key in OBSERVER_FAILURES) or
            sum(failures[key] for key in OBSERVER_FAILURES) > 16 or
            not failures['record_valid'] and any(failures[key] != 0 for key in OBSERVER_FAILURES)):
        raise ValueError('Invalid browser observer failure facts')
    if value['browser_observer_closed'] and not observer_failures_clear(failures):
        raise ValueError('Browser observer closure facts disagree')

    imported = value['key_import_diagnostics']
    if (type(imported) is not dict or set(imported) != {'schema', 'record_valid', 'attempts'} or
            type(imported.get('schema')) is not int or imported['schema'] != 1 or type(imported.get('record_valid')) is not bool or
            type(imported.get('attempts')) is not list or len(imported['attempts']) > 2 or
            not imported['record_valid'] and imported['attempts']):
        raise ValueError('Invalid key import diagnostics schema')
    for index, row in enumerate(imported['attempts']):
        if not key_import_row_valid(row, index + 1) or index and imported['attempts'][index - 1]['status'] != 'passed':
            raise ValueError('Invalid key import diagnostics facts')
