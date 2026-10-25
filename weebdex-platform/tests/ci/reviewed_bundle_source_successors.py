"""Exact named comparison stages only; never normalize runtime/build sources."""
import hashlib
import json
from pathlib import Path
ROOT = Path(__file__).resolve().parents[3]
METADATA = (ROOT / 'weebdex-platform/docs/fixtures/reviewed-bundle-source-successors.json').read_bytes()
METADATA_SHA256 = 'aff713f0a29179ee8701b1fd1b63e4cc6756ee2ef9cc84e57a7510ec614738c8'
def digest(value):
    return hashlib.sha256(value).hexdigest()
def association(metadata=METADATA):
    if not isinstance(metadata, bytes) or digest(metadata) != METADATA_SHA256:
        raise ValueError('Unknown bundle comparison metadata')
    return json.loads(metadata)
UPDATES_INTEGRATION_PATHS = frozenset(row['path'] for row in association()['rows'] if row['stage'] == 'updates-integration')
UPDATES_COMPOSITION_PATH = 'weebdex-platform/scripts/prepare-frontend-integration.mjs'
def restore_bundle_source(stage, path, content, metadata=METADATA):
    rows = [row for row in association(metadata)['rows'] if row['stage'] == stage and row['path'] == path]
    if len(rows) != 1:
        raise ValueError('Unknown bundle comparison stage or path')
    row = rows[0]
    if not isinstance(content, bytes) or len(content) != row['current_bytes'] or digest(content) != row['current_sha256']:
        raise ValueError('Unknown current source bytes: ' + path)
    for edit in row['literal_reversals']:
        current = edit['current'].encode()
        if not current or content.count(current) != 1:
            raise ValueError('Unknown bundle comparison literal: ' + path)
        content = content.replace(current, edit['previous'].encode(), 1)
    if len(content) != row['previous_bytes'] or digest(content) != row['previous_sha256']:
        raise ValueError('Unknown previous source bytes: ' + path)
    return content
