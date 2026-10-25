#!/usr/bin/env python3
"""Private, quiesced database+storage checkpoints. Default is a read-only plan.
No service control, key creation, credential output, overwrite, or automatic deletion.
"""
import argparse
import ctypes
import ctypes.util
import hashlib
import json
import os
from pathlib import Path
import shutil
import socket
import subprocess
import time

SCHEMA = 'weebdex-private-checkpoint-v1'
PROJECT = Path(__file__).resolve().parents[2]
CHUNK = 1024 * 1024
MAX_MANIFEST_BYTES = 64 * CHUNK

class Refused(RuntimeError):
    pass

def require(ok, message):
    if not ok:
        raise Refused(message)

def absolute(value):
    p = Path(value)
    require(p.is_absolute() and '..' not in p.parts and p.resolve() == p,
            'Use an absolute physical path without symlinks or parent components')
    return p

def overlap(a, b):
    return a == b or a in b.parents or b in a.parents

class Budget:
    def __init__(self, seconds=1800, max_files=1000000, max_bytes=1024**4,
                 min_free=4*1024**3, filesystem=None):
        require(1 <= seconds <= 86400 and 1 <= max_files <= 10000000 and 1 <= max_bytes < 2**63,
                'Finite positive time/file/byte bounds are required')
        require(min_free >= 0, 'Free-space reserve must be nonnegative')
        self.end = time.monotonic() + seconds
        self.max_files, self.max_bytes, self.min_free = max_files, max_bytes, min_free
        self.filesystem = filesystem
        self.output_limit = None
    def check(self, writing=False):
        require(time.monotonic() < self.end, 'Checkpoint deadline reached')
        if writing:
            require(shutil.disk_usage(self.filesystem).free >= self.min_free,
                    'Checkpoint free-space reserve reached')
            if self.output_limit and self.output_limit[0].exists():
                require(self.output_limit[0].stat().st_size <= self.output_limit[1],
                        'Checkpoint output byte bound reached')

def walk_bounded(root, budget, wrapper=False):
    seen = 0 if wrapper else 1
    yield root
    pending = [root]
    while pending:
        directory = pending.pop()
        children = []
        with os.scandir(directory) as entries:
            for entry in entries:
                budget.check()
                seen += 1
                require(seen <= budget.max_files, 'Storage entry bound exceeded')
                require(not entry.is_symlink(), 'Storage links are unsupported')
                children.append(Path(entry.path))
        children.sort()
        yield from children
        pending.extend(reversed([p for p in children if p.is_dir()]))

def digest(path, budget):
    h = hashlib.sha256()
    total = 0
    with os.fdopen(os.open(path, os.O_RDONLY | os.O_NOFOLLOW), 'rb') as f:
        while chunk := f.read(CHUNK):
            budget.check()
            total += len(chunk)
            require(total <= max(budget.max_bytes, MAX_MANIFEST_BYTES), 'Hashed input exceeded its byte bound')
            h.update(chunk)
    return h.hexdigest()

def scan(roots, budget):
    rows, total = [], 0
    for name, root in sorted(roots.items()):
        root = absolute(root)
        require(root.is_dir(), 'A configured storage root is unavailable')
        for p in walk_bounded(root, budget):
            budget.check()
            require(not p.is_symlink(), 'Storage links are unsupported; checkpoint refused')
            s = p.stat()
            require(p.is_file() or p.is_dir(), 'Storage special files are unsupported')
            require(not p.is_file() or s.st_nlink == 1, 'Storage hardlinks are unsupported')
            rel = p.relative_to(root).as_posix()
            row = {'root': name, 'path': rel, 'kind': 'directory' if p.is_dir() else 'file',
                   'bytes': s.st_size if p.is_file() else 0,
                   'identity': [s.st_dev, s.st_ino, s.st_mtime_ns, s.st_size]}
            if p.is_file():
                total += s.st_size
                require(total <= budget.max_bytes, 'Storage byte bound exceeded')
                row['sha256'] = digest(p, budget)
            rows.append(row)
            require(len(rows) <= budget.max_files, 'Storage entry bound exceeded')
    return rows

def discover(config_path, node_path=None):
    config_path = absolute(config_path)
    require(config_path.is_file() and config_path.stat().st_size <= 4*CHUNK, 'Configuration is unavailable or oversized')
    config = json.loads(config_path.read_text())
    config_files = [config_path]
    publication = config.get('protocol_publication')
    if node_path:
        node_file = absolute(node_path)
        require(node_file.is_file() and node_file.stat().st_size <= 4*CHUNK, 'Original configuration is unavailable or oversized')
        node = json.loads(node_file.read_text())
        config_files.append(node_file)
    else:
        node = (publication or {}).get('native_node')
    require(isinstance(node, dict), 'An explicit original node configuration is required')
    storage = (publication or {}).get('storage') or node.get('storage')
    require(isinstance(storage, dict) and storage.get('path'), 'Original storage is required')
    if node.get('storage'):
        require(absolute(node['storage']['path']) == absolute(storage['path']),
                'Publication and original listener storage paths disagree')
    require(isinstance(node.get('backend'), dict), 'Original projection storage is required')
    roots = {'native': absolute(config['uploads_dir']), 'originals': absolute(storage['path']),
             'projection': absolute(node['backend']['data_dir'])}
    for a, root in roots.items():
        require(not overlap(root, PROJECT), 'Storage cannot overlap project/source artifacts')
        for b, other in roots.items():
            require(a == b or not overlap(root, other), 'Storage roots must be disjoint')
    require(all(not overlap(path, root) for path in config_files for root in roots.values()),
            'Configuration files must be separate from checkpointed storage')
    secrets = [(publication or {}).get('operator_key_file'), (publication or {}).get('group_keys_dir'),
               node.get('service_key_file'), node.get('group_keys_dir')]
    for secret in filter(None, secrets):
        p = absolute(secret)
        require(all(not overlap(p, root) for root in roots.values()),
                'Credential/key roots must be separate from checkpointed storage')
    ui = absolute(config['ui_dir'])
    require(all(not overlap(ui, root) for root in roots.values()), 'Public UI overlaps private storage')
    ports = [config['bind'], node['bind']]
    return config, roots, ports

def stopped(roots, listeners):
    for listener in listeners:
        host, port = listener.rsplit(':', 1)
        host = host.strip('[]')
        require(host in ['127.0.0.1', '::1'], 'Only explicit numeric loopback listeners are supported')
        try:
            with socket.create_connection((host, int(port)), timeout=.3):
                raise Refused('A configured application/original listener is still running')
        except ConnectionRefusedError:
            pass
        except Refused:
            raise
        except OSError:
            raise Refused('Listener shutdown could not be verified') from None
    require(Path('/proc').is_dir(), 'Linux process-reference verification is required')
    prefixes = [str(p) + os.sep for p in roots.values()]
    for proc in Path('/proc').glob('[0-9]*'):
        try:
            args = (proc/'cmdline').read_bytes().split(b'\0')
            refs = [a.decode(errors='replace').split('=', 1)[-1] for a in args]
            refs += [os.readlink(fd) for fd in (proc/'fd').iterdir()]
            refs.append(os.readlink(proc/'cwd'))
            require(not any(value == pre[:-1] or value.startswith(pre) for pre in prefixes for value in refs),
                    'A process still references checkpoint storage')
        except FileNotFoundError:
            continue
        except PermissionError:
            raise Refused('Process references could not all be inspected') from None

def private_destination(path, roots):
    path = absolute(path)
    require(not path.exists() and path.parent.is_dir(), 'Destination must be a new path in an existing directory')
    require(not overlap(path, PROJECT) and all(not overlap(path, p) for p in roots.values()),
            'Private destination overlaps source or storage')
    return path

class ConnOption(ctypes.Structure):
    _fields_ = [(name, ctypes.c_char_p) for name in
                ['keyword', 'envvar', 'compiled', 'val', 'label', 'dispchar']] + [('dispsize', ctypes.c_int)]

def pg_environment(config, variable):
    dsn = os.environ.get(variable)
    require(bool(dsn) and len(dsn) <= 65536, 'A bounded private database environment value is required')
    # Official libpq parses both runtime URL and keyword DSNs without connecting.
    # Translate explicit values to its own environment defaults: no secret is put
    # in argv or a generated config file. pg_restore still needs an explicit -d.
    library = ctypes.util.find_library('pq')
    require(library is not None, 'The matching official libpq client library is required')
    pq = ctypes.CDLL(library)
    pq.PQconninfoParse.argtypes = [ctypes.c_char_p, ctypes.POINTER(ctypes.c_void_p)]
    pq.PQconninfoParse.restype = ctypes.POINTER(ConnOption)
    pq.PQconninfoFree.argtypes = [ctypes.POINTER(ConnOption)]
    pq.PQfreemem.argtypes = [ctypes.c_void_p]
    error = ctypes.c_void_p()
    options = pq.PQconninfoParse(dsn.encode(), ctypes.byref(error))
    # Do not pass unrelated mail/application secrets to the PostgreSQL children.
    env = {k:v for k,v in os.environ.items() if k.startswith('PG') or k in
           ['PATH','HOME','LANG','LC_ALL','LC_CTYPE','TZ','LD_LIBRARY_PATH','SYSTEMROOT']}
    env.pop('PGSERVICE', None); env.pop('PGSERVICEFILE', None)
    explicit = {}
    try:
        require(bool(options), 'The private database connection value is invalid')
        i = 0
        while options[i].keyword:
            option = options[i]; i += 1
            if option.val is not None:
                require(option.envvar is not None, 'An unsupported database connection option is configured')
                key, value = option.keyword.decode(), option.val.decode()
                require(key != 'service', 'Expand private service configuration into the existing runtime DSN first')
                explicit[key] = value
                env[option.envvar.decode()] = value
    finally:
        if options: pq.PQconninfoFree(options)
        if error.value: pq.PQfreemem(error)
    name = explicit.get('dbname', '')
    require(0 < len(name.encode()) <= 63 and not any(c in name for c in ['=', '\t', '\n', '\r', '\0'])
            and not name.startswith(('postgres://', 'postgresql://')), 'An explicit ordinary database name is required')
    require(not config.get('production') or env.get('PGSSLMODE') == 'verify-full',
            'Production checkpoint tools require sslmode=verify-full')
    env['PGDATABASE'] = name
    return env

def run_tool(name, args, env, budget, capture=False):
    executable = shutil.which(name)
    require(executable is not None, 'Install the matching official PostgreSQL tools separately')
    child = subprocess.Popen([executable, *args], env=env, stdin=subprocess.DEVNULL,
                             stdout=subprocess.PIPE if capture else subprocess.DEVNULL,
                             stderr=subprocess.DEVNULL)
    try:
        while child.poll() is None:
            budget.check(writing=True)
            time.sleep(.1)
        require(child.returncode == 0, 'PostgreSQL checkpoint command failed; no success is asserted')
        value = child.stdout.read(1025) if capture else b''
        require(len(value) <= 1024, 'PostgreSQL preflight response exceeded its bound')
        return value.decode().strip()
    finally:
        if child.poll() is None:
            child.terminate()
            try:
                child.wait(timeout=5)
            except subprocess.TimeoutExpired:
                child.kill()
                child.wait(timeout=5)
        if child.stdout:
            child.stdout.close()

def copy_file(source, destination, budget, expected_bytes):
    before = source.stat()
    require(source.is_file() and not source.is_symlink(), 'Only regular source files are supported')
    require(before.st_size == expected_bytes, 'Source size changed before checkpoint copy')
    with os.fdopen(os.open(source, os.O_RDONLY | os.O_NOFOLLOW), 'rb') as src, destination.open('xb') as dst:
        opened = os.fstat(src.fileno())
        require((before.st_dev, before.st_ino) == (opened.st_dev, opened.st_ino), 'Source identity changed before copy')
        os.chmod(destination, 0o600)
        copied = 0
        while chunk := src.read(CHUNK):
            budget.check(writing=True)
            copied += len(chunk)
            require(copied <= expected_bytes, 'A source grew beyond its checkpoint byte bound')
            dst.write(chunk)
        dst.flush()
        os.fsync(dst.fileno())
    after = source.stat()
    require(copied == expected_bytes and (before.st_dev, before.st_ino, before.st_mtime_ns, before.st_size) ==
            (after.st_dev, after.st_ino, after.st_mtime_ns, after.st_size), 'A source file changed during checkpoint copy')
    require(digest(source, budget) == digest(destination, budget), 'Copied file integrity mismatch')

def sync_dir(path):
    fd = os.open(path, os.O_RDONLY | os.O_DIRECTORY)
    try:
        os.fsync(fd)
    finally:
        os.close(fd)

def write_json(path, value):
    encoded = json.dumps(value, sort_keys=True, indent=2) + '\n'
    require(len(encoded.encode()) <= MAX_MANIFEST_BYTES, 'Checkpoint manifest byte bound exceeded')
    with path.open('x') as f:
        os.chmod(path, 0o600)
        f.write(encoded); f.flush(); os.fsync(f.fileno())

def backup(args, runner=run_tool):
    config, roots, listeners = discover(args.config, args.node_config)
    output = private_destination(args.bundle, {**roots, 'public_ui': absolute(config['ui_dir'])})
    budget = Budget(args.timeout, args.max_files, args.max_bytes, args.min_free_bytes, output.parent)
    release = release_hash(args, budget)
    plan = {'mode': 'backup', 'dryRun': not args.execute, 'roots': {k: str(v) for k, v in roots.items()},
            'bundle': str(output), 'databaseEnvironment': args.database_env,
            'privateArtifact': True, 'signingKeysAndConfigurationIncluded': False,
            'databaseContainsSensitiveAccountData': True,
            'releaseManifestSha256': release,
            'requires': ['all writers quiesced', 'closed listeners/process-reference check', 'full database plus all storage roots']}
    if not args.execute:
        return plan
    require(args.quiesced, 'Explicit --quiesced acknowledgement is required; script never stops services')
    stopped(roots, listeners)
    env = pg_environment(config, args.database_env)
    budget.check(writing=True)
    before = scan(roots, budget)
    storage_bytes = sum(row['bytes'] for row in before)
    output.mkdir(mode=0o700)
    database = output/'database.dump'
    budget.output_limit = (database, budget.max_bytes-storage_bytes)
    runner('pg_dump', ['--no-password', '--format=custom', '--no-owner', '--no-acl', '--file', str(database)], env, budget)
    require(database.is_file() and database.stat().st_size+storage_bytes <= budget.max_bytes, 'Database dump is missing or exceeds byte bound')
    os.chmod(database, 0o600)
    with database.open('rb') as f:
        os.fsync(f.fileno())
    for name in roots:
        (output/'storage'/name).mkdir(parents=True, mode=0o700)
    for row in before:
        destination = output/'storage'/row['root']/row['path']
        if row['kind'] == 'directory':
            destination.mkdir(exist_ok=True, mode=0o700)
        else:
            copy_file(roots[row['root']]/row['path'], destination, budget, row['bytes'])
    require(before == scan(roots, budget), 'Storage changed; incomplete private checkpoint retained')
    stopped(roots, listeners)
    entries = [{k: v for k, v in row.items() if k != 'identity'} for row in before]
    manifest = {'schema': SCHEMA, 'privateArtifact': True, 'operatorAssertedAllWritersQuiesced': True,
                'database': {'bytes': database.stat().st_size, 'sha256': digest(database, budget)},
                'sourceConfigSha256': digest(absolute(args.config), budget),
                'releaseManifestSha256': release,
                'originalNodeConfigSha256': digest(absolute(args.node_config), budget) if args.node_config else None,
                'roots': list(sorted(roots)), 'entries': entries,
                'databaseContainsPrivateAccountData': True, 'signingKeysAndConfigurationExcluded': True}
    write_json(output/'manifest.json', manifest)
    # Complete marker is last; partial outputs are never automatically removed.
    write_json(output/'COMPLETE.json', {'manifestSha256': digest(output/'manifest.json', budget)})
    for directory in sorted([output, *[p for p in output.rglob('*') if p.is_dir()]], reverse=True):
        sync_dir(directory)
    sync_dir(output.parent)
    return {'mode': 'backup', 'complete': True, 'privateArtifact': True, 'entries': len(entries)}

def private_json(path):
    require(path.is_file() and not path.is_symlink() and path.stat().st_size <= MAX_MANIFEST_BYTES,
            'Checkpoint metadata is unavailable, linked, or oversized')
    return json.loads(path.read_text())

def release_hash(args, budget):
    require(args.release_manifest, 'An existing release manifest is required')
    path = absolute(args.release_manifest)
    require(path.is_file() and path.stat().st_size <= 4*CHUNK, 'Release manifest is unavailable or oversized')
    return digest(path, budget)

def verify(bundle, budget):
    bundle = absolute(bundle)
    require(bundle.is_dir() and not bundle.is_symlink(), 'Private checkpoint directory is unavailable')
    require(not overlap(bundle, PROJECT) and bundle.stat().st_mode & 0o077 == 0,
            'Checkpoint must be private and outside project/public artifacts')
    require({p.name for p in bundle.iterdir()} == {'COMPLETE.json','manifest.json','database.dump','storage'},
            'Checkpoint is incomplete or has unaccounted top-level members')
    marker = private_json(bundle/'COMPLETE.json')
    require(marker == {'manifestSha256': digest(bundle/'manifest.json', budget)}, 'Checkpoint completion/integrity marker mismatch')
    manifest = private_json(bundle/'manifest.json')
    require(manifest.get('schema') == SCHEMA and manifest.get('privateArtifact') is True, 'Unsupported private checkpoint')
    require(manifest['roots'] == ['native', 'originals', 'projection'], 'All three storage roots are required')
    require(len(manifest['entries']) <= budget.max_files, 'Checkpoint entry bound exceeded')
    database = bundle/'database.dump'
    require(not database.is_symlink() and database.is_file() and database.stat().st_nlink == 1 and database.stat().st_size == manifest['database']['bytes']
            and digest(database, budget) == manifest['database']['sha256'], 'Database dump integrity mismatch')
    expected = set(); total = database.stat().st_size
    require(total <= budget.max_bytes, 'Checkpoint byte bound exceeded')
    for row in manifest['entries']:
        budget.check()
        rel = Path(row['path'])
        require(row['root'] in manifest['roots'] and not rel.is_absolute() and '..' not in rel.parts,
                'Checkpoint path escaped its storage root')
        path = bundle/'storage'/row['root']/rel
        require(path not in expected and not path.is_symlink(), 'Duplicate or linked checkpoint path')
        expected.add(path)
        require(row['kind'] in ['directory', 'file'], 'Unsupported checkpoint member')
        require(isinstance(row['bytes'],int) and row['bytes'] >= 0, 'Invalid checkpoint byte count')
        if row['kind'] == 'directory':
            require(path.is_dir(), 'Checkpoint directory is missing')
        else:
            total += row['bytes']; require(total <= budget.max_bytes, 'Checkpoint byte bound exceeded')
            require(path.is_file() and path.stat().st_nlink == 1 and path.stat().st_size == row['bytes'] and digest(path, budget) == row['sha256'], 'Storage integrity mismatch')
    actual = set(walk_bounded(bundle/'storage', budget, wrapper=True)) - {bundle/'storage'}
    require(actual == expected, 'Checkpoint has unaccounted storage members')
    require(not any(p.is_symlink() for p in bundle.rglob('*')), 'Checkpoint links are unsupported')
    return manifest

EMPTY_SQL = """SELECT current_database(),
 (SELECT count(*) FROM pg_class c JOIN pg_namespace n ON n.oid=c.relnamespace WHERE n.nspname !~ '^pg_' AND n.nspname<>'information_schema')+
 (SELECT count(*) FROM pg_proc p JOIN pg_namespace n ON n.oid=p.pronamespace WHERE n.nspname !~ '^pg_' AND n.nspname<>'information_schema')+
 (SELECT count(*) FROM pg_type t JOIN pg_namespace n ON n.oid=t.typnamespace WHERE n.nspname !~ '^pg_' AND n.nspname<>'information_schema')+
 (SELECT count(*) FROM pg_namespace WHERE nspname NOT IN('public','information_schema') AND nspname !~ '^pg_');"""

def restore(args, runner=run_tool):
    config, roots, listeners = discover(args.config, args.node_config)
    require(not overlap(absolute(args.bundle), absolute(config['ui_dir'])), 'Private checkpoint overlaps public UI')
    budget = Budget(args.timeout, args.max_files, args.max_bytes, args.min_free_bytes, absolute(args.bundle).parent)
    manifest = verify(args.bundle, budget)
    require(manifest.get('releaseManifestSha256') == release_hash(args, budget), 'Restore requires the exact checkpoint release manifest')
    for root in roots.values():
        private_destination(root, {'bundle': absolute(args.bundle)})
    if not args.execute:
        return {'mode': 'restore', 'dryRun': True, 'verifiedPrivateCheckpoint': True,
                'entries': len(manifest['entries']), 'targetRoots': {k: str(v) for k, v in roots.items()},
                'databaseEnvironment': args.database_env, 'requires': ['new empty database', '--confirm-empty-database NAME', '--quiesced']}
    require(args.quiesced and args.confirm_empty_database, 'Explicit quiescence and target database name are required')
    stopped(roots, listeners)
    env = pg_environment(config, args.database_env)
    # Read-only preflight. Restore never creates/drops a database or uses --clean.
    answer = runner('psql', ['--no-password', '-X', '-q', '-A', '-t', '-F', '\t', '-v', 'ON_ERROR_STOP=1', '-c', EMPTY_SQL], env, budget, True)
    require(answer == args.confirm_empty_database + '\t0', 'Target name mismatch or nonempty application database')
    for root in roots.values():
        budget.filesystem = root.parent; budget.check(writing=True); root.mkdir(mode=0o700)
    for row in manifest['entries']:
        destination = roots[row['root']]/row['path']
        if row['kind'] == 'directory':
            destination.mkdir(mode=0o700, exist_ok=True)
        else:
            budget.filesystem = destination.parent
            copy_file(absolute(args.bundle)/'storage'/row['root']/row['path'], destination, budget, row['bytes'])
    restored = scan(roots, budget)
    require([{k:v for k,v in row.items() if k!='identity'} for row in restored] == manifest['entries'], 'Restored storage verification failed')
    for root in roots.values():
        for directory in sorted([root, *[p for p in root.rglob('*') if p.is_dir()]], reverse=True): sync_dir(directory)
        sync_dir(root.parent)
    budget.filesystem = next(iter(roots.values())).parent
    runner('pg_restore', ['--no-password', '--dbname', env['PGDATABASE'], '--single-transaction', '--exit-on-error',
                          '--no-owner', '--no-acl', str(absolute(args.bundle)/'database.dump')], env, budget)
    return {'mode': 'restore', 'complete': True, 'databaseRestoredTransactionally': True,
            'requiresBeforeServing': ['original schema verify', 'platform verify', 'authority/readiness and exact media/restart checks'],
            'signingKeysRestored': False}

def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('mode', choices=['backup', 'restore', 'verify'])
    p.add_argument('--config'); p.add_argument('--node-config'); p.add_argument('--bundle', required=True)
    p.add_argument('--release-manifest')
    p.add_argument('--database-env', default='PLATFORM_DATABASE_URL')
    p.add_argument('--execute', action='store_true'); p.add_argument('--quiesced', action='store_true')
    p.add_argument('--confirm-empty-database')
    p.add_argument('--timeout', type=int, default=1800); p.add_argument('--max-files', type=int, default=1000000)
    p.add_argument('--max-bytes', type=int, default=1024**4); p.add_argument('--min-free-bytes', type=int, default=4*1024**3)
    args = p.parse_args()
    try:
        require(args.mode == 'verify' or args.config, '--config is required')
        if args.mode == 'verify':
            require(not args.execute, 'Verify is always read-only')
            m = verify(args.bundle, Budget(args.timeout, args.max_files, args.max_bytes))
            value = {'verifiedPrivateCheckpoint': True, 'entries': len(m['entries']), 'schema': SCHEMA}
        else:
            value = backup(args) if args.mode == 'backup' else restore(args)
        print(json.dumps(value, sort_keys=True, indent=2))
        return 0
    except (Refused, OSError, ValueError, KeyError, TypeError, AttributeError) as error:
        # Do not print connection strings, config values, SQL output or tool stderr.
        print(json.dumps({'success': False, 'errorClass': type(error).__name__,
                          'message': str(error) if isinstance(error, Refused) else 'Checkpoint input/operation failed; no success asserted',
                          'partialPrivateOutputsMayRemain': True}))
        return 1

if __name__ == '__main__':
    raise SystemExit(main())
