"""Reconstruct the immutable historical integrated pinset without Git history."""
import copy
import hashlib
import json
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[3]
EVIDENCE = ROOT / 'weebdex-platform/test-evidence/public-signed-features-20261022'


def digest(data):
    return hashlib.sha256(data).hexdigest()


def encoded(value):
    return (json.dumps(value, indent=2) + '\n').encode()


def historical_pinset(association_bytes):
    def require(condition):
        if not condition:
            raise ValueError('Unknown signed-feature source association')

    require(digest(association_bytes) == '11fd691810cee5c3679940f91f82a118eaa75303efcc6b62764eb89bde2bc5c0')
    association = json.loads(association_bytes)
    initial_bytes = (EVIDENCE / 'source-pins-initial.json').read_bytes()
    workflow_bytes = (EVIDENCE / association['historical_workflow_association']).read_bytes()
    require(digest(initial_bytes) == '3c41c318d9bffdacfe613c2fbc7a03371dc5604e461d2aa3024ce1da154ed301')
    require(digest(workflow_bytes) == association['historical_workflow_association_sha256'])
    workflow = json.loads(workflow_bytes)
    predecessor = json.loads(initial_bytes)
    change = workflow['changes'][0]
    require(predecessor['sources'].count(change['prior']) == 1)
    predecessor['sources'][predecessor['sources'].index(change['prior'])] = change['current']
    require(digest(encoded(predecessor)) == association['prior_pinset_sha256'] == workflow['current_pinset_sha256'])
    require(association['browser_executed'] is False and association['hosted_executed'] is False)
    rows = {row['path']: row for row in predecessor['sources']}
    seen = set()
    for change in association['changes']:
        path = change['current']['path']
        require(path not in seen and rows.get(path) == change['prior'])
        seen.add(path)
        rows[path] = change['current']
    expected = copy.deepcopy(predecessor)
    expected['sources'] = [rows[path] for path in sorted(rows)]
    require(digest(encoded(expected)) == association['current_pinset_sha256'])
    return expected


def verify_successor(current_bytes, association_bytes):
    expected = historical_pinset(association_bytes)
    if current_bytes != encoded(expected):
        raise ValueError('Unknown signed-feature source association')
    return expected


class IntegratedFeatureSourceSuccessor(unittest.TestCase):
    def setUp(self):
        self.association = (EVIDENCE / 'integrated-source-successor.json').read_bytes()
        self.current = encoded(historical_pinset(self.association))

    def test_exact_reviewed_historical_successor_and_prior_chain(self):
        value = verify_successor(self.current, self.association)
        self.assertEqual(len(value['sources']), 40)
        association = json.loads(self.association)
        self.assertEqual(association['reviewed_integration_commit'], '55a55d6578d991285caaef1eff362ad2f9fc6f1c')
        self.assertEqual(len(association['changes']), 10)
        self.assertEqual(sum(change['prior'] is None for change in association['changes']), 4)

    def test_unknown_and_partially_reversed_pinsets_are_rejected(self):
        unknown = json.loads(self.current)
        unknown['sources'][0]['sha256'] = '0' * 64
        partial = json.loads(self.current)
        change = next(change for change in json.loads(self.association)['changes'] if change['prior'])
        partial['sources'][partial['sources'].index(change['current'])] = change['prior']
        for data in [self.current + b' ', encoded(unknown), encoded(partial)]:
            with self.subTest(digest=digest(data)), self.assertRaises(ValueError):
                verify_successor(data, self.association)
        with self.assertRaises(ValueError):
            verify_successor(self.current, self.association + b' ')

    def test_every_historical_pin_rejects_unknown_associations(self):
        for index, row in enumerate(json.loads(self.current)['sources']):
            unknown = json.loads(self.current)
            unknown['sources'][index]['sha256'] = '0' * 64
            with self.subTest(path=row['path']), self.assertRaises(ValueError):
                verify_successor(encoded(unknown), self.association)
