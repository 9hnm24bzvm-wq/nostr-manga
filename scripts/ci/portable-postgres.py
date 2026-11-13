"""Disposable CI fixture. Never attaches to an existing database or reuses data."""
import glob
import json
import os
from pathlib import Path
import secrets
import shutil
import socket
import subprocess
import sys

root = Path(os.environ['RUNNER_TEMP']) / 'mangapub-portable-postgres'
state = root / 'owned-state.json'
suffix = '.exe' if os.name == 'nt' else ''

def run(args, **kwargs):
    return subprocess.run([str(a) for a in args], check=True, timeout=120, **kwargs)

def owned():
    data = json.loads(state.read_text())
    if data['run'] != os.environ['GITHUB_RUN_ID'] or data['attempt'] != os.environ['GITHUB_RUN_ATTEMPT']:
        raise SystemExit('fixture belongs to a different run; refusing action')
    return data

action = sys.argv[1:] if len(sys.argv) == 2 else []
if action == ['stop']:
    if state.exists():
        data = owned()
        # pg_ctl uses only our exclusive data directory, never a service name.
        if (root / 'data' / 'postmaster.pid').exists():
            run([Path(data['bin']) / ('pg_ctl' + suffix), '-D', root / 'data', '-m', 'fast', '-w', 'stop'])
    raise SystemExit(0)
if action == ['test']:
    owned()
    command = ['cargo', '+1.90.0', 'test', '--locked', '--manifest-path',
               'manga-protocol-lab/server/Cargo.toml', '--lib',
               'backend::native_tests::native_projection_semantics', '--', '--exact', '--ignored']
    result = subprocess.run(command, capture_output=True, text=True, timeout=900)
    print(result.stdout, end='')
    print(result.stderr, end='', file=sys.stderr)
    if result.returncode != 0 or 'test result: ok. 1 passed;' not in result.stdout:
        raise SystemExit('native projection test failed or did not execute exactly once')
    raise SystemExit(0)
if action != ['start']:
    raise SystemExit('expected start, test or stop')
root.mkdir(exist_ok=False)  # any stale fixture is an error
candidates = []
if shutil.which('initdb'):
    candidates.append(Path(shutil.which('initdb')).parent)
if os.environ.get('PGBIN'):
    candidates.append(Path(os.environ['PGBIN']))
if os.environ.get('PGROOT'):
    candidates.append(Path(os.environ['PGROOT']) / 'bin')
candidates += [Path(p) for p in glob.glob(r'C:\Program Files\PostgreSQL\*\bin' if os.name == 'nt' else '/usr/lib/postgresql/*/bin')]
valid = [p for p in candidates if all((p / (n + suffix)).is_file() for n in ('initdb', 'pg_ctl', 'createdb'))]
if not valid:
    raise SystemExit('PostgreSQL server tools unavailable; no test was run')
binary = sorted(valid, key=lambda p: p.as_posix())[-1].resolve()
run([binary / ('initdb' + suffix), '--version'])
password = secrets.token_hex(24)
print('::add-mask::' + password, flush=True)
pwfile = root / 'password'
with pwfile.open('x') as output:
    output.write(password)
if os.name != 'nt':
    pwfile.chmod(0o600)
run([binary / ('initdb' + suffix), '-D', root / 'data', '-U', 'postgres',
     '--auth-local=trust', '--auth-host=scram-sha-256', '--pwfile=' + str(pwfile),
     '--encoding=UTF8', '--locale=C'])
# Select a free port; if another process wins the race, pg_ctl fails and we stop.
with socket.socket() as candidate:
    candidate.bind(('127.0.0.1', 0))
    port = candidate.getsockname()[1]
state.write_text(json.dumps({'run': os.environ['GITHUB_RUN_ID'], 'attempt': os.environ['GITHUB_RUN_ATTEMPT'], 'bin': str(binary), 'port': port}))
run([binary / ('pg_ctl' + suffix), '-D', root / 'data', '-l', root / 'server.log',
     '-o', f'-h 127.0.0.1 -p {port}', '-w', 'start'])
env = dict(os.environ, PGPASSWORD=password)
run([binary / ('createdb' + suffix), '-h', '127.0.0.1', '-p', str(port), '-U', 'postgres', 'manga_test'], env=env)
with open(os.environ['GITHUB_ENV'], 'a') as output:
    output.write(f'MANGA_TEST_DATABASE_URL=host=127.0.0.1 port={port} user=postgres password={password} dbname=manga_test\n')
