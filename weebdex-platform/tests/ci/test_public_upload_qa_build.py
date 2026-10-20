"""Finite synthetic artifact admission controls; no Vite/browser/build runs."""
import copy
import importlib.util
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest import mock

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location('finite_upload_qa_build', ROOT / 'scripts/ci/public-upload-qa-build.py')
qa = importlib.util.module_from_spec(spec)
spec.loader.exec_module(qa)


def emit_graph(root, ui):
    """Explicit stand-in, never a receipt claiming actual Vite graph identity."""
    for name in qa.SOURCES.values():
        path = root / name
        if not path.exists():
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text('synthetic pinned source ' + name)
    chunks = []
    entries = [('_app/immutable/entry/app.js', [], ['_app/immutable/chunks/client.js'], False, False, False),
               ('_app/immutable/chunks/client.js', ['_app/immutable/chunks/transport.js'], [], False, True, False),
               ('_app/immutable/chunks/transport.js', [], [], True, False, False),
               (qa.READER, ['_app/immutable/chunks/transport.js'], [], False, False, True)]
    for name, imports, dynamic, transport, client, reader in entries:
        path = ui / name
        path.parent.mkdir(parents=True, exist_ok=True)
        data = ('// explicit finite chunk ' + name).encode()
        path.write_bytes(data)
        chunks.append({'file_name': name, 'sha256': qa.sha(data), 'imports': imports,
                       'dynamic_imports': dynamic, 'exports': [qa.EXPORT] if reader else [],
                       'contains_transport': transport, 'contains_client': client, 'contains_reader': reader})
    graph = {'schema': 1, 'enabled': True, 'reader_url': '/' + qa.READER,
             'graph_url': '/' + qa.GRAPH, 'reader_export': qa.EXPORT,
             'transport_module_id': qa.SOURCES['transport'], 'client_module_id': qa.SOURCES['client'],
             'transport_module_count': 1, 'client_module_count': 1, 'reader_module_count': 1,
             'reader_chunk': qa.READER, 'transport_chunk': entries[2][0], 'client_chunk': entries[1][0],
             'reader_reaches_transport': True, 'client_reaches_transport': True,
             'client_entry_chunks': [entries[0][0]], 'entry_reaches_client': True, 'chunks': chunks}
    for kind, name in qa.SOURCES.items():
        graph[kind + '_source_sha256'] = qa.sha((root / name).read_bytes())
    write_graph(ui, graph)
    return graph


def write_graph(ui, graph):
    path = ui / qa.GRAPH
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(graph))


class UploadQaBuild(unittest.TestCase):
    def test_name_refusal_conjuncts_have_fixed_private_codes(self):
        cases = [('private-marker', 'list-type'),
                 (['n' + str(index) + '.js' for index in range(129)], 'entry-cap'),
                 (['okay.js', {'private-marker': True}], 'member-type'),
                 (['same.js', 'same.js'], 'duplicate')]
        for value, code in cases:
            with self.subTest(code=code), self.assertRaisesRegex(
                    RuntimeError, '^qa-graph-names-' + code + '$') as caught:
                qa.names(value)
            self.assertNotIn('private-marker', str(caught.exception))

    def test_name_fixed_codes_keep_type_cap_uniqueness_and_path_gates(self):
        valid = ['n' + str(index) + '.js' for index in range(128)]
        self.assertIs(qa.names(valid), valid)
        for value in [None, True, (), set(), {}, 1]:
            with self.subTest(kind=type(value).__name__), self.assertRaisesRegex(
                    RuntimeError, '^qa-graph-names-list-type$'):
                qa.names(value)
        for value in [[True], [1], [None], [[]]]:
            with self.subTest(value=value), self.assertRaisesRegex(
                    RuntimeError, '^qa-graph-names-member-type$'):
                qa.names(value)
        with self.assertRaisesRegex(RuntimeError, '^qa-graph-names-entry-cap$'):
            qa.names(['a.js', 'b.js', 'c.js'], 2)
        for value in [['../private-marker'], ['/private-marker'], ['a//private-marker']]:
            with self.subTest(value=value), self.assertRaisesRegex(
                    RuntimeError, '^Current upload QA build evidence was refused$'):
                qa.names(value)

    def fixture(self, directory):
        root = Path(directory)
        ui = root / 'artifacts/frontend'
        ui.mkdir(parents=True)
        (ui / 'index.html').write_text('synthetic generation')
        return root, ui, emit_graph(root, ui)

    def test_only_absent_zero_and_one_gates_are_admitted(self):
        self.assertFalse(qa.gate(None))
        self.assertFalse(qa.gate('0'))
        self.assertTrue(qa.gate('1'))
        for value in ['', 'true', '01', ' 1', 1, True, [], {}]:
            with self.subTest(value=value), self.assertRaises(RuntimeError):
                qa.gate(value)

    def test_caller_environment_cannot_enable_the_native_generation(self):
        for value in [None, '0', '1']:
            original = {'UNRELATED': 'preserved'}
            if value is not None:
                original['WEEBDEX_QA_UPLOAD_REFUSALS'] = value
            before = dict(original)
            self.assertEqual(qa.native_environment(original),
                             {'UNRELATED': 'preserved', 'WEEBDEX_QA_UPLOAD_REFUSALS': '0'})
            self.assertEqual(original, before)
        with self.assertRaises(RuntimeError):
            qa.native_environment({'WEEBDEX_QA_UPLOAD_REFUSALS': 'unknown'})

    def test_synthetic_current_graph_returns_exact_actual_member_hashes(self):
        with tempfile.TemporaryDirectory() as directory:
            root, ui, _ = self.fixture(directory)
            self.assertEqual(qa.verify(root, ui, True), {
                'schema': 1, 'graph_sha256': qa.sha((ui / qa.GRAPH).read_bytes()),
                'reader_sha256': qa.sha((ui / qa.READER).read_bytes())})

    def test_missing_and_duplicate_and_malformed_graph_refuse(self):
        for content in [None, b'{', b'{"schema":1,"schema":1}', b'[]', b'\xff']:
            with self.subTest(content=content), tempfile.TemporaryDirectory() as directory:
                root, ui, _ = self.fixture(directory)
                path = ui / qa.GRAPH
                if content is None:
                    path.unlink()
                else:
                    path.write_bytes(content)
                with self.assertRaises((RuntimeError, FileNotFoundError)):
                    qa.verify(root, ui, True)

    def test_unknown_wrong_typed_and_forged_fixed_fields_refuse(self):
        changes = [('extra', True), ('schema', True), ('enabled', 1),
                   ('reader_url', '/elsewhere.js'), ('reader_export', 'clearRing'),
                   ('transport_module_id', '../transport.ts'), ('transport_module_count', 2),
                   ('client_module_count', True), ('reader_module_count', 0),
                   ('entry_reaches_client', False), ('reader_reaches_transport', False)]
        for key, value in changes:
            with self.subTest(key=key), tempfile.TemporaryDirectory() as directory:
                root, ui, graph = self.fixture(directory)
                graph[key] = value
                write_graph(ui, graph)
                with self.assertRaises(RuntimeError):
                    qa.verify(root, ui, True)

    def test_all_four_current_source_hashes_are_required(self):
        for kind, name in qa.SOURCES.items():
            with self.subTest(kind=kind), tempfile.TemporaryDirectory() as directory:
                root, ui, _ = self.fixture(directory)
                path = root / name
                path.write_bytes(path.read_bytes() + b' changed')
                with self.assertRaises(RuntimeError):
                    qa.verify(root, ui, True)

    def test_actual_reader_and_transitive_chunk_bytes_cannot_change(self):
        for name in [qa.READER, '_app/immutable/chunks/transport.js']:
            with self.subTest(name=name), tempfile.TemporaryDirectory() as directory:
                root, ui, _ = self.fixture(directory)
                path = ui / name
                path.write_bytes(path.read_bytes() + b' changed')
                with self.assertRaises(RuntimeError):
                    qa.verify(root, ui, True)

    def test_one_shared_module_marker_and_static_reader_edge_are_required(self):
        for kind in ['duplicate', 'missing', 'reader-dynamic-only', 'client-disconnected', 'entry-disconnected']:
            with self.subTest(kind=kind), tempfile.TemporaryDirectory() as directory:
                root, ui, graph = self.fixture(directory)
                app, client, transport, reader = graph['chunks']
                if kind == 'duplicate':
                    client['contains_transport'] = True
                elif kind == 'missing':
                    transport['contains_transport'] = False
                elif kind == 'reader-dynamic-only':
                    reader['dynamic_imports'], reader['imports'] = reader['imports'], []
                elif kind == 'client-disconnected':
                    client['imports'] = []
                else:
                    app['dynamic_imports'] = []
                write_graph(ui, graph)
                with self.assertRaises(RuntimeError):
                    qa.verify(root, ui, True)

    def test_unsafe_duplicate_missing_import_and_extra_reader_export_refuse(self):
        for kind in ['traversal', 'duplicate', 'missing-import', 'extra-export', 'duplicate-import']:
            with self.subTest(kind=kind), tempfile.TemporaryDirectory() as directory:
                root, ui, graph = self.fixture(directory)
                if kind == 'traversal':
                    graph['chunks'][0]['file_name'] = '../outside.js'
                elif kind == 'duplicate':
                    graph['chunks'].append(copy.deepcopy(graph['chunks'][0]))
                elif kind == 'missing-import':
                    graph['chunks'][0]['dynamic_imports'] = ['missing.js']
                elif kind == 'extra-export':
                    graph['chunks'][-1]['exports'].append('clearRing')
                else:
                    graph['chunks'][-1]['imports'] *= 2
                write_graph(ui, graph)
                with self.assertRaises(RuntimeError):
                    qa.verify(root, ui, True)

    def test_linked_graph_reader_and_parent_directory_are_refused(self):
        for name in [qa.GRAPH, qa.READER, '_app/immutable/chunks/transport.js']:
            with self.subTest(name=name), tempfile.TemporaryDirectory() as directory:
                root, ui, _ = self.fixture(directory)
                path = ui / name
                other = root / 'external'
                other.write_bytes(path.read_bytes())
                path.unlink()
                path.symlink_to(other)
                with self.assertRaises(RuntimeError):
                    qa.verify(root, ui, True)

    def test_oversized_graph_and_chunk_refuse_before_unbounded_reads(self):
        self.assertEqual(qa.MAX_GRAPH, 2 * 1024 * 1024)
        self.assertEqual(qa.MAX_JS, 32 * 1024 * 1024)
        # Keep the synthetic boundary fixture below the finite process file cap.
        # The production constants above and actual admission code stay intact.
        for target, constant in [(qa.GRAPH, 'MAX_GRAPH'), (qa.READER, 'MAX_JS')]:
            with self.subTest(target=target), tempfile.TemporaryDirectory() as directory, mock.patch.object(qa, constant, 4096):
                root, ui, _ = self.fixture(directory)
                with (ui / target).open('wb') as stream:
                    stream.truncate(4097)
                with self.assertRaises(RuntimeError):
                    qa.verify(root, ui, True)

    def test_native_default_off_refuses_reader_graph_and_sentinel_leaks(self):
        for leak in [None, qa.READER, qa.GRAPH, 'getter', 'marker']:
            with self.subTest(leak=leak), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                ui = root / 'ui'
                ui.mkdir()
                (ui / 'app.js').write_text('ordinary synthetic client')
                if leak in [qa.READER, qa.GRAPH]:
                    target = ui / leak
                    target.parent.mkdir(parents=True, exist_ok=True)
                    target.write_text('{}')
                elif leak:
                    (ui / 'app.js').write_text(qa.EXPORT if leak == 'getter' else 'QA_UPLOAD_REFUSAL_RECORDING')
                if leak is None:
                    self.assertEqual(qa.verify(root, ui, False), {'schema': 1, 'enabled': False})
                else:
                    with self.assertRaises(RuntimeError):
                        qa.verify(root, ui, False)


if __name__ == '__main__':
    unittest.main()
