"""Eight-case pipe-only fixture; importing this file does not launch anything.

Use only under the separately reviewed finite process owner. No socket, HTTP
client, dependency install, source copy, or aggregate reader substitution.
"""
import argparse
import asyncio
import base64
import hashlib
import json
import os
from pathlib import Path
import shutil
import stat
import sys
import tempfile
import time
from urllib.parse import parse_qs, urlsplit

CASES = ('complete progress', 'partial remount progress', 'paged group disclosure',
         'edition paging and independent winner', 'single-edition title Start',
         'multiple-edition title Start chooser', 'observed device Continue', 'identity mismatch reload status')
UUID = lambda value: '00000000-0000-8000-8000-' + str(value).zfill(12)
ORIGIN = 'http://127.0.0.1:1'  # Synthetic URL label, never listened on or contacted.
PNG = base64.b64decode('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=')
CONFIG = dict(bounded_controls=True, actions=[dict(chapter=100, preload=0, page_turns=[1, 2], dwell_ms=1150),
                                             dict(chapter=101, preload=0, page_turns=[1], dwell_ms=0)])
pack = lambda value: json.dumps(value, separators=(',', ':'), ensure_ascii=True).encode()
sha = lambda value: hashlib.sha256(value).hexdigest()


def dependency_inventory(root, dom):
    """Same physical row format as the reviewed local-page v5 inventory."""
    queue = [root / 'svelte', root / 'typescript', dom / 'jsdom', dom / 'fake-indexeddb']
    packages, absent = {}, []
    while queue:
        package = queue.pop(0).resolve()
        if str(package) in packages:
            continue
        if not (package.is_relative_to(root) or package.is_relative_to(dom)):
            raise ValueError('Dependency resolution escaped the pinned root')
        metadata = json.loads((package / 'package.json').read_bytes())
        rows, total = [], 0
        for parent, dirs, files in os.walk(package, followlinks=False):
            dirs[:] = sorted(name for name in dirs if name != 'node_modules')
            for name in dirs + sorted(files):
                item = Path(parent) / name
                info = item.lstat()
                row = dict(path=str(item.relative_to(package)), mode=stat.S_IMODE(info.st_mode))
                if stat.S_ISLNK(info.st_mode):
                    raise ValueError('Unexpected dependency symlink')
                if item.is_dir():
                    row['type'] = 'directory'
                else:
                    if not stat.S_ISREG(info.st_mode):
                        raise ValueError('Unexpected dependency input kind')
                    row.update(type='file', bytes=info.st_size, sha256=sha(item.read_bytes()))
                    total += info.st_size
                rows.append(row)
        packages[str(package)] = dict(name=metadata['name'], version=metadata['version'], entries=len(rows), bytes=total,
                                     sha256=sha(json.dumps(rows, sort_keys=True, separators=(',', ':')).encode()))
        for kind in ('dependencies', 'optionalDependencies', 'peerDependencies'):
            for name in sorted(metadata.get(kind, {})):
                current, located = package, None
                while True:
                    candidate = current / 'node_modules' / name
                    if (candidate / 'package.json').is_file():
                        located = candidate.resolve()
                        break
                    if current == current.parent:
                        break
                    current = current.parent
                if located:
                    queue.append(located)
                elif kind == 'optionalDependencies' or kind == 'peerDependencies' and metadata.get('peerDependenciesMeta', {}).get(name, {}).get('optional') is True:
                    absent.append(dict(**{'from': str(package)}, kind=kind, name=name))
                else:
                    raise ValueError('Missing declared dependency')
    return dict(packages=dict(sorted(packages.items())), absentOptional=sorted(absent, key=lambda row: json.dumps(row, sort_keys=True, separators=(',', ':'))),
                locks={str(file): sha(file.read_bytes()) for file in [root.parent / 'package.json', root.parent / 'yarn.lock', dom.parent / 'package.json', dom.parent / 'package-lock.json', dom / '.package-lock.json']})


class Fixture:
    def __init__(self):
        self.requests = self.response_bytes = self.cancellations = 0
        self.identity_changed = False
        self.observed = []
        self.started = time.monotonic()
        self.rows = {UUID(i): dict(id=UUID(i), volume='1', chapter=str(i - 99 if i != 103 else 1), language='en',
                                  publishedAt='2026-10-19 00:00:00+00', version=1,
                                  groupIds=[UUID(502 if i == 103 else 501)], sharedGroupScore=0,
                                  witness=sha(('fixture-row-' + str(i)).encode())) for i in range(100, 104)}

    def selector(self, query):
        allowed = {'operation', 'limit', 'direction', 'current', 'groupChapter', 'after', 'translatedLanguage[]', 'groups[]'}
        if set(query) - allowed or any(len(values) != 1 for values in query.values()):
            raise ValueError('Unadmitted selector query')
        if query.get('translatedLanguage[]', ['en']) != ['en'] or query.get('groups[]', [UUID(501)]) != [UUID(501)]:
            raise ValueError('Unexpected exact selector filter')
        value = lambda key, default=None: query.get(key, [default])[0]
        operation, limit = value('operation'), int(value('limit'))
        if operation not in {'groups', 'editions', 'first', 'last', 'around', 'next', 'previous', 'resolve-group'} or not 1 <= limit <= 100:
            raise ValueError('Unadmitted selector operation or page size')
        current = self.rows.get(value('current'))
        if value('current') and not current:
            raise ValueError('Unknown current chapter')
        single = 'groups[]' in query
        representatives = [self.rows[UUID(i)] for i in ([100] if single else [103, 101, 102])]
        selected = self.rows.get(value('groupChapter', value('current')))
        data, cursor = [], None
        body = dict(version=1, mangaId=UUID(1), operation=operation, orderingProfile='public-retained-v1',
                    consistency='live-keyset-v1', crossRequestSnapshot=False, history='device-local-only',
                    continueProof='observed-only', scope=sha(pack({key: values for key, values in sorted(query.items()) if key != 'after'})),
                    catalogIdentity=dict(catalogNamespace=UUID(10 if self.identity_changed else 9), serviceKey=None),
                    state='ready', restartRequired=False, limit=limit)
        if current:
            body['current'] = current
        if operation == 'groups':
            direction = value('direction', 'newest')
            if direction not in {'newest', 'oldest'}:
                raise ValueError('Invalid group direction')
            ordered = representatives if direction == 'oldest' else representatives[::-1]
            after = value('after')
            if after not in {None, 'groups_next'}:
                raise ValueError('Invalid group cursor')
            start = 2 if after else 0
            data = ordered[start:start + 2]
            cursor = 'groups_next' if start + 2 < len(ordered) else None
        elif operation in {'editions', 'resolve-group'}:
            if not selected:
                raise ValueError('Missing selected chapter group')
            editions = [self.rows[UUID(i)] for i in ([100] if single else [100, 103])] if selected['chapter'] == '1' else [selected]
            winner = editions[-1]
            body.update(groupAnchor=selected, winner=winner, hasMultipleEditions=len(editions) > 1)
            after = value('after')
            if after not in {None, 'editions_next'}:
                raise ValueError('Invalid edition cursor')
            data = [winner] if operation == 'resolve-group' else editions[1:2] if after else editions[:1]
            cursor = 'editions_next' if operation == 'editions' and len(editions) > 1 and not after else None
        elif operation in {'first', 'last'}:
            data = [representatives[0 if operation == 'first' else -1]]
            body['hasMultipleEditions'] = not single and data[0]['chapter'] == '1'
        else:
            if not current:
                raise ValueError('Missing current chapter')
            index = next(i for i, row in enumerate(representatives) if row['chapter'] == current['chapter'])
            previous = representatives[index - 1] if index else None
            following = representatives[index + 1] if index + 1 < len(representatives) else None
            if operation == 'around':
                data = [representatives[index]]
                body.update(previous=previous, next=following)
            else:
                target = following if operation == 'next' else previous
                data = [target] if target else []
        body.update(data=data, nextCursor=cursor, state='ready' if data else 'boundary')
        return body

    def response(self, request):
        self.requests += 1
        if self.requests > 100 or time.monotonic() - self.started > 15:
            raise ValueError('Fixture total request/deadline cap')
        parsed = urlsplit(request['url'])
        if parsed.scheme + '://' + parsed.netloc != ORIGIN or parsed.fragment:
            raise ValueError('Fixture destination escape')
        path, query = parsed.path, parse_qs(parsed.query, keep_blank_values=True)
        method = request['method']
        body = base64.b64decode(request['body'], validate=True)
        if len(body) > 65536 or method != ('POST' if '/analytics/' in path or path.endswith('/native-forum/contexts') else 'GET'):
            raise ValueError('Fixture method/body refusal')
        if method == 'GET' and body:
            raise ValueError('Unexpected GET payload')
        status, mime = 200, 'application/json'
        if path == '/api/protocol/capabilities' and not query:
            value = dict(catalog_namespace=UUID(9), service_key=None)
        elif path == '/api/auth/check' and not query:
            status, value = 401, dict(error='anonymous')
        elif path == '/api/v1/manga/' + UUID(1) + '/reader':
            value = self.selector(query)
        elif path == '/api/v1/manga':
            if query.get('ids[]') != [UUID(1)]:
                raise ValueError('Unexpected parent selection')
            value = dict(result='ok', response='collection', data=[dict(id=UUID(1), type='manga',
                         attributes=dict(title={'en': 'Owned synthetic title'}, originalLanguage='ja', status='ongoing', contentRating='safe', tags=[]), relationships=[])], limit=100, offset=0, total=1)
        elif path == '/api/v1/nostr/native-forum/contexts':
            resources = json.loads(body)['resources']
            if not resources or len(resources) > 4 or any(row.get('kind') != 'chapter' or row.get('id') not in self.rows for row in resources):
                raise ValueError('Unexpected forum-context lookup')
            value = dict(result='ok', data=[dict(row, thread=None) for row in resources], unavailable=[])
        elif path.startswith('/api/v1/chapter/') and path.removeprefix('/api/v1/chapter/') in self.rows:
            row = self.rows[path.removeprefix('/api/v1/chapter/')]
            value = dict(result='ok', response='entity', data=dict(id=row['id'], type='chapter',
                         attributes=dict(title=None, volume=row['volume'], chapter=row['chapter'], translatedLanguage='en', pages=3, publishAt='2026-10-19T00:00:00Z', version=1),
                         relationships=[dict(id=UUID(1), type='manga'), dict(id=row['groupIds'][0], type='scanlation_group', attributes=dict(name='Owned edition', version=1))]))
        elif path.startswith('/api/v1/at-home/server/') and path.removeprefix('/api/v1/at-home/server/') in self.rows:
            value = dict(result='ok', baseUrl=ORIGIN, chapter=dict(hash='synthetic', data=['0.png', '1.png', '2.png'], dataSaver=[]),
                         nostr=dict(originalsOnly=True, pageDimensions=[[1, 1]] * 3))
        elif path in {'/api/v1/analytics/chapter/' + key + '/views' for key in self.rows}:
            status, value = 204, None
        elif path in {'/data/synthetic/' + str(i) + '.png' for i in range(3)}:
            mime, value = 'image/png', PNG
        else:
            raise ValueError('Unadmitted fixture route; aggregate hydration is never substituted')
        payload = value if isinstance(value, bytes) else b'' if status == 204 else pack(value)
        self.response_bytes += len(payload)
        if len(payload) > 256 * 1024 or self.response_bytes > 4 * 1024**2:
            raise ValueError('Fixture byte cap')
        self.observed.append(dict(path=path, operation=query.get('operation', [None])[0], status=status, bytes=len(payload)))
        return dict(type='response', id=request['id'], status=status, headers={'content-type': mime}, body=base64.b64encode(payload).decode())


def verify_inputs(manifest, expected):
    raw = manifest.read_bytes()
    if sha(raw) != expected:
        raise ValueError('Manifest hash changed')
    value = json.loads(raw)
    for row in value['applicationSources']:
        file = Path(value[row['kind'] + 'Root']) / row['relative']
        if file.is_symlink() or file.resolve() != file or sha(file.read_bytes()) != row['sha256']:
            raise ValueError('Exact mounted application input changed')
    node = Path(value['node']['path'])
    if sha(node.read_bytes()) != value['node']['sha256']:
        raise ValueError('Pinned Node changed')
    if dependency_inventory(Path(value['nodeModules']), Path(value['domNodeModules'])) != value['dependencies']:
        raise ValueError('Pinned compiler/runtime dependency closure changed')
    return value


async def run(args):
    manifest = verify_inputs(args.manifest, args.manifest_sha256)
    output = args.output.resolve()
    if str(output) != manifest['outputRoot'] or output.exists():
        raise ValueError('Fresh exactly reserved output directory required')
    output.mkdir()
    if os.statvfs(output).f_frsize > 4096:
        raise ValueError('Generated allocation reserve requires blocks at most 4096 bytes')
    generation = Path(tempfile.mkdtemp(prefix='weebdex-mounted-load-', dir=output))
    process = None
    fixture, result, stderr = Fixture(), None, bytearray()
    failure, raw_exit, protocol_bytes = None, None, 0
    try:
        env = dict(os.environ, WEEBDEX_BOUNDED_MOUNTED_FIXTURE='1', WEEBDEX_MOUNTED_SOURCE_MANIFEST=str(args.manifest),
                   WEEBDEX_MOUNTED_SOURCE_MANIFEST_SHA256=args.manifest_sha256, WEEBDEX_LOAD_NODE_MODULES=manifest['nodeModules'], WEEBDEX_LOAD_DOM_NODE_MODULES=manifest['domNodeModules'])
        for name in ['NODE_OPTIONS', 'NODE_PATH', 'NODE_COMPILE_CACHE']:
            env.pop(name, None)
        env['NODE_DISABLE_COMPILE_CACHE'] = '1'
        process = await asyncio.create_subprocess_exec(manifest['node']['path'], '--max-old-space-size=256', '--experimental-vm-modules',
                    '--conditions=browser', str(Path(manifest['currentRoot']) / 'scripts/integrated-load/mounted-reader.mjs'),
                    stdin=asyncio.subprocess.PIPE, stdout=asyncio.subprocess.PIPE, stderr=asyncio.subprocess.PIPE, env=env, limit=65536)
        async def send(value):
            payload = pack(value) + b'\n'
            if len(payload) > 65536:
                raise ValueError('IPC line cap')
            process.stdin.write(payload)
            await process.stdin.drain()
        async def drain_stderr():
            while chunk := await process.stderr.read(1024):
                if len(stderr) + len(chunk) > 8192:
                    raise ValueError('Mounted stderr cap')
                stderr.extend(chunk)
        stderr_task = asyncio.create_task(drain_stderr())
        await send(dict(type='init', origin=ORIGIN, config=CONFIG, generation_dir=str(generation)))
        async with asyncio.timeout(30):
            while line := await process.stdout.readline():
                protocol_bytes += len(line)
                if protocol_bytes > 128 * 1024:
                    raise ValueError('IPC total byte cap')
                message = json.loads(line)
                if message['type'] == 'request':
                    await send(fixture.response(message))
                elif message['type'] == 'cancel':
                    fixture.cancellations += 1
                elif message['type'] == 'fixture-phase':
                    if fixture.identity_changed or message.get('phase') != 'identity-mismatch':
                        raise ValueError('Unadmitted fixture phase')
                    fixture.identity_changed = True
                    await send(dict(type='fixture-ack', id=message['id']))
                elif message['type'] == 'result' and result is None:
                    result = message['result']
                else:
                    raise ValueError('Unexpected mounted IPC message')
            raw_exit = await process.wait()
            await stderr_task
        verify_inputs(args.manifest, args.manifest_sha256)
        allocated = sum(file.lstat().st_blocks * 512 for file in generation.iterdir()) + generation.lstat().st_blocks * 512
        if allocated > 256 * 1024 or not result or not result.get('passed') or raw_exit != 0:
            raise ValueError('Mounted raw result, exit or generated allocation failed')
        proof = result.get('node_diagnostics')
        if (type(proof) is not dict or set(proof) != {'accepted', 'provider_scheduled', 'provider_completed', 'failures', 'setup_closed', 'listener_cleanup'}
                or type(proof['accepted']) is not dict or set(proof['accepted']) != {'node-type-stripping', 'node-vm-modules'}
                or any(type(value) is not int or value != 1 for value in proof['accepted'].values())
                or any(type(proof[key]) is not int or proof[key] != expected for key, expected in [('provider_scheduled', 2), ('provider_completed', 2), ('failures', 0)])
                or proof['setup_closed'] is not True or proof['listener_cleanup'] is not True):
            raise ValueError('Exact owned Node warning dispatch proof required')
        cases = result.get('mounted_cases', [])
        if tuple(row.get('name') for row in cases) != CASES or any(row.get('outcome') != 'passed' or len(row.get('assertions', [])) < 2 for row in cases):
            raise ValueError('Eight actual client assertion outcomes required')
        if result.get('http_attempts') != fixture.requests or fixture.cancellations or not fixture.identity_changed:
            raise ValueError('Unsettled/cancelled fixture request or missing identity case')
    except BaseException as error:
        failure = type(error).__name__ + ': ' + str(error)
    finally:
        if process and process.returncode is None:
            process.kill()
            try:
                await asyncio.wait_for(process.wait(), 2)
            except TimeoutError:
                failure = (failure or '') + '; child did not settle after kill'
        if process:
            raw_exit = process.returncode
        if 'stderr_task' in locals() and not stderr_task.done():
            stderr_task.cancel()
        if 'stderr_task' in locals():
            await asyncio.gather(stderr_task, return_exceptions=True)
        shutil.rmtree(generation)
    evidence = dict(passed=failure is None, raw_child_exit=raw_exit, failure=failure, client_result=result,
                    request_count=fixture.requests, response_bytes=fixture.response_bytes, cancellations=fixture.cancellations,
                    pipe_only=True, actual_svelte_mount=True if any(row.get('outcome') == 'passed' for row in (result or {}).get('mounted_cases', [])) else None,
                    mount_observation='completed actual mounted case' if any(row.get('outcome') == 'passed' for row in (result or {}).get('mounted_cases', [])) else 'not established',
                    backend_qualified=False, browser_qualified=False,
                    generated_cleanup=not generation.exists())
    payload = pack(evidence)
    if len(payload) + len(stderr) > 16 * 1024:
        raise ValueError('Mounted result plus raw stderr exceeds pre-write 16 KiB cap')
    (output / 'mounted-result.json').write_bytes(payload)
    (output / 'mounted-stderr.log').write_bytes(stderr)
    print('TAP version 13')
    rows = {row['name']: row for row in (result or {}).get('mounted_cases', [])}
    for index, name in enumerate(CASES, 1):
        print(('ok' if rows.get(name, {}).get('outcome') == 'passed' else 'not ok') + f' {index} - {name}')
    print('1..8')
    print('# raw child exit: ' + str(raw_exit))
    if failure:
        print('Bail out! Overall mounted qualification refused: ' + failure)
    return 0 if failure is None else 1


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--manifest', type=Path, required=True)
    parser.add_argument('--manifest-sha256', required=True)
    parser.add_argument('--output', type=Path, required=True)
    raise SystemExit(asyncio.run(run(parser.parse_args())))
