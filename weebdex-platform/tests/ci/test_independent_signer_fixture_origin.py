"""Actual fixture-origin validation/configuration, without starting any service."""
import ast
import json
import re
import tempfile
import types
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
TREE = ast.parse((ROOT / 'tests/runtime/browser_fixture.py').read_text())
FIXTURE = next(n for n in TREE.body if isinstance(n, ast.ClassDef) and n.name == 'Fixture')


def actual(name, method=False):
    nodes = FIXTURE.body if method else TREE.body
    node = next(n for n in nodes if isinstance(n, ast.FunctionDef) and n.name == name)
    namespace = {'re': re, 'json': json, 'ROOT': ROOT,
                 'write_json': lambda path, value: path.write_text(json.dumps(value))}
    exec(compile(ast.Module(body=[node], type_ignores=[]), '<actual-independent-relay-origin>', 'exec'), namespace)
    return namespace[name]


class IndependentSignerFixtureOrigin(unittest.TestCase):
    def test_absent_flag_preserves_no_extra_capability(self):
        self.assertIsNone(actual('independent_signer_relay_origin')(None))

    def test_exact_unprivileged_ipv4_loopback_origin(self):
        for port in [1024, 45123, 65535]:
            origin = f'ws://127.0.0.1:{port}'
            self.assertEqual(actual('independent_signer_relay_origin')(origin), origin)

    def test_refuses_secrets_paths_dns_external_and_noncanonical_inputs(self):
        check = actual('independent_signer_relay_origin')
        for value in [False, 45123, [], {}, '', 'ws://127.0.0.1:1023', 'ws://127.0.0.1:65536',
                      'ws://127.0.0.1:045123', 'ws://127.0.0.1', 'ws://127.0.0.1:80',
                      'wss://127.0.0.1:45123', 'http://127.0.0.1:45123', 'ws://localhost:45123',
                      'ws://[::1]:45123', 'ws://0.0.0.0:45123', 'ws://192.0.2.1:45123',
                      'ws://127.0.0.1.example.invalid:45123', 'ws://user@127.0.0.1:45123',
                      'ws://127.0.0.1:45123/relay', 'ws://127.0.0.1:45123/',
                      'ws://127.0.0.1:45123?secret=synthetic', 'ws://127.0.0.1:45123#fragment',
                      ' ws://127.0.0.1:45123', 'ws://127.0.0.1:45123\n',
                      'bunker://' + 'a' * 64 + '?relay=ws://127.0.0.1:45123',
                      ['ws://127.0.0.1:45123', 'ws://127.0.0.1:45124']]:
            with self.subTest(value=value), self.assertRaises(ValueError):
                check(value)

    def configure(self, extra):
        directory = tempfile.TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        run = Path(directory.name)
        (run / 'platform.json').write_text(json.dumps({'public_origin': 'http://127.0.0.1:45120'}))
        runtime = {'expected_publisher': 'a' * 64, 'native_node': {'synthetic': True}}
        (run / 'protocol-runtime.json').write_text(json.dumps(runtime))
        calls = []
        def provision(*args, **kwargs):
            calls.append((args, kwargs))
            return {'relay_origin': 'ws://127.0.0.1:45121', 'service_key': 'b' * 64,
                    'operator_pubkey': 'a' * 64}
        fixture = types.SimpleNamespace(run=run, origin='http://127.0.0.1:45120',
            protocol_origin='http://127.0.0.1:45121', independent_signer_relay_origin=extra,
            provision_protocol=provision, receipt={}, ui_regression_controls=False,
            pg_home_fault_controls=False, public_upload_controls=False,
            signed_feature_controls=False, recipient_enabled=False)
        actual('protocol_setup', True)(fixture)
        return json.loads((run / 'platform.json').read_text()), fixture.receipt, calls, runtime

    def test_actual_default_protocol_setup_is_unchanged(self):
        config, receipt, calls, runtime = self.configure(None)
        self.assertEqual(config['protocol_connect_origins'], ['http://127.0.0.1:45121', 'ws://127.0.0.1:45121'])
        self.assertNotIn('independent_signer_relay', receipt)
        self.assertEqual(config['protocol_publication'], runtime)
        self.assertEqual([call[0][0] for call in calls], ['setup'])

    def test_actual_opt_in_adds_only_one_origin_and_no_provisioning_action(self):
        config, receipt, calls, runtime = self.configure('ws://127.0.0.1:45123')
        self.assertEqual(config['protocol_connect_origins'], ['http://127.0.0.1:45121', 'ws://127.0.0.1:45121', 'ws://127.0.0.1:45123'])
        self.assertEqual(config['protocol_publication'], runtime)
        self.assertEqual([call[0][0] for call in calls], ['setup'])
        self.assertEqual(receipt['independent_signer_relay'], {
            'fixture_only': True, 'relay_origin': 'ws://127.0.0.1:45123',
            'separate_supervisor_required': True, 'signer_started_by_fixture': False,
            'private_signer_material_received': False, 'account_authority_granted': False})

    def test_port_reserved_without_starting_signer_or_fixture(self):
        initializer = next(n for n in FIXTURE.body if isinstance(n, ast.FunctionDef) and n.name == '__init__')
        assignment = next(n for n in initializer.body if isinstance(n, ast.Assign)
                          and any(isinstance(t, ast.Name) and t.id == 'ports' for t in n.targets))
        for origin, expected in [(None, set()), ('ws://127.0.0.1:45123', {45123})]:
            self.assertEqual(eval(compile(ast.Expression(assignment.value), '<actual-reserved-ports>', 'eval'),
                                  {'self': types.SimpleNamespace(independent_signer_relay_origin=origin)}), expected)

    def test_cli_gate_is_explicit_and_synthetic_only(self):
        main = next(n for n in TREE.body if isinstance(n, ast.FunctionDef) and n.name == 'main')
        option = next(n for n in ast.walk(main) if isinstance(n, ast.Call)
                      and isinstance(n.func, ast.Attribute) and n.func.attr == 'add_argument'
                      and n.args and isinstance(n.args[0], ast.Constant)
                      and n.args[0].value == '--independent-signer-relay-origin')
        self.assertEqual(next(k.value.id for k in option.keywords if k.arg == 'type'), 'independent_signer_relay_origin')
        gate = next(n for n in main.body if isinstance(n, ast.If)
                    and 'independent signer relay requires a fresh combined synthetic fixture' in ast.unparse(n))
        for enabled, combined, artwork in [(True, False, False), (True, True, True), (True, True, False), (False, False, False)]:
            args = types.SimpleNamespace(independent_signer_relay_origin='ws://127.0.0.1:45123' if enabled else None,
                                         protocol_provisioner=combined, ubunchu_noncommercial=artwork)
            refused = bool(eval(compile(ast.Expression(gate.test), '<actual-cli-gate>', 'eval'), {'args': args}))
            self.assertEqual(refused, enabled and (not combined or artwork))


if __name__ == '__main__':
    unittest.main()
