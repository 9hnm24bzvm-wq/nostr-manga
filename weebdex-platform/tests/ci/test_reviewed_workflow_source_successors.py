"""Exact comparison controls; no historical fixture becomes executable input."""
import copy
import hashlib
import json
from pathlib import Path
import unittest
from unittest.mock import patch

import reviewed_history_source_successors as history
import test_composition_feature_source_successor as composition
from reviewed_workflow_source_successors import (
    ROOT, METADATA, METADATA_SHA256, PATHS, association, digest,
    restore_workflow_source,
)


class WorkflowSourceSuccessors(unittest.TestCase):
    def setUp(self):
        self.metadata = association()
        self.rows = self.metadata['rows']
        self.current = {row['path']: (ROOT / row['path']).read_bytes() for row in self.rows}
        self.previous = {row['path']: (ROOT / row['previous_fixture']).read_bytes() for row in self.rows}

    def test_exact_two_disjoint_mappings_preserve_historical_pinset_and_driver(self):
        self.assertEqual(digest(METADATA), METADATA_SHA256)
        self.assertEqual(PATHS, (
            '.github/workflows/hosted-browser-e2e.yml',
            'weebdex-platform/tests/ci/test_native_upload_diagnostic_workflow.py',
        ))
        self.assertEqual(len(history.HISTORY_PATHS), 4)
        self.assertFalse(set(PATHS).intersection(history.HISTORY_PATHS))
        self.assertEqual(history.PATHS, history.HISTORY_PATHS + PATHS)
        self.assertEqual(self.metadata['schema'], 1)
        self.assertEqual(self.metadata['scope'], 'retained-public-workflow-source-comparison-only-v1')
        self.assertIs(self.metadata['comparison_only'], True)
        for flag in ['browser_executed', 'hosted_executed']:
            self.assertIs(self.metadata[flag], False)
        pin_bytes = (composition.EVIDENCE / 'source-pins.json').read_bytes()
        self.assertEqual(digest(pin_bytes), '0a9c9b85c074bd55a0587112b5b4aaa5cd6b2659b1d31c09a0caf6fe3c43ad8a')
        pins = {row['path']: row for row in json.loads(pin_bytes)['sources']}
        expected = [
            (8857, '29c61963198b921a2e22419cce67d38a4ebd9c868012701bfff2b0bb87dff994',
             8661, '76d40e0370cdb7c5fb57ae9b4380c39252ff820c1ff2d3dc0a6f0c15fca1d135',
             '073d48c9545e528d569a9fa4be45c10d07911f4a'),
            (12607, 'b9a81766796ffb15e70b92f67c98e48ba23785781b4028b094379672e653ca32',
             12392, '194289c9cc71deca014700c46d77e8cc983ac555e2aca903a79513907826664b',
             '81826b93af6385b79f50ba68a0fb9d72a92039a3'),
        ]
        for row, identity in zip(self.rows, expected):
            path = row['path']
            with self.subTest(path=path):
                self.assertEqual(tuple(row[key] for key in [
                    'current_bytes', 'current_sha256', 'previous_bytes',
                    'previous_sha256', 'previous_git_blob',
                ]), identity)
                self.assertEqual(row['previous_source_commit'], 'c5ff2170a7d2624abe238528ae960d3bef8c916f')
                previous = restore_workflow_source(path, self.current[path])
                self.assertEqual(previous, self.previous[path])
                self.assertEqual(history.restore_history_source(path, self.current[path]), previous)
                self.assertEqual((len(previous), digest(previous)), (pins[path]['bytes'], pins[path]['sha256']))
                git_blob = b'blob ' + str(len(previous)).encode('ascii') + b'\0' + previous
                self.assertEqual(hashlib.sha1(git_blob).hexdigest(), row['previous_git_blob'])
                self.assertNotEqual(previous, self.current[path])
                self.assertEqual((ROOT / path).read_bytes(), self.current[path])
                self.assertEqual((ROOT / row['previous_fixture']).read_bytes(), self.previous[path])
        driver = (ROOT / 'weebdex-platform/tests/ci/test_signed_feature_source_pins.py').read_bytes()
        self.assertEqual((len(driver), digest(driver)), (
            2055, 'a2851efe16c88d1fd14ce48493e24aa415f8c8ac57a397779539822bbd3a3a85'))
        self.assertEqual((composition.EVIDENCE / 'source-pins.json').read_bytes(), pin_bytes)

    def test_unknown_current_path_type_and_predecessor_refused_before_fixture_read(self):
        class StringSubclass(str):
            pass

        class BytesSubclass(bytes):
            pass

        for row in self.rows:
            path = row['path']
            current = self.current[path]
            candidates = [None, 1, current.decode(), bytearray(current), memoryview(current),
                          BytesSubclass(current), current + b' ', current[:-1],
                          bytes([current[0] ^ 1]) + current[1:], self.previous[path],
                          current + current]
            candidates.extend(self.current[other] for other in PATHS if other != path)
            for candidate in candidates:
                with self.subTest(path=path, type=type(candidate)), \
                     patch.object(Path, 'read_bytes') as read:
                    with self.assertRaises(ValueError):
                        restore_workflow_source(path, candidate)
                    read.assert_not_called()
            for unknown in ['', path + ' ', '../' + path, None, 1, [], {},
                            Path(path), path.encode(), StringSubclass(path)]:
                with self.subTest(path=repr(unknown)), patch.object(Path, 'read_bytes') as read:
                    with self.assertRaises(ValueError):
                        restore_workflow_source(unknown, current)
                    read.assert_not_called()

    def test_missing_or_tampered_historical_fixture_never_counts_as_match(self):
        for row in self.rows:
            path = row['path']
            previous = self.previous[path]
            candidates = [None, previous.decode(), bytearray(previous), previous + b' ',
                          previous[:-1], bytes([previous[0] ^ 1]) + previous[1:],
                          self.current[path]]
            candidates.extend(self.previous[other] for other in PATHS if other != path)
            for candidate in candidates:
                with self.subTest(path=path, type=type(candidate)), \
                     patch.object(Path, 'read_bytes', return_value=candidate):
                    with self.assertRaises(ValueError):
                        restore_workflow_source(path, self.current[path])
            with self.subTest(path=path), patch.object(Path, 'read_bytes', side_effect=FileNotFoundError):
                with self.assertRaises(ValueError):
                    restore_workflow_source(path, self.current[path])

    def test_full_metadata_bytes_fields_order_and_identity_are_immutable(self):
        def encoded(value):
            return (json.dumps(value, indent=2) + '\n').encode()

        candidates = [None, METADATA.decode(), bytearray(METADATA), memoryview(METADATA),
                      b'{}', METADATA + b' ', METADATA[:-1],
                      encoded(dict(reversed(list(self.metadata.items()))))]
        for location in [(), ('rows', 0), ('rows', 1)]:
            original = self.metadata
            for key in location:
                original = original[key]
            for key in [*original, 'unreviewed']:
                for operation in ['delete', 'change']:
                    value = copy.deepcopy(self.metadata)
                    target = value
                    for part in location:
                        target = target[part]
                    if operation == 'delete':
                        if key not in target:
                            continue
                        del target[key]
                    else:
                        target[key] = 'unknown'
                    candidates.append(encoded(value))
        for operation in ['missing', 'duplicate', 'reordered', 'swapped']:
            value = copy.deepcopy(self.metadata)
            if operation == 'missing':
                value['rows'].pop()
            elif operation == 'duplicate':
                value['rows'][1] = copy.deepcopy(value['rows'][0])
            elif operation == 'reordered':
                value['rows'].reverse()
            else:
                for row in value['rows']:
                    for suffix in ['bytes', 'sha256']:
                        old, new = 'previous_' + suffix, 'current_' + suffix
                        row[old], row[new] = row[new], row[old]
            candidates.append(encoded(value))
        for candidate in candidates:
            with self.subTest(type=type(candidate)), patch.object(Path, 'read_bytes') as read:
                with self.assertRaises(ValueError):
                    association(candidate)
                for path in PATHS:
                    with self.assertRaises(ValueError):
                        restore_workflow_source(path, self.current[path], candidate)
                read.assert_not_called()

    def test_original_history_metadata_gate_still_precedes_the_disjoint_adapter(self):
        for metadata in [history.METADATA + b' ', history.METADATA.decode(), METADATA, b'{}']:
            for path in PATHS:
                with self.subTest(path=path, type=type(metadata)), \
                     patch.object(history, 'restore_workflow_source') as restore:
                    with self.assertRaises(ValueError):
                        history.restore_history_source(path, self.current[path], metadata)
                    restore.assert_not_called()

    def test_composition_only_adapts_physical_current_sources_not_explicit_overrides(self):
        pin_bytes = (composition.EVIDENCE / 'source-pins.json').read_bytes()
        receipt = (composition.EVIDENCE / 'composition-source-successor.json').read_bytes()
        with patch.object(composition, 'restore_history_source', wraps=history.restore_history_source) as restore:
            self.assertEqual(composition.verify_successor(pin_bytes, receipt), json.loads(pin_bytes))
        for path in PATHS:
            restore.assert_any_call(path, self.current[path])
        with patch.object(composition, 'restore_history_source', wraps=history.restore_history_source) as restore:
            self.assertEqual(composition.verify_successor(pin_bytes, receipt, self.previous), json.loads(pin_bytes))
            self.assertTrue(all(call.args[0] not in PATHS for call in restore.call_args_list))
        for path in PATHS:
            for content in [self.current[path], self.previous[path] + b' ', None]:
                with self.subTest(path=path, type=type(content)), \
                     patch.object(composition, 'restore_history_source', wraps=history.restore_history_source) as restore:
                    with self.assertRaises(ValueError):
                        composition.verify_successor(pin_bytes, receipt, {path: content})
                    self.assertTrue(all(call.args[0] != path for call in restore.call_args_list))


if __name__ == '__main__':
    unittest.main()
