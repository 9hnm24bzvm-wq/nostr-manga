"""Finite opt-in transport and admission checks. No database/browser is started."""
import ast
import copy
import re
import threading
import time
import types
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SOURCE = (ROOT / 'tests/runtime/browser_fixture.py').read_text()
TREE = ast.parse(SOURCE)
PHASE = 'public-signed-feature-recovery'
CONTROLS = {'signed-feature-account', 'signed-feature-snapshot'}
KEY = 'ab' * 32


def actual(name, method=False):
    nodes = next(n for n in TREE.body if isinstance(n, ast.ClassDef) and n.name == 'Fixture').body if method else TREE.body
    node = next(n for n in nodes if isinstance(n, ast.FunctionDef) and n.name == name)
    ns = {'ROOT': ROOT, 'PUBLIC_SIGNED_FEATURE_PHASE': PHASE,
          'SIGNED_FEATURE_CONTROLS': CONTROLS, 're': re, 'time': time,
          'write_json': lambda *_: None}
    if method:
        ns['signed_feature_payload'] = actual('signed_feature_payload')
    exec(compile(ast.Module(body=[node], type_ignores=[]), '<actual-signed-feature-fixture>', 'exec'), ns)
    return ns[name]


class SignedFeatureFixture(unittest.TestCase):
    def test_phase_requires_separate_exact_opt_in_script_name_env_and_300s_cap(self):
        admit = actual('signed_feature_command')
        args = [PHASE, {'BROWSER_PHASE': PHASE}, ROOT/'scripts/browser-acceptance.mjs', 300, True]
        self.assertTrue(admit(*args))
        self.assertTrue(admit(*args[:3], 15, True))
        self.assertFalse(admit('public-upload-recovery', {'BROWSER_PHASE': 'public-upload-recovery'}, args[2], 600, False))
        for index, bad in [(0, 'another'), (1, {}), (1, {'BROWSER_PHASE': 'another'}),
                           (2, ROOT/'tests/ui/another.mjs'), (3, 301), (3, 14),
                           (3, True), (3, '300'), (4, False), (4, 1)]:
            changed = copy.deepcopy(args)
            changed[index] = bad
            with self.subTest(index=index, bad=bad), self.assertRaises(ValueError):
                admit(*changed)

    def test_payload_accepts_only_public_key_for_two_fixed_actions(self):
        payload = actual('signed_feature_payload')
        for action in CONTROLS:
            self.assertEqual(payload(action, {'pubkey': KEY}), {'pubkey': KEY})
            for bad in [None, [], {}, {'pubkey': KEY, 'private_key': KEY}, {'pubkey': KEY, 'allowed': True},
                        {'pubkey': KEY, 'session_id': 'id'}, {'pubkey': KEY.upper()}, {'pubkey': 'ab'*31},
                        {'pubkey': '0x'+KEY}, {'pubkey': True}, {'pubkey': 123}]:
                with self.subTest(action=action, bad=bad), self.assertRaises(ValueError):
                    payload(action, bad)
        for action in ['account', 'upload-account', 'signed-feature-other', 'source-restore']:
            with self.assertRaises(ValueError):
                payload(action, {'pubkey': KEY})

    def fixture(self):
        calls = []
        def provision(action, payload, timeout):
            calls.append((action, payload, timeout))
            return {'fixture_only': True, 'scenario': 'public-signed-features-v1', 'pubkey': KEY}
        value = types.SimpleNamespace(signed_feature_controls=True, protocol_enabled=True,
            stop=threading.Event(), maintenance=threading.Event(), browser=None,
            browser_kind=None, browser_phase=None, browser_deadline=time.monotonic()+200,
            ready_at=time.time(), receipt={'deadline_unix': time.time()+200,
                'signed_features': {'enabled': True}}, lock=threading.Lock(),
            signed_feature_actions=0, provision_protocol=provision, run=Path('/unused'))
        return value, calls

    def test_exact_control_budget_and_public_only_receipt(self):
        action = actual('signed_feature', method=True)
        fixture, calls = self.fixture()
        action(fixture, 'signed-feature-account', {'pubkey': KEY})
        self.assertTrue(fixture.receipt['signed_features']['enabled'])
        for _ in range(23):
            action(fixture, 'signed-feature-snapshot', {'pubkey': KEY})
        with self.assertRaises(ValueError):
            action(fixture, 'signed-feature-snapshot', {'pubkey': KEY})
        self.assertEqual(len(calls), 24)
        self.assertTrue(all(row[2] == 20 for row in calls))

    def test_disabled_draining_foreign_phase_and_deadline_do_not_start_actions(self):
        action = actual('signed_feature', method=True)
        for change in [lambda f: setattr(f, 'signed_feature_controls', False),
                       lambda f: setattr(f, 'protocol_enabled', False),
                       lambda f: f.stop.set(), lambda f: f.maintenance.set(),
                       lambda f: setattr(f, 'ready_at', None),
                       lambda f: f.receipt.update(deadline_unix=time.time()+20),
                       lambda f: setattr(f, 'browser_kind', 'public-upload-recovery'),
                       lambda f: setattr(f, 'browser_phase', 'alias'),
                       lambda f: setattr(f, 'browser_deadline', time.monotonic()+20)]:
            fixture, calls = self.fixture()
            fixture.browser = types.SimpleNamespace(poll=lambda: None)
            fixture.browser_kind = fixture.browser_phase = PHASE
            change(fixture)
            with self.assertRaises(ValueError):
                action(fixture, 'signed-feature-account', {'pubkey': KEY})
            self.assertEqual(calls, [])

    def test_live_exact_phase_can_observe_but_mismatched_receipts_fail_closed(self):
        action = actual('signed_feature', method=True)
        fixture, calls = self.fixture()
        fixture.browser = types.SimpleNamespace(poll=lambda: None)
        fixture.browser_kind = fixture.browser_phase = PHASE
        self.assertEqual(action(fixture, 'signed-feature-snapshot', {'pubkey': KEY})['pubkey'], KEY)
        for patch in [{'scenario': 'public-upload-v1'}, {'pubkey': 'cd'*32}, {'fixture_only': False}]:
            fixture.provision_protocol = lambda *args, **kwargs: dict(fixture_only=True, scenario='public-signed-features-v1', pubkey=KEY) | patch
            with self.assertRaises(RuntimeError):
                action(fixture, 'signed-feature-account', {'pubkey': KEY})

    def test_cli_marker_and_separate_control_wiring(self):
        self.assertIn("parser.add_argument('--signed-feature-controls',action='store_true'", SOURCE)
        self.assertIn("args.signed_feature_controls and (not args.protocol_provisioner or args.ubunchu_noncommercial)", SOURCE)
        self.assertIn("'signed_feature_controls':self.signed_feature_controls,'synthetic_images':not args.ubunchu_noncommercial", SOURCE)
        self.assertIn('signed_feature_command(phase,options,script,timeout,self.signed_feature_controls)', SOURCE)
        self.assertIn('elif signed_feature:operation=self.signed_feature(action,payload)', SOURCE)
        self.assertIn("if not fixture.signed_feature_controls:return self.respond(404", SOURCE)
        self.assertIn("if origin is not None and origin!=fixture.origin: return self.respond(403", SOURCE)
        # No relaxation of the separate existing upload journey.
        self.assertIn('self.browser_kind!=PUBLIC_UPLOAD_PHASE', SOURCE)
        self.assertIn('(180 if feature_phase else 600 if signed_control_phase else 300)', SOURCE)


if __name__ == '__main__':
    unittest.main()
