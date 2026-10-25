#!/usr/bin/env python3
"""Plan by default. Actual bounded app probe requires explicit source-bound inputs.

No production target, dependency install, build, external credentials or browser
interception. This source-prepared adapter has not been runtime-qualified.
"""
import argparse
import hashlib
import importlib.util
import json
import math
import os
from pathlib import Path
import re
import signal
import subprocess
import sys
import time
from http.client import HTTPConnection
from types import SimpleNamespace
from urllib.parse import urlsplit

ROOT = Path(__file__).resolve().parents[2]
REPO = ROOT.parent
GIB = 1024 ** 3
FIELDS = {'binary', 'protocol_provisioner', 'ui_dir', 'release_manifest',
          'frontend_artifact_manifest', 'pins', 'pins_sha256', 'source_commit',
          'build_association', 'build_association_sha256', 'functional_gate', 'functional_gate_sha256'}
TOTAL_SECONDS = 600


class Refused(RuntimeError):
    pass


def require(value, code='admission'):
    if not value:
        raise Refused(code)


def module(name, file):
    spec = importlib.util.spec_from_file_location(name, file)
    value = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(value)
    return value


def digest(file):
    sha = hashlib.sha256()
    with file.open('rb') as source:
        for chunk in iter(lambda: source.read(65536), b''):
            sha.update(chunk)
    return sha.hexdigest()


def read_json(file):
    require(file.is_file() and not file.is_symlink() and file.stat().st_size <= 4 * 1024 ** 2, 'json-bounds')
    return json.loads(file.read_text())


def plan():
    return {'schema': 'real-application-capacity-plan-v1', 'status': 'prepared-not-executed',
        'runtime_qualified': False, 'capacity_qualified': False,
        'required_runtime': ['actual immutable platform/frontend release', 'owned PostgreSQL with verified fsync/full_page_writes/synchronous_commit on',
            'actual embedded native publisher/strfry and generated signed fixture', 'native sandboxed Chromium through exact-origin streaming proxy'],
        'total_seconds': TOTAL_SECONDS, 'proposed_sections_seconds': {'setup': 150, 'native_browser_including_warmup_and_drain': 180, 'inner_cleanup': 245, 'outer_cleanup_reserve': 25},
        'limits': {'launch_free_bytes': 4 * GIB + 256 * 1024 ** 2, 'live_free_bytes': 4 * GIB + 128 * 1024 ** 2,
            'owned_rss_bytes': GIB, 'cgroup_headroom_bytes': 512 * 1024 ** 2, 'host_memory_floor_bytes': 768 * 1024 ** 2,
            'setup_http_attempts': 300, 'browser_http_attempts': 2200, 'setup_response_bytes': 64 * 1024 ** 2,
            'browser_response_bytes': 448 * 1024 ** 2, 'setup_upload_bytes': 1024 ** 2, 'browser_upload_bytes': 1024 ** 2,
            'browser_contexts': 4, 'inflight_http': 16, 'queued_http': 32, 'evidence_growth_bytes': 256 * 1024 ** 2},
        'workloads': ['real reader mount and actual page-selector change; native decoded images; transaction-complete chapter puts and read-only final row verification',
            'actual search form and library page navigation; explicit sensitivity mix, not a monthly-demand model'],
        'separate_unimplemented': ['optional signed-publication load', '30K/500K library browser publication/recovery', 'cold/warm representative large corpus', 'server pool-acquisition timings'],
        'limitations': ['Existing small generated corpus is reused, not the proposed32-title/256-chapter corpus.',
            'Four isolated browser devices, not24 users; nominal phases10/20/30/20s at HTTP ceilings2/12/30/60 per second.',
            'Proxy credits bound forwarded encoded payload, not NIC/parser buffers or HTTP framing.',
            'Setup supervisor HTTP has a separate measured bucket; internal provisioner/database/native-worker activity is server setup, not page traffic.',
            'Exact method/URL reconciliation requires an actual proxy attempt or terminal CDP cache-only observation with zero encoded wire length.',
            'No default progress HTTP lane; observed local transactions are not server saves, physical durability proof or resolved higher-level save calls.',
            'All source-only tests remain finite machinery tests; native runtime execution is still held.']}


def config_for(path, output, expected_digest=None):
    require(path.is_file() and not path.is_symlink() and path.stat().st_size <= 65536, 'config-bounds')
    with path.open('rb') as source:
        encoded = source.read(65537)
    require(len(encoded) <= 65536, 'config-bounds')
    actual_digest = hashlib.sha256(encoded).hexdigest()
    require(expected_digest is None or expected_digest == actual_digest, 'config-snapshot-digest')
    value = json.loads(encoded)
    require(type(value) is dict and set(value) == FIELDS, 'config-fields')
    for field in ('pins_sha256', 'build_association_sha256', 'functional_gate_sha256'):
        require(isinstance(value[field], str) and re.fullmatch('[0-9a-f]{64}', value[field]), 'config-hash')
    require(isinstance(value['source_commit'], str) and re.fullmatch('[0-9a-f]{40}', value['source_commit']), 'commit')
    for field in FIELDS - {'pins_sha256', 'source_commit', 'build_association_sha256', 'functional_gate_sha256'}:
        require(isinstance(value[field], str) and Path(value[field]).is_absolute(), 'absolute-path')
        value[field] = Path(value[field])
    value.update(output=output, seconds=TOTAL_SECONDS, admission_config_sha256=actual_digest)
    return SimpleNamespace(**value)


def snapshot_config(args):
    value = {key: str(getattr(args, key)) for key in sorted(FIELDS)}
    encoded = (json.dumps(value, sort_keys=True, separators=(',', ':')) + '\n').encode()
    target = args.output / 'admitted-config.json'
    fd = os.open(target, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    with os.fdopen(fd, 'wb') as stream:
        stream.write(encoded)
    args.admission_config_sha256 = hashlib.sha256(encoded).hexdigest()
    return target


def admission_identity(value):
    keys = ('source_commit', 'pins_sha256', 'build_association_sha256', 'functional_gate_sha256', 'admission_config_sha256')
    return {key: value.get(key) if type(value) is dict else getattr(value, key) for key in keys}


def git(*args):
    return subprocess.check_output(['git', *args], cwd=REPO, text=True, timeout=10).strip()


def preflight(args, *, owned_output=False):
    require(git('rev-parse', 'HEAD') == args.source_commit and not git('status', '--porcelain', '--untracked-files=no'), 'source-freeze')
    restore = module('capacity_release_validation', ROOT / 'scripts/ci/qualify-full-application-restore.py')
    # The outer owner creates the evidence directory. The reused validator
    # checks a new child admission path, not a pretend nonexistent output root.
    checked = SimpleNamespace(**vars(args))
    if owned_output:
        require(args.output.is_dir() and not args.output.is_symlink() and args.output.stat().st_mode & 0o077 == 0, 'owned-output')
        checked.output = args.output / 'reserved-admission'
    pins = restore.preflight(checked)
    original_output = args.output
    vars(args).update(vars(checked))
    args.output = original_output
    args.signed_feature_controls = False
    require(digest(args.build_association) == args.build_association_sha256, 'association-hash')
    association = read_json(args.build_association)
    # This is a separately reviewed operator build association, not an inferred
    # relationship between whichever executable and whichever source are present.
    source_digest = hashlib.sha256(json.dumps(pins['source'], sort_keys=True, separators=(',', ':')).encode()).hexdigest()
    require(association == {'format': 'reviewed-capacity-build-association-v1', 'source_commit': args.source_commit,
        'source_inventory_sha256': source_digest, 'release_manifest_sha256': digest(args.release_manifest),
        'platform_sha256': pins['tools']['platform']['sha256'], 'provisioner_sha256': pins['tools']['provisioner']['sha256']}, 'build-source-association')
    require(digest(args.functional_gate) == args.functional_gate_sha256, 'functional-gate-hash')
    gate = read_json(args.functional_gate)
    require(type(gate) is dict and set(gate) == {'format', 'source_commit', 'requirements', 'evidence'} and
        gate['format'] == 'reviewed-capacity-functional-gate-v1' and gate['source_commit'] == args.source_commit and
        gate['requirements'] == {'upload_originals_replacement_recovery': True, 'signed_chapter_authority': True,
                                'catalogue_account_library': True, 'native_reader_browser': True} and
        type(gate['evidence']) is list and 1 <= len(gate['evidence']) <= 32, 'functional-gate')
    for row in gate['evidence']:
        require(type(row) is dict and set(row) == {'path', 'sha256'} and isinstance(row['path'], str) and isinstance(row['sha256'], str), 'functional-evidence')
        file = Path(row['path'])
        require(file.is_absolute() and file.resolve() == file and file.is_relative_to(ROOT / 'test-evidence') and file.is_file() and not file.is_symlink() and digest(file) == row['sha256'], 'functional-evidence')
    return restore, pins


class SetupHTTP:
    """Single-thread setup transport, no redirect/retry and streaming payload cap."""
    def __init__(self, origins, seconds=150, connect=HTTPConnection):
        self.origins = set(origins); self.connect = connect; self.deadline = time.monotonic() + seconds
        self.records = []; self.bytes = 0; self.upload = 0; self.active = 0

    def check(self):
        require(time.monotonic() < self.deadline, 'setup-deadline')

    def open(self, request, timeout=3):
        self.check()
        url = request if isinstance(request, str) else request.full_url
        value = urlsplit(url); origin = f'{value.scheme}://{value.netloc}'
        require(origin in self.origins and value.scheme == 'http' and value.hostname == '127.0.0.1' and not value.username and not value.password and not value.fragment, 'setup-origin')
        require(len(self.records) < 300 and self.active == 0, 'setup-attempts')
        body = b'' if isinstance(request, str) else request.data or b''
        method = 'GET' if isinstance(request, str) else request.get_method()
        require(method in ('GET', 'POST') and isinstance(body, bytes) and len(body) <= 65536 and self.upload + len(body) <= 1024 ** 2, 'setup-upload')
        headers = {} if isinstance(request, str) else dict(request.header_items())
        require(not any(key.lower() in {'host', 'content-length', 'transfer-encoding', 'connection', 'proxy-authorization'} for key in headers), 'setup-headers')
        self.upload += len(body)
        row = {'status': None, 'method': method, 'response_bytes': 0, 'upload_bytes': len(body), 'complete': False, 'terminal': False, 'outcome': 'started'}
        self.records.append(row)
        conn = self.connect(value.hostname, value.port, timeout=min(float(timeout), 3, max(.001, self.deadline - time.monotonic())))
        self.active += 1
        try:
            conn.request(method, value.path + ('?' + value.query if value.query else ''), body=body, headers=headers)
            response = conn.getresponse(); row['status'] = response.status
            require(not 300 <= response.status < 400, 'setup-redirect')
            return SetupResponse(self, conn, response, row)
        except BaseException:
            row['terminal'] = True; row['outcome'] = 'connection-or-header-failure'
            conn.close(); self.active -= 1; raise

    def snapshot(self):
        return {'attempts': len(self.records), 'response_payload_consumed': self.bytes, 'upload_payload_reserved': self.upload, 'active': self.active,
                'records': self.records, 'all_settled': all(row['terminal'] for row in self.records), 'wire_and_os_buffers_bounded': False}


class SetupResponse:
    def __init__(self, owner, connection, response, row):
        self.owner, self.connection, self.response, self.row = owner, connection, response, row
        self.status, self.headers, self.closed = response.status, response.headers, False
        def fields(name):
            return self.headers.get_all(name, []) if hasattr(self.headers, 'get_all') else [value for key, value in self.headers.items() if key.lower() == name.lower()]
        values, transfer = fields('Content-Length'), fields('Transfer-Encoding')
        require(len(values) <= 1 and (not values or re.fullmatch('0|[1-9][0-9]*', values[0])), 'setup-length')
        # Match HTTPResponse's actual framing decision, not a broader OWS-normalized value.
        require(not transfer or len(transfer) == 1 and transfer[0].lower() == 'chunked', 'setup-transfer-encoding')
        require(not values or not transfer, 'setup-framing')
        self.declared = int(values[0]) if values else None
        self.received = 0

    def read(self, size=-1):
        require(type(size) is int and size >= -1, 'setup-read')
        chunks = []; total = 0
        while size == -1 or total < size:
            self.owner.check(); available = 64 * 1024 ** 2 - self.owner.bytes
            require(available > 0, 'setup-response-limit')
            data = self.response.read(min(65536, available, size - total if size != -1 else 65536))
            self.owner.bytes += len(data); self.row['response_bytes'] += len(data); chunks.append(data); total += len(data)
            self.received += len(data)
            require(self.declared is None or self.received <= self.declared, 'setup-length-overrun')
            if not data or self.response.isclosed():
                require(self.declared is None or self.received == self.declared, 'setup-length-incomplete')
                self.row['complete'] = True; break
        return b''.join(chunks)

    def __enter__(self):
        return self

    def __exit__(self, *_):
        try:
            if not self.row['complete']:
                # Never silently omit unread setup payload from accounting.
                self.read()
        finally:
            self.row['terminal'] = True
            self.row['outcome'] = 'complete' if self.row['complete'] else 'body-failure'
            self.connection.close()
            if not self.closed:
                self.owner.active -= 1; self.closed = True


def server_sample(fixture):
    value = fixture.psql("""SET statement_timeout='1500ms'; SELECT json_build_object(
      'connections',(SELECT count(*) FROM pg_stat_activity WHERE datname=current_database() AND pid<>pg_backend_pid()),
      'active',(SELECT count(*) FROM pg_stat_activity WHERE datname=current_database() AND state='active' AND pid<>pg_backend_pid()),
      'waiting',(SELECT count(*) FROM pg_stat_activity WHERE datname=current_database() AND wait_event_type IS NOT NULL AND pid<>pg_backend_pid()),
      'source_outbox_pending',(SELECT count(*) FROM public.backend_outbox WHERE completed_at IS NULL),
      'source_outbox_oldest_seconds',(SELECT COALESCE(EXTRACT(EPOCH FROM clock_timestamp()-min(created_at)),0) FROM public.backend_outbox WHERE completed_at IS NULL),
      'platform_commands_pending',(SELECT count(*) FROM platform.protocol_publication_commands WHERE state<>'materialized'))::text;""")
    row = json.loads(value)
    require(set(row) == {'connections', 'active', 'waiting', 'source_outbox_pending', 'source_outbox_oldest_seconds', 'platform_commands_pending'} and all(type(value) in (int, float) and math.isfinite(value) and value >= 0 for value in row.values()), 'server-sample')
    return row


def durable_pg_command(command):
    """Change only the two source-pinned disposable-fixture defaults before spawn."""
    command = list(command)
    for old, new in [('fsync=off', 'fsync=on'), ('full_page_writes=off', 'full_page_writes=on')]:
        require(command.count(old) == 1 and command.index(old) > 0 and command[command.index(old) - 1] == '-c', 'postgres-source-settings')
        command[command.index(old)] = new
    require(not any(str(item).startswith('synchronous_commit=') for item in command), 'postgres-source-settings')
    return command + ['-c', 'synchronous_commit=on']


def worker(args):
    restore, pins = preflight(args, owned_output=True)
    fixture_module = restore.fixture_module
    class CapacityFixture(fixture_module.Fixture):
        def spawn(self, name, command, env=None):
            if name == 'postgres':
                command = durable_pg_command(command)
            return super().spawn(name, command, env)

        def start_pg(self):
            super().start_pg()
            # Actual settings are on from initial postmaster launch, not toggled
            # after seeding or claimed from configuration text alone.
            observed = self.psql('SHOW fsync; SHOW full_page_writes; SHOW synchronous_commit;', database='postgres')
            require(observed.splitlines() == ['on', 'on', 'on'], 'postgres-durability')
            self.capacity_durable_settings = True

    fixture = None; cleanup = None; observed = None; native = None; error = None; setup = None
    started = time.monotonic(); samples = []
    previous_alarm = signal.signal(signal.SIGALRM, lambda *_: (_ for _ in ()).throw(Refused('setup-deadline')))
    try:
        signal.setitimer(signal.ITIMER_REAL, 150)
        fixture = CapacityFixture(args)
        setup = SetupHTTP([fixture.origin, fixture.protocol_origin, fixture.mail_origin])
        fixture_module.OPENER = setup
        fixture.start(); setup.check()
        require(fixture.capacity_durable_settings, 'postgres-durability')
        seed = fixture.receipt['protocol_seed']
        require(seed.get('fully_ready') is True and len(seed['chapters']) >= 1, 'signed-corpus')
        from urllib.request import Request
        query = {'catalog_namespace': seed['catalog_namespace'], 'resources': [{'kind': 'manga', 'id': seed['series']['native_id']}, {'kind': 'chapter', 'id': seed['chapters'][0]['native_id']}]}
        with setup.open(Request(fixture.origin + '/api/v1/legacy/resolve', data=json.dumps(query).encode(), headers={'Content-Type': 'application/json'}, method='POST')) as response:
            require(response.status == 200, 'actual-uuid-resolution'); resolved = json.loads(response.read(65536))
        require(resolved.get('unresolved') == [] and len(resolved['data']) == 2, 'actual-uuid-resolution')
        identifiers = {row['kind']: row['id'] for row in resolved['data']}
        require(set(identifiers) == {'chapter', 'manga'} and all(re.fullmatch('[0-9a-f-]{36}', value) for value in identifiers.values()), 'actual-uuid-resolution')
        native_report = fixture.run / 'capacity-native.json'
        input_file = fixture.run / 'capacity-input.json'
        input_file.write_text(json.dumps(dict(identifiers, origin=fixture.origin, report=str(native_report))))
        input_file.chmod(0o600)
        signal.setitimer(signal.ITIMER_REAL, 0)
        native = fixture.spawn('browser-capacity', ['node', str(ROOT / 'scripts/integrated-load/native-browser.mjs'), str(input_file)])
        native_deadline = time.monotonic() + 180
        while native.poll() is None:
            require(time.monotonic() < native_deadline and time.monotonic() - started < 330, 'native-deadline')
            require(len(samples) < 90, 'server-sample-limit')
            samples.append(dict(elapsed_seconds=time.monotonic() - started, **server_sample(fixture)))
            time.sleep(2)
        require(native.returncode == 0, 'native-child')
        observed = read_json(native_report)
        require(observed.get('status') == 'passed-bounded-native-probe-only' and observed.get('capacity_qualified') is False, 'native-result')
        samples.append(dict(elapsed_seconds=time.monotonic() - started, **server_sample(fixture)))
        require(samples[-1]['source_outbox_pending'] == 0 and samples[-1]['platform_commands_pending'] == 0, 'pending-server-work')
        require(restore.inventory(args) == pins, 'source-or-release-changed')
    except BaseException as exc:
        error = exc
    finally:
        signal.setitimer(signal.ITIMER_REAL, 0)
        signal.signal(signal.SIGALRM, previous_alarm)
        if fixture is not None:
            try:
                cleanup = fixture.cleanup(error)
            except BaseException as exc:
                error = error or exc
        report = {'schema': 'real-app-capacity-worker-v1', 'status': 'failed', 'capacity_qualified': False,
            'source_commit': args.source_commit, 'pins_sha256': args.pins_sha256, 'build_association_sha256': args.build_association_sha256,
            'functional_gate_sha256': args.functional_gate_sha256,
            'admission_config_sha256': args.admission_config_sha256,
            'error': error.args[0] if isinstance(error, Refused) else type(error).__name__ if error else None,
            'setup_http': setup.snapshot() if setup else None, 'native': observed,
            'server_samples': samples, 'server_metrics_unavailable': ['pool-acquisition latency', 'per-handler server latency'],
            'durable_pg_verified': bool(fixture and getattr(fixture, 'capacity_durable_settings', False)),
            'cleanup': {'verified_stopped': bool(cleanup and cleanup.get('status') == 'verified_stopped'),
                'unforced': bool(cleanup and cleanup.get('final_teardown_clean') and cleanup.get('lifecycle_clean'))}}
        if error is None and observed and report['setup_http']['all_settled'] and report['setup_http']['active'] == 0 and report['cleanup']['verified_stopped'] and report['cleanup']['unforced']:
            report['status'] = 'passed-bounded-real-application-probe-only'
        (args.output / 'worker-summary.json').write_text(json.dumps(report, indent=2) + '\n')
    return report


def cgroup_headroom(root=Path('/sys/fs/cgroup')):
    maximum, current = (root / 'memory.max').read_text().strip(), (root / 'memory.current').read_text().strip()
    require(maximum.isdigit() and current.isdigit() and int(maximum) >= int(current), 'finite-cgroup-required')
    return int(maximum) - int(current)


def process_birth(pid):
    return int(Path('/proc/' + str(pid) + '/stat').read_text().rsplit(')', 1)[1].split()[19])


def require_outer_owner():
    identity = os.environ.get('WEEBDEX_CAPACITY_OWNER', '').split(':')
    require(len(identity) == 2 and all(value.isdigit() for value in identity), 'outer-owner-required')
    require(int(identity[0]) == os.getppid() and process_birth(os.getppid()) == int(identity[1]), 'outer-owner-birth')


def resource_summary(samples):
    require(type(samples) is list and samples, 'missing-resource-samples')
    ticks = {}
    for sample in samples:
        for row in sample['owned']:
            if row.get('cpu_ticks') is not None:
                key = (row['pid'], row['birth'])
                low, high = ticks.get(key, (row['cpu_ticks'], row['cpu_ticks']))
                ticks[key] = min(low, row['cpu_ticks']), max(high, row['cpu_ticks'])
    return {'sample_count': len(samples), 'minimum_free_bytes': min(row['freeBytes'] for row in samples),
        'minimum_host_available_bytes': min(row['availableMemoryBytes'] for row in samples),
        'minimum_cgroup_headroom_bytes': min(row['cgroup_headroom_bytes'] for row in samples),
        'peak_owned_rss_bytes': max(sum(value['rssBytes'] for value in row['owned']) for row in samples),
        'peak_owned_processes': max(len(row['owned']) for row in samples),
        'evidence_growth_bytes': max(row['evidenceBytes'] for row in samples) - samples[0]['evidenceBytes'],
        'observed_cpu_ticks': sum(high - low for low, high in ticks.values()),
        'clock_ticks_per_second': os.sysconf('SC_CLK_TCK'), 'cpu_measurement': 'sampled_birth_matched_delta_not_full_lifetime'}


def execute(args, config_path):
    restore, pins = preflight(args)
    require(cgroup_headroom() >= 512 * 1024 ** 2, 'cgroup-headroom')
    args.output.mkdir(mode=0o700)
    config_path = snapshot_config(args)
    expected_identity = admission_identity(args)
    owner_module = module('capacity_owned_lifecycle', ROOT / 'tests/ci/qualification_process_owner.py')
    original = owner_module.QualificationOwner
    class CapacityOwner(original):
        def resource_sample(self, rows):
            result = super().resource_sample(rows)
            result['cgroup_headroom_bytes'] = cgroup_headroom()
            require(result['cgroup_headroom_bytes'] >= 512 * 1024 ** 2, 'cgroup-headroom')
            for row in result['owned']:
                try:
                    fields = Path('/proc/' + str(row['pid']) + '/stat').read_text().rsplit(')', 1)[1].split()
                    require(int(fields[19]) == row['birth'], 'cpu-birth')
                    row['cpu_ticks'] = int(fields[11]) + int(fields[12])
                except FileNotFoundError:
                    row['cpu_ticks'] = None
            return result
    owner_module.QualificationOwner = CapacityOwner
    owner_module.TREE_RSS_LIMIT = GIB  # Stronger than the existing4GiB owner ceiling.
    safe = {key: value for key, value in os.environ.items() if key in ('PATH', 'HOME', 'LANG', 'LC_ALL', 'LC_CTYPE', 'PLATFORM_TEST_PG_BIN')}
    safe.update(PYTHONDONTWRITEBYTECODE='1', TZ='UTC', WEEBDEX_CAPACITY_OWNER=str(os.getpid()) + ':' + str(process_birth(os.getpid())), WEEBDEX_CAPACITY_CONFIG_SHA256=args.admission_config_sha256)
    owner = owner_module.run([sys.executable, str(Path(__file__).resolve()), '--worker', '--config', str(config_path), '--output', str(args.output)], REPO, safe, args.output, 'real-app-capacity', deadline=TOTAL_SECONDS, evidence_roots=[ROOT / 'test-runtime'])
    result = read_json(args.output / 'worker-summary.json')
    unchanged = restore.inventory(args) == pins and git('rev-parse', 'HEAD') == args.source_commit and not git('status', '--porcelain', '--untracked-files=no')
    identity_matches = admission_identity(result) == expected_identity
    passed = owner['status'] == 'passed' and result['status'] == 'passed-bounded-real-application-probe-only' and unchanged and identity_matches
    # Export the fixed-schema summary, not supervisor.json, runtime DBs/keys or logs.
    result['outer_cleanup'] = {key: owner['cleanup'][key] for key in ('verified_stopped', 'forced', 'stdout_eof')}
    result['resources'] = resource_summary(owner['samples'])
    result['source_and_release_unchanged_after_cleanup'] = unchanged
    result['worker_admission_matches_owner'] = identity_matches
    result['status'] = 'passed-bounded-real-application-probe-only' if passed else 'failed-bounded-real-application-probe'
    (args.output / 'summary.json').write_text(json.dumps(result, indent=2) + '\n')
    return passed


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--execute-synthetic', action='store_true')
    parser.add_argument('--worker', action='store_true', help=argparse.SUPPRESS)
    parser.add_argument('--config', type=Path)
    parser.add_argument('--output', type=Path)
    args = parser.parse_args()
    if not args.execute_synthetic and not args.worker:
        print(json.dumps(plan(), indent=2)); return 0
    require(args.config is not None and args.output is not None, 'explicit-inputs')
    expected = os.environ.get('WEEBDEX_CAPACITY_CONFIG_SHA256') if args.worker else None
    if args.worker:
        require(expected is not None and re.fullmatch('[0-9a-f]{64}', expected), 'config-snapshot-digest')
    value = config_for(args.config.resolve(), args.output.resolve(), expected)
    if args.worker:
        require_outer_owner()
        # Outer owner creates this directory before dispatch; reuse only its exact
        # empty result target. The release validator still needs a new output path.
        require(value.output.is_dir() and not (value.output / 'worker-summary.json').exists(), 'owned-output')
        return 0 if worker(value)['status'] == 'passed-bounded-real-application-probe-only' else 1
    return 0 if execute(value, args.config.resolve()) else 1


if __name__ == '__main__':
    try:
        raise SystemExit(main())
    except Refused as error:
        print('Capacity preparation refused: ' + str(error), file=sys.stderr); raise SystemExit(1)
