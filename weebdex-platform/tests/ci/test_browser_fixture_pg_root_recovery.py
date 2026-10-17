"""AST-only PostgreSQL tool-root checks; never import or start the fixture."""
import ast
from pathlib import Path
import unittest


SOURCE = Path(__file__).resolve().parents[1] / 'runtime/browser_fixture.py'
TREE = ast.parse(SOURCE.read_text())
RESOLVER = next(node for node in TREE.body
                if isinstance(node, ast.FunctionDef) and node.name == 'resolve_pg_bin')
NAMESPACE = {'Path': Path, '__builtins__': {'ValueError': ValueError,
                                         'isinstance': isinstance, 'str': str}}
exec(compile(ast.Module(body=[RESOLVER], type_ignores=[]), str(SOURCE), 'exec'), NAMESPACE)
resolve_pg_bin = NAMESPACE['resolve_pg_bin']
FIXTURE = next(node for node in TREE.body
               if isinstance(node, ast.ClassDef) and node.name == 'Fixture')


class PostgresRootTests(unittest.TestCase):
    def test_absent_override_preserves_default(self):
        default = Path('/default/vendor/pgsql/bin')
        self.assertIs(resolve_pg_bin({}, default), default)

    def test_absolute_override_is_selected_without_filesystem_probes(self):
        selected = '/uncreated official tools/pgsql/bin'
        self.assertEqual(resolve_pg_bin({'PLATFORM_TEST_PG_BIN': selected}, Path('/default')),
                         Path(selected))
        self.assertEqual({node.func.id for node in ast.walk(RESOLVER)
                          if isinstance(node, ast.Call) and isinstance(node.func, ast.Name)},
                         {'Path', 'ValueError', 'isinstance'})
        self.assertFalse(any(isinstance(node, (ast.Import, ast.ImportFrom))
                             or isinstance(node, ast.Call) and isinstance(node.func, ast.Attribute)
                             and node.func.attr != 'is_absolute'
                             for node in ast.walk(RESOLVER)))

    def test_invalid_explicit_overrides_never_fall_back(self):
        for value in ['', ' ', '.', 'relative/pgsql/bin', '../pgsql/bin', None, 17,
                      '/absolute/with\0nul']:
            with self.subTest(value=value), self.assertRaisesRegex(ValueError, 'PLATFORM_TEST_PG_BIN'):
                resolve_pg_bin({'PLATFORM_TEST_PG_BIN': value}, Path('/valid/default'))

    def test_shell_metacharacters_are_only_path_characters(self):
        value = '/tools/a; echo should-not-run/$(placeholder)/bin'
        self.assertEqual(str(resolve_pg_bin({'PLATFORM_TEST_PG_BIN': value}, Path('/default'))), value)

    def test_module_uses_explicit_environment_and_original_default(self):
        selected = next(node.value for node in TREE.body
                        if isinstance(node, ast.Assign)
                        and any(isinstance(target, ast.Name) and target.id == 'PG_BIN'
                                for target in node.targets))
        expected = ast.parse("resolve_pg_bin(os.environ, ROOT.parent / 'manga-protocol-lab/vendor/pgsql/bin')",
                             mode='eval').body
        self.assertEqual(ast.dump(selected), ast.dump(expected))

    def test_postgres_commands_stay_argv_based_with_private_auth_and_listener(self):
        methods = {node.name: node for node in FIXTURE.body if isinstance(node, ast.FunctionDef)}
        commands = {}
        for method in ['psql', 'start_pg']:
            for call in ast.walk(methods[method]):
                if not isinstance(call, ast.Call) or not isinstance(call.func, ast.Attribute):
                    continue
                if call.func.attr not in {'run_owned', 'spawn'}:
                    continue
                argv = call.args[1 if call.func.attr == 'spawn' else 0]
                self.assertIsInstance(argv, ast.List)
                executable = argv.elts[0]
                self.assertIsInstance(executable, ast.Call)
                self.assertEqual(executable.func.id, 'str')
                root = executable.args[0]
                self.assertEqual(root.left.id, 'PG_BIN')
                commands[root.right.value] = argv
                self.assertFalse(any(keyword.arg == 'shell' for keyword in call.keywords))
        self.assertEqual(set(commands), {'psql', 'initdb', 'postgres', 'pg_isready'})
        initdb = [node.value for node in commands['initdb'].elts if isinstance(node, ast.Constant)]
        self.assertIn('--auth-local=reject', initdb)
        self.assertIn('--auth-host=trust', initdb)
        postgres = commands['postgres'].elts
        flags = {node.value: index for index, node in enumerate(postgres)
                 if isinstance(node, ast.Constant) and node.value in {'-k', '-h'}}
        self.assertEqual(postgres[flags['-k'] + 1].value, '')
        self.assertEqual(postgres[flags['-h'] + 1].value, '127.0.0.1')

    def test_runtime_environment_keeps_loader_path_and_receipt_records_root(self):
        environment = next(node for node in FIXTURE.body
                           if isinstance(node, ast.FunctionDef) and node.name == 'environment')
        allowed = next(node.value for node in environment.body
                       if isinstance(node, ast.Assign)
                       and any(isinstance(target, ast.Name) and target.id == 'allowed'
                               for target in node.targets))
        self.assertIn('LD_LIBRARY_PATH', {node.value for node in allowed.elts})
        initializer = next(node for node in FIXTURE.body
                           if isinstance(node, ast.FunctionDef) and node.name == '__init__')
        receipt = next(node.value for node in ast.walk(initializer)
                       if isinstance(node, ast.Assign)
                       and any(isinstance(target, ast.Attribute) and target.attr == 'receipt'
                               for target in node.targets))
        values = {key.value: value for key, value in zip(receipt.keys, receipt.values)}
        self.assertEqual(ast.dump(values['postgres_bin_root']), ast.dump(ast.parse('str(PG_BIN)', mode='eval').body))


if __name__ == '__main__':
    unittest.main()
