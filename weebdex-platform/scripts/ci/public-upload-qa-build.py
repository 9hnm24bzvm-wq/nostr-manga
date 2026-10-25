"""Read-only admission of the just-built, opt-in upload QA client graph.

The graph is produced by the current Vite plugin, then checked against actual
output bytes and current source inputs here. Synthetic finite graphs exercise
these refusal rules; they do not establish a successful real Vite build.
"""
import hashlib
import json
from pathlib import Path
import re

READER = '_app/immutable/qa/public-upload-refusals.js'
GRAPH = '_app/immutable/qa/public-upload-refusals.graph.json'
EXPORT = 'readUploadRequestRefusals'
SOURCES = {
    'transport': 'platform-integrations/public-api/nostr-upload-request.ts',
    'client': 'platform-integrations/public-api/upload-client.ts',
    'plugin': 'scripts/public-upload-refusal-qa.mjs',
    'config': 'scripts/vite.platform.config.mjs',
}
MAX_GRAPH = 2 * 1024 * 1024
MAX_CHUNKS = 2048
MAX_JS = 32 * 1024 * 1024
MAX_TOTAL_JS = 256 * 1024 * 1024


def insist(value):
    if not value:
        raise RuntimeError('Current upload QA build evidence was refused')


def gate(value):
    insist(value is None or type(value) is str and value in {'0', '1'})
    return value == '1'


def native_environment(environment):
    gate(environment.get('WEEBDEX_QA_UPLOAD_REFUSALS'))
    return dict(environment, WEEBDEX_QA_UPLOAD_REFUSALS='0')


def sha(data):
    return hashlib.sha256(data).hexdigest()


def unique(pairs):
    value = {}
    for key, item in pairs:
        insist(key not in value)
        value[key] = item
    return value


def path_name(value):
    insist(type(value) is str and len(value) <= 512 and
           re.fullmatch(r'[A-Za-z0-9_./-]+', value) is not None and
           not Path(value).is_absolute() and str(Path(value)) == value and
           '..' not in Path(value).parts)
    return value


def read(root, name, maximum):
    path = root / path_name(name)
    insist(root.is_dir() and not root.is_symlink())
    current = path
    while current != root:
        insist(not current.is_symlink())
        current = current.parent
    insist(path.is_file() and path.stat().st_size <= maximum)
    data = path.read_bytes()
    insist(len(data) <= maximum)
    return data


def fields(value, wanted):
    insist(type(value) is dict and set(value) == set(wanted))


def record(value):
    fields(value, {'schema', 'graph_sha256', 'reader_sha256'})
    insist(type(value['schema']) is int and value['schema'] == 1)
    for key in ['graph_sha256', 'reader_sha256']:
        insist(type(value[key]) is str and re.fullmatch(r'[0-9a-f]{64}', value[key]) is not None)
    return value


def names(value, maximum=128):
    # Fixed private traceback codes identify the refused conjunct without
    # echoing graph values. Admission and the existing bounds are unchanged.
    if type(value) is not list:
        raise RuntimeError('qa-graph-names-list-type')
    if len(value) > maximum:
        raise RuntimeError('qa-graph-names-entry-cap')
    if not all(type(item) is str for item in value):
        raise RuntimeError('qa-graph-names-member-type')
    if len(set(value)) != len(value):
        raise RuntimeError('qa-graph-names-duplicate')
    for item in value:
        path_name(item)
    return value


def reaches(chunks, start, target, dynamic=False):
    pending, seen = [start], set()
    while pending:
        name = pending.pop()
        if name == target:
            return True
        if name in seen:
            continue
        seen.add(name)
        insist(name in chunks)
        row = chunks[name]
        pending.extend(row['imports'])
        if dynamic:
            pending.extend(row['dynamic_imports'])
    return False


def verify(root, ui, enabled):
    root, ui = Path(root), Path(ui)
    insist(type(enabled) is bool and ui.is_dir() and not ui.is_symlink())
    paths = list(ui.rglob('*'))
    insist(len(paths) <= 8192 and not any(path.is_symlink() for path in paths))
    if not enabled:
        insist(not (ui / READER).exists() and not (ui / GRAPH).exists())
        total = 0
        for path in paths:
            if path.suffix != '.js' or not path.is_file():
                continue
            data = read(ui, str(path.relative_to(ui)), MAX_JS)
            total += len(data)
            insist(total <= MAX_TOTAL_JS and EXPORT.encode() not in data and
                   b'QA_UPLOAD_REFUSAL_' not in data)
        return {'schema': 1, 'enabled': False}
    encoded = read(ui, GRAPH, MAX_GRAPH)
    try:
        graph = json.loads(encoded, object_pairs_hook=unique)
    except (ValueError, UnicodeError) as error:
        raise RuntimeError('Current upload QA build evidence was refused') from error
    fields(graph, {'schema', 'enabled', 'reader_url', 'graph_url', 'reader_export',
                   'transport_module_id', 'transport_source_sha256',
                   'client_module_id', 'client_source_sha256',
                   'plugin_source_sha256', 'config_source_sha256',
                   'transport_module_count', 'client_module_count', 'reader_module_count',
                   'reader_chunk', 'transport_chunk', 'client_chunk',
                   'reader_reaches_transport', 'client_reaches_transport',
                   'client_entry_chunks', 'entry_reaches_client', 'chunks'})
    insist(type(graph['schema']) is int and graph['schema'] == 1 and graph['enabled'] is True and
           graph['reader_url'] == '/' + READER and graph['graph_url'] == '/' + GRAPH and
           graph['reader_export'] == EXPORT and graph['reader_chunk'] == READER and
           graph['reader_reaches_transport'] is True and graph['client_reaches_transport'] is True and
           graph['entry_reaches_client'] is True)
    for kind, name in SOURCES.items():
        insist(graph[kind + '_source_sha256'] == sha(read(root, name, MAX_JS)))
    for kind in ['transport', 'client']:
        insist(graph[kind + '_module_id'] == SOURCES[kind])
    for kind in ['transport', 'client', 'reader']:
        insist(type(graph[kind + '_module_count']) is int and graph[kind + '_module_count'] == 1)
        path_name(graph[kind + '_chunk'])
    rows = graph['chunks']
    insist(type(rows) is list and 1 <= len(rows) <= MAX_CHUNKS)
    chunks, total = {}, 0
    for row in rows:
        fields(row, {'file_name', 'sha256', 'imports', 'dynamic_imports', 'exports',
                     'contains_transport', 'contains_client', 'contains_reader'})
        name = path_name(row['file_name'])
        insist(name.endswith('.js') and name not in chunks)
        data = read(ui, name, MAX_JS)
        total += len(data)
        insist(total <= MAX_TOTAL_JS and row['sha256'] == sha(data))
        names(row['imports'])
        names(row['dynamic_imports'])
        insist(type(row['exports']) is list and len(row['exports']) <= 1024 and
               all(type(value) is str and 1 <= len(value) <= 256 and not re.search(r'[\x00-\x1f\x7f]', value)
                   for value in row['exports']) and len(set(row['exports'])) == len(row['exports']))
        for kind in ['transport', 'client', 'reader']:
            insist(type(row['contains_' + kind]) is bool)
        chunks[name] = row
    for kind in ['transport', 'client', 'reader']:
        marked = [name for name, row in chunks.items() if row['contains_' + kind]]
        insist(marked == [graph[kind + '_chunk']])
    for row in chunks.values():
        insist(all(name in chunks for name in row['imports'] + row['dynamic_imports']))
    entries = names(graph['client_entry_chunks'], MAX_CHUNKS)
    insist(entries and READER not in entries and all(name in chunks for name in entries))
    insist(chunks[READER]['exports'] == [EXPORT] and
           reaches(chunks, READER, graph['transport_chunk']) and
           reaches(chunks, graph['client_chunk'], graph['transport_chunk']) and
           any(reaches(chunks, entry, graph['client_chunk'], True) for entry in entries))
    return {'schema': 1, 'graph_sha256': sha(encoded),
            'reader_sha256': sha(read(ui, READER, MAX_JS))}
