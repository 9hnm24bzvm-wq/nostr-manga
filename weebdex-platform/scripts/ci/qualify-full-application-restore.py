#!/usr/bin/env python3
"""Bounded synthetic application recovery. Planning is the default; no browser.

Only a real execution can emit passed-synthetic-data-restore-with-retained-credentials. This is not
production DR, independent host/key recovery or pending native-upload/title-command qualification. Private runtime
state is never copied into the report. See docs/full-application-restore.md.
"""
import argparse
import contextlib
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import secrets
import signal
import subprocess
import sys
import tempfile
import threading
import time
from http.client import HTTPConnection
from http.cookies import SimpleCookie

ROOT = Path(__file__).resolve().parents[2]
REPO = ROOT.parent
SOURCE_DB = 'weebdex_browser_test'
TARGET_DB = 'weebdex_restore_test'
SENTINEL_DB = 'weebdex_restore_occupied_test'
LIMIT_BYTES = 1024**3
RESERVE = 4 * 1024**3
STAGES = ('preflight', 'baseline', 'account', 'quiesce', 'pending-outbox',
          'handoff', 'backup-plan', 'backup', 'source-revocation', 'verify-bundle', 'restore-plan',
          'nonempty-refusal', 'restore', 'compare', 'restored-start',
          'restored-checks', 'restart-stop', 'restart-start', 'restart-checks',
          'source-unchanged', 'cleanup', 'complete')
REQUIRED = ('all_schemas_equal', 'all_business_rows_equal', 'all_sequences_equal', 'three_roots_equal',
            'restored_authentication_fenced', 'autonomous_roles_paused',
            'old_cookie_api_email_and_flows_rejected', 'fresh_login_verified', 'source_credentials_revoked_after_backup',
            'nonempty_sentinel_unchanged', 'source_unchanged', 'restored_checks',
            'restart_checks', 'original_outbox_reconciled', 'cleanup_clean',
            'retained_credentials_match', 'synthetic_server_keys_absent',
            'native_reader_media_hash_range_etag_verified',
            'original_endpoint_media_hashes_verified')
SCOPE = {'separately_retained_matching_credentials_required': True,
         'independent_full_host_recovery_qualified': False,
         'credential_recovery_qualified': False,
         'managed_group_key_recovery_qualified': False,
         'original_endpoint_range_qualified': False}


def module(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    value = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(value)
    return value


fixture_module = module('full_restore_fixture', ROOT / 'tests/runtime/browser_fixture.py')
checkpoint = module('full_restore_checkpoint', ROOT / 'scripts/private-checkpoint.py')


class Refused(RuntimeError):
    pass


def require(value):
    if not value:
        raise Refused('Synthetic restore qualification refused')


def sha(path):
    h = hashlib.sha256()
    with path.open('rb') as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b''):
            h.update(chunk)
    return h.hexdigest()


def file_row(path):
    require(path.is_file() and not path.is_symlink() and path.stat().st_size <= 512 * 1024**2)
    return {'bytes': path.stat().st_size, 'sha256': sha(path)}


def source_paths():
    directories = ['weebdex-platform/backend/src', 'weebdex-platform/backend/migrations',
        'weebdex-platform/backend/examples', 'manga-protocol-lab/server/src',
        'manga-protocol-lab/server/migrations', 'weebdex-platform/frontend/src',
        'weebdex-platform/frontend/static', 'weebdex-platform/frontend/scripts',
        'weebdex-platform/platform-integrations', 'weebdex-platform/scripts',
        'weebdex-platform/tests', 'weebdex-platform/backend/tests',
        'manga-protocol-lab/server/tests']
    fixed = ['weebdex-platform/backend/Cargo.toml', 'weebdex-platform/backend/Cargo.lock',
        'manga-protocol-lab/server/Cargo.toml', 'manga-protocol-lab/server/Cargo.lock',
        'weebdex-platform/frontend/package.json', 'weebdex-platform/frontend/yarn.lock',
        'weebdex-platform/scripts/private-checkpoint.py',
        'weebdex-platform/scripts/ci/qualify-full-application-restore.py',
        'weebdex-platform/tests/runtime/browser_fixture.py']
    paths = {REPO / name for name in fixed}
    for name in directories:
        directory = REPO / name
        require(directory.is_dir() and not directory.is_symlink())
        for current, dirs, files in os.walk(directory, followlinks=False):
            for name in dirs + files:
                require(not (Path(current) / name).is_symlink())
            dirs[:] = [name for name in dirs if name != '__pycache__']
            paths.update(Path(current) / name for name in files if not name.endswith(('.pyc', '.pyo')))
    # Frontend composition also depends on its root Vite/Svelte/TypeScript config.
    for path in (REPO / 'weebdex-platform/frontend').iterdir():
        if path.suffix in ('.js', '.mjs', '.ts', '.json'):
            require(path.is_file() and not path.is_symlink())
            paths.add(path)
    require(100 <= len(paths) <= 10000)
    return sorted(paths)


def tools_for(args):
    result = {'platform': args.binary, 'provisioner': args.protocol_provisioner,
              'strfry': REPO / 'manga-protocol-lab/vendor/strfry/strfry',
              'python': Path(sys.executable).resolve()}
    result.update({name: fixture_module.PG_BIN / name for name in
                   ('postgres', 'initdb', 'psql', 'pg_dump', 'pg_restore', 'pg_isready')})
    return result


def inventory(args):
    return {'format': 'synthetic-full-restore-pins-v1',
        'source': {str(p.relative_to(REPO)): file_row(p) for p in source_paths()},
        'tools': {name: file_row(p) for name, p in tools_for(args).items()},
        'release': file_row(args.release_manifest),
        'frontend_manifest': file_row(args.frontend_artifact_manifest)}


def read_json(path, ceiling=4 * 1024**2):
    require(path.is_file() and not path.is_symlink() and path.stat().st_size <= ceiling)
    return json.loads(path.read_text())


def validate_release(args, pins):
    release = read_json(args.release_manifest)
    require(release.get('format') == 1 and isinstance(release.get('artifacts'), list))
    entries = {}
    for row in release['artifacts']:
        require(set(row) == {'path', 'bytes', 'sha256'})
        name = row['path']
        require(isinstance(name, str) and name not in entries)
        entries[name] = {key: row[key] for key in ('bytes', 'sha256')}
    frontend = read_json(args.frontend_artifact_manifest)
    require(isinstance(frontend.get('files'), list))
    expected = {'artifacts/backend/weebdex-platform': pins['tools']['platform']}
    total = 0
    for row in frontend['files']:
        name = row['path']
        require(isinstance(name, str) and not Path(name).is_absolute()
                and '..' not in Path(name).parts and name == str(Path(name)))
        key = 'artifacts/frontend/' + name
        require(key not in expected)
        value = {key: row[key] for key in ('bytes', 'sha256')}
        require(value == file_row(args.ui_dir / name))
        expected[key] = value
        total += row['bytes']
    actual = set()
    for path in args.ui_dir.rglob('*'):
        require(not path.is_symlink() and (path.is_file() or path.is_dir()))
        if path.is_file():
            actual.add('artifacts/frontend/' + str(path.relative_to(args.ui_dir)))
    require(actual == set(expected) - {'artifacts/backend/weebdex-platform'})
    require(entries == expected and frontend['file_count'] == len(frontend['files'])
            and frontend['total_bytes'] == total and total <= 64 * 1024**2)
    require(release['lockfiles'] == {name: sha(ROOT / name) for name in
                                   ('backend/Cargo.lock', 'frontend/yarn.lock')})
    return sha(args.ui_dir / 'index.html')


def preflight(args):
    require(60 <= args.seconds <= 900)
    require(args.pins_sha256 and re.fullmatch('[0-9a-f]{64}', args.pins_sha256))
    require(sha(args.pins) == args.pins_sha256)
    pins = read_json(args.pins)
    require(pins == inventory(args))
    for path in tools_for(args).values():
        require(os.access(path, os.X_OK))
    args.frontend_index_sha256 = validate_release(args, pins)
    args.frontend_artifact_manifest_sha256 = pins['frontend_manifest']['sha256']
    args.expected_sha256 = pins['tools']['platform']['sha256']
    args.provisioner_sha256 = pins['tools']['provisioner']['sha256']
    # Existing fixture/provisioner root, key and database fences stay unchanged.
    args.lifetime_seconds = args.seconds
    args.second_protocol_recipient = args.ui_regression_controls = False
    args.public_upload_controls = args.pg_home_fault_controls = args.ubunchu_noncommercial = False
    require(args.output.is_absolute() and args.output.resolve() == args.output
            and not args.output.exists() and args.output.parent.is_dir())
    require(args.output.is_relative_to(ROOT / 'test-evidence'))
    return pins


def clean_success(report):
    return (report.get('runtime_executed') is True and report.get('stage') == 'complete'
            and all(report.get(key) is True for key in REQUIRED)
            and report.get('error_class') is None and report.get('budget_failure') is None
            and report.get('native_pending_upload_qualified') is False
            and report.get('production_dr_qualified') is False
            and all(report.get(key) is value for key, value in SCOPE.items()))


def relocated(config, roots):
    value = json.loads(json.dumps(config))
    value['uploads_dir'] = str(roots['native'])
    publication = value['protocol_publication']
    publication['storage']['path'] = str(roots['originals'])
    publication['native_node']['storage']['path'] = str(roots['originals'])
    publication['native_node']['backend']['data_dir'] = str(roots['projection'])
    return value



def retained_credentials(config, owner):
    """Private, in-memory continuity only; never a key recovery/public fingerprint."""
    require(owner.is_absolute() and owner.resolve() == owner and owner.is_dir())
    publication = config['protocol_publication']
    node = publication['native_node']
    operator = owner / 'fixture-keys/operator.key'
    service = owner / 'fixture-keys/service.key'
    groups = owner / 'original-group-keys'
    require(publication['operator_key_file'] == str(operator)
            and node['service_key_file'] == str(service)
            and publication['group_keys_dir'] == str(groups)
            and node['group_keys_dir'] == str(groups))
    for directory in (operator.parent, groups):
        require(directory.is_dir() and directory.resolve() == directory
                and directory.stat().st_mode & 0o077 == 0)
    # This operator-published corpus has no managed group keys. Refuse additions
    # rather than silently claiming group-key continuity/recovery or retaining one.
    require(not any(groups.iterdir()))
    result = []
    for path in (operator, service):
        require(path.is_file() and path.resolve() == path)
        metadata = path.stat()
        require(0 < metadata.st_size <= 128 and metadata.st_mode & 0o077 == 0
                and metadata.st_nlink == 1)
        result.append(file_row(path))
    return result


def stable_rows(rows):
    return [{k: v for k, v in row.items() if k != 'identity'} for row in rows]


def row_projection(name, business=False):
    """Compare every row and every business column, not an omitted auth table."""
    columns = checkpoint.RESTORE_FENCE_COLUMNS.get(name, ()) if business else ()
    return 'to_jsonb(t)' + (" - ARRAY[" + ','.join("'" + c + "'" for c in columns) + "]::text[]" if columns else '')


def paused_config(config):
    value = json.loads(json.dumps(config))
    node = value['protocol_publication']['native_node']
    for flag in checkpoint.PAUSED_NODE_FLAGS:
        node[flag] = False
    node['storage_audit_interval_seconds'] = 0
    node['discovery_private_overrides'] = []
    for role in ('enrollment', 'public_serving'):
        if node.get(role) is not None:
            node[role]['enabled'] = False
    checkpoint.require_paused_roles(value)
    return value


class ResourceGuard:
    """Finite watchdog; a failed budget can never become a successful receipt."""
    def __init__(self, owner, private, seconds):
        self.owner, self.private = owner, private
        self.end = time.monotonic() + seconds
        self.failure = None
        self.done = threading.Event()
        self.thread = threading.Thread(target=self.monitor, daemon=True)
        self.peak = {'allocated_bytes': 0, 'owned_rss_bytes': 0}

    def sample(self):
        require(time.monotonic() < self.end)
        require(checkpoint.shutil.disk_usage(self.private).free >= RESERVE + 128 * 1024**2)
        memory = dict(line.split(':', 1) for line in Path('/proc/meminfo').read_text().splitlines())
        require(int(memory['MemAvailable'].split()[0]) * 1024 >= 2 * 1024**3)
        size = entries = 0
        for root in (self.owner.run, self.private):
            for directory, dirs, files in os.walk(root, followlinks=False):
                for name in dirs + files:
                    entries += 1
                    require(entries <= 100000)
                    path = Path(directory) / name
                    try:
                        require(not path.is_symlink())
                        size += path.stat().st_blocks * 512
                    except FileNotFoundError:
                        pass  # PostgreSQL temporary files may disappear during sampling.
        rss = 0
        for row in self.owner.owned_members():
            try:
                rss += int((Path('/proc') / str(row['pid']) / 'statm').read_text().split()[1]) * os.sysconf('SC_PAGE_SIZE')
            except FileNotFoundError:
                pass
        self.peak['allocated_bytes'] = max(size, self.peak['allocated_bytes'])
        self.peak['owned_rss_bytes'] = max(rss, self.peak['owned_rss_bytes'])
        require(size <= LIMIT_BYTES and rss <= 1536 * 1024**2)

    def monitor(self):
        while not self.done.wait(.5):
            try:
                self.sample()
            except BaseException:
                self.failure = 'resource-or-time-bound'
                self.owner.stop.set()
                # Registered birth identities only. Cleanup owns joining/reaping.
                for process in list(self.owner.children.values()):
                    if process.poll() is None:
                        with contextlib.suppress(ProcessLookupError):
                            self.owner.signal_owned(process, signal.SIGTERM, leader_only=True)
                return

    def check(self):
        require(self.failure is None and not self.owner.stop.is_set())
        self.sample()

    def close(self):
        self.done.set()
        if self.thread.ident is not None:
            self.thread.join(timeout=3)
        require(not self.thread.is_alive())


class RestoreFixture(fixture_module.Fixture):
    def spawn(self, name, command, env=None):
        if name == 'postgres':
            # This is a backup/restart drill, not the browser fixture's fast DB.
            command = [{'fsync=off': 'fsync=on', 'full_page_writes=off': 'full_page_writes=on'}.get(arg, arg)
                       for arg in command]
        return super().spawn(name, command, env)


class Drill:
    def __init__(self, args, report):
        self.args, self.report = args, report
        (ROOT / 'test-runtime').mkdir(exist_ok=True)
        self.fixture = RestoreFixture(args)
        self.private = Path(tempfile.mkdtemp(prefix='weebdex-full-restore-'))
        self.private.chmod(0o700)
        marker = read_json(self.fixture.run / 'fixture-owned.json')
        marker['full_restore_qualification'] = True
        fixture_module.write_json(self.fixture.run / 'fixture-owned.json', marker)
        self.guard = ResourceGuard(self.fixture, self.private, args.seconds)
        self.password = secrets.token_urlsafe(32)  # Synthetic account only, memory only.
        self.token = None
        self.bundle = self.private / 'bundle'
        self.database = SOURCE_DB

    def stage(self, name):
        require(name in STAGES)
        self.report['stage'] = name
        self.guard.check()

    def sql(self, text, database=None):
        require((database or self.database) in (SOURCE_DB, TARGET_DB, SENTINEL_DB, 'postgres'))
        self.guard.check()
        return self.fixture.psql(text, database or self.database)

    def http(self, path, *, protocol=False, headers=None, payload=None, auth=False):
        self.guard.check()
        require(path.startswith('/') and not path.startswith('//') and len(path) <= 1024)
        headers = dict(headers or {})
        headers['Origin'] = self.fixture.origin
        if auth:
            headers['Cookie'] = 'weebdex_session=' + self.token
        if payload is not None:
            headers['Content-Type'] = 'application/json'
        # A direct numeric loopback connection cannot follow a redirect/proxy.
        connection = HTTPConnection('127.0.0.1', self.fixture.protocol_port if protocol else self.fixture.http_port, timeout=5)
        try:
            connection.request('POST' if payload is not None else 'GET', path,
                               body=json.dumps(payload).encode() if payload is not None else None, headers=headers)
            with connection.getresponse() as response:
                body = response.read(8 * 1024**2 + 1)
                require(len(body) <= 8 * 1024**2)
                return response.status, body, {key.lower(): value for key, value in response.headers.items()}
        finally:
            connection.close()

    def browser_flow(self, kind):
        require(kind in ('registration', 'login', 'recovery', 'verification'))
        status, body, headers = self.http('/auth/self-service/' + kind + '/browser')
        require(status == 200)
        flow = json.loads(body)
        require(re.fullmatch('[a-f0-9-]{36}', flow['id']))
        cookie = SimpleCookie()
        cookie.load(headers.get('set-cookie', ''))
        browser = cookie.get('weebdex_browser')
        require(browser is not None and re.fullmatch('[A-Za-z0-9_-]{43}', browser.value))
        return flow, 'weebdex_browser=' + browser.value

    def password_auth(self, kind):
        require(kind in ('registration', 'login'))
        flow, browser = self.browser_flow(kind)
        csrf = [n['attributes']['value'] for n in flow['ui']['nodes']
                if n['attributes'].get('name') == 'csrf_token']
        require(len(csrf) == 1)
        payload = {'method': 'password', 'csrf_token': csrf[0], 'password': self.password}
        if kind == 'registration':
            payload['traits'] = {'email': 'restore@example.invalid', 'username': 'restore-fixture-user'}
        else:
            payload['identifier'] = 'restore-fixture-user'
        status, body, headers = self.http('/auth/self-service/' + kind + '?flow=' + flow['id'],
                                         headers={'Cookie': browser}, payload=payload)
        require(status == 200)
        result = json.loads(body)
        require(result['session']['active'] is True)
        user = result['session']['identity']['id']
        require(re.fullmatch('[a-f0-9-]{36}', user))
        cookie = SimpleCookie()
        cookie.load(headers.get('set-cookie', ''))
        session = cookie.get('weebdex_session')
        require(session is not None and re.fullmatch('[A-Za-z0-9_-]{43}', session.value))
        self.token = session.value
        return user

    def seed_account(self):
        self.user_id = self.password_auth('registration')
        # Narrow fixture-only verification/grants; password/session issuance uses
        # real public browser flows, never a made-up session or password hash.
        self.sql("UPDATE platform.users SET verified=TRUE,permissions="
            "'[\"self.preferences.update\",\"self.library.read\"]'::jsonb WHERE id='" + self.user_id + "';")
        status, body, _ = self.http('/api/user/lists', auth=True,
            payload={'name': 'Synthetic private restore list', 'manga_ids': ['SeedManga1']})
        require(status == 201)
        value = json.loads(body)
        self.list_id = value['id']
        require(re.fullmatch('[A-Za-z0-9_-]{1,80}', self.list_id) and value['visibility'] == 'private')
        self.list_snapshot = value
        self.backup_token = self.token
        self.api_token = secrets.token_urlsafe(32)
        api_hash = hashlib.sha256(self.api_token.encode()).hexdigest()
        self.sql("INSERT INTO platform.api_clients(id,user_id,name,secret_hash,scopes) VALUES "
            f"('RestoreApi','{self.user_id}','Synthetic restore client',decode('{api_hash}','hex'),'[\"self.library.read\"]'::jsonb);")
        require(self.http('/api/list/' + self.list_id, headers={'Authorization': 'Bearer ' + self.api_token})[0] == 200)
        self.old_flows = []
        for kind in ('recovery', 'verification'):
            flow, browser = self.browser_flow(kind)
            token = secrets.token_urlsafe(32)
            digest = hashlib.sha256(token.encode()).hexdigest()
            recipient = hashlib.sha256(b'restore@example.invalid').hexdigest()
            self.sql("INSERT INTO platform.auth_email_tokens(id,kind,user_id,token_hash,recipient_hash,flow_id,return_to,delivery_state,expires_at) VALUES "
                f"('Restore-{kind}','{kind}','{self.user_id}',decode('{digest}','hex'),decode('{recipient}','hex'),'{flow['id']}','/','sent',clock_timestamp()+interval '15 minutes');")
            require(self.http('/auth/self-service/' + kind + '/flows?id=' + flow['id'], headers={'Cookie': browser})[0] == 200)
            self.old_flows.append((kind, flow['id'], browser, token))

    def revoke_source_credentials(self):
        # Model credentials live at the recovery point but revoked/consumed
        # later. This deliberate source mutation precedes source-stability pins.
        self.sql("BEGIN; UPDATE platform.sessions SET revoked_at=clock_timestamp() WHERE user_id='" + self.user_id + "';"
            "UPDATE platform.api_clients SET revoked_at=clock_timestamp() WHERE id='RestoreApi';"
            "UPDATE platform.auth_flows SET used_at=clock_timestamp() WHERE id IN (SELECT flow_id FROM platform.auth_email_tokens WHERE user_id='" + self.user_id + "');"
            "UPDATE platform.auth_email_tokens SET used_at=clock_timestamp() WHERE user_id='" + self.user_id + "'; COMMIT;", SOURCE_DB)
        self.source_snapshot = self.database_snapshot(SOURCE_DB)
        self.report['source_credentials_revoked_after_backup'] = True

    def database_snapshot(self, database, *, business=False):
        names = self.sql("SELECT n.nspname||'.'||c.relname FROM pg_class c JOIN pg_namespace n ON n.oid=c.relnamespace "
            "WHERE c.relkind='r' AND n.nspname IN ('public','platform') ORDER BY 1;", database).splitlines()
        require(100 <= len(names) <= 500 and all(re.fullmatch('(public|platform)\\.[a-z0-9_]+', n) for n in names))
        require(not business or set(checkpoint.RESTORE_FENCE_COLUMNS) <= set(names))
        statements = [f"SELECT '{name}',count(*),md5(COALESCE(string_agg(({row_projection(name, business)})::text,E'\\n' ORDER BY ({row_projection(name, business)})::text),'')) FROM {name} t;" for name in names]
        rows = self.sql('\n'.join(statements), database)
        require(len(rows) <= 128 * 1024 and len(rows.splitlines()) == len(names))
        sequences = self.sql("SELECT n.nspname||'.'||c.relname FROM pg_class c JOIN pg_namespace n ON n.oid=c.relnamespace "
            "WHERE c.relkind='S' AND n.nspname IN ('public','platform') ORDER BY 1;", database).splitlines()
        require(len(sequences) <= 500 and all(re.fullmatch('(public|platform)\\.[a-z0-9_]+', n) for n in sequences))
        sequence_rows = self.sql('\n'.join(f"SELECT '{name}',last_value,is_called FROM {name};" for name in sequences), database) if sequences else ''
        env = self.fixture.environment()
        env.update(PGHOST='127.0.0.1', PGPORT=str(self.fixture.pg_port), PGDATABASE=database)
        schema = self.fixture.run_owned([str(fixture_module.PG_BIN / 'pg_dump'), '--schema-only',
            '--no-owner', '--no-acl', '--no-password'], env=env, text=True, timeout=30)
        require(schema.returncode == 0 and len(schema.stdout) <= 16 * 1024**2)
        # Patched PostgreSQL emits a random psql safety nonce, not schema content.
        schemas = re.sub(r'(?m)^\\(?:un)?restrict [A-Za-z0-9]+\n', '', schema.stdout)
        return {'tables': len(names), 'rows_sha256': hashlib.sha256(rows.encode()).hexdigest(),
                'sequences_sha256': hashlib.sha256(sequence_rows.encode()).hexdigest(),
                'schema_sha256': hashlib.sha256(schemas.encode()).hexdigest()}

    def checkpoint_args(self, config, *, execute=False, database=SOURCE_DB):
        os.environ['FULL_RESTORE_SYNTHETIC_DSN'] = f'host=127.0.0.1 port={self.fixture.pg_port} dbname={database}'
        return argparse.Namespace(config=str(config), node_config=None,
            release_manifest=str(self.args.release_manifest), bundle=str(self.bundle),
            database_env='FULL_RESTORE_SYNTHETIC_DSN', execute=execute, quiesced=execute,
            timeout=min(90, max(1, int(self.guard.end - time.monotonic()))), max_files=100000,
            max_bytes=256 * 1024**2, min_free_bytes=RESERVE + 128 * 1024**2,
            confirm_empty_database=database)

    def pg_tool(self, name, argv, env, budget, capture=False):
        require(name in ('psql', 'pg_dump', 'pg_restore'))
        self.guard.check()
        budget.check(writing=True)
        result = self.fixture.run_owned([str(fixture_module.PG_BIN / name), *argv], env=env,
            timeout=max(.1, min(budget.end, self.guard.end) - time.monotonic()), text=True)
        require(result.returncode == 0 and len(result.stdout) <= 1024**2 and len(result.stderr) <= 1024**2)
        budget.check(writing=True)
        return result.stdout.strip() if capture else ''

    def handoff(self):
        self.config = read_json(self.fixture.run / 'platform.json')
        self.credential_state = retained_credentials(self.config, self.fixture.run)
        self.original_roots = {'native': self.fixture.run / 'uploads',
            'originals': self.fixture.run / 'original-storage', 'projection': self.fixture.run / 'original-backend'}
        self.listeners = [self.config['bind'], self.config['protocol_publication']['native_node']['bind']]
        checkpoint.stopped(self.original_roots, self.listeners)
        budget = checkpoint.Budget(60, 100000, 256 * 1024**2, RESERVE, self.private)
        before = checkpoint.scan(self.original_roots, budget)
        stage = self.private / 'stopped-source'
        stage.mkdir(mode=0o700)
        self.source_roots = {name: stage / name for name in self.original_roots}
        for root in self.source_roots.values():
            root.mkdir(mode=0o700)
        for row in before:
            target = self.source_roots[row['root']] / row['path']
            if row['kind'] == 'directory':
                target.mkdir(mode=0o700, exist_ok=True)
            else:
                checkpoint.copy_file(self.original_roots[row['root']] / row['path'], target, budget, row['bytes'])
        require(before == checkpoint.scan(self.original_roots, budget))
        require(stable_rows(before) == stable_rows(checkpoint.scan(self.source_roots, budget)))
        checkpoint.stopped(self.original_roots, self.listeners)
        self.source_rows = before
        self.source_config = self.private / 'source.json'
        checkpoint.write_json(self.source_config, relocated(self.config, self.source_roots))
        self.source_snapshot = self.database_snapshot(SOURCE_DB)
        self.source_business_snapshot = self.database_snapshot(SOURCE_DB, business=True)
        self.original_restore_epoch = self.sql('SELECT restore_generation,auth_not_before FROM public.node_identity WHERE singleton;', SOURCE_DB).split('|')
        require(len(self.original_restore_epoch) == 2 and all(v.isdigit() for v in self.original_restore_epoch))
        self.report['table_count'] = self.source_snapshot['tables']
        self.report['stopped_handoff_verified'] = True

    def target_config(self, label):
        parent = self.private / label
        parent.mkdir(mode=0o700)
        roots = {name: parent / name for name in self.source_roots}
        config = self.private / (label + '.json')
        checkpoint.write_json(config, paused_config(relocated(self.config, roots)))
        return config, roots

    def pending_state(self, database):
        event = self.pending_id
        require(re.fullmatch('[a-f0-9]{64}', event))
        value = self.sql("SELECT (SELECT count(*) FROM events WHERE id=decode('" + event + "','hex')),"
            "count(*),count(*) FILTER(WHERE completed_at IS NULL),"
            "(SELECT md5(raw_event::text) FROM events WHERE id=decode('" + event + "','hex')) "
            "FROM backend_outbox WHERE event_id=decode('" + event + "','hex') AND action='import';", database).split('|')
        require(len(value) == 4)
        return value

    def stop_checked(self):
        child = self.fixture.children['platform']
        self.fixture.signal_owned(child, signal.SIGTERM, leader_only=True)
        deadline = min(self.guard.end, time.monotonic() + 10)
        withdrawn = False
        while time.monotonic() < deadline:
            try:
                if self.http('/readyz')[0] != 200:
                    withdrawn = True
                    break
            except OSError:
                withdrawn = True
                break
            time.sleep(.05)
        require(withdrawn)
        self.fixture.stop_listeners()
        self.report['readiness_withdrawals'] = self.report.get('readiness_withdrawals', 0) + 1

    def check_retained_credentials(self):
        require(retained_credentials(read_json(self.restore_config), self.fixture.run)
                == self.credential_state)
        self.report['retained_credentials_match'] = True

    def verify_fence(self):
        generation, epoch = map(int, self.original_restore_epoch)
        require(self.sql("SELECT "
            "NOT EXISTS(SELECT 1 FROM platform.sessions WHERE revoked_at IS NULL) AND "
            "NOT EXISTS(SELECT 1 FROM platform.api_clients WHERE revoked_at IS NULL) AND "
            "NOT EXISTS(SELECT 1 FROM platform.auth_flows WHERE used_at IS NULL) AND "
            "NOT EXISTS(SELECT 1 FROM platform.auth_email_tokens WHERE used_at IS NULL) AND "
            "NOT EXISTS(SELECT 1 FROM platform.account_disable_confirmations WHERE used_at IS NULL) AND "
            "NOT EXISTS(SELECT 1 FROM platform.scheduled_uploads WHERE state NOT IN ('complete','cancelled') "
            "AND (state<>'blocked' OR lease_token IS NOT NULL OR lease_until IS NOT NULL OR failure_code IS DISTINCT FROM 'paused_after_restore')) AND "
            "NOT EXISTS(SELECT 1 FROM public.sync_schedules WHERE enabled OR last_error IS DISTINCT FROM 'paused_after_restore') AND "
            "NOT EXISTS(SELECT 1 FROM public.public_export_roles WHERE enabled) AND "
            "NOT EXISTS(SELECT 1 FROM public.inventory_snapshot_slots WHERE session_id IS NOT NULL OR peer IS NOT NULL "
            "OR peer_epoch IS NOT NULL OR prefix<>'' OR filter<>'' OR snapshot<>''::bytea OR expires_at<>'-infinity'::timestamptz OR rounds<>0 OR protocol_bytes<>0) AND "
            f"(SELECT count(*) FROM public.node_identity WHERE singleton AND restore_generation={generation + 1} "
            f"AND auth_not_before>{epoch} AND restored_at IS NOT NULL)=1;", TARGET_DB) == 't')
        checkpoint.require_paused_roles(read_json(self.restore_config))
        self.report['restored_authentication_fenced'] = True
        self.report['autonomous_roles_paused'] = True

    def restored_authentication(self):
        require(self.http('/api/auth/check', headers={'Cookie': 'weebdex_session=' + self.backup_token})[0] == 401)
        require(self.http('/api/list/' + self.list_id, headers={'Authorization': 'Bearer ' + self.api_token})[0] == 404)
        for kind, flow, browser, token in self.old_flows:
            require(self.http('/auth/self-service/' + kind + '/flows?id=' + flow, headers={'Cookie': browser})[0] == 410)
            require(self.http('/auth/self-service/' + kind + '?token=' + token)[0] == 400)
        self.report['old_cookie_api_email_and_flows_rejected'] = True
        require(self.password_auth('login') == self.user_id and self.token != self.backup_token)
        require(self.http('/api/auth/check', auth=True)[0] == 200)
        self.report['fresh_login_verified'] = True

    def start_restored(self):
        require(inventory(self.args) == read_json(self.args.pins))
        self.check_retained_credentials()
        # Only matching verify/check-config/serve. Never migrate a restored database.
        self.database = TARGET_DB
        self.fixture.platform_env = self.fixture.environment()
        self.fixture.platform_env.update(PLATFORM_CONFIG=str(self.restore_config),
            PLATFORM_DATABASE_URL=f'host=127.0.0.1 port={self.fixture.pg_port} dbname={TARGET_DB}')
        for action in ('check-config', 'verify'):
            result = self.fixture.run_owned([str(self.args.binary), action], env=self.fixture.platform_env, timeout=30)
            require(result.returncode == 0)
        self.fixture.verify_frontend(self.fixture.ui_dir)
        self.fixture.generation += 1
        self.fixture.spawn('platform', [str(self.args.binary), 'serve'], self.fixture.platform_env)
        deadline = min(self.guard.end, time.monotonic() + 60)
        while time.monotonic() < deadline:
            require(self.fixture.children['platform'].poll() is None)
            try:
                if self.http('/readyz')[0] == 200 and self.pending_state(TARGET_DB)[:3] == ['1', '1', '0']:
                    return
            except OSError:
                pass
            time.sleep(.2)
        raise Refused('Restored readiness unconfirmed')

    def media_check(self, path, expected, protocol=False, range_required=True):
        status, body, headers = self.http(path, protocol=protocol)
        require(status == 200 and hashlib.sha256(body).hexdigest() == expected)
        if range_required:
            require(len(body) >= 6 and headers.get('etag'))
            ranged, part, h = self.http(path, protocol=protocol, headers={'Range': 'bytes=2-5'})
            require(ranged == 206 and part == body[2:6] and h.get('content-range') == f'bytes 2-5/{len(body)}')
            require(self.http(path, protocol=protocol, headers={'If-None-Match': headers['etag']})[0] == 304)

    def capture_baseline(self):
        self.seed = self.fixture.receipt['protocol_seed']
        require(self.seed.get('fully_ready') is True)
        self.event_hashes = {}
        self.media = {}
        for item in [self.seed['series'], *self.seed['chapters']]:
            event_id = item['event_id']
            status, body, _ = self.http('/events/' + event_id, protocol=True)
            require(status == 200 and json.loads(body)['id'] == event_id)
            self.event_hashes[event_id] = hashlib.sha256(body).hexdigest()
        status, body, _ = self.http('/data/SeedChapt1/SeedPg1001.png')
        require(status == 200)
        self.media['/data/SeedChapt1/SeedPg1001.png'] = hashlib.sha256(body).hexdigest()
        # Importer-owned native aliases must resolve the actual original bytes.
        for chapter in self.seed['chapters']:
            status, body, _ = self.http('/api/chapter/' + chapter['native_id'])
            require(status == 200)
            value = json.loads(body)
            for page in value['data']:
                path = '/data/' + chapter['native_id'] + '/' + page['name']
                status, body, _ = self.http(path)
                require(status == 200)
                self.media[path] = hashlib.sha256(body).hexdigest()
        require(2 <= len(self.media) <= 10)
        seed_input = read_json(self.fixture.run / 'protocol-seed-input.json')
        self.original_hashes = [sha(Path(seed_input['cover']))] + [sha(Path(p)) for c in seed_input['chapters'] for p in c['pages']]
        require(set(self.media.values()) >= set(self.original_hashes[1:]))

    def restored_checks(self):
        require(self.http('/readyz')[0] == 200)
        self.restored_authentication()
        status, body, _ = self.http('/api/protocol/capabilities')
        capabilities = json.loads(body)
        require(status == 200 and capabilities['available'] is True
                and capabilities['catalog_namespace'] == self.seed['catalog_namespace']
                and capabilities['service_key'] == self.fixture.receipt['service_key'])
        for event, digest in self.event_hashes.items():
            status, body, _ = self.http('/events/' + event, protocol=True)
            require(status == 200 and hashlib.sha256(body).hexdigest() == digest)
        for path, digest in self.media.items():
            self.media_check(path, digest)
        self.report['native_reader_media_hash_range_etag_verified'] = True
        for digest in self.original_hashes:
            self.media_check('/originals/' + digest, digest, protocol=True, range_required=False)
        self.report['original_endpoint_media_hashes_verified'] = True
        status, body, _ = self.http('/api/list/' + self.list_id, auth=True)
        require(status == 200 and json.loads(body) == self.list_snapshot)
        require(self.http('/api/list/' + self.list_id)[0] == 404)
        require(self.http('/api/auth/check', auth=True)[0] == 200)
        current = self.pending_state(TARGET_DB)
        require(current[:3] == ['1', '1', '0'] and current[3] == self.pending_before[3])
        status, body, _ = self.http('/events/' + self.pending_id, protocol=True)
        require(status == 200 and json.loads(body)['id'] == self.pending_id)
        self.report['original_outbox_reconciled'] = True

    def run(self):
        self.guard.sample()
        self.guard.thread.start()
        self.stage('baseline')
        self.report['runtime_executed'] = True
        self.fixture.start()
        self.capture_baseline()
        self.stage('account')
        self.seed_account()
        self.stage('quiesce')
        # Close/join the only external fixture writer surface before draining.
        self.fixture.sink.shutdown()
        self.fixture.sink.server_close()
        self.fixture.sink_thread.join(timeout=5)
        require(not self.fixture.sink_thread.is_alive())
        self.fixture.sink = None
        with self.fixture.protocol_lock:
            pass
        self.stop_checked()
        self.stage('pending-outbox')
        pending = self.fixture.provision_protocol('restore-pending-note', {}, timeout=20)
        require(pending.get('pending_original_outbox_items') == 1
                and pending.get('native_upload_intent_covered') is False)
        self.pending_id = pending['event_id']
        self.pending_before = self.pending_state(SOURCE_DB)
        require(self.pending_before[:3] == ['1', '1', '1'])
        self.stage('handoff')
        self.handoff()
        self.stage('backup-plan')
        require(checkpoint.backup(self.checkpoint_args(self.source_config), self.pg_tool)['dryRun'] is True)
        require(not self.bundle.exists())
        self.stage('backup')
        require(checkpoint.backup(self.checkpoint_args(self.source_config, execute=True), self.pg_tool)['complete'] is True)
        self.stage('source-revocation')
        self.revoke_source_credentials()
        self.stage('verify-bundle')
        manifest = checkpoint.verify(self.bundle, checkpoint.Budget(60, 100000, 256 * 1024**2))
        require(manifest['releaseManifestSha256'] == sha(self.args.release_manifest))
        self.restore_config, self.restore_roots = self.target_config('restored')
        refusal_config, refusal_roots = self.target_config('occupied-refusal')
        self.sql(f'CREATE DATABASE {TARGET_DB} TEMPLATE template0; CREATE DATABASE {SENTINEL_DB} TEMPLATE template0;', 'postgres')
        self.sql("CREATE TABLE sentinel(value text); INSERT INTO sentinel VALUES('synthetic-untouched');", SENTINEL_DB)
        self.stage('restore-plan')
        require(checkpoint.restore(self.checkpoint_args(self.restore_config, database=TARGET_DB), self.pg_tool)['dryRun'] is True)
        require(all(not p.exists() for p in self.restore_roots.values()))
        self.stage('nonempty-refusal')
        try:
            checkpoint.restore(self.checkpoint_args(refusal_config, execute=True, database=SENTINEL_DB), self.pg_tool)
        except checkpoint.Refused as error:
            require(str(error) == 'Target name mismatch or nonempty application database')
        else:
            raise Refused('Occupied database was not refused')
        require(all(not p.exists() for p in refusal_roots.values())
                and self.sql('SELECT value FROM sentinel;', SENTINEL_DB) == 'synthetic-untouched')
        self.report['nonempty_sentinel_unchanged'] = True
        self.stage('restore')
        require(checkpoint.restore(self.checkpoint_args(self.restore_config, execute=True, database=TARGET_DB), self.pg_tool)['complete'] is True)
        self.stage('compare')
        restored = self.database_snapshot(TARGET_DB, business=True)
        require(restored == self.source_business_snapshot)
        self.verify_fence()
        self.report['all_schemas_equal'] = self.report['all_business_rows_equal'] = self.report['all_sequences_equal'] = True
        require(stable_rows(checkpoint.scan(self.restore_roots, checkpoint.Budget(30))) == manifest['entries'])
        self.report['three_roots_equal'] = True
        for label in ('restored', 'restart'):
            if label == 'restart':
                self.stage('restart-stop')
                self.stop_checked()
            self.stage(label + '-start')
            self.start_restored()
            self.stage(label + '-checks')
            self.restored_checks()
            self.report[label + '_checks'] = True
        self.stage('source-unchanged')
        self.check_retained_credentials()
        require(self.database_snapshot(SOURCE_DB) == self.source_snapshot)
        require(self.pending_state(SOURCE_DB) == self.pending_before)
        require(checkpoint.scan(self.original_roots, checkpoint.Budget(30)) == self.source_rows)
        require(stable_rows(checkpoint.scan(self.source_roots, checkpoint.Budget(30))) == manifest['entries'])
        require(inventory(self.args) == read_json(self.args.pins))
        self.report['source_unchanged'] = True


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('binary', 'protocol-provisioner', 'ui-dir', 'release-manifest', 'frontend-artifact-manifest'):
        parser.add_argument('--' + name, type=Path, required=True)
    parser.add_argument('--pins', type=Path)
    parser.add_argument('--pins-sha256')
    parser.add_argument('--inventory', action='store_true', help='Print read-only candidate pins; review and retain separately')
    parser.add_argument('--execute-synthetic', action='store_true')
    parser.add_argument('--seconds', type=int, default=600)
    parser.add_argument('--output', type=Path)
    args = parser.parse_args()
    report = {'format': 'synthetic-full-restore-result-v1', 'stage': 'preflight',
        'runtime_executed': False, 'browser_executed': False, 'production_dr_qualified': False,
        'native_pending_upload_qualified': False, 'error_class': None, 'budget_failure': None, **SCOPE}
    drill = None
    # No inherited provider/application secrets reach child processes or libpq.
    safe = {key: value for key, value in os.environ.items() if key in
            ('PATH', 'HOME', 'TMPDIR', 'TMP', 'TEMP', 'LANG', 'LC_ALL', 'LC_CTYPE',
             'LD_LIBRARY_PATH', 'SSL_CERT_FILE', 'SSL_CERT_DIR')}
    os.environ.clear()
    os.environ.update(safe)
    os.umask(0o077)
    try:
        if args.inventory:
            require(not args.execute_synthetic)
            print(json.dumps(inventory(args), sort_keys=True, indent=2))
            return 0
        require(args.pins is not None and args.output is not None)
        pins = preflight(args)
        report['pins_sha256'] = args.pins_sha256
        report['release_sha256'] = pins['release']['sha256']
        commit = subprocess.run(['git', 'rev-parse', 'HEAD'], cwd=REPO, capture_output=True, text=True, timeout=5)
        require(commit.returncode == 0 and re.fullmatch('[0-9a-f]{40}\n', commit.stdout))
        report['source_commit'] = commit.stdout.strip()
        if not args.execute_synthetic:
            print(json.dumps({'status': 'plan-only', 'runtime_executed': False,
                              'pins_sha256': args.pins_sha256, 'seconds': args.seconds, **SCOPE}))
            return 0
        require(checkpoint.shutil.disk_usage(tempfile.gettempdir()).free >= RESERVE + LIMIT_BYTES + 256 * 1024**2)
        with open(os.devnull, 'w') as quiet, contextlib.redirect_stdout(quiet):
            drill = Drill(args, report)
            for signum in (signal.SIGINT, signal.SIGTERM):
                signal.signal(signum, lambda *_: drill.fixture.stop.set())
            drill.run()
    except BaseException as error:
        report['error_class'] = type(error).__name__ if type(error).__name__ in (
            'Refused', 'OSError', 'RuntimeError', 'ValueError', 'KeyError', 'TypeError',
            'FileNotFoundError', 'PermissionError', 'TimeoutExpired', 'KeyboardInterrupt') else 'operation-failed'
    finally:
        if drill is not None:
            try:
                drill.guard.close()
                report['budget_failure'] = drill.guard.failure
                report['resource_peaks'] = drill.guard.peak
                with open(os.devnull, 'w') as quiet, contextlib.redirect_stdout(quiet):
                    cleanup = drill.fixture.cleanup(RuntimeError('qualification-failed') if report['error_class'] or report['budget_failure'] else None)
                report['synthetic_server_keys_absent'] = cleanup.get('synthetic_server_keys_absent') is True
                report['cleanup_clean'] = (cleanup.get('status') == 'verified_stopped'
                    and cleanup.get('lifecycle_clean') is True and cleanup.get('final_teardown_clean') is True
                    and cleanup.get('synthetic_server_keys_absent') is True)
                if report['error_class'] is None and report['budget_failure'] is None and report['cleanup_clean']:
                    report['stage'] = 'complete'
            except BaseException:
                report['cleanup_clean'] = False
                report['error_class'] = report['error_class'] or 'cleanup-failed'
                with contextlib.suppress(BaseException), open(os.devnull, 'w') as quiet, contextlib.redirect_stdout(quiet):
                    drill.fixture.cleanup(RuntimeError('cleanup-failed'))
    passed = clean_success(report)
    report['status'] = 'passed-synthetic-data-restore-with-retained-credentials' if passed else 'failed-or-blocked-synthetic-data-restore-with-retained-credentials'
    # Only fixed, bounded labels/counts/hashes above may enter public evidence.
    encoded = json.dumps(report, sort_keys=True, indent=2) + '\n'
    require(len(encoded) < 16384)
    if args.output and report.get('runtime_executed'):
        require(not args.output.exists())
        args.output.mkdir(mode=0o700)
        (args.output / 'receipt.json').write_text(encoded)
    print(encoded, end='')
    return 0 if passed else 1


if __name__ == '__main__':
    sys.exit(main())
