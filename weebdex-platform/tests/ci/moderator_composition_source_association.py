"""Exact comparison-only moderator layer, matching the JS predecessor chain."""
import hashlib
import json
from pathlib import Path
from reviewed_bundle_source_successors import UPDATES_COMPOSITION_PATH, restore_bundle_source
ROOT = Path(__file__).resolve().parents[3]
METADATA = (ROOT / 'weebdex-platform/docs/fixtures/moderator-composition-source-association.json').read_bytes()
METADATA_SHA256 = '7f6d38a7a67cb4843b4bec0061d4dd292297524af9909d0b75bb193c374a54db'
def digest(value):
    return hashlib.sha256(value).hexdigest()
def association(metadata=METADATA):
    if not isinstance(metadata, bytes) or digest(metadata) != METADATA_SHA256:
        raise ValueError('Unknown moderator composition metadata')
    return json.loads(metadata)
PATHS = frozenset(row['path'] for row in association()['rows'])
def restore_moderator_composition_source(path, content, metadata=METADATA):
    row = next((row for row in association(metadata)['rows'] if row['path'] == path), None)
    if row is None or not isinstance(content, bytes):
        raise ValueError('Unknown moderator composition source')
    if path == UPDATES_COMPOSITION_PATH:
        content = restore_bundle_source('updates-composition', path, content)
    if len(content) != row['current_bytes'] or digest(content) != row['current_sha256']:
        raise ValueError('Unknown current moderator composition bytes')
    for edit in row['literal_reversals']:
        current = edit['current'].encode()
        if not current or content.count(current) != 1:
            raise ValueError('Unknown moderator composition literal')
        content = content.replace(current, edit['previous'].encode(), 1)
    if len(content) != row['previous_bytes'] or digest(content) != row['previous_sha256']:
        raise ValueError('Unknown predecessor composition bytes')
    return content
