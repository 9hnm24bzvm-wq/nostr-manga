"""Exact editor/driver comparison only; never substitute reconstructed runtime bytes."""
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
METADATA_SHA256 = 'c62216c07016789f0cbc25dd0277ebddb4ae1a707edefcb22cb0a4d749263d1c'
METADATA = (ROOT / 'weebdex-platform/docs/fixtures/signed-local-page-source-associations.json').read_bytes()
EDITOR = 'weebdex-platform/platform-integrations/public-api/PublicChapterEditor.svelte'
DRIVER = 'weebdex-platform/tests/ci/test_signed_feature_source_pins.py'
DRIVER_METADATA_SHA256 = '2764693600cde177426c8e948ba0dea2f7f7d97cc5f6ade7ab472078d9c87148'
DRIVER_METADATA = (ROOT / 'weebdex-platform/docs/fixtures/signed-local-page-driver-source-association.json').read_bytes()
PATHS = frozenset([EDITOR, DRIVER])


def require(condition):
    if not condition:
        raise ValueError('Unknown reviewed local page source association')


def digest(value):
    return hashlib.sha256(value).hexdigest()


def association(metadata=METADATA):
    require(type(metadata) is bytes and digest(metadata) == METADATA_SHA256)
    return json.loads(metadata)


def driver_association(metadata=DRIVER_METADATA):
    require(type(metadata) is bytes and digest(metadata) == DRIVER_METADATA_SHA256)
    return json.loads(metadata)


def restore_local_page_source(path, content, metadata=METADATA, *, driver_metadata=DRIVER_METADATA):
    value = association(metadata)
    require(type(path) is str and path in PATHS)
    if path == DRIVER:
        row = driver_association(driver_metadata)['row']
    else:
        row = next(row for row in value['rows'] if row['kind'] == 'chapterEditor')
    require(row['path'] == path and type(content) is bytes)
    require(len(content) == row['current_bytes'] and digest(content) == row['current_sha256'])
    for edit in row['literal_reversals']:
        current = edit['current'].encode()
        require(bool(current) and content.count(current) == 1)
        content = content.replace(current, edit['previous'].encode(), 1)
    require(len(content) == row['previous_bytes'] and digest(content) == row['previous_sha256'])
    return content
