"""Authored actual-function comparison controls; no browser/backend execution."""
import copy
import json
import unittest
from pathlib import Path
from unittest.mock import patch
import test_composition_feature_source_successor as composition
from reviewed_integration_source_successors import (
    PATHS as INTEGRATION_PATHS, restore_integrated_source,
)
from reviewed_history_source_successors import (
    PATHS as HISTORY_PATHS, restore_history_source,
)
from reviewed_local_page_source_successors import (
    ROOT, EDITOR, PATHS, METADATA, METADATA_SHA256, digest, association,
    DRIVER, DRIVER_METADATA, DRIVER_METADATA_SHA256, driver_association,
    restore_local_page_source,
)


class LocalPageSourceAssociation(unittest.TestCase):
    def setUp(self):
        self.value = association()
        self.row = next(row for row in self.value['rows'] if row['kind'] == 'chapterEditor')
        self.source = (ROOT / EDITOR).read_bytes()

    def test_exact_editor_complete_bytes_and_all_six_unique_reversals(self):
        self.assertEqual(digest(METADATA), METADATA_SHA256)
        self.assertEqual(PATHS, frozenset([EDITOR, DRIVER]))
        proof = self.source
        self.assertEqual(len(self.row['literal_reversals']), 6)
        for edit in self.row['literal_reversals']:
            current = edit['current'].encode()
            self.assertEqual(self.source.count(current), 1)
            self.assertEqual(proof.count(current), 1)
            proof = proof.replace(current, edit['previous'].encode(), 1)
        restored = restore_local_page_source(EDITOR, self.source)
        self.assertEqual(restored, proof)
        self.assertEqual(len(restored), self.row['previous_bytes'])
        self.assertEqual(digest(restored), self.row['previous_sha256'])
        self.assertEqual((ROOT / EDITOR).read_bytes(), self.source)

    def test_unknown_path_type_and_old_missing_partial_duplicate_bytes_refused(self):
        for path in ['', '__proto__', 'constructor', None, 1, EDITOR + ' ']:
            with self.subTest(path=path), self.assertRaises(ValueError):
                restore_local_page_source(path, self.source)
        values = [None, 1, self.source.decode(), bytearray(self.source), self.source + b' ', self.source[1:],
                  restore_local_page_source(EDITOR, self.source)]
        for edit in self.row['literal_reversals']:
            current = edit['current'].encode()
            values.extend([self.source.replace(current, b''), self.source.replace(current, edit['previous'].encode()), self.source + current])
        for value in values:
            with self.subTest(type=type(value)), self.assertRaises(ValueError):
                restore_local_page_source(EDITOR, value)

    def test_unknown_metadata_fields_encoding_and_each_literal_refused(self):
        values = [METADATA + b' ', METADATA.decode(), b'{}']
        for key in self.value:
            for operation in ['delete', 'change']:
                value = copy.deepcopy(self.value)
                if operation == 'delete':
                    del value[key]
                else:
                    value[key] = 'unknown'
                values.append((json.dumps(value, indent=2) + '\n').encode())
        for index, row in enumerate(self.value['rows']):
            for seam in range(len(row['literal_reversals'])):
                for key in ['current', 'previous']:
                    value = copy.deepcopy(self.value)
                    value['rows'][index]['literal_reversals'][seam][key] += ' '
                    values.append((json.dumps(value, indent=2) + '\n').encode())
        for value in values:
            with self.subTest(type=type(value)), self.assertRaises(ValueError):
                restore_local_page_source(EDITOR, self.source, value)

    def test_preserved_historical_pinset_matches_exact_editor_predecessor(self):
        for row in self.value['preserved_historical_artifacts']:
            content = (ROOT / row['path']).read_bytes()
            self.assertEqual(len(content), row['bytes'])
            self.assertEqual(digest(content), row['sha256'])
        pin = next(row for row in self.value['preserved_historical_artifacts'] if row['path'].endswith('/source-pins.json'))
        pins = json.loads((ROOT / pin['path']).read_bytes())
        self.assertEqual(len(pins['sources']), 40)
        self.assertIs(pins['browser_executed'], False)
        self.assertIs(pins['hosted_executed'], False)
        row = next(row for row in pins['sources'] if row['path'] == EDITOR)
        restored = restore_local_page_source(EDITOR, self.source)
        self.assertEqual(len(restored), row['bytes'])
        self.assertEqual(digest(restored), row['sha256'])


class LocalPageDriverSourceAssociation(unittest.TestCase):
    def setUp(self):
        self.value = driver_association()
        self.row = self.value['row']
        self.source = (ROOT / DRIVER).read_bytes()
        self.editor_source = (ROOT / EDITOR).read_bytes()
        self.pin_bytes = (composition.EVIDENCE / 'source-pins.json').read_bytes()
        self.composition_metadata = (composition.EVIDENCE / 'composition-source-successor.json').read_bytes()

    def historical_driver(self):
        prior = restore_local_page_source(DRIVER, self.source)
        integrated = restore_integrated_source(DRIVER, prior)
        return restore_history_source(DRIVER, integrated)

    def test_exact_driver_two_unique_reversals_and_unchanged_original_chain(self):
        self.assertEqual(digest(DRIVER_METADATA), DRIVER_METADATA_SHA256)
        self.assertEqual(PATHS, frozenset([EDITOR, DRIVER]))
        self.assertEqual(self.row['path'], DRIVER)
        self.assertEqual((len(self.source), digest(self.source)), (
            2055, 'a2851efe16c88d1fd14ce48493e24aa415f8c8ac57a397779539822bbd3a3a85'))
        self.assertEqual(len(self.row['literal_reversals']), 2)
        proof = self.source
        for edit in self.row['literal_reversals']:
            current = edit['current'].encode()
            self.assertTrue(current)
            self.assertEqual(edit['previous'], '')
            self.assertEqual(self.source.count(current), 1)
            self.assertEqual(proof.count(current), 1)
            proof = proof.replace(current, edit['previous'].encode(), 1)
        prior = restore_local_page_source(DRIVER, self.source)
        self.assertEqual(prior, proof)
        self.assertEqual((len(prior), digest(prior)), (
            1834, '5e603d716a87caf9f143ee423738985aa4e51e5137600c6707857908fdfad263'))
        self.assertIn(DRIVER, INTEGRATION_PATHS)
        self.assertIn(DRIVER, HISTORY_PATHS)
        integrated = restore_integrated_source(DRIVER, prior)
        self.assertEqual((len(integrated), digest(integrated)), (
            1617, '7e4179b1fab7dfd0a7915878747ef733b44a5788162ddad911df652de92f6476'))
        historical = restore_history_source(DRIVER, integrated)
        self.assertEqual((len(historical), digest(historical)), (
            1432, '660ec6d7478eb2d50d368910aa6719868bebc6e4fcd74174aee453c0e27250fd'))
        pin = next(row for row in json.loads(self.pin_bytes)['sources'] if row['path'] == DRIVER)
        self.assertEqual((len(historical), digest(historical)), (pin['bytes'], pin['sha256']))
        self.assertEqual((ROOT / DRIVER).read_bytes(), self.source)
        self.assertEqual((ROOT / EDITOR).read_bytes(), self.editor_source)

    def test_driver_unknown_path_type_old_partial_double_and_duplicate_bytes_refused(self):
        class StringSubclass(str):
            pass

        class BytesSubclass(bytes):
            pass

        for path in ['', '__proto__', 'constructor', None, 1, [], {}, Path(DRIVER),
                     DRIVER.encode(), StringSubclass(DRIVER), DRIVER + ' ']:
            with self.subTest(path=repr(path)), self.assertRaises(ValueError):
                restore_local_page_source(path, self.source)
        prior = restore_local_page_source(DRIVER, self.source)
        integrated = restore_integrated_source(DRIVER, prior)
        values = [None, 1, self.source.decode(), bytearray(self.source), memoryview(self.source),
                  BytesSubclass(self.source), self.source + b' ', self.source[1:],
                  bytes([self.source[0] ^ 1]) + self.source[1:], prior, integrated,
                  self.historical_driver(), self.source + self.source, self.editor_source]
        for edit in self.row['literal_reversals']:
            current = edit['current'].encode()
            values.extend([self.source.replace(current, edit['previous'].encode(), 1),
                           self.source + current, self.source.replace(current, current + current, 1)])
        for value in values:
            with self.subTest(type=type(value), length=len(value) if hasattr(value, '__len__') else None):
                with self.assertRaises(ValueError):
                    restore_local_page_source(DRIVER, value)
        with self.assertRaises(ValueError):
            restore_local_page_source(DRIVER, restore_local_page_source(DRIVER, self.source))
        with self.assertRaises(ValueError):
            restore_local_page_source(EDITOR, self.source)

    def test_separate_driver_metadata_type_changed_missing_unknown_and_reordered_refused(self):
        def encoded(value):
            return (json.dumps(value, indent=2) + '\n').encode()

        values = [None, 1, DRIVER_METADATA.decode(), bytearray(DRIVER_METADATA),
                  memoryview(DRIVER_METADATA), b'{}', DRIVER_METADATA + b' ',
                  json.dumps(self.value, separators=(',', ':')).encode(),
                  encoded(dict(reversed(list(self.value.items()))))]
        for location in [(), ('row',), ('preserved_local_page_metadata',)]:
            original = self.value
            for key in location:
                original = original[key]
            for key in [*original, 'unreviewed']:
                for operation in ['delete', 'change']:
                    value = copy.deepcopy(self.value)
                    target = value
                    for part in location:
                        target = target[part]
                    if operation == 'delete':
                        if key not in target:
                            continue
                        del target[key]
                    else:
                        target[key] = 'unknown'
                    values.append(encoded(value))
        for index in range(len(self.row['literal_reversals'])):
            for key in ['current', 'previous']:
                for operation in ['delete', 'change']:
                    value = copy.deepcopy(self.value)
                    edit = value['row']['literal_reversals'][index]
                    if operation == 'delete':
                        del edit[key]
                    else:
                        edit[key] += ' '
                    values.append(encoded(value))
            value = copy.deepcopy(self.value)
            value['row']['literal_reversals'][index]['unreviewed'] = True
            values.append(encoded(value))
        for operation in ['missing', 'partial', 'double', 'reordered', 'swapped']:
            value = copy.deepcopy(self.value)
            edits = value['row']['literal_reversals']
            if operation == 'missing':
                edits.clear()
            elif operation == 'partial':
                edits.pop()
            elif operation == 'double':
                edits.extend(copy.deepcopy(edits))
            elif operation == 'reordered':
                edits.reverse()
            else:
                for edit in edits:
                    edit['current'], edit['previous'] = edit['previous'], edit['current']
            values.append(encoded(value))
        value = copy.deepcopy(self.value)
        value['row'] = dict(reversed(list(value['row'].items())))
        values.append(encoded(value))
        for value in values:
            with self.subTest(type=type(value), representation=repr(value)[:100]):
                with self.assertRaises(ValueError):
                    driver_association(value)
                with self.assertRaises(ValueError):
                    restore_local_page_source(DRIVER, self.source, driver_metadata=value)

    def test_original_local_page_metadata_gate_still_applies_to_both_paths(self):
        self.assertEqual(METADATA_SHA256, 'c62216c07016789f0cbc25dd0277ebddb4ae1a707edefcb22cb0a4d749263d1c')
        self.assertEqual(digest(METADATA), METADATA_SHA256)
        for value in [None, METADATA.decode(), METADATA + b' ', b'{}', DRIVER_METADATA]:
            for path, content in [(DRIVER, self.source), (EDITOR, self.editor_source)]:
                with self.subTest(path=path, type=type(value)), self.assertRaises(ValueError):
                    restore_local_page_source(path, content, value)
        with self.assertRaises(ValueError):
            restore_local_page_source(DRIVER, self.source, driver_metadata=METADATA)

    def test_skipping_or_reordering_original_reconstruction_stages_is_refused(self):
        prior = restore_local_page_source(DRIVER, self.source)
        for operation, content in [
            (restore_integrated_source, self.source),
            (restore_history_source, self.source),
            (restore_history_source, prior),
            (restore_local_page_source, restore_integrated_source(DRIVER, prior)),
        ]:
            with self.subTest(operation=operation.__name__, digest=digest(content)):
                with self.assertRaises(ValueError):
                    operation(DRIVER, content)

    def test_composition_reads_physical_local_page_sources_before_original_chain(self):
        events = []

        def record(stage, operation):
            def restore(path, content):
                events.append((path, stage, len(content), digest(content)))
                return operation(path, content)
            return restore

        with patch.object(composition, 'restore_local_page_source', side_effect=record('local', restore_local_page_source)), \
             patch.object(composition, 'restore_integrated_source', side_effect=record('integration', restore_integrated_source)), \
             patch.object(composition, 'restore_history_source', side_effect=record('history', restore_history_source)):
            result = composition.verify_successor(self.pin_bytes, self.composition_metadata)
        self.assertEqual(result, json.loads(self.pin_bytes))
        self.assertEqual(len(result['sources']), 40)
        self.assertEqual([event[1:] for event in events if event[0] == DRIVER], [
            ('local', 2055, 'a2851efe16c88d1fd14ce48493e24aa415f8c8ac57a397779539822bbd3a3a85'),
            ('integration', 1834, '5e603d716a87caf9f143ee423738985aa4e51e5137600c6707857908fdfad263'),
            ('history', 1617, '7e4179b1fab7dfd0a7915878747ef733b44a5788162ddad911df652de92f6476'),
        ])
        self.assertEqual([event[1] for event in events if event[0] == EDITOR][0], 'local')
        self.assertEqual({event[0] for event in events if event[1] == 'local'}, {EDITOR, DRIVER})
        self.assertEqual((ROOT / DRIVER).read_bytes(), self.source)
        self.assertEqual((ROOT / EDITOR).read_bytes(), self.editor_source)

    def test_composition_explicit_exact_historical_overrides_bypass_every_reconstruction(self):
        overrides = {DRIVER: self.historical_driver(),
                     EDITOR: restore_local_page_source(EDITOR, self.editor_source)}
        with patch.object(composition, 'restore_local_page_source', wraps=restore_local_page_source) as local, \
             patch.object(composition, 'restore_integrated_source', wraps=restore_integrated_source) as integrated, \
             patch.object(composition, 'restore_history_source', wraps=restore_history_source) as history:
            self.assertEqual(composition.verify_successor(self.pin_bytes, self.composition_metadata, overrides),
                             json.loads(self.pin_bytes))
        local.assert_not_called()
        for operation in [integrated, history]:
            self.assertTrue(all(call.args[0] not in overrides for call in operation.call_args_list))
        self.assertEqual((ROOT / DRIVER).read_bytes(), self.source)
        self.assertEqual((ROOT / EDITOR).read_bytes(), self.editor_source)

    def test_composition_never_normalizes_explicit_current_partial_or_unknown_driver_overrides(self):
        prior = restore_local_page_source(DRIVER, self.source)
        values = [self.source, prior, restore_integrated_source(DRIVER, prior),
                  self.source + b' ', self.historical_driver() + b' ', None, self.source.decode()]
        for edit in self.row['literal_reversals']:
            values.append(self.source.replace(edit['current'].encode(), edit['previous'].encode(), 1))
        for content in values:
            with self.subTest(type=type(content)), \
                 patch.object(composition, 'restore_local_page_source', wraps=restore_local_page_source) as local, \
                 patch.object(composition, 'restore_integrated_source', wraps=restore_integrated_source) as integrated, \
                 patch.object(composition, 'restore_history_source', wraps=restore_history_source) as history:
                with self.assertRaises(ValueError):
                    composition.verify_successor(self.pin_bytes, self.composition_metadata, {DRIVER: content})
                for operation in [local, integrated, history]:
                    self.assertTrue(all(call.args[0] != DRIVER for call in operation.call_args_list))

    def test_composition_unknown_physical_driver_bytes_fail_before_original_chain(self):
        original_read = Path.read_bytes
        for unknown in [self.source + b' ', self.source[1:],
                        bytes([self.source[0] ^ 1]) + self.source[1:],
                        restore_local_page_source(DRIVER, self.source)]:
            def read_bytes(path):
                return unknown if path == ROOT / DRIVER else original_read(path)

            with self.subTest(digest=digest(unknown)), \
                 patch.object(Path, 'read_bytes', autospec=True, side_effect=read_bytes), \
                 patch.object(composition, 'restore_local_page_source', wraps=restore_local_page_source) as local, \
                 patch.object(composition, 'restore_integrated_source', wraps=restore_integrated_source) as integrated, \
                 patch.object(composition, 'restore_history_source', wraps=restore_history_source) as history:
                with self.assertRaises(ValueError):
                    composition.verify_successor(self.pin_bytes, self.composition_metadata)
                local.assert_any_call(DRIVER, unknown)
                for operation in [integrated, history]:
                    self.assertTrue(all(call.args[0] != DRIVER for call in operation.call_args_list))
        self.assertEqual((ROOT / DRIVER).read_bytes(), self.source)

    def test_preserved_metadata_pinsets_and_comparison_sources_are_not_runtime_rewrites(self):
        preserved = {
            'docs/fixtures/signed-local-page-source-associations.json':
                'c62216c07016789f0cbc25dd0277ebddb4ae1a707edefcb22cb0a4d749263d1c',
            'docs/fixtures/integrated-schedule-catalogue-membership-source-association.json':
                '4f2aa3ce1919c48263336780c0c0fc352ef1332200d9ab348ada6a2835f02c8d',
            'docs/fixtures/device-history-source-association.json':
                'c06a19d189112fb67edcd36e7bc766649b6084284e74a4b3711ea91b8547a33f',
            'test-evidence/public-signed-features-20261022/source-pins.json':
                '0a9c9b85c074bd55a0587112b5b4aaa5cd6b2659b1d31c09a0caf6fe3c43ad8a',
        }
        for path, expected in preserved.items():
            with self.subTest(path=path):
                self.assertEqual(digest((ROOT / 'weebdex-platform' / path).read_bytes()), expected)
        old = association()
        self.assertEqual(self.value['preserved_local_page_metadata'], {
            'path': 'weebdex-platform/docs/fixtures/signed-local-page-source-associations.json',
            'bytes': len(METADATA), 'sha256': METADATA_SHA256,
        })
        old_driver = next(row for row in old['consumer_baseline_artifacts'] if row['path'] == DRIVER)
        self.assertEqual((old_driver['bytes'], old_driver['sha256']),
                         (self.row['previous_bytes'], self.row['previous_sha256']))
        pins = json.loads(self.pin_bytes)
        self.assertNotIn('weebdex-platform/tests/ci/test_composition_feature_source_successor.py',
                         {row['path'] for row in pins['sources']})
        before = {path: (ROOT / path).read_bytes() for path in [
            DRIVER, EDITOR, 'weebdex-platform/docs/fixtures/signed-local-page-driver-source-association.json',
            *['weebdex-platform/' + path for path in preserved],
        ]}
        self.historical_driver()
        restore_local_page_source(EDITOR, self.editor_source)
        for path, content in before.items():
            with self.subTest(path=path):
                self.assertEqual((ROOT / path).read_bytes(), content)


if __name__ == '__main__':
    unittest.main()
