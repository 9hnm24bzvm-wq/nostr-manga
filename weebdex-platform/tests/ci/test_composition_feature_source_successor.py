"""Bind the exact composition successor, not execution or binary acceptance.

Historical pinsets and overlay bytes are reconstructed using checked-in
receipts and literal reversal guards only. No Git history is needed at runtime.
"""
import copy
import json
import unittest
from reviewed_integration_source_successors import PATHS as INTEGRATION_PATHS, restore_integrated_source
from reviewed_history_source_successors import PATHS as HISTORY_PATHS, restore_history_source
from reviewed_local_page_source_successors import PATHS as LOCAL_PAGE_PATHS, restore_local_page_source

from test_integrated_feature_source_successor import EVIDENCE, ROOT, digest, encoded
from test_language_feature_source_successor import historical_language_pinset


PATH = 'weebdex-platform/platform-integrations/public-api/overlays.mjs'
COMMIT = '9929324330a2da2b313c70a5130ce174e8e51080'
ASSOCIATION_SHA256 = 'cf3f58e72b99bce1ec8107f0975ca6e23c6aa6e8238a768408ab73bc0fe58b38'
CURRENT_PINSET_SHA256 = '0a9c9b85c074bd55a0587112b5b4aaa5cd6b2659b1d31c09a0caf6fe3c43ad8a'


def require(condition):
    if not condition:
        raise ValueError('Unknown public-language composition source association')


def exact_association(association_bytes):
    require(isinstance(association_bytes, bytes))
    # The complete byte digest rejects unknown fields, changed claims, reordered
    # metadata, partial reversals and even semantically equivalent reformatting.
    require(digest(association_bytes) == ASSOCIATION_SHA256)
    return json.loads(association_bytes)


def composition_pinset(association_bytes):
    association = exact_association(association_bytes)
    historical_bytes = (EVIDENCE / association['historical_language_association']).read_bytes()
    require(digest(historical_bytes) == association['historical_language_association_sha256'])
    predecessor = historical_language_pinset(historical_bytes)
    require(digest(encoded(predecessor)) == association['prior_pinset_sha256'])
    require(association['reviewed_integration_commit'] == COMMIT)
    require(association['browser_executed'] is False and association['hosted_executed'] is False)
    require(len(association['changes']) == 1)
    change = association['changes'][0]
    require(change['reviewed_source_commit'] == COMMIT)
    require(change['prior']['path'] == change['current']['path'] == PATH)
    require(predecessor['sources'].count(change['prior']) == 1)
    expected = copy.deepcopy(predecessor)
    expected['sources'][expected['sources'].index(change['prior'])] = change['current']
    require(len(expected['sources']) == 40)
    require(digest(encoded(expected)) == association['current_pinset_sha256'] == CURRENT_PINSET_SHA256)
    return expected


def restore_historical_overlay(content, association_bytes):
    """Accept only current reviewed input; historical bytes are comparison output."""
    change = exact_association(association_bytes)['changes'][0]
    require(isinstance(content, bytes))
    require(len(content) == change['current']['bytes'] and digest(content) == change['current']['sha256'])
    for edit in change['reverse_edits']:
        successor = edit['successor'].encode()
        require(content.count(successor) == 1)
        content = content.replace(successor, edit['predecessor'].encode(), 1)
    require(len(content) == change['prior']['bytes'] and digest(content) == change['prior']['sha256'])
    return content


def verify_successor(current_bytes, association_bytes, source_overrides=None):
    expected = composition_pinset(association_bytes)
    require(current_bytes == encoded(expected))
    overrides = {} if source_overrides is None else source_overrides
    paths = {row['path'] for row in expected['sources']}
    require(len(paths) == 40 and set(overrides).issubset(paths))
    for row in expected['sources']:
        content = overrides[row['path']] if row['path'] in overrides else (ROOT / row['path']).read_bytes()
        require(isinstance(content, bytes))
        if row['path'] in LOCAL_PAGE_PATHS and row['path'] not in overrides:
            content = restore_local_page_source(row['path'], content)
        if row['path'] in INTEGRATION_PATHS and row['path'] not in overrides:
            content = restore_integrated_source(row['path'], content)
        if row['path'] in HISTORY_PATHS and row['path'] not in overrides:
            content = restore_history_source(row['path'], content)
        require(len(content) == row['bytes'] and digest(content) == row['sha256'])
        if row['path'] == PATH:
            restore_historical_overlay(content, association_bytes)
    return expected


class CompositionFeatureSourceSuccessor(unittest.TestCase):
    def setUp(self):
        self.current = (EVIDENCE / 'source-pins.json').read_bytes()
        self.association = (EVIDENCE / 'composition-source-successor.json').read_bytes()

    def test_exact_one_row_successor_validates_all_40_current_sources(self):
        value = verify_successor(self.current, self.association)
        predecessor = historical_language_pinset((EVIDENCE / 'language-source-successor.json').read_bytes())
        changes = [(prior, current) for prior, current in zip(predecessor['sources'], value['sources'])
                   if prior != current]
        self.assertEqual(len(value['sources']), 40)
        self.assertEqual(len(changes), 1)
        prior, current = changes[0]
        self.assertEqual(prior, {'path': PATH, 'bytes': 9973,
                               'sha256': 'a8e4d816b2733b11cbf0e82495a3fec369aca9c53ad748eef8867a8bb6b9c606'})
        self.assertEqual(current, {'path': PATH, 'bytes': 10784,
                                 'sha256': '5d5eaeedfd462822091252b42b898ddd655a4534549e9b04a4d358be53e92bf0'})

    def test_initial_workflow_integrated_language_and_execution_receipts_remain_immutable(self):
        historical = {
            'source-pins-initial.json': '3c41c318d9bffdacfe613c2fbc7a03371dc5604e461d2aa3024ce1da154ed301',
            'workflow-test-successor.json': '626b4ccf0d12131ca0227011c539caa1ae6974c0c71d67c5ffa5d7ff3c5a6a22',
            'integrated-source-successor.json': '11fd691810cee5c3679940f91f82a118eaa75303efcc6b62764eb89bde2bc5c0',
            'language-source-successor.json': '4f636eac0ba9a1598efbf24a4c59e2fb72295f2a8a0a73b5b37350a33ae1a402',
            'checks.json': '3afabf63f2d4ed4b9cabb446358fdadd7521b59a4ee7499a89a908c26bb90649',
        }
        for name, expected in historical.items():
            with self.subTest(receipt=name):
                self.assertEqual(digest((EVIDENCE / name).read_bytes()), expected)

    def test_historical_language_pinset_is_reconstructed_not_relabelled_current(self):
        historical = encoded(historical_language_pinset((EVIDENCE / 'language-source-successor.json').read_bytes()))
        self.assertEqual(digest(historical), 'd885d48f6816eb458566694c62fac617482ff4e757dd10a53a6e0e0693fd81cb')
        self.assertEqual(digest(self.current), CURRENT_PINSET_SHA256)
        self.assertNotEqual(historical, self.current)
        with self.assertRaises(ValueError):
            verify_successor(historical, self.association)

    def test_literal_reversal_recovers_exact_historical_overlay_only(self):
        content = restore_integrated_source(PATH, (ROOT / PATH).read_bytes())
        restored = restore_historical_overlay(content, self.association)
        self.assertEqual(len(restored), 9973)
        self.assertEqual(digest(restored), 'a8e4d816b2733b11cbf0e82495a3fec369aca9c53ad748eef8867a8bb6b9c606')
        for candidate in [restored, content + b' ', content[:-1]]:
            with self.subTest(digest=digest(candidate)), self.assertRaises(ValueError):
                restore_historical_overlay(candidate, self.association)
        with self.assertRaises(ValueError):
            verify_successor(self.current, self.association, {PATH: restored})

    def test_every_literal_partial_reversal_is_refused(self):
        content = restore_integrated_source(PATH, (ROOT / PATH).read_bytes())
        for edit in json.loads(self.association)['changes'][0]['reverse_edits']:
            partial = content.replace(edit['successor'].encode(), edit['predecessor'].encode(), 1)
            with self.subTest(edit=edit['successor']), self.assertRaises(ValueError):
                restore_historical_overlay(partial, self.association)

    def test_unknown_partial_reordered_and_reversed_pinsets_are_refused(self):
        current = json.loads(self.current)
        unknown = copy.deepcopy(current)
        unknown['sources'][0]['sha256'] = '0' * 64
        missing = copy.deepcopy(current)
        missing['sources'].pop()
        duplicate = copy.deepcopy(current)
        duplicate['sources'][1] = duplicate['sources'][0]
        reordered = copy.deepcopy(current)
        reordered['sources'][0], reordered['sources'][1] = reordered['sources'][1], reordered['sources'][0]
        partial = copy.deepcopy(current)
        change = json.loads(self.association)['changes'][0]
        row = partial['sources'][partial['sources'].index(change['current'])]
        row['sha256'] = change['prior']['sha256']
        reversed_pinset = copy.deepcopy(current)
        reversed_pinset['sources'][reversed_pinset['sources'].index(change['current'])] = change['prior']
        candidates = [self.current + b' ', encoded(unknown), encoded(missing), encoded(duplicate),
                      encoded(reordered), encoded(partial), encoded(reversed_pinset)]
        for candidate in candidates:
            with self.subTest(digest=digest(candidate)), self.assertRaises(ValueError):
                verify_successor(candidate, self.association)

    def test_every_current_pin_rejects_unknown_associations(self):
        for index, row in enumerate(json.loads(self.current)['sources']):
            unknown = json.loads(self.current)
            unknown['sources'][index]['sha256'] = '0' * 64
            with self.subTest(path=row['path']), self.assertRaises(ValueError):
                verify_successor(encoded(unknown), self.association)

    def test_every_current_source_rejects_arbitrary_bytes(self):
        for row in json.loads(self.current)['sources']:
            content = (ROOT / row['path']).read_bytes()
            same_length = bytes([content[0] ^ 1]) + content[1:]
            for candidate in [same_length, content + b'\n# unknown bytes\n', content[:-1]]:
                with self.subTest(path=row['path'], digest=digest(candidate)), self.assertRaises(ValueError):
                    verify_successor(self.current, self.association, {row['path']: candidate})

    def test_unknown_or_non_byte_source_overrides_are_refused(self):
        for overrides in [{'unknown/source': b''}, {PATH: None}, {PATH: 'not source bytes'}]:
            with self.subTest(overrides=overrides), self.assertRaises(ValueError):
                verify_successor(self.current, self.association, overrides)

    def test_changed_missing_unknown_and_reordered_metadata_is_refused(self):
        association = json.loads(self.association)
        candidates = [self.association + b' ', encoded(dict(reversed(list(association.items()))))]
        for key in association:
            missing = copy.deepcopy(association)
            del missing[key]
            candidates.append(encoded(missing))
            changed = copy.deepcopy(association)
            changed[key] = 'unknown metadata'
            candidates.append(encoded(changed))
        unknown = copy.deepcopy(association)
        unknown['unreviewed'] = True
        candidates.append(encoded(unknown))
        reversed_change = copy.deepcopy(association)
        change = reversed_change['changes'][0]
        change['prior'], change['current'] = change['current'], change['prior']
        candidates.append(encoded(reversed_change))
        for index in range(len(association['changes'][0]['reverse_edits'])):
            changed = copy.deepcopy(association)
            changed['changes'][0]['reverse_edits'][index]['successor'] += '\n'
            candidates.append(encoded(changed))
        reordered = copy.deepcopy(association)
        reordered['changes'][0]['reverse_edits'].reverse()
        candidates.append(encoded(reordered))
        for candidate in candidates:
            with self.subTest(digest=digest(candidate)), self.assertRaises(ValueError):
                verify_successor(self.current, candidate)


if __name__ == '__main__':
    unittest.main()
