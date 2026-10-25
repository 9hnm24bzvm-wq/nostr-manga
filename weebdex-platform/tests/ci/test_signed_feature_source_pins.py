"""Pinned reviewed source, not a claim of browser execution."""
import hashlib
import json
from pathlib import Path
import unittest
from reviewed_integration_source_successors import PATHS as INTEGRATION_PATHS, restore_integrated_source
from reviewed_history_source_successors import PATHS, restore_history_source
from reviewed_local_page_source_successors import PATHS as LOCAL_PAGE_PATHS, restore_local_page_source
ROOT = Path(__file__).resolve().parents[3]
PIN = ROOT / 'weebdex-platform/test-evidence/public-signed-features-20261022/source-pins.json'
class SignedFeaturePins(unittest.TestCase):
    def test_exact_reviewed_source_associations_preserve_historical_pins(self):
        value = json.loads(PIN.read_text())
        self.assertEqual(set(value), {'schema', 'scope', 'browser_executed', 'hosted_executed', 'sources'})
        self.assertEqual(value['schema'], 1)
        self.assertEqual(value['scope'], 'prospective-public-signed-features-v1')
        self.assertIs(value['browser_executed'], False)
        self.assertIs(value['hosted_executed'], False)
        self.assertEqual(len(value['sources']), 40)
        seen = set()
        for row in value['sources']:
            self.assertEqual(set(row), {'path', 'bytes', 'sha256'})
            path = Path(row['path'])
            self.assertFalse(path.is_absolute())
            self.assertNotIn('..', path.parts)
            self.assertNotIn(str(path), seen)
            seen.add(str(path))
            content = (ROOT / path).read_bytes()
            if str(path) in LOCAL_PAGE_PATHS:
                content = restore_local_page_source(str(path), content)
            if str(path) in INTEGRATION_PATHS:
                content = restore_integrated_source(str(path), content)
            if str(path) in PATHS:
                content = restore_history_source(str(path), content)
            self.assertEqual(len(content), row['bytes'], str(path))
            self.assertEqual(hashlib.sha256(content).hexdigest(), row['sha256'], str(path))
if __name__ == '__main__':
    unittest.main()
