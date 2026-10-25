"""Exact source comparison only; historical fixtures are never executed."""
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
METADATA = (ROOT / 'weebdex-platform/tests/ci/fixtures/reviewed-workflow-source-associations.json').read_bytes()
METADATA_SHA256 = 'f9593226bbbd24e3e28c34667e122d519e3a664f41d19f8d2e5a094ab5ebcb86'


def require(condition):
    if not condition:
        raise ValueError('Unknown reviewed workflow source association')


def digest(value):
    return hashlib.sha256(value).hexdigest()


def association(metadata=METADATA):
    require(type(metadata) is bytes and digest(metadata) == METADATA_SHA256)
    return json.loads(metadata)


PATHS = tuple(row['path'] for row in association()['rows'])


def restore_workflow_source(path, content, metadata=METADATA):
    value = association(metadata)
    require(type(path) is str and path in PATHS and type(content) is bytes)
    row = next(row for row in value['rows'] if row['path'] == path)
    # Check physical current bytes before reading any comparison fixture.
    require(len(content) == row['current_bytes'] and digest(content) == row['current_sha256'])
    try:
        previous = (ROOT / row['previous_fixture']).read_bytes()
    except OSError as error:
        raise ValueError('Unavailable reviewed workflow source fixture') from error
    require(type(previous) is bytes and len(previous) == row['previous_bytes'])
    require(digest(previous) == row['previous_sha256'])
    return previous
