#!/usr/bin/env python3
"""Linux-only owner of run-auth-tests.sh's disposable PostgreSQL fixture."""
import ctypes
import errno
import json
import os
from pathlib import Path
import signal
import socket
import subprocess
import sys
import tempfile
import time

ROOT = Path(__file__).resolve().parents[2]
START_SECONDS, TEST_SECONDS = 10, 180
STOP_PHASES = ((signal.SIGINT, 15), (signal.SIGTERM, 5), (signal.SIGKILL, 5))


def row(pid, proc=Path('/proc')):
    try:
        fields = (proc / str(pid) / 'stat').read_text().rsplit(')', 1)[1].split()
        return dict(pid=pid, state=fields[0], ppid=int(fields[1]), group=int(fields[2]),
                    session=int(fields[3]), start_ticks=int(fields[19]))
    except (FileNotFoundError, ProcessLookupError):
        return None


def identity(value):
    return {key: value[key] for key in ('pid', 'start_ticks', 'group', 'session')}


def same(expected, proc=Path('/proc')):
    current = row(expected['pid'], proc)
    return current if current and identity(current) == expected else None


def members(known, proc=Path('/proc'), owner=None, unknown=None):
    owner = os.getpid() if owner is None else owner
    rows = {int(p.name): row(int(p.name), proc) for p in proc.iterdir() if p.name.isdigit()}
    result = []
    for current in rows.values():
        if not current:
            continue
        ancestor, seen, lineage = current, set(), []
        while ancestor and ancestor['pid'] not in seen:
            seen.add(ancestor['pid'])
            # Stable authority is a previously captured birth, never a session
            # number or adoption alone. PG workers establish their own sessions.
            if (ancestor['pid'], ancestor['start_ticks']) in known:
                for value in lineage + [ancestor]:
                    key = (value['pid'], value['start_ticks'])
                    known[key] = identity(value)
                    if unknown is not None:
                        unknown.pop(key, None)
                result.append(current)
                break
            lineage.append(ancestor)
            parent = rows.get(ancestor['ppid'])
            if parent and parent['start_ticks'] > ancestor['start_ticks']:
                break
            ancestor = parent
    if unknown is not None:
        for current in rows.values():
            if current and current['ppid'] == owner:
                key = (current['pid'], current['start_ticks'])
                if key not in known:
                    unknown[key] = identity(current)
    return result


def send(expected, signum, proc=Path('/proc')):
    if not same(expected, proc):
        return
    fd = os.pidfd_open(expected['pid'])
    try:
        if same(expected, proc):
            signal.pidfd_send_signal(fd, signum)
    finally:
        os.close(fd)


def owns_listener(expected, port, proc=Path('/proc')):
    current = same(expected, proc)
    if not current or current['state'] == 'Z':
        return False
    listeners = []
    for table in ('tcp', 'tcp6'):
        for line in (proc / 'net' / table).read_text().splitlines()[1:]:
            fields = line.split()
            address, number = fields[1].split(':')
            if fields[3] == '0A' and int(number, 16) == port:
                listeners.append((table, address, fields[9]))
    sockets = {os.readlink(p) for p in (proc / str(expected['pid']) / 'fd').iterdir()}
    return (len(listeners) == 1 and listeners[0][:2] == ('tcp', '0100007F')
            and 'socket:[' + listeners[0][2] + ']' in sockets and same(expected, proc) is not None)


def closed(port):
    with socket.socket() as probe:
        probe.settimeout(.2)
        return probe.connect_ex(('127.0.0.1', port)) == errno.ECONNREFUSED


def stop(children, *, known=None, unknown=None, clock=time.monotonic, sleep=time.sleep, inspect=None, emit=send, reap=os.waitpid, state=None):
    forced = False
    roots = {child.pid for child, _ in children}
    known = known if known is not None else {(value['pid'], value['start_ticks']): value for _, value in children}
    inspect = inspect or (lambda ledger: members(ledger, unknown=unknown))
    def live_members():
        remaining = inspect(known)
        for value in remaining:
            if value['state'] == 'Z' and value['ppid'] == os.getpid() and value['pid'] not in roots:
                reap(value['pid'], os.WNOHANG)
        return [value for value in remaining if value['state'] != 'Z']

    for signum, seconds in STOP_PHASES:
        deadline, sent = clock() + seconds, set()
        while clock() < deadline:
            live = live_members()
            if not live:
                for child, _ in children:
                    child.wait(timeout=0)
                return forced, []
            forced = forced or signum != signal.SIGINT
            if state is not None:
                state['forced'] = forced
            for value in live:
                # PostgreSQL's fast shutdown is first requested only from its
                # postmaster. Escalation reaches individually owned descendants.
                key = (value['pid'], value['start_ticks'])
                if key not in sent and (signum != signal.SIGINT or value['pid'] in roots):
                    try:
                        emit(identity(value), signum)
                        sent.add(key)
                    except ProcessLookupError:
                        pass
            sleep(.05)
    remaining = live_members()
    if not remaining:
        for child, _ in children:
            child.wait(timeout=0)
    return forced, [identity(value) for value in remaining]


def run():
    pg_bin = Path(sys.argv[1]).resolve()
    if not hasattr(os, 'pidfd_open') or not hasattr(signal, 'pidfd_send_signal'):
        raise RuntimeError('identity-safe pidfd signalling is required')
    if ctypes.CDLL(None, use_errno=True).prctl(36, 1, 0, 0, 0) != 0:
        raise RuntimeError('owned descendant subreaper is required')
    (ROOT / 'test-runtime').mkdir(exist_ok=True)
    run_dir = Path(tempfile.mkdtemp(prefix='auth-', dir=ROOT / 'test-runtime'))
    run_dir.chmod(0o700)
    with socket.socket() as reservation:
        reservation.bind(('127.0.0.1', 0))
        port = reservation.getsockname()[1]
    if not 1024 < port < 65536 or port == 5432:
        raise RuntimeError('non-default private port required')
    # Ambient libpq service/options must not redirect the fixture's PG tools.
    env = {key: value for key, value in os.environ.items() if not key.startswith('PG')}
    env['PGCONNECT_TIMEOUT'] = '5'
    children, status, error, cancelled, pg, test, pg_identity = [], 72, None, None, None, None, None
    known, unknown = {}, {}

    def launch(command, output):
        child = subprocess.Popen(command, env=env, stdout=output, stderr=subprocess.STDOUT,
                                 stdin=subprocess.DEVNULL, start_new_session=True)
        children.append((child, None))
        current = row(child.pid)
        owned = current and current['ppid'] == os.getpid() and current['session'] == current['group'] == child.pid
        if not owned:
            raise RuntimeError('owned child birth/session unavailable')
        children[-1] = (child, identity(current))
        known[(child.pid, current['start_ticks'])] = identity(current)
        return child

    def interrupted(signum, _frame):
        nonlocal cancelled
        cancelled = 128 + signum

    def check_cancelled():
        if cancelled:
            raise InterruptedError(cancelled)

    def wait_for(child, seconds):
        deadline = time.monotonic() + seconds
        while time.monotonic() < deadline:
            check_cancelled()
            members(known, unknown=unknown)
            try:
                result = child.wait(timeout=min(.2, max(.01, deadline - time.monotonic())))
                members(known, unknown=unknown)
                check_cancelled()
                return result
            except subprocess.TimeoutExpired:
                pass
        raise subprocess.TimeoutExpired(child.args, seconds)

    signal.signal(signal.SIGINT, interrupted)
    signal.signal(signal.SIGTERM, interrupted)
    try:
        with (run_dir / 'initdb.log').open('w') as output:
            launch([str(pg_bin / 'initdb'), '-D', str(run_dir / 'data'), '--auth-local=reject',
                    '--auth-host=trust', '--no-locale', '--encoding=UTF8'], output)
            wait_for(children[-1][0], 30)
        if children[-1][0].returncode:
            raise RuntimeError('isolated initdb failed')
        with (run_dir / 'postgres.log').open('w') as output:
            pg = launch([str(pg_bin / 'postgres'), '-D', str(run_dir / 'data'), '-k', '', '-h',
                         '127.0.0.1', '-p', str(port), '-c', 'max_connections=12', '-c', 'shared_buffers=16MB',
                         '-c', 'fsync=off', '-c', 'full_page_writes=off'], output)
        pg_identity = children[-1][1]
        deadline = time.monotonic() + START_SECONDS
        while time.monotonic() < deadline:
            check_cancelled()
            members(known, unknown=unknown)
            current = same(pg_identity)
            if not current or current['state'] == 'Z':
                raise RuntimeError('isolated postgres exited before readiness')
            if owns_listener(pg_identity, port):
                probe = launch([str(pg_bin / 'pg_isready'), '-h', '127.0.0.1', '-p', str(port),
                                '-d', 'postgres', '-t', '1'], subprocess.DEVNULL)
                wait_for(probe, min(1.2, max(.01, deadline - time.monotonic())))
                if probe.returncode == 0 and owns_listener(pg_identity, port):
                    break
            time.sleep(.05)
        else:
            raise RuntimeError('owned postgres startup deadline')
        if not owns_listener(pg_identity, port):
            raise RuntimeError('postgres socket ownership changed before createdb')
        check_cancelled()
        createdb = launch([str(pg_bin / 'createdb'), '-h', '127.0.0.1', '-p', str(port),
                           '--maintenance-db=postgres', 'weebdex_platform_test'], None)
        if wait_for(createdb, 10):
            raise RuntimeError('isolated createdb failed')
        if not owns_listener(pg_identity, port):
            raise RuntimeError('postgres socket ownership changed before tests')
        env['PLATFORM_TEST_RUNTIME_DIR'] = str(run_dir)
        env['PLATFORM_AUTH_TEST_DATABASE_URL'] = f'host=127.0.0.1 port={port} dbname=weebdex_platform_test'
        (run_dir / 'fixture.json').write_text(json.dumps(dict(database='weebdex_platform_test',
            transport='private loopback TCP', fixture_only=True)) + '\n')
        command = sys.argv[2:] or ['cargo', 'test', '--manifest-path', str(ROOT / 'backend/Cargo.toml'),
                                  '--test', 'auth_http', '--', '--ignored', '--test-threads=1']
        check_cancelled()
        test = launch(command, None)
        status = wait_for(test, TEST_SECONDS)
        status = status if status >= 0 else 128 - status
    except InterruptedError as exc:
        status, error = int(exc.args[0]), 'interrupted'
    except subprocess.TimeoutExpired:
        status, error = 124, 'owned command deadline'
    except Exception as exc:
        error = type(exc).__name__ + ': ' + str(exc)
    finally:
        signal.signal(signal.SIGINT, signal.SIG_IGN)
        signal.signal(signal.SIGTERM, signal.SIG_IGN)
        if cancelled:
            status, error = cancelled, error or 'interrupted'
        forced, remaining, cleanup_errors, cleanup_state = False, [], [], {}
        try:
            members(known, unknown=unknown)
            pg_current = same(pg_identity) if pg else None
            if pg and (not pg_current or pg_current['state'] == 'Z'):
                error = error or 'postgres exited before cleanup'
        except Exception as exc:
            cleanup_errors.append(type(exc).__name__)
        if any(expected is None for _, expected in children):
            cleanup_errors.append('child ownership unconfirmed')
        try:
            forced, remaining = stop([(child, expected) for child, expected in children if expected is not None],
                                     known=known, unknown=unknown, state=cleanup_state)
        except Exception as exc:
            forced = cleanup_state.get('forced', False)
            cleanup_errors.append(type(exc).__name__)
        try:
            members(known, unknown=unknown)
        except Exception as exc:
            cleanup_errors.append(type(exc).__name__)
        try:
            port_closed = closed(port)
        except OSError:
            port_closed = False
        verified = not remaining and not cleanup_errors and not unknown and port_closed
        receipt = dict(postgres_cleanup='verified_stopped' if verified else 'cleanup_unconfirmed',
                       exit=status, forced=forced, closed_port=port_closed, remaining_owned_processes=remaining,
                       cleanup_errors=cleanup_errors, error=error, test_timeout_seconds=TEST_SECONDS,
                       captured_births=list(known.values()), unconfirmed_births=list(unknown.values()),
                       unconfirmed_child_pids=sorted({child.pid for child, expected in children if expected is None}
                                                    | {value['pid'] for value in unknown.values()}),
                       test_returncode=test.returncode if test else None,
                       postgres_returncode=pg.returncode if pg else None,
                       startup_timeout_seconds=START_SECONDS, cleanup_phase_seconds=[n for _, n in STOP_PHASES])
        (run_dir / 'cleanup.json').write_text(json.dumps(receipt, indent=2) + '\n')
    if error:
        print(error, file=sys.stderr)
    return 71 if forced or not verified else (72 if error and status == 0 else status)


if __name__ == '__main__':
    sys.exit(run())
