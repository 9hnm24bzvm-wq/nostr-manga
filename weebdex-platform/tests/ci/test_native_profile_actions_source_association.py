"""Finite exact-byte source controls, not browser or HTTP execution."""
from reviewed_bounded_reader_source_successors import restore_bounded_reader_source
from moderator_composition_source_association import PATHS as MODERATOR_PATHS, restore_moderator_composition_source
import copy
import json
import unittest
from native_profile_actions_source_association import ROOT, METADATA, METADATA_SHA256, PATHS, association, digest, restore_native_profile_actions_source
class NativeProfileActionsAssociation(unittest.TestCase):
    def test_exact_current_and_predecessor_sources(self):
        value = association()
        self.assertEqual(digest(METADATA), METADATA_SHA256)
        self.assertEqual(len(PATHS), 4)
        self.assertEqual(value['predecessor_commit'], 'd4028ee')
        for flag in ['browser_executed', 'http_executed']: self.assertIs(value[flag], False)
        for row in value['rows']:
            physical = (ROOT / row['path']).read_bytes()
            current = physical
            if row['path'] == 'weebdex-platform/scripts/prepare-frontend-integration.mjs':
                current = restore_bounded_reader_source('bounded-reader-to-frozen-main:scripts/prepare-frontend-integration.mjs', row['path'], current)
            if row['path'] in MODERATOR_PATHS:
                current = restore_moderator_composition_source(row['path'], current)
            previous = restore_native_profile_actions_source(row['path'], current)
            self.assertEqual((ROOT / row['path']).read_bytes(), physical)
            self.assertEqual(digest(previous), row['previous_sha256'])
            self.assertEqual(len(previous), row['previous_bytes'])
            candidates = [previous, current + b' ', current[1:], None, {}, bytearray(current), current.decode()]
            for edit in row['literal_reversals']:
                self.assertEqual(current.count(edit['current'].encode()), 1)
                candidates.append(current.replace(edit['current'].encode(), edit['previous'].encode()))
            for candidate in candidates:
                with self.assertRaises(ValueError): restore_native_profile_actions_source(row['path'], candidate)
            for path in [None, '', row['path'] + 'unknown']:
                with self.assertRaises(ValueError): restore_native_profile_actions_source(path, current)
    def test_every_metadata_field_row_literal_and_encoding_is_digest_bound(self):
        value = association()
        candidates = [METADATA + b' ', METADATA[1:], METADATA.decode()]
        def encoded(value): return (json.dumps(value, indent=2) + '\n').encode()
        for key in value:
            changed = copy.deepcopy(value); del changed[key]; candidates.append(encoded(changed))
            changed = copy.deepcopy(value); changed[key] = 'unknown'; candidates.append(encoded(changed))
        for i, row in enumerate(value['rows']):
            for key in row:
                changed = copy.deepcopy(value); del changed['rows'][i][key]; candidates.append(encoded(changed))
            for j, edit in enumerate(row['literal_reversals']):
                for key in edit:
                    changed = copy.deepcopy(value); changed['rows'][i]['literal_reversals'][j][key] += ' '; candidates.append(encoded(changed))
        changed = copy.deepcopy(value); changed['rows'].reverse(); candidates.append(encoded(changed))
        for candidate in candidates:
            with self.assertRaises(ValueError): association(candidate)
if __name__ == '__main__': unittest.main()
