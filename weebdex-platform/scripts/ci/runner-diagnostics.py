"""Fixed process-exit provenance; no exception text, stack, URLs or private values."""
STAGES = {'initializing', 'launching-browser', 'creating-context', 'running-checks', 'closing-context',
          'closing-browser', 'closing-proxy', 'finalizing-report', 'complete'}
FATAL_KINDS = {'none', 'uncaught-exception', 'unhandled-rejection'}
FATAL_CODES = {'none', 'unknown', 'ECONNRESET', 'EPIPE', 'ERR_STREAM_DESTROYED', 'ERR_SERVER_NOT_RUNNING'}


def proxy_projection(raw):
    raw = raw if type(raw) is dict else {}
    counts = {'connect_denials', 'expected_connect_resets', 'remaining_socket_count'}
    flags = {'observations_truncated', 'shutdown_started', 'closed'}
    valid = (set(raw) == counts | flags and all(type(raw.get(key)) is int and 0 <= raw[key] <= 65535 for key in counts) and
             all(type(raw.get(key)) is bool for key in flags))
    if raw.get('closed') is True:
        valid = valid and raw.get('shutdown_started') is True and raw.get('remaining_socket_count') == 0
    value = {'schema': 1, 'record_valid': bool(valid)}
    value.update({key: raw.get(key) if type(raw.get(key)) is int and 0 <= raw[key] <= 65535 else 0 for key in counts})
    value.update({key: raw.get(key) is True for key in flags})
    return value


def project(acceptance, owned):
    raw = acceptance.get('runner_lifecycle')
    raw = raw if type(raw) is dict else {}
    valid = (set(raw) == {'stage', 'fatal_kind', 'fatal_code', 'fatal_stage', 'exit_observed', 'exit_code'} and
             type(raw.get('stage')) is str and raw['stage'] in STAGES and
             type(raw.get('fatal_kind')) is str and raw['fatal_kind'] in FATAL_KINDS and
             type(raw.get('fatal_code')) is str and raw['fatal_code'] in FATAL_CODES and
             type(raw.get('fatal_stage')) is str and raw['fatal_stage'] in STAGES | {'none'} and
             type(raw.get('exit_observed')) is bool and
             (raw.get('exit_code') is None or type(raw.get('exit_code')) is int and 0 <= raw['exit_code'] <= 255) and
             (raw.get('exit_observed') is True or raw.get('exit_code') is None) and
             ((raw.get('fatal_kind') == 'none') is (raw.get('fatal_stage') == 'none')) and
             ((raw.get('fatal_kind') == 'none') is (raw.get('fatal_code') == 'none')))
    code = owned.get('returncode')
    code = code if type(code) is int and -64 <= code <= 255 else None
    return {'schema': 1, 'record_valid': bool(valid),
            'stage': raw.get('stage') if type(raw.get('stage')) is str and raw['stage'] in STAGES else 'unavailable',
            'fatal_kind': raw.get('fatal_kind') if type(raw.get('fatal_kind')) is str and raw['fatal_kind'] in FATAL_KINDS else 'unavailable',
            'fatal_code': raw.get('fatal_code') if type(raw.get('fatal_code')) is str and raw['fatal_code'] in FATAL_CODES else 'unavailable',
            'fatal_stage': raw.get('fatal_stage') if type(raw.get('fatal_stage')) is str and raw['fatal_stage'] in STAGES | {'none'} else 'unavailable',
            'exit_observed': raw.get('exit_observed') is True,
            'exit_code': raw.get('exit_code') if type(raw.get('exit_code')) is int and 0 <= raw['exit_code'] <= 255 else None,
            'owned_exit_code': code,
            'report_finalized': type(acceptance.get('passed')) is bool and type(acceptance.get('completed_utc')) is str and bool(acceptance['completed_utc']),
            'report_passed': acceptance.get('passed') is True,
            'proxy_diagnostics': proxy_projection(acceptance.get('proxy_lifecycle')),
            'private_values_omitted': True, 'raw_messages_exported': False}


def passed(value):
    return (value['record_valid'] and value['stage'] == 'complete' and value['fatal_kind'] == 'none' and
            value['fatal_stage'] == 'none' and value['exit_observed'] and value['exit_code'] == 0 and
            value['owned_exit_code'] == 0 and value['report_finalized'] and value['report_passed'] and
            value['fatal_code'] == 'none' and value['proxy_diagnostics']['record_valid'] and
            value['proxy_diagnostics']['shutdown_started'] and value['proxy_diagnostics']['closed'] and
            value['proxy_diagnostics']['remaining_socket_count'] == 0)
