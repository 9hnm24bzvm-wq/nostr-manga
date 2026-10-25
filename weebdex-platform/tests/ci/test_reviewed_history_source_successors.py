from reviewed_local_page_source_successors import DRIVER, restore_local_page_source
from reviewed_bounded_reader_source_successors import restore_bounded_reader_source
import unittest
from reviewed_integration_source_successors import PATHS as INTEGRATION_PATHS, restore_integrated_source
from reviewed_history_source_successors import ROOT, METADATA, HISTORY_PATHS as PATHS, PATHS as ALL_PATHS, association, restore_history_source
from reviewed_workflow_source_successors import PATHS as WORKFLOW_PATHS
class HistorySourceSuccessors(unittest.TestCase):
    def test_exact_four_reconstructions_and_fail_closed_inputs(self):
        self.assertEqual(len(PATHS),4)
        self.assertEqual(PATHS,tuple(row['path'] for row in association()['rows']))
        self.assertFalse(set(PATHS).intersection(WORKFLOW_PATHS))
        self.assertEqual(ALL_PATHS,PATHS+WORKFLOW_PATHS)
        for row in association()['rows']:
            physical=(ROOT/row['path']).read_bytes()
            current=restore_local_page_source(row['path'],physical) if row['path']==DRIVER else physical
            if row['path']=='weebdex-platform/platform-integrations/local-library/library.ts': current=restore_bounded_reader_source('bounded-reader-to-frozen-main:platform-integrations/local-library/library.ts',row['path'],current)
            if row['path'] in INTEGRATION_PATHS: current=restore_integrated_source(row['path'],current)
            previous=restore_history_source(row['path'],current)
            self.assertEqual((ROOT/row['path']).read_bytes(),physical)
            self.assertEqual(len(previous),row['previous_bytes'])
            for bad in [previous,current+b' ',current[1:]]:
                with self.assertRaises(ValueError):restore_history_source(row['path'],bad)
            with self.assertRaises(ValueError):restore_history_source(row['path']+'unknown',current)
            with self.assertRaises(ValueError):restore_history_source(row['path'],current,METADATA+b' ')
    def test_metadata_claims_remain_non_execution(self):
        value=association();self.assertFalse(value['browser_executed']);self.assertFalse(value['capacity_qualified'])
        self.assertEqual(value['reviewed_product'],'eeafdcdad2abcef4cc96bb11086c89da774942d3')
if __name__=='__main__':unittest.main()
