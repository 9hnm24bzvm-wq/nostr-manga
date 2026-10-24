"""Comparison only; never substitute historical bytes for product execution."""
import hashlib
import json
from pathlib import Path
from reviewed_workflow_source_successors import PATHS as WORKFLOW_PATHS, restore_workflow_source
ROOT=Path(__file__).resolve().parents[3]
METADATA=(ROOT/'weebdex-platform/docs/fixtures/device-history-source-association.json').read_bytes()
SHA256='c06a19d189112fb67edcd36e7bc766649b6084284e74a4b3711ea91b8547a33f'
def require(value):
    if not value:
        raise ValueError('Unknown reviewed history source association')
def association(metadata=METADATA):
    require(isinstance(metadata,bytes) and hashlib.sha256(metadata).hexdigest()==SHA256)
    return json.loads(metadata)
HISTORY_PATHS=tuple(row['path'] for row in association()['rows'])
require(not set(HISTORY_PATHS).intersection(WORKFLOW_PATHS))
PATHS=HISTORY_PATHS+WORKFLOW_PATHS
def restore_history_source(path,content,metadata=METADATA):
    rows=[row for row in association(metadata)['rows'] if row['path']==path]
    if path in WORKFLOW_PATHS:
        return restore_workflow_source(path,content)
    require(len(rows)==1 and isinstance(content,bytes))
    row=rows[0]
    require(len(content)==row['current_bytes'] and hashlib.sha256(content).hexdigest()==row['current_sha256'])
    for edit in row['literal_reversals']:
        current=edit['current'].encode();require(current and content.count(current)==1)
        content=content.replace(current,edit['previous'].encode(),1)
    require(len(content)==row['previous_bytes'] and hashlib.sha256(content).hexdigest()==row['previous_sha256'])
    return content
