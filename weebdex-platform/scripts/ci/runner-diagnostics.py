"""Fixed process-exit provenance; no exception text, stack, URLs or private values."""
STAGES = {'initializing', 'launching-browser', 'creating-context', 'running-checks', 'closing-context',
          'closing-browser', 'closing-proxy', 'finalizing-report', 'complete'}
FATAL_KINDS = {'none', 'uncaught-exception', 'unhandled-rejection'}


def project(acceptance, owned):
    raw = acceptance.get('runner_lifecycle')
    raw = raw if type(raw) is dict else {}
    valid = (set(raw) == {'stage', 'fatal_kind', 'fatal_stage', 'exit_observed', 'exit_code'} and
             type(raw.get('stage')) is str and raw['stage'] in STAGES and
             type(raw.get('fatal_kind')) is str and raw['fatal_kind'] in FATAL_KINDS and
             type(raw.get('fatal_stage')) is str and raw['fatal_stage'] in STAGES | {'none'} and
             type(raw.get('exit_observed')) is bool and
             (raw.get('exit_code') is None or type(raw.get('exit_code')) is int and 0 <= raw['exit_code'] <= 255) and
             (raw.get('exit_observed') is True or raw.get('exit_code') is None) and
             ((raw.get('fatal_kind') == 'none') is (raw.get('fatal_stage') == 'none')))
    code = owned.get('returncode')
    code = code if type(code) is int and -64 <= code <= 255 else None
    return {'schema': 1, 'record_valid': bool(valid),
            'stage': raw.get('stage') if type(raw.get('stage')) is str and raw['stage'] in STAGES else 'unavailable',
            'fatal_kind': raw.get('fatal_kind') if type(raw.get('fatal_kind')) is str and raw['fatal_kind'] in FATAL_KINDS else 'unavailable',
            'fatal_stage': raw.get('fatal_stage') if type(raw.get('fatal_stage')) is str and raw['fatal_stage'] in STAGES | {'none'} else 'unavailable',
            'exit_observed': raw.get('exit_observed') is True,
            'exit_code': raw.get('exit_code') if type(raw.get('exit_code')) is int and 0 <= raw['exit_code'] <= 255 else None,
            'owned_exit_code': code,
            'report_finalized': type(acceptance.get('passed')) is bool and type(acceptance.get('completed_utc')) is str and bool(acceptance['completed_utc']),
            'report_passed': acceptance.get('passed') is True,
            'private_values_omitted': True, 'raw_messages_exported': False}


def passed(value):
    return (value['record_valid'] and value['stage'] == 'complete' and value['fatal_kind'] == 'none' and
            value['fatal_stage'] == 'none' and value['exit_observed'] and value['exit_code'] == 0 and
            value['owned_exit_code'] == 0 and value['report_finalized'] and value['report_passed'])
