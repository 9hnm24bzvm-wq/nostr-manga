"""Reconstruct the immutable historical public-language pinset without Git."""
import copy
import json
import unittest

from test_integrated_feature_source_successor import (
    EVIDENCE, digest, encoded, historical_pinset,
)


def historical_language_pinset(association_bytes):
    def require(condition):
        if not condition:
            raise ValueError('Unknown public-language source association')

    require(digest(association_bytes) == '4f636eac0ba9a1598efbf24a4c59e2fb72295f2a8a0a73b5b37350a33ae1a402')
    association = json.loads(association_bytes)
    historical_bytes = (EVIDENCE / association['historical_integrated_association']).read_bytes()
    require(digest(historical_bytes) == association['historical_integrated_association_sha256'])
    predecessor = historical_pinset(historical_bytes)
    require(digest(encoded(predecessor)) == association['prior_pinset_sha256'])
    require(association['browser_executed'] is False and association['hosted_executed'] is False)
    require(len(association['changes']) == 1)
    change = association['changes'][0]
    require(change['prior']['path'] == change['current']['path'])
    require(predecessor['sources'].count(change['prior']) == 1)
    expected = copy.deepcopy(predecessor)
    expected['sources'][expected['sources'].index(change['prior'])] = change['current']
    require(digest(encoded(expected)) == association['current_pinset_sha256'])
    return expected


def verify_successor(historical_bytes, association_bytes):
    expected = historical_language_pinset(association_bytes)
    if historical_bytes != encoded(expected):
        raise ValueError('Unknown public-language source association')
    return expected


class LanguageFeatureSourceSuccessor(unittest.TestCase):
    def setUp(self):
        self.association = (EVIDENCE / 'language-source-successor.json').read_bytes()
        # The language receipt remains historical when the current filename
        # advances to the separately reviewed composition successor.
        self.current = encoded(historical_language_pinset(self.association))

    def test_exact_reviewed_historical_successor_preserves_historical_chain(self):
        value = verify_successor(self.current, self.association)
        self.assertEqual(len(value['sources']), 40)
        association = json.loads(self.association)
        self.assertEqual(association['reviewed_integration_commit'], '99483383cf6f50802945a731a5f1665c64038c42')
        change = association['changes'][0]
        self.assertEqual(change['current']['path'], 'weebdex-platform/platform-integrations/public-api/PublicChapterEditor.svelte')
        self.assertEqual(change['prior']['sha256'], 'b368c2419b3eb1babe92bc3c13d274265249cf7c2200f63fb43415467689d328')
        self.assertEqual(change['current']['sha256'], 'e4f1c783774682001b814d1a6ccf59041e193ab15590c9f3c4d51459703e58fd')

    def test_unknown_or_reversed_pinsets_and_metadata_are_rejected(self):
        unknown = json.loads(self.current)
        unknown['sources'][0]['sha256'] = '0' * 64
        prior = encoded(historical_pinset((EVIDENCE / 'integrated-source-successor.json').read_bytes()))
        for data in [self.current + b' ', encoded(unknown), prior]:
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
