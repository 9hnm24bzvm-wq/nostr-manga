"""Finite mounted-client qualification. No traffic on import or --plan.

Only creates its own synthetic ephemeral loopback server. There is intentionally
no URL option: running against an application is a separate, unadmitted gate.
"""
import argparse
import asyncio
import base64
from collections import Counter
from contextlib import asynccontextmanager
import hashlib
import importlib.util
import json
import math
import os
from pathlib import Path
import sys
import subprocess
import time
import tempfile

from aiohttp import web
from integrated_load_budget import BudgetedHTTP, SharedBudget, BudgetRefused, TransportDeadline

def request_failure_reason(error):
    if isinstance(error, BudgetRefused):
        return error.reason
    if isinstance(error, TransportDeadline):
        return 'deadline' if error.shared_deadline else 'TransportFailure'
    return type(error).__name__


def mounted_failure_reason(error):
    return 'deadline' if isinstance(error, TimeoutError) else 'mounted-run-failed'


ROOT = Path(__file__).resolve().parents[1]
UUID = lambda value: '00000000-0000-8000-8000-' + str(value).zfill(12)
# Owned 1x1 PNG, deliberately tiny/hot. Not a production image distribution.
PNG = base64.b64decode('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=')
DEFAULT = dict(actions=[dict(chapter=100, preload=0, page_turns=[1, 2], dwell_ms=1150)],
               chapters=3, pages=3, selector_pages=0, rate=20, duration_s=15, max_http_attempts=100,
               max_response_body_bytes=4 * 1024**2, max_inflight=4)


def validate_config(value):
    if type(value) is not dict or set(value) - set(DEFAULT):
        raise ValueError('Only explicit supported synthetic scenario fields are allowed')
    c = {**DEFAULT, **value}
    for key, low, high in [('chapters', 1, 101), ('pages', 1, 8), ('selector_pages', 0, 3), ('rate', 1, 60),
                           ('duration_s', 1, 30), ('max_http_attempts', 1, 250),
                           ('max_response_body_bytes', 1, 8 * 1024**2), ('max_inflight', 1, 8)]:
        if type(c[key]) is not int or not low <= c[key] <= high:
            raise ValueError('Synthetic scenario bound exceeded: ' + key)
    if type(c['actions']) is not list or not 1 <= len(c['actions']) <= 8:
        raise ValueError('One to eight explicit serial mounted actions required')
    actions = []
    for row in c['actions']:
        if type(row) is not dict or set(row) != {'chapter', 'preload', 'page_turns', 'dwell_ms'}:
            raise ValueError('Explicit chapter/preload/page_turns/dwell_ms required')
        for key, low, high in [('chapter', 100, 99 + c['chapters']), ('preload', 0, 8), ('dwell_ms', 0, 2000)]:
            if type(row[key]) is not int or not low <= row[key] <= high:
                raise ValueError('Invalid mounted action ' + key)
        if type(row['page_turns']) is not list or len(row['page_turns']) > 16 or any(type(p) is not int or not 0 <= p < c['pages'] for p in row['page_turns']):
            raise ValueError('Page turns must be actual zero-based fixture positions')
        actions.append(dict(row, page_turns=list(row['page_turns'])))
    return dict(c, actions=actions)


def plan(config):
    return dict(schema='integrated-mounted-synthetic-v1', config=config,
                scope='one serial device; actual mounted reader subcomponents and catalogue adapters; owned synthetic HTTP only',
                expected_http_per_action=None, browser_measured=False, capacity_qualified=False,
                public_sync=dict(enabled=False, consent_fraction=None, cadence=None),
                unmeasured=['native browser, disk IndexedDB, offline/tab/crash behavior', 'whole route shell/static/cover/comment fan-out',
                            'actual application/PostgreSQL/native worker performance', '100TB cold corpus, cache distribution and target monthly capacity'])


def bounded_selector(chapters, query):
    """Owned current-reader fixture, not the historical eager catalogue adapter.

    Envelope/row semantics match bounded-mounted-fixture.py; this scenario only
    admits the actual around and newest-groups requests it exercises. Cursors
    are opaque witnesses of the exact scope and last row, never bare offsets.
    """
    value = lambda key: query.get(key, [None])[0]
    operation = value('operation')
    required = {'operation', 'limit', 'current', 'translatedLanguage[]'}
    allowed = required | ({'direction', 'after'} if operation == 'groups' else set())
    if (operation not in {'around', 'groups'} or not required <= set(query)
            or set(query) - allowed or any(len(values) != 1 for values in query.values())
            or value('limit') != '50' or value('translatedLanguage[]') != 'en'
            or operation == 'groups' and value('direction') != 'newest'):
        raise ValueError('Unadmitted bounded selector query')
    rows = [dict(id=chapter['id'], volume='1', chapter=chapter['attributes']['chapter'], language='en',
                 publishedAt='2026-10-19 00:00:00+00', version=1, groupIds=[], sharedGroupScore=0,
                 witness=hashlib.sha256(('owned-selector-row:' + chapter['id']).encode()).hexdigest())
            for chapter in chapters]
    current = next((row for row in rows if row['id'] == value('current')), None)
    if current is None:
        raise ValueError('Unknown bounded current chapter')
    scope = hashlib.sha256(json.dumps([UUID(1), {key: values for key, values in sorted(query.items()) if key != 'after'}],
                                     separators=(',', ':')).encode()).hexdigest()
    result = dict(version=1, mangaId=UUID(1), operation=operation, orderingProfile='public-retained-v1',
                  consistency='live-keyset-v1', crossRequestSnapshot=False, history='device-local-only',
                  continueProof='observed-only', scope=scope,
                  catalogIdentity=dict(catalogNamespace=UUID(9), serviceKey=None), state='ready',
                  restartRequired=False, limit=50, current=current, nextCursor=None)
    if operation == 'around':
        index = rows.index(current)
        result.update(data=[current], previous=rows[index - 1] if index else None,
                      next=rows[index + 1] if index + 1 < len(rows) else None)
    else:
        ordered = rows[::-1]
        cursor = lambda row: 'g_' + hashlib.sha256(json.dumps([scope, row['id'], row['witness']], separators=(',', ':')).encode()).hexdigest()
        start = 0
        if value('after') is not None:
            anchors = {cursor(row): index + 1 for index, row in enumerate(ordered)
                       if (index + 1) % 50 == 0 and index + 1 < len(ordered)}
            if value('after') not in anchors:
                raise ValueError('Unknown or differently scoped bounded cursor')
            start = anchors[value('after')]
        data = ordered[start:start + 50]
        result.update(data=data, nextCursor=cursor(data[-1]) if start + len(data) < len(ordered) else None,
                      state='ready' if data else 'boundary')
    return result


@asynccontextmanager
async def synthetic_fixture(config, fault=None):
    """Independent request-side observations; private exact request details not exported."""
    rows, sockets = [], set()
    manga = dict(id=UUID(1), type='manga', attributes=dict(title={'en': 'Owned synthetic title'},
        originalLanguage='ja', status='ongoing', contentRating='safe', tags=[]), relationships=[])
    chapters = [dict(id=UUID(i + 100), type='chapter', attributes=dict(title=None, volume='1', chapter=str(i + 1),
        translatedLanguage='en', pages=config['pages'], publishAt='2026-10-19T00:00:00Z'),
        relationships=[dict(id=UUID(1), type='manga')]) for i in range(config['chapters'])]
    origin = None
    async def handle(request):
        body = await request.read()
        row = dict(method=request.method, path=request.path, query=list(request.query.items()),
                   request_body_bytes=len(body), status=None, response_body_bytes=0)
        rows.append(row)
        if request.transport:
            sockets.add(request.transport)
        def response(data=None, status=200, raw=None, mime='application/json'):
            payload = raw if raw is not None else json.dumps(data, separators=(',', ':')).encode()
            row.update(status=status, response_body_bytes=len(payload))
            return web.Response(status=status, body=payload, content_type=mime)
        if fault:
            overridden = await fault(request, row, response)
            if overridden is not None:
                return overridden
        path = request.path
        expected_method = 'POST' if '/analytics/' in path or path.endswith('/native-forum/contexts') else 'GET'
        if request.method != expected_method:
            return response(dict(error='unexpected fixture method'), 405)
        if path == '/api/protocol/capabilities':
            return response(dict(catalog_namespace=UUID(9), service_key=None))
        if path == '/api/auth/check':
            return response(dict(error='anonymous'), 401)
        if path == '/api/v1/manga':
            if set(request.query.getall('ids[]', [])) != {UUID(1)}:
                return response(dict(error='unexpected parent selection'), 422)
            return response(dict(result='ok', response='collection', data=[manga], limit=100, offset=0, total=1))
        if path == '/api/v1/manga/' + UUID(1) + '/reader':
            query = {key: request.query.getall(key) for key in request.query}
            try:
                selected = bounded_selector(chapters, query)
            except ValueError:
                return response(dict(error='unadmitted bounded selector'), 422)
            row['selector'] = dict(operation=selected['operation'], limit=selected['limit'],
                returned=len(selected['data']), scope=selected['scope'], after=query.get('after', [None])[0],
                next_cursor=selected['nextCursor'])
            return response(selected)
        # Historical eager adapter fixture only; the current mounted provider
        # must not fetch this complete catalogue or its aggregate endpoint.
        if path == '/api/v1/chapter':
            if request.query.get('manga') != UUID(1) or request.query.get('limit') != '100':
                return response(dict(error='unexpected selector'), 422)
            offset = int(request.query.get('offset', '0'))
            return response(dict(result='ok', response='collection', data=chapters[offset:offset + 100],
                                 limit=100, offset=offset, total=len(chapters)))
        if path == '/api/v1/manga/' + UUID(1) + '/aggregate':
            return response(dict(result='ok', volumes={}))
        if path == '/api/v1/nostr/native-forum/contexts' and request.method == 'POST':
            resources = json.loads(body)['resources']
            if any(r['kind'] != 'chapter' or r['id'] not in {c['id'] for c in chapters} for r in resources):
                return response(dict(error='fixture scope escape'), 422)
            return response(dict(result='ok', data=[dict(r, thread=None) for r in resources], unavailable=[]))
        for chapter in chapters:
            if path == '/api/v1/chapter/' + chapter['id']:
                return response(dict(result='ok', response='entity', data=chapter))
            if path == '/api/v1/at-home/server/' + chapter['id']:
                return response(dict(result='ok', baseUrl=origin, chapter=dict(hash='synthetic',
                    data=[f'{i}.png' for i in range(config['pages'])], dataSaver=[]),
                    nostr=dict(originalsOnly=True, pageDimensions=[[1, 1]] * config['pages'])))
            if path == '/api/v1/analytics/chapter/' + chapter['id'] + '/views' and request.method == 'POST':
                return response(status=204, raw=b'')
        if path in {'/data/synthetic/' + str(i) + '.png' for i in range(config['pages'])}:
            return response(raw=PNG, mime='image/png')
        return response(dict(error='unexpected fixture route'), 404)
    app = web.Application(client_max_size=65536)
    app.router.add_route('*', '/{tail:.*}', handle)
    runner = web.AppRunner(app, access_log=None, shutdown_timeout=.2)
    await runner.setup()
    site = web.TCPSite(runner, '127.0.0.1', 0)
    await site.start()
    origin = 'http://127.0.0.1:' + str(site._server.sockets[0].getsockname()[1])
    try:
        yield origin, rows
    finally:
        await runner.cleanup()
        for transport in sockets:
            transport.close()


def classify(url):
    from urllib.parse import urlsplit
    path = urlsplit(url).path
    if path in {'/api/protocol/capabilities', '/api/auth/check'}:
        return 'setup'
    if path.startswith('/api/v1/analytics/'):
        return 'analytics'
    if path.startswith('/data/'):
        return 'media'
    if path == '/api/v1/manga' or path.startswith('/api/v1/nostr/native-forum/'):
        return 'catalogue'
    return 'reader'


def check_runtime():
    directory = os.environ.get('WEEBDEX_LOAD_NODE_MODULES')
    if not directory or not Path(directory).is_absolute():
        raise ValueError('WEEBDEX_LOAD_NODE_MODULES must name an existing pinned dependency root; no install is attempted')
    pins = {'svelte': '5.55.1', 'jsdom': '27.0.1', 'fake-indexeddb': '6.2.4', 'typescript': '5.9.3'}
    for name, version in pins.items():
        try:
            actual = json.loads((Path(directory) / name / 'package.json').read_text())['version']
        except (OSError, ValueError, KeyError):
            raise ValueError('Pinned synthetic runtime prerequisite missing: ' + name) from None
        if actual != version:
            raise ValueError('Pinned synthetic runtime version mismatch: ' + name)


EXPORT_HELPER = 'tests/ci/qualification_source_export.py'
EXPORT_HELPER_SHA256 = '20a48521da92d67f589afb8879e402e3950969ac6f77cc1700933f6434aae587'
EXPORT_ENV = ('WEEBDEX_FINITE_EXPORT_MANIFEST', 'WEEBDEX_FINITE_EXPORT_MANIFEST_SHA256')


def export_source(driver_hashes):
    """Optional explicit context, never an automatic fallback for missing Git."""
    manifest, expected = (os.environ.get(key) for key in EXPORT_ENV)
    if manifest is None and expected is None:
        return None
    if not manifest or not expected or not Path(manifest).is_absolute():
        raise ValueError('Both explicit export manifest and independent SHA-256 are required')
    helper = ROOT / EXPORT_HELPER
    if helper.is_symlink() or hashlib.sha256(helper.read_bytes()).hexdigest() != EXPORT_HELPER_SHA256:
        raise ValueError('Reviewed export provenance helper differs')
    spec = importlib.util.spec_from_file_location('mounted_export_source', helper)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    source = module.ExportSource(ROOT.parent, manifest, expected)
    verify_export_drivers(source, driver_hashes)
    return source


def verify_export_drivers(source, driver_hashes):
    names = ['weebdex-platform/' + name for name in driver_hashes]
    helper = 'weebdex-platform/' + EXPORT_HELPER
    verified = source.verify_files([*names, helper])
    if verified[helper] != EXPORT_HELPER_SHA256 or any(
            verified['weebdex-platform/' + name] != digest for name, digest in driver_hashes.items()):
        raise ValueError('Mounted driver identity differs from the reviewed export')
    return verified


def source_identity(driver_hashes, exported):
    if exported is None:
        # Default behavior still requires actual current Git metadata.
        return {'source_commit': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip()}
    verified = verify_export_drivers(exported, driver_hashes)
    return {'source_provenance': {
        'kind': 'externally-pinned-export-consumer-subset',
        'manifest_sha256': exported.expected_sha256,
        'manifest_scope': exported.manifest['scope'],
        'declared_origin_commit': exported.manifest['origin']['commit'],
        'declared_origin_repository_tree': exported.manifest['origin']['tree'],
        'origin_commit_ancestry_verified': False,
        'manifest_selected_object_identities_verified': True,
        'consumer_source_files_verified': verified,
        'complete_export_bytes_verified_by_consumer': False,
        'outer_full_before_after_inventory_required': True,
    }}


async def run(config, *, fault=None):
    check_runtime()
    config = validate_config(config)
    result = plan(config)
    driver_files = ['scripts/integrated_load_scenarios.py', 'scripts/integrated_load_budget.py', 'scripts/integrated-load/mounted-reader.mjs']
    driver_hashes = {name: hashlib.sha256((ROOT / name).read_bytes()).hexdigest() for name in driver_files}
    exported = export_source(driver_hashes)  # Refuse invalid context before any fixture or child.
    budget = SharedBudget(max_http_attempts=config['max_http_attempts'],
        max_response_body_bytes=config['max_response_body_bytes'], max_upload_body_bytes=65536,
        duration_s=config['duration_s'], max_inflight=config['max_inflight'])
    tasks, requests, failures, child = set(), {}, [], None
    child_result = None
    serial = asyncio.Lock()
    last_dispatch = 0
    seen = set()
    server_errors = 0
    closed = False
    with tempfile.TemporaryDirectory(prefix='weebdex-mounted-load-') as generation_dir:
        async with synthetic_fixture(config, fault) as (origin, observed):
            async with BudgetedHTTP(budget, [origin], request_timeout_s=min(config['duration_s'], 10)) as transport:
                async def send(message):
                    if child and child.returncode is None and not closed:
                        child.stdin.write((json.dumps(message, separators=(',', ':')) + '\n').encode())
                        await child.stdin.drain()
                async def request(message):
                    nonlocal last_dispatch, server_errors
                    ident = message['id']
                    try:
                        if type(ident) is not int or ident in seen or len(seen) >= 512:
                            raise ValueError('Invalid IPC request identity')
                        seen.add(ident)
                        if set(message) != {'type', 'id', 'url', 'method', 'body'} or message['method'] not in {'GET', 'POST'}:
                            raise ValueError('Unsupported IPC request')
                        payload = base64.b64decode(message['body'], validate=True)
                        if len(payload) > 65536:
                            raise ValueError('IPC request body bound')
                        # Pacing and admission are global, including setup and all retries.
                        async with serial:
                            await asyncio.sleep(max(0, last_dispatch + 1 / config['rate'] - time.monotonic()))
                            last_dispatch = time.monotonic()
                        response = await transport.request(classify(message['url']), message['method'], message['url'], body=payload,
                            headers={'Content-Type': 'application/json'} if payload else {}, capture_limit=1024**2)
                        if response['status'] not in {200, 204}:
                            failures.append('unexpected-http-' + str(response['status']))
                        if response['status'] >= 500:
                            server_errors += 1
                            if server_errors >= 2:
                                raise RuntimeError('Two unexplained server errors')
                        await send(dict(type='response', id=ident, status=response['status'], headers=response['headers'],
                                        body=base64.b64encode(response['body']).decode()))
                    except asyncio.CancelledError:
                        raise
                    except Exception as error:
                        reason = request_failure_reason(error)
                        failures.append(reason)
                        await send(dict(type='response', id=ident, error=reason))
                        # Shared stop: no client retry after admission/transport failure.
                        raise RuntimeError('Shared mounted transport stopped') from None
                script = ROOT / 'scripts/integrated-load/mounted-reader.mjs'
                env = dict(PATH=os.environ.get('PATH', '/usr/bin:/bin'), WEEBDEX_LOAD_NODE_MODULES=os.environ['WEEBDEX_LOAD_NODE_MODULES'], NODE_USE_ENV_PROXY='0')
                child = await asyncio.create_subprocess_exec('node', '--experimental-vm-modules', '--conditions=browser', str(script),
                    stdin=asyncio.subprocess.PIPE, stdout=asyncio.subprocess.PIPE, stderr=asyncio.subprocess.PIPE,
                    env=env, limit=2 * 1024**2)
                # Stderr is drained but not exported: dependency paths/raw exceptions remain private.
                async def drain_stderr():
                    count = 0
                    while chunk := await child.stderr.read(4096):
                        count += len(chunk)
                        if count > 65536:
                            raise RuntimeError('Child diagnostic limit')
                try:
                    async with asyncio.timeout_at(budget.deadline):
                        await send(dict(type='init', origin=origin, config=config, generation_dir=generation_dir))
                        async with asyncio.TaskGroup() as group:
                            group.create_task(drain_stderr())
                            while True:
                                line = await child.stdout.readline()
                                if not line:
                                    break
                                if len(line) > 1024**2:
                                    raise ValueError('IPC line limit')
                                message = json.loads(line)
                                if message.get('type') == 'request':
                                    if len(tasks) >= 32:
                                        raise ValueError('IPC pending request bound')
                                    task = group.create_task(request(message)); tasks.add(task); requests[message['id']] = task; task.add_done_callback(tasks.discard)
                                elif message.get('type') == 'cancel' and type(message.get('id')) is int:
                                    task = requests.get(message['id'])
                                    if task:
                                        task.cancel()
                                elif message.get('type') == 'fatal':
                                    failures.append('child-integrity-or-authority-stop')
                                    raise RuntimeError('Mounted client failed closed')
                                elif message.get('type') == 'result' and child_result is None:
                                    child_result = message['result']
                                else:
                                    raise ValueError('Unexpected mounted IPC message')
                            await child.wait()
                except (Exception, BaseExceptionGroup) as error:
                    failures.append(mounted_failure_reason(error))
                finally:
                    closed = True
                    for task in tasks:
                        task.cancel()
                    await asyncio.gather(*tasks, return_exceptions=True)
                    if child.returncode is None:
                        child.terminate()
                        try:
                            await asyncio.wait_for(child.wait(), 1)
                        except TimeoutError:
                            child.kill(); await child.wait()
                    if child.stdin:
                        child.stdin.close()
            result.update(transport=budget.snapshot(), child_exit=child.returncode,
                          child=child_result, failures=failures,
                          fixture=dict(http_requests=len(observed), request_body_bytes=sum(r['request_body_bytes'] for r in observed),
                            response_body_bytes_prepared=sum(r['response_body_bytes'] for r in observed),
                            methods=dict(Counter(r['method'] for r in observed)), prepared_statuses=dict(Counter(str(r['status']) for r in observed)),
                            paths=dict(Counter(r['path'] for r in observed)),
                            selectors=[r['selector'] for r in observed if 'selector' in r]))
    result.update(source_identity(driver_hashes, exported))
    result['driver_sha256'] = driver_hashes
    result['driver_unchanged'] = all(hashlib.sha256((ROOT / name).read_bytes()).hexdigest() == digest for name, digest in driver_hashes.items())
    if not result['driver_unchanged']:
        failures.append('driver-source-changed')
    if not child_result or child_result.get('passed') is not True:
        failures.append('child-semantic-failure')
    if child.returncode != 0:
        failures.append('child-exit-nonzero')
    elapsed = time.monotonic() - budget.start
    result['elapsed_s'] = elapsed
    result['observed_http_attempts_per_second'] = len(budget.records) / elapsed
    result['timings'] = {}
    for lane in sorted({r['lane'] for r in budget.records}):
        result['timings'][lane] = {}
        for metric in ['response_headers_ms', 'first_body_ms', 'complete_body_ms']:
            values = sorted(r[metric] for r in budget.records if r['lane'] == lane and r[metric] is not None)
            summary = dict(observations=len(values))
            for key, quantile in [('p50', .5), ('p95', .95), ('p99', .99)]:
                summary[key] = values[max(0, math.ceil(len(values) * quantile) - 1)] if values else None
            result['timings'][lane][metric] = summary
    result['cleanup'] = dict(child_reaped=child.returncode is not None, active_http=budget.inflight,
                             read_credit=budget.read_reserved, listener_closed=True, generated_directory_removed=not Path(generation_dir).exists())
    result['passed'] = bool(not failures and child.returncode == 0 and child_result and child_result.get('passed') is True
        and budget.inflight == 0 and budget.read_reserved == 0 and len(observed) == len(budget.records)
        and child_result.get('http_attempts') == len(budget.records))
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument('--synthetic', action='store_true', help='Run finite owned fixtures, never an application target')
    mode.add_argument('--plan', action='store_true', help='Print the default no-traffic plan explicitly')
    parser.add_argument('--config', type=Path)
    args = parser.parse_args()
    config = validate_config(json.loads(args.config.read_text()) if args.config else {})
    result = asyncio.run(run(config)) if args.synthetic else plan(config)
    print(json.dumps(result, indent=2))
    if args.synthetic and not result['passed']:
        raise SystemExit(1)


if __name__ == '__main__':
    main()
