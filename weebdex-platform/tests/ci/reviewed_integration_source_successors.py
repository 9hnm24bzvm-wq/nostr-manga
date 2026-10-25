"""Comparison only: never substitute predecessor bytes in runtime or build inputs."""
from reviewed_bundle_source_successors import UPDATES_INTEGRATION_PATHS, restore_bundle_source
from native_profile_actions_source_association import PATHS as PROFILE_PATHS, restore_native_profile_actions_source
import hashlib
import json
from pathlib import Path
ROOT = Path(__file__).resolve().parents[3]
METADATA = (ROOT / 'weebdex-platform/docs/fixtures/integrated-schedule-catalogue-membership-source-association.json').read_bytes()
METADATA_SHA256 = '4f2aa3ce1919c48263336780c0c0fc352ef1332200d9ab348ada6a2835f02c8d'
def digest(value):
    return hashlib.sha256(value).hexdigest()
def association(metadata=METADATA):
    if not isinstance(metadata, bytes) or digest(metadata) != METADATA_SHA256:
        raise ValueError('Unknown integrated source association metadata')
    return json.loads(metadata)
PATHS = frozenset(row['path'] for row in association()['rows'])
def restore_integrated_source(path, content, metadata=METADATA):
    row = next((row for row in association(metadata)['rows'] if row['path'] == path), None)
    if row is None:
        raise ValueError('Unknown integrated source path')
    if path in UPDATES_INTEGRATION_PATHS:
        content = restore_bundle_source('updates-integration', path, content)
    if path in PROFILE_PATHS:
        try:
            content = restore_native_profile_actions_source(path, content)
        except ValueError as error:
            raise ValueError('Unknown current source bytes: ' + path) from error
    if not isinstance(content, bytes) or len(content) != row['current_bytes'] or digest(content) != row['current_sha256']:
        raise ValueError('Unknown current source bytes: ' + path)
    for edit in row['literal_reversals']:
        current = edit['current'].encode()
        if not current or content.count(current) != 1:
            raise ValueError('Unknown integrated source literal: ' + path)
        content = content.replace(current, edit['previous'].encode(), 1)
    if len(content) != row['previous_bytes'] or digest(content) != row['previous_sha256']:
        raise ValueError('Unknown previous source bytes: ' + path)
    return content
