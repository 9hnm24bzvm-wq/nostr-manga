"""Exact comparison only; never substitute old modules into current execution."""
import hashlib
import json
from pathlib import Path
ROOT = Path(__file__).resolve().parents[3]
METADATA = (ROOT / 'weebdex-platform/tests/fixtures/native-profile-actions/source-association.json').read_bytes()
METADATA_SHA256 = '100517251eec79b108c69e4ecd607c79669d74b168f328b2ff1bc2e38892f80b'
def digest(value):
    return hashlib.sha256(value).hexdigest()
def association(metadata=METADATA):
    if not isinstance(metadata, bytes) or digest(metadata) != METADATA_SHA256:
        raise ValueError('Unknown native-profile action association')
    return json.loads(metadata)
PATHS = frozenset(row['path'] for row in association()['rows'])
def restore_native_profile_actions_source(path, content):
    row = next((row for row in association()['rows'] if row['path'] == path), None)
    if row is None or not isinstance(content, bytes):
        raise ValueError('Unknown native-profile action source')
    if len(content) != row['current_bytes'] or digest(content) != row['current_sha256']:
        raise ValueError('Unknown native-profile action current bytes')
    for edit in row['literal_reversals']:
        current = edit['current'].encode()
        if not current or content.count(current) != 1:
            raise ValueError('Unknown native-profile action literal')
        content = content.replace(current, edit['previous'].encode(), 1)
    if len(content) != row['previous_bytes'] or digest(content) != row['previous_sha256']:
        raise ValueError('Unknown native-profile action previous bytes')
    return content
