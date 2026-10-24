"""Finite Python parity for explicit updates and composition source stages."""
from reviewed_history_source_successors import restore_history_source
from reviewed_bounded_reader_source_successors import METADATA as READER_METADATA, METADATA_SHA256 as READER_METADATA_SHA256, association as reader_association, restore_bounded_reader_source
import copy
import json
import unittest
from reviewed_bundle_source_successors import ROOT, METADATA, METADATA_SHA256, association, digest, restore_bundle_source
from moderator_composition_source_association import METADATA as MODERATOR_METADATA, association as moderator_association, restore_moderator_composition_source
from native_profile_actions_source_association import restore_native_profile_actions_source

def encoded(value):
    return (json.dumps(value, indent=2, ensure_ascii=False) + '\n').encode()

def reader_preparation_comparison(path, physical):
    if path == 'weebdex-platform/scripts/prepare-frontend-integration.mjs':
        return restore_bounded_reader_source('bounded-reader-to-frozen-main:scripts/prepare-frontend-integration.mjs', path, physical)
    return physical

class BundleSourceSuccessors(unittest.TestCase):
    def test_exact_updates_stages_and_all_partial_source_refusals(self):
        value = association()
        self.assertEqual(digest(METADATA), METADATA_SHA256)
        self.assertEqual(len(value['rows']), 5)
        self.assertEqual(value['physical_source_commit'], '5a4cc2530e4dd81f3f1888b39588a6afd27be98d')
        for key in ['browser_executed', 'http_executed', 'capacity_qualified']:
            self.assertIs(value[key], False)
        for row in value['rows'][:3]:
            physical = (ROOT / row['path']).read_bytes()
            current = reader_preparation_comparison(row['path'], physical)
            previous = restore_bundle_source(row['stage'], row['path'], current)
            self.assertEqual(digest(current), row['current_sha256'])
            self.assertEqual(len(current), row['current_bytes'])
            self.assertEqual(digest(previous), row['previous_sha256'])
            self.assertEqual(len(previous), row['previous_bytes'])
            candidates = [previous, current + b' ', current[1:], bytes([current[0] ^ 1]) + current[1:], None, {}, bytearray(current), current.decode()]
            for edit in row['literal_reversals']:
                self.assertEqual(current.count(edit['current'].encode()), 1)
                candidates.extend([current.replace(edit['current'].encode(), edit['previous'].encode(), 1), current + edit['current'].encode()])
            for candidate in candidates:
                with self.assertRaises(ValueError): restore_bundle_source(row['stage'], row['path'], candidate)
            for stage in ['', None, 'unknown', *[other['stage'] for other in value['rows'] if other['stage'] != row['stage']]]:
                with self.assertRaises(ValueError): restore_bundle_source(stage, row['path'], current)
            for path in ['', None, '../' + row['path'], row['path'] + 'unknown']:
                with self.assertRaises(ValueError): restore_bundle_source(row['stage'], path, current)
            self.assertEqual((ROOT / row['path']).read_bytes(), physical)

    def test_metadata_binds_all_rows_stages_literals_and_claims(self):
        value = association()
        candidates = [METADATA + b' ', METADATA[1:], METADATA.decode(), encoded(dict(reversed(list(value.items()))))]
        for key in value:
            changed = copy.deepcopy(value); del changed[key]; candidates.append(encoded(changed))
        for i, row in enumerate(value['rows']):
            for key in row:
                changed = copy.deepcopy(value); del changed['rows'][i][key]; candidates.append(encoded(changed))
            for j, edit in enumerate(row['literal_reversals']):
                for key in edit:
                    changed = copy.deepcopy(value); changed['rows'][i]['literal_reversals'][j][key] += ' '; candidates.append(encoded(changed))
        changed = copy.deepcopy(value); changed['rows'].reverse(); candidates.append(encoded(changed))
        changed = copy.deepcopy(value); changed['rows'][0]['literal_reversals'].reverse(); candidates.append(encoded(changed))
        changed = copy.deepcopy(value); changed['unknown'] = True; candidates.append(encoded(changed))
        row = value['rows'][0]; current = (ROOT / row['path']).read_bytes()
        for candidate in candidates:
            with self.assertRaises(ValueError): association(candidate)
            with self.assertRaises(ValueError): restore_bundle_source(row['stage'], row['path'], current, candidate)

    def test_exact_updates_moderator_profile_order_and_moderator_metadata_refusals(self):
        path = 'weebdex-platform/scripts/prepare-frontend-integration.mjs'
        physical = (ROOT / path).read_bytes()
        current = reader_preparation_comparison(path, physical)
        updates = restore_bundle_source('updates-composition', path, current)
        moderator = restore_moderator_composition_source(path, current)
        profile = restore_native_profile_actions_source(path, moderator)
        self.assertEqual([len(value) for value in [physical, current, updates, moderator, profile]], [31867, 30561, 29891, 29847, 29805])
        self.assertEqual(digest(physical), 'e55efc65d6efda7ee894aa29d5671ba3c35ee827d9f3b7cababf1125092353f4')
        self.assertEqual(digest(current), 'd498597c8b214f6e4f5025eb6216e0c2b1516d4d1bb27933a727f0c3cfedc38c')
        self.assertEqual((ROOT / path).read_bytes(), physical)
        for wrong in [current, updates, moderator, profile]:
            with self.assertRaises(ValueError): restore_bounded_reader_source('bounded-reader-to-frozen-main:scripts/prepare-frontend-integration.mjs', path, wrong)
        with self.assertRaises(ValueError): restore_bundle_source('updates-composition', path, physical)
        self.assertEqual(digest(updates), 'bdb676c70e3d9eb4a5488b602ad6a778cc25be3b0a196a95fa87d4642fa10543')
        self.assertEqual(digest(moderator), '38011a1d691495466ac05ba441cf9cdc0bbe5d4b743569bf72539d6687344aba')
        self.assertEqual(digest(profile), '56d8e9005ab1ea6af8e299adc25ac210da7d58f794b00d45d6b7996f4b4d7812')
        for bad in [physical, updates, moderator, profile, current + b' ', current[1:], None, {}, bytearray(current), current.decode()]:
            with self.assertRaises(ValueError): restore_moderator_composition_source(path, bad)
        for bad in [physical, current, updates, profile]:
            with self.assertRaises(ValueError): restore_native_profile_actions_source(path, bad)
        value = moderator_association()
        candidates = [MODERATOR_METADATA + b' ', MODERATOR_METADATA[1:], MODERATOR_METADATA.decode()]
        for key in value:
            changed = copy.deepcopy(value); del changed[key]; candidates.append(encoded(changed))
        for i, row in enumerate(value['rows']):
            for key in row:
                changed = copy.deepcopy(value); del changed['rows'][i][key]; candidates.append(encoded(changed))
            for j, edit in enumerate(row['literal_reversals']):
                for key in edit:
                    changed = copy.deepcopy(value); changed['rows'][i]['literal_reversals'][j][key] += ' '; candidates.append(encoded(changed))
        for candidate in candidates:
            with self.assertRaises(ValueError): moderator_association(candidate)
            with self.assertRaises(ValueError): restore_moderator_composition_source(path, current, candidate)
        for row in value['rows']:
            physical = (ROOT / row['path']).read_bytes()
            input_comparison = reader_preparation_comparison(row['path'], physical)
            previous = restore_moderator_composition_source(row['path'], input_comparison)
            self.assertEqual((ROOT / row['path']).read_bytes(), physical)
            self.assertEqual(digest(previous), row['previous_sha256'])
            self.assertEqual(len(previous), row['previous_bytes'])
            comparison = updates if row['path'] == path else physical
            candidates = [previous, physical + b' ', physical[1:], None, {}, bytearray(physical)]
            for edit in row['literal_reversals']:
                self.assertEqual(comparison.count(edit['current'].encode()), 1)
                candidates.append(comparison.replace(edit['current'].encode(), edit['previous'].encode(), 1))
            for candidate in candidates:
                with self.assertRaises(ValueError): restore_moderator_composition_source(row['path'], candidate)
            with self.assertRaises(ValueError): restore_moderator_composition_source(row['path'] + 'unknown', physical)


class BoundedReaderSourceSuccessors(unittest.TestCase):
    def test_five_exact_combined_stages_and_nineteen_ordered_seams(self):
        value = reader_association()
        self.assertEqual(digest(READER_METADATA), READER_METADATA_SHA256)
        self.assertEqual(value['physical_source_commit'], '9c221a556ed4dbe3fcbda889d67b3f403c5a37d7')
        self.assertEqual(value['physical_source_tree'], '8661c3cb5a6efe1fb71f1da847f04552645b2763')
        self.assertEqual(value['previous_source_commit'], 'f7c598cb57f9a2afbecc107c1d81bb4b30ac4102')
        self.assertEqual(value['row_count'], 5)
        self.assertEqual(value['literal_reversal_count'], 19)
        self.assertEqual([len(row['literal_reversals']) for row in value['rows']], [7, 2, 2, 3, 5])
        self.assertEqual([row['path'] for row in value['rows']], ['weebdex-platform/' + path for path in [
            'scripts/prepare-frontend-integration.mjs', 'scripts/compile-frontend-source.mjs',
            'scripts/check-composed-frontend.mjs', 'scripts/vite.platform.config.mjs',
            'platform-integrations/local-library/library.ts']])
        for flag in ['tests_executed', 'build_executed', 'browser_executed', 'http_executed', 'capacity_qualified', 'external_mounted_preparation_applied']:
            self.assertIs(value[flag], False)
        for row in value['rows']:
            physical = (ROOT / row['path']).read_bytes()
            previous = restore_bounded_reader_source(row['stage'], row['path'], physical)
            self.assertIs(type(previous), bytes)
            self.assertEqual(row['stage'], 'bounded-reader-to-frozen-main:' + row['path'].removeprefix('weebdex-platform/'))
            self.assertEqual((len(physical), digest(physical)), (row['current_bytes'], row['current_sha256']))
            self.assertEqual((len(previous), digest(previous)), (row['previous_bytes'], row['previous_sha256']))
            self.assertEqual([edit['ordinal'] for edit in row['literal_reversals']], list(range(1, len(row['literal_reversals']) + 1)))
            self.assertEqual((ROOT / row['path']).read_bytes(), physical)

    def test_each_input_rejects_historical_partial_duplicate_reformatted_and_reordered_bytes(self):
        value = reader_association()
        for row in value['rows']:
            physical = (ROOT / row['path']).read_bytes()
            previous = restore_bounded_reader_source(row['stage'], row['path'], physical)
            candidates = [previous, physical + b' ', physical[1:], physical[:-1], bytes([physical[0] ^ 1]) + physical[1:], None, {}, [], 0, bytearray(physical), memoryview(physical), physical.decode()]
            for edit in row['literal_reversals']:
                current, old = edit['current'].encode(), edit['previous'].encode()
                self.assertEqual(physical.count(current), 1)
                candidates.extend([physical.replace(current, old, 1), physical + current, physical.replace(current, current + b' ', 1)])
            first, second = [edit['current'].encode() for edit in row['literal_reversals'][:2]]
            marker = b'__READER_COMPARISON_SWAP_SENTINEL__'
            self.assertNotIn(marker, physical)
            candidates.append(physical.replace(first, marker, 1).replace(second, first, 1).replace(marker, second, 1))
            for candidate in candidates:
                with self.assertRaises(ValueError): restore_bounded_reader_source(row['stage'], row['path'], candidate)
            for stage in ['', None, 'unknown', 'consumer-to-frozen-main:' + row['path'], *[other['stage'] for other in value['rows'] if other != row]]:
                with self.assertRaises(ValueError): restore_bounded_reader_source(stage, row['path'], physical)
            for path in ['', None, '../' + row['path'], row['path'] + 'unknown', row['path'].removeprefix('weebdex-platform/'), *[other['path'] for other in value['rows'] if other != row]]:
                with self.assertRaises(ValueError): restore_bounded_reader_source(row['stage'], path, physical)
            self.assertEqual((ROOT / row['path']).read_bytes(), physical)

    def test_complete_metadata_encoding_and_every_nested_fact_fail_closed(self):
        value = reader_association()
        row = value['rows'][0]
        physical = (ROOT / row['path']).read_bytes()
        def reject(metadata):
            with self.assertRaises(ValueError): reader_association(metadata)
            with self.assertRaises(ValueError): restore_bounded_reader_source(row['stage'], row['path'], physical, metadata)
        for metadata in [READER_METADATA.decode(), None, {}, bytearray(READER_METADATA), READER_METADATA[1:], READER_METADATA + b' ', json.dumps(value).encode(), encoded(dict(reversed(list(value.items()))))]:
            reject(metadata)
        def visit(item, keys=()):
            entries = enumerate(item) if isinstance(item, list) else item.items()
            for key, child in entries:
                changed = copy.deepcopy(value)
                parent = changed
                for segment in keys: parent = parent[segment]
                del parent[key]
                reject(encoded(changed))
                if isinstance(child, (dict, list)):
                    visit(child, (*keys, key))
                else:
                    changed = copy.deepcopy(value)
                    parent = changed
                    for segment in keys: parent = parent[segment]
                    parent[key] = child + ' ' if isinstance(child, str) else not child if isinstance(child, bool) else child + 1
                    reject(encoded(changed))
        visit(value)
        for field in ['rows', 'actual_dependency_pins', 'preserved_metadata', 'capsule_dispositions']:
            changed = copy.deepcopy(value); changed[field].reverse(); reject(encoded(changed))
            changed = copy.deepcopy(value); changed[field].append(changed[field][0]); reject(encoded(changed))
        changed = copy.deepcopy(value); changed['rows'][0]['literal_reversals'].reverse(); reject(encoded(changed))
        changed = copy.deepcopy(value); changed['rows'][0]['literal_reversals'].pop(); reject(encoded(changed))
        changed = copy.deepcopy(value); changed['rows'][0]['literal_reversals'].append(changed['rows'][0]['literal_reversals'][0]); reject(encoded(changed))
        for keys in [(), ('rows', 0), ('rows', 0, 'literal_reversals', 0)]:
            changed = copy.deepcopy(value); target = changed
            for key in keys: target = target[key]
            target['unknown'] = True; reject(encoded(changed))

    def test_actual_dependencies_and_retained_capsules_are_independent_physical_pins(self):
        value = reader_association()
        # Strict consumer contract successors preserve the historical metadata exactly.
        strict_successors = {
            'weebdex-platform/platform-integrations/bounded-reader/presentation.ts': ((1455, 'f609e06d87da8529b40c7f21ceddcf8165ca47158161d769f1f125c6e11f4d39'), (1462, '2756117714dfa19955938a7e072b8ffe8d8549b446db3d327b969bd97cf3ab61')),
            'weebdex-platform/platform-integrations/bounded-reader/EditionSelector.svelte': ((481, 'd3d28a1164341dfce9eb9cfad7df90b77c1c3cfae4f2211828b1bf881da0673c'), (555, '1f384b4e5348091db37a0462150e890390ff9e05ed752199abf755a7d9b8e607')),
            'weebdex-platform/platform-integrations/bounded-reader/anchors.json': ((48213, '57a4c1020b4afcb7f66c345d3d0ff070d24cb649a90623a3b3ccd504d56d3e23'), (48293, 'feafb34479f4d81413f44cc24d786beb431e9bebffc6d6be24de1753139d2fae')),
        }
        def assert_physical(pin):
            content = (ROOT / pin['path']).read_bytes()
            if pin['path'] in strict_successors:
                before, after = strict_successors[pin['path']]
                self.assertEqual((pin['bytes'], pin['sha256']), before, pin['path'])
                self.assertEqual((len(content), digest(content)), after, pin['path'])
                return
            if pin['path'] == 'weebdex-platform/scripts/integrated-load/mounted-reader.mjs':
                # Preserve historical metadata; pin the test-harness-only successor separately.
                self.assertEqual((pin['bytes'], pin['sha256']), (44683, 'b6d7b0a15681feada198897b35a4c497dde8b800226a011eba85fe8fe910e977'))
                self.assertEqual((len(content), digest(content)), (45414, 'badb60c9f59b93f2267a5a50bda41364acc7086d7bd2cff4e38a0d8153c998ee'))
                return
            self.assertEqual((len(content), digest(content)), (pin['bytes'], pin['sha256']), pin['path'])
        self.assertEqual(len(value['actual_dependency_pins']), 37)
        for pin in value['actual_dependency_pins']: assert_physical(pin)
        self.assertEqual(len(value['preserved_metadata']), 11)
        for pin in value['preserved_metadata']: assert_physical(pin)
        assert_physical(value['preserved_local_page_metadata'])
        self.assertEqual(value['preserved_local_page_metadata']['bytes'], 73175)
        self.assertEqual(value['preserved_local_page_metadata']['sha256'], 'c62216c07016789f0cbc25dd0277ebddb4ae1a707edefcb22cb0a4d749263d1c')
        self.assertEqual(len(value['capsule_dispositions']), 2)
        forum, mounted = value['capsule_dispositions']
        assert_physical(forum['existing_capsule'])
        self.assertEqual((forum['existing_capsule']['bytes'], forum['existing_capsule']['sha256']), (7346, '1bf6022f610dc312936a20a92e566788191e2210171a1893e3276b526435c4d0'))
        for pin in forum['current_runtime_inputs']: assert_physical(pin)
        self.assertEqual([(pin['bytes'], pin['sha256']) for pin in forum['current_runtime_inputs']], [(2553, '748fc2997a528057400d4d9368aa58b8ab8c0fd1c3f5e4adacf9b5eb11d6b819'), (16458, 'afe757c8eb7391fdb8b77d74bcf885c56849d782559efdb9625976724f5b1b71')])
        self.assertEqual((mounted['historical_source']['bytes'], mounted['historical_source']['sha256']), (21816, 'b99b2a0ebbe1e0b0ffb40f4836091ad8423f0450b27cf0427bd23899462bf851'))
        self.assertEqual((mounted['current_source']['bytes'], mounted['current_source']['sha256']), (44683, 'b6d7b0a15681feada198897b35a4c497dde8b800226a011eba85fe8fe910e977'))
        assert_physical(mounted['current_source'])
        self.assertIn('retained in Git only', mounted['disposition'])
        self.assertNotIn('proposed_capsule_path_if_needed_later', mounted)
        for capsule in [forum, mounted]: self.assertNotIn(capsule['historical_source']['path'], [row['path'] for row in value['rows']])
        guard = value['unchanged_language_guard']
        physical = (ROOT / guard['path']).read_bytes()
        start, end = guard['start_marker_utf8'].encode(), guard['end_marker_utf8'].encode()
        self.assertEqual(physical.count(start), 1); self.assertEqual(physical.count(end), 1)
        extracted = physical[physical.index(start):physical.index(end)]
        self.assertEqual((len(extracted), digest(extracted)), (801, '213da1c168ac55a52403f2520959fa2ea8e7a3d87d8b36e4d559dafc822bd9c0'))

    def test_library_reader_then_history_exact_once(self):
        path = 'weebdex-platform/platform-integrations/local-library/library.ts'
        stage = 'bounded-reader-to-frozen-main:platform-integrations/local-library/library.ts'
        physical = (ROOT / path).read_bytes()
        reader = restore_bounded_reader_source(stage, path, physical)
        history = restore_history_source(path, reader)
        self.assertEqual([len(content) for content in [physical, reader, history]], [37875, 35991, 35984])
        self.assertEqual([digest(content) for content in [physical, reader, history]], ['aba0286cf8cb386d703fa2580f937722fe4e68f383a14b73f3a5ebd5830e1838', '18a90ef68078943f7fbc25bcac80479f0311079f745ef924acd538a00c45cb0e', '7a731e22f8242e278068779393ff1b3b75e3e9306d608598bdb066d5e18a3eef'])
        with self.assertRaises(ValueError): restore_history_source(path, physical)
        for content in [reader, history]:
            with self.assertRaises(ValueError): restore_bounded_reader_source(stage, path, content)
        self.assertEqual((ROOT / path).read_bytes(), physical)

if __name__ == '__main__': unittest.main()
