"""Exact bounded-reader comparisons only; never normalize physical runtime inputs."""
import hashlib
import json
from pathlib import Path
ROOT = Path(__file__).resolve().parents[3]
METADATA = (ROOT / 'weebdex-platform/docs/fixtures/bounded-reader-source-associations.json').read_bytes()
METADATA_SHA256 = '1a245ce7237911e3bc68532637c60861714dec7ec6b42289c496710afde8f716'
def digest(value):
    return hashlib.sha256(value).hexdigest()
def association(metadata=METADATA):
    if type(metadata) is not bytes or len(metadata) != 42359 or digest(metadata) != METADATA_SHA256:
        raise ValueError('Unknown bounded reader comparison metadata')
    return json.loads(metadata)
PATHS = frozenset(row['path'] for row in association()['rows'])
def restore_bounded_reader_source(stage, path, content, metadata=METADATA):
    rows = [row for row in association(metadata)['rows'] if row['stage'] == stage and row['path'] == path]
    if type(stage) is not str or type(path) is not str or len(rows) != 1:
        raise ValueError('Unknown bounded reader comparison stage or path')
    row = rows[0]
    if type(content) is not bytes or len(content) != row['current_bytes'] or digest(content) != row['current_sha256']:
        raise ValueError('Unknown current source bytes: ' + path)
    cursor = 0
    for index, edit in enumerate(row['literal_reversals']):
        current, previous = edit['current'].encode(), edit['previous'].encode()
        position = content.find(current)
        if (edit['ordinal'] != index + 1 or not current or content.count(current) != 1 or position < cursor
                or len(current) != edit['current_bytes'] or digest(current) != edit['current_literal_sha256']
                or len(previous) != edit['previous_bytes'] or digest(previous) != edit['previous_literal_sha256']):
            raise ValueError('Unknown bounded reader comparison literal or order: ' + path)
        content = content[:position] + previous + content[position + len(current):]
        cursor = position + len(previous)
    if len(content) != row['previous_bytes'] or digest(content) != row['previous_sha256']:
        raise ValueError('Unknown previous source bytes: ' + path)
    return content
