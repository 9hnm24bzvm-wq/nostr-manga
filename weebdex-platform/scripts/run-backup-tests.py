#!/usr/bin/env python3
"""Selected recovery drill; raw logs, DBs and archives remain private.

Uses the existing owned PostgreSQL fixture. No installer, browser phase,
publication, production connection or backup upload occurs here.
"""
import argparse
import json
import os
import re
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[2]
TEST = 'offline_full_publisher_backup_restore_fences_auth_and_exact_retry'


def project_summary(raw, log_text, supervisor_exit_code):
    """Only a naturally terminated, fully clean, exact one-test run can pass."""
    executed = (len(re.findall(r'^running 1 test$', log_text, re.M)) == 1
                and len(re.findall(r'^test '+re.escape(TEST)+r' \.\.\. ok$', log_text, re.M)) == 1
                and len(re.findall(r'^test result: ok\. 1 passed; 0 failed; 0 ignored;', log_text, re.M)) == 1)
    # The supervisor may terminate leaked descendants and still return zero.
    # That is a failed lifecycle, even if it proves the group is empty afterward.
    cleanup = (raw.get('terminal_receipt') is True
               and raw.get('cleanup_verified') is True
               and raw.get('cleanup_signals') == []
               and raw.get('surviving_owned_process_ids') == []
               and raw.get('prior_or_unrelated_processes_touched') is False)
    exit_zero = type(raw.get('process_exit_code')) is int and raw['process_exit_code'] == 0
    no_deadline = raw.get('deadline_exceeded') is False
    passed = supervisor_exit_code == 0 and exit_zero and cleanup and no_deadline and executed
    summary = {'format':1,'test_id':TEST,'status':'passed' if passed else 'failed',
               'exit_zero':exit_zero,'cleanup_verified':cleanup,
               'timed_out':not no_deadline,'selected_tests':1,
               'private_runtime_uploaded':False,'browser_phases':0,'executed_tests':1 if executed else 0}
    validate_keys = {'format','test_id','status','exit_zero','cleanup_verified','timed_out','selected_tests','private_runtime_uploaded','browser_phases','executed_tests'}
    assert set(summary)==validate_keys and summary['test_id']==TEST
    assert summary['status'] in ('passed','failed') and summary['selected_tests']==1 and summary['browser_phases']==0
    assert all(type(summary[k]) is bool for k in ('exit_zero','cleanup_verified','timed_out','private_runtime_uploaded'))
    assert summary['private_runtime_uploaded'] is False and summary['executed_tests'] in (0,1)
    assert summary['status'] != 'passed' or (summary['executed_tests'] == 1 and cleanup and no_deadline and exit_zero)
    return summary


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--deadline', type=int, default=1800)
    parser.add_argument('--output', type=Path, default=ROOT/'weebdex-platform/.build/backup-recovery')
    args = parser.parse_args()
    if not 60 <= args.deadline <= 3600:
        parser.error('deadline must be 60..3600 seconds')
    out = args.output.resolve()
    out.mkdir(parents=True, exist_ok=False, mode=0o700)
    os.chmod(out,0o700)
    command = [sys.executable,str(ROOT/'weebdex-platform/scripts/run-bounded.py'),
        '--cwd',str(ROOT),'--deadline',str(args.deadline),'--receipt',str(out/'private-lifecycle.json'),
        '--log',str(out/'private-test.log'),'--',
        'bash',str(ROOT/'weebdex-platform/scripts/run-auth-tests.sh'),'cargo','test','--locked',
        '--manifest-path',str(ROOT/'weebdex-platform/backend/Cargo.toml'),'--test','protocol_bridge_http',
        TEST,'--','--ignored','--exact','--test-threads=1']
    # The bounded supervisor's raw output contains local process metadata. Keep it
    # private; publish only the closed, validated summary below.
    with (out/'private-supervisor.log').open('xb') as log:
        result = subprocess.run(command,stdout=log,stderr=subprocess.STDOUT)
    raw = json.loads((out/'private-lifecycle.json').read_text())
    log_text = (out/'private-test.log').read_text(errors='replace')
    summary = project_summary(raw, log_text, result.returncode)
    (out/'public-summary.json').write_text(json.dumps(summary,indent=2)+'\n')
    print(json.dumps(summary))
    return 0 if summary['status'] == 'passed' else 1


if __name__ == '__main__':
    sys.exit(main())
