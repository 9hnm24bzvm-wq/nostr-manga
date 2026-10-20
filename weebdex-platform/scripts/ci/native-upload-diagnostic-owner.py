#!/usr/bin/env python3
"""One-publication gate and finite external process owner; private receipts only."""
import argparse
import ctypes
import json
import os
from pathlib import Path
import re
import resource
import signal
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[3]
MARKER = 'diagnostic(native-upload): approved bounded run 20261020'
PARENT = '76d96509fd4272ec6852fa5817ae68d42324aae7'
ALLOWED = frozenset({
    'weebdex-platform/scripts/ci/native-upload-diagnostic-owner.py',
    'weebdex-platform/tests/ci/test_native_upload_diagnostic_workflow.py',
    'weebdex-platform/docs/verification/native-upload-diagnostic.md',
    'weebdex-platform/scripts/ci/run-native-upload-diagnostic.mjs',
    'weebdex-platform/scripts/ci/native-upload-diagnostic-report.mjs',
    'weebdex-platform/tests/ci/test_native_upload_diagnostic.mjs',
})


def preflight(event, environment, git):
    assert environment['GITHUB_EVENT_NAME'] == 'push'
    assert environment['GITHUB_REPOSITORY'] == '9hnm24bzvm-wq/nostr-manga'
    assert environment['GITHUB_REF'] == 'refs/heads/ci/browser-failure-diagnostics'
    sha = environment['GITHUB_SHA']
    assert re.fullmatch(r'[0-9a-f]{40}', sha)
    assert event['before'] == PARENT and event['after'] == sha
    assert event['head_commit']['message'] == MARKER
    assert event['repository']['private'] is False
    assert event.get('deleted') is False and event.get('forced') is False
    assert git('rev-parse', 'HEAD').strip() == sha
    assert git('rev-list', '--parents', '-n', '1', 'HEAD').strip().split() == [sha, PARENT]
    changed = set(git('diff-tree', '--no-commit-id', '--name-only', '-r', 'HEAD').splitlines())
    assert changed == ALLOWED
    assert git('status', '--porcelain') == ''
    return True


def census():
    result = {}
    for stat in Path('/proc').glob('[0-9]*/stat'):
        try:
            values = stat.read_text().rsplit(')', 1)[1].split()
            result[int(stat.parent.name)] = (int(values[19]), int(values[1]), values[0])
        except (FileNotFoundError, ProcessLookupError):
            pass
    return result


def run_owned(argv, cwd, log_path, receipt_path, environment, deadline=270):
    started = time.monotonic()
    forced = False
    libc = ctypes.CDLL(None, use_errno=True)
    old_subreaper = ctypes.c_int()
    if libc.prctl(37, ctypes.byref(old_subreaper), 0, 0, 0) != 0 or libc.prctl(36, 1, 0, 0, 0) != 0:
        raise RuntimeError('Owned descendant tracking unavailable')
    baseline = census()
    known = {}
    def owned():
        rows = census()
        changed = True
        while changed:
            changed = False
            for pid, (birth, parent, state) in rows.items():
                if pid in known or pid in baseline:
                    continue
                if parent == os.getpid() or parent in known and rows.get(parent, (None,))[0] == known[parent]:
                    known[pid] = birth; changed = True
        return [pid for pid, birth in known.items() if pid in rows and rows[pid][0] == birth and rows[pid][2] != 'Z']
    def terminate(signum):
        for pid in owned():
            try:
                fd = os.pidfd_open(pid)
                try:
                    if census().get(pid, (None,))[0] == known[pid]:
                        signal.pidfd_send_signal(fd, signum)
                finally:
                    os.close(fd)
            except ProcessLookupError:
                pass
    previous = {}
    with log_path.open('xb') as log:
        process = subprocess.Popen(argv, cwd=cwd, env=environment, stdin=subprocess.DEVNULL,
                                   stdout=log, stderr=subprocess.STDOUT, start_new_session=True,
                                   preexec_fn=lambda: resource.setrlimit(resource.RLIMIT_FSIZE, (2 * 1024 * 1024, 2 * 1024 * 1024)))
        birth = census().get(process.pid)
        if birth: known[process.pid] = birth[0]
        try:
            while process.poll() is None and time.monotonic() - started < deadline:
                owned(); time.sleep(.025)
            if process.poll() is None: forced = True
        except InterruptedError:
            forced = True
        finally:
            # Termination cannot interrupt the bounded ownership cleanup itself.
            for sig in [signal.SIGTERM, signal.SIGINT]:
                previous[sig] = signal.signal(sig, signal.SIG_IGN)
            if owned():
                forced = True; terminate(signal.SIGTERM)
                until = time.monotonic() + 5
                while owned() and time.monotonic() < until: time.sleep(.025)
                if owned(): terminate(signal.SIGKILL)
            try: process.wait(timeout=5)
            except subprocess.TimeoutExpired: forced = True
            until = time.monotonic() + 5
            while owned() and time.monotonic() < until: time.sleep(.025)
            remaining = owned()
            for pid in known:
                try: os.waitpid(pid, os.WNOHANG)
                except ChildProcessError: pass
            libc.prctl(36, old_subreaper.value, 0, 0, 0)
            for sig, previous_handler in previous.items(): signal.signal(sig, previous_handler)
    receipt = {'schema': 1, 'terminal': process.returncode is not None,
               'exit_code': process.returncode, 'forced': forced,
               'remaining_owned_process_count': len(remaining),
               'cleanup_verified': not remaining,
               'within_deadline': time.monotonic() - started <= deadline + 15}
    receipt_path.write_text(json.dumps(receipt, sort_keys=True, separators=(',', ':')) + '\n')
    return 0 if process.returncode == 0 and not forced and not remaining and receipt['within_deadline'] else 1


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('mode', choices=['preflight', 'run', 'verify-owner'])
    args = parser.parse_args()
    stage = ROOT / 'weebdex-platform/.build/native-upload-diagnostic'
    if args.mode == 'preflight':
        raw = Path(os.environ['GITHUB_EVENT_PATH']).read_bytes()
        assert len(raw) <= 1024 * 1024
        preflight(json.loads(raw), os.environ, lambda *a: subprocess.check_output(['git', *a], cwd=ROOT, text=True))
    elif args.mode == 'run':
        stage.mkdir(parents=True, exist_ok=True)
        # No GitHub token, runner credential, signer or user environment is inherited.
        env = {key: os.environ[key] for key in ['PATH', 'HOME', 'LANG'] if key in os.environ}
        env.update({'CI': 'true', 'NODE_OPTIONS': '--max-old-space-size=512', 'TZ': 'UTC'})
        env['PLAYWRIGHT_BROWSERS_PATH'] = str(ROOT / '.ci/native-diagnostic-browsers')
        argv = ['node', str(ROOT / 'weebdex-platform/scripts/ci/run-native-upload-diagnostic.mjs'),
                '--output', str(stage / 'report.json'), '--playwright-root', str(ROOT / '.ci/browser')]
        def interrupted(_signum, _frame):
            raise InterruptedError('Owned diagnostic interrupted')
        signal.signal(signal.SIGTERM, interrupted)
        signal.signal(signal.SIGINT, interrupted)
        return run_owned(argv, ROOT, stage / 'private-run.log', stage / 'outer.json', env)
    else:
        value = json.loads((stage / 'outer.json').read_text())
        assert value['terminal'] is True and value['forced'] is False
        assert value['cleanup_verified'] is True and value['remaining_owned_process_count'] == 0
        assert value['within_deadline'] is True
    return 0


if __name__ == '__main__':
    try:
        sys.exit(main())
    except Exception:
        print('Dedicated diagnostic boundary refused; no raw details exported.')
        sys.exit(1)
