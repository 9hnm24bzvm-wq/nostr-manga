"""Current bytes to reviewed bytes are comparison evidence, never execution."""
from reviewed_local_page_source_successors import DRIVER, restore_local_page_source
from reviewed_bundle_source_successors import UPDATES_INTEGRATION_PATHS, restore_bundle_source
from native_profile_actions_source_association import PATHS as PROFILE_PATHS, restore_native_profile_actions_source
import copy
import json
import unittest
from reviewed_integration_source_successors import ROOT, METADATA, METADATA_SHA256, PATHS, association, digest, restore_integrated_source

def encoded(value):
    return (json.dumps(value, indent=2, ensure_ascii=False) + '\n').encode()

class IntegratedSourceSuccessors(unittest.TestCase):
    def test_exact_seven_comparison_rows_and_non_execution_claims(self):
        value=association()
        self.assertEqual(digest(METADATA), METADATA_SHA256)
        self.assertEqual(len(PATHS),7)
        self.assertEqual([row['kind'] for row in value['rows']], ['product_source']*6+['comparison_driver'])
        self.assertEqual(value['integrated_product_source'],'11ea3ea9f593c134f2ba478bb84fa683f4b14149')
        for flag in ['browser_executed','http_executed','capacity_qualified']:self.assertIs(value[flag],False)
        for row in value['rows']:
            physical=(ROOT/row['path']).read_bytes()
            associated=restore_local_page_source(row['path'],physical) if row['path']==DRIVER else physical
            previous=restore_integrated_source(row['path'],associated)
            current=restore_native_profile_actions_source(row['path'],associated) if row['path'] in PROFILE_PATHS else restore_bundle_source('updates-integration',row['path'],associated) if row['path'] in UPDATES_INTEGRATION_PATHS else associated
            self.assertEqual((ROOT/row['path']).read_bytes(),physical)
            self.assertEqual(len(current),row['current_bytes']);self.assertEqual(digest(current),row['current_sha256'])
            self.assertEqual(len(previous),row['previous_bytes']);self.assertEqual(digest(previous),row['previous_sha256']);self.assertNotEqual(previous,current)
    def test_unknown_sources_paths_and_each_partial_literal_fail_closed(self):
        for row in association()['rows']:
            physical=(ROOT/row['path']).read_bytes()
            associated=restore_local_page_source(row['path'],physical) if row['path']==DRIVER else physical
            previous=restore_integrated_source(row['path'],associated)
            current=restore_native_profile_actions_source(row['path'],associated) if row['path'] in PROFILE_PATHS else restore_bundle_source('updates-integration',row['path'],associated) if row['path'] in UPDATES_INTEGRATION_PATHS else associated
            self.assertEqual((ROOT/row['path']).read_bytes(),physical)
            candidates=[previous,current+b' ',current[1:],bytes([current[0]^1])+current[1:],None,bytearray(current),current.decode()]
            for edit in row['literal_reversals']:
                self.assertEqual(current.count(edit['current'].encode()),1)
                candidates.extend([current.replace(edit['current'].encode(),edit['previous'].encode(),1),current+edit['current'].encode()])
            for candidate in candidates:
                with self.subTest(path=row['path']),self.assertRaises(ValueError):restore_integrated_source(row['path'],candidate)
            for path in [row['path']+'unknown','',None,'../'+row['path']]:
                with self.assertRaises(ValueError):restore_integrated_source(path,current)
    def test_full_metadata_digest_including_all_fields_and_literal_bytes(self):
        value=association();candidates=[METADATA+b' ',METADATA[1:],METADATA.decode(),encoded(dict(reversed(list(value.items()))))]
        for key in value:
            changed=copy.deepcopy(value);del changed[key];candidates.append(encoded(changed))
            changed=copy.deepcopy(value);changed[key]='unknown';candidates.append(encoded(changed))
        for i,row in enumerate(value['rows']):
            for key in row:
                changed=copy.deepcopy(value);del changed['rows'][i][key];candidates.append(encoded(changed))
            for j,edit in enumerate(row['literal_reversals']):
                for key in edit:
                    changed=copy.deepcopy(value);changed['rows'][i]['literal_reversals'][j][key]+=' ';candidates.append(encoded(changed))
        changed=copy.deepcopy(value);changed['rows'].reverse();candidates.append(encoded(changed))
        changed=copy.deepcopy(value);changed['unknown']=True;candidates.append(encoded(changed))
        current=(ROOT/value['rows'][0]['path']).read_bytes()
        for candidate in candidates:
            with self.assertRaises(ValueError):association(candidate)
            with self.assertRaises(ValueError):restore_integrated_source(value['rows'][0]['path'],current,candidate)
if __name__=='__main__':unittest.main()
