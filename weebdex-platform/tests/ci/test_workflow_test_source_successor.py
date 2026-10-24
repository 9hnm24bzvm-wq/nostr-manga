"""Keep the historical signed-feature pinset distinct from one reviewed successor."""
import copy
import hashlib
import json
from pathlib import Path
import unittest
from reviewed_workflow_source_successors import restore_workflow_source

ROOT = Path(__file__).resolve().parents[3]
EVIDENCE = ROOT / 'weebdex-platform/test-evidence/public-signed-features-20261022'
PATH = 'weebdex-platform/tests/ci/test_native_upload_diagnostic_workflow.py'


class WorkflowTestSuccessor(unittest.TestCase):
    def test_exact_one_file_successor_preserves_historical_pinset_and_rejects_unknown_source(self):
        historical_bytes = (EVIDENCE / 'source-pins-initial.json').read_bytes()
        association = json.loads((EVIDENCE / 'workflow-test-successor.json').read_text())
        self.assertEqual(set(association), {'schema', 'scope', 'browser_executed', 'hosted_executed', 'historical_pinset', 'historical_pinset_sha256', 'current_pinset', 'current_pinset_sha256', 'source_commit', 'changes'})
        self.assertEqual(association['schema'], 1)
        self.assertEqual(association['scope'], 'prospective-public-signed-features-workflow-test-successor-v1')
        self.assertIs(association['browser_executed'], False)
        self.assertIs(association['hosted_executed'], False)
        self.assertEqual(association['source_commit'], 'c5ff2170a7d2624abe238528ae960d3bef8c916f')
        self.assertEqual(association['historical_pinset'], 'source-pins-initial.json')
        self.assertEqual(association['current_pinset'], 'source-pins.json')
        self.assertEqual(hashlib.sha256(historical_bytes).hexdigest(), '3c41c318d9bffdacfe613c2fbc7a03371dc5604e461d2aa3024ce1da154ed301')
        self.assertEqual(association['historical_pinset_sha256'], hashlib.sha256(historical_bytes).hexdigest())
        self.assertEqual(len(association['changes']), 1)
        change = association['changes'][0]
        self.assertEqual(set(change), {'prior', 'current', 'reason'})
        self.assertEqual(change['prior'], {'path': PATH, 'bytes': 8116, 'sha256': 'dd0b7513878eb2b917017983830135de78f7cf6187191aa2236b000ec63b9549'})
        self.assertEqual(change['current'], {'path': PATH, 'bytes': 12392, 'sha256': '194289c9cc71deca014700c46d77e8cc983ac555e2aca903a79513907826664b'})
        self.assertIn('no workflow or product edit', change['reason'])
        historical = json.loads(historical_bytes)
        expected = copy.deepcopy(historical)
        rows = [index for index, row in enumerate(expected['sources']) if row == change['prior']]
        self.assertEqual(len(rows), 1)
        expected['sources'][rows[0]] = change['current']
        # This immutable receipt describes the earlier workflow-only successor,
        # not a later integrated pinset kept at the same current filename.
        current_bytes = (json.dumps(expected, indent=2) + '\n').encode()
        self.assertEqual(association['current_pinset_sha256'], '07a8f07ff187cd49a8ec93a33309021021b9e8e70ccc139b7e4fab47a81bb790')
        self.assertEqual(hashlib.sha256(current_bytes).hexdigest(), association['current_pinset_sha256'])
        content = (ROOT / PATH).read_bytes()
        # The current compatibility test is a later exact reviewed source.
        # Compare genuine historical bytes only after admitting its full digest.
        historical_content = restore_workflow_source(PATH, content)
        self.assertEqual(len(historical_content), change['current']['bytes'])
        self.assertEqual(hashlib.sha256(historical_content).hexdigest(), change['current']['sha256'])
        for data, admitted in [(content, True), (content + b'\n# unreviewed\n', False), (content[:-1], False)]:
            if admitted:
                data = restore_workflow_source(PATH, data)
            else:
                with self.assertRaises(ValueError):
                    restore_workflow_source(PATH, data)
            self.assertEqual(len(data) == change['current']['bytes'] and hashlib.sha256(data).hexdigest() == change['current']['sha256'], admitted)
