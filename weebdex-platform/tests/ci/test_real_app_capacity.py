"""Finite source/admission/stream seams only. No socket, service or browser."""
import importlib.util
import io
import json
import os
from pathlib import Path
import tempfile
import unittest
from email.message import Message
from http.client import HTTPResponse
from unittest.mock import patch
from urllib.request import Request

SCRIPT = Path(__file__).resolve().parents[2] / 'scripts/ci/qualify-real-app-capacity.py'
spec = importlib.util.spec_from_file_location('real_capacity_tested', SCRIPT)
subject = importlib.util.module_from_spec(spec)
spec.loader.exec_module(subject)


class Response:
    def __init__(self, data=b'actual fixture seam', status=200):
        self.data = io.BytesIO(data); self.size = len(data); self.status = status; self.headers = {}

    def read(self, amount):
        return self.data.read(amount)

    def isclosed(self):
        return self.data.tell() == self.size


class Connection:
    def __init__(self, response):
        self.response = response; self.requests = []; self.closed = False

    def request(self, method, target, body, headers):
        self.requests.append((method, target, body, headers))

    def getresponse(self):
        return self.response

    def close(self):
        self.closed = True


class CapacityPreparationTests(unittest.TestCase):
    def test_import_and_plan_are_inert_and_keep_required_boundaries(self):
        with patch.object(subject, 'module', side_effect=AssertionError('Runtime imported')), patch.object(subject.subprocess, 'check_output', side_effect=AssertionError('Process launched')):
            result = subject.plan()
        self.assertEqual(result['status'], 'prepared-not-executed')
        self.assertFalse(result['capacity_qualified'])
        self.assertFalse(result['runtime_qualified'])
        self.assertEqual(sum(result['proposed_sections_seconds'].values()), 600)
        limits = result['limits']
        self.assertEqual(limits['setup_http_attempts'] + limits['browser_http_attempts'], 2500)
        self.assertEqual(limits['setup_response_bytes'] + limits['browser_response_bytes'], 512 * 1024 ** 2)
        self.assertEqual(limits['launch_free_bytes'], 4 * 1024 ** 3 + 256 * 1024 ** 2)
        self.assertTrue(any('30K/500K' in value for value in result['separate_unimplemented']))

    def test_old_nondurable_fixture_flags_are_changed_before_spawn_and_drift_refused(self):
        original = ['/owned/postgres', '-D', '/private/data', '-c', 'fsync=off', '-c', 'full_page_writes=off']
        actual = subject.durable_pg_command(original)
        self.assertEqual(actual, ['/owned/postgres', '-D', '/private/data', '-c', 'fsync=on', '-c', 'full_page_writes=on', '-c', 'synchronous_commit=on'])
        self.assertIn('fsync=off', original)
        for command in [original + ['fsync=off'], original[:-2], original + ['-c', 'synchronous_commit=off'], ['postgres', 'fsync=off', '-c', 'full_page_writes=off']]:
            with self.subTest(command=command), self.assertRaises(subject.Refused):
                subject.durable_pg_command(command)

    def test_setup_preserves_real_encoded_target_body_and_stream_counts(self):
        connection = Connection(Response(b'abcdef'))
        owner = subject.SetupHTTP(['http://127.0.0.1:32123'], connect=lambda *_args, **_kwargs: connection)
        request = Request('http://127.0.0.1:32123/a?ids%5B%5D=x', data=b'{}', headers={'Content-Type': 'application/json'}, method='POST')
        with owner.open(request) as response:
            self.assertEqual(response.read(2), b'ab')
        # Unread remainder is charged and drained, never omitted.
        self.assertEqual(owner.bytes, 6); self.assertEqual(owner.upload, 2); self.assertEqual(owner.active, 0)
        self.assertTrue(owner.snapshot()['all_settled']); self.assertTrue(connection.closed)
        self.assertEqual(connection.requests[0][:3], ('POST', '/a?ids%5B%5D=x', b'{}'))
        encoded = json.dumps(owner.snapshot())
        self.assertNotIn('127.0.0.1', encoded); self.assertNotIn('ids%5B', encoded); self.assertNotIn('abcdef', encoded)

    def test_setup_cannot_read_one_extra_body_byte_past_global_credit(self):
        connection = Connection(Response(b'abc'))
        owner = subject.SetupHTTP(['http://127.0.0.1:32123'], connect=lambda *_args, **_kwargs: connection)
        owner.bytes = 64 * 1024 ** 2 - 2
        with self.assertRaisesRegex(subject.Refused, 'response-limit'):
            with owner.open('http://127.0.0.1:32123/a') as response:
                response.read()
        self.assertEqual(connection.response.data.tell(), 2)
        self.assertEqual(owner.active, 0); self.assertTrue(connection.closed)
        self.assertFalse(owner.records[0]['complete']); self.assertTrue(owner.records[0]['terminal'])

    def test_setup_redirect_external_authority_and_attempt_exhaustion_are_not_followed(self):
        connection = Connection(Response(b'', status=302))
        owner = subject.SetupHTTP(['http://127.0.0.1:32123'], connect=lambda *_args, **_kwargs: connection)
        with self.assertRaisesRegex(subject.Refused, 'redirect'):
            owner.open('http://127.0.0.1:32123/a')
        self.assertEqual(owner.records[0]['status'], 302); self.assertEqual(len(connection.requests), 1)
        for target in ['https://example.invalid/a', 'http://user@127.0.0.1:32123/a', 'http://127.0.0.1:32123/a#x']:
            with self.assertRaisesRegex(subject.Refused, 'origin'):
                owner.open(target)
        owner.records = [{}] * 300
        with self.assertRaisesRegex(subject.Refused, 'attempts'):
            owner.open('http://127.0.0.1:32123/a')

    def test_setup_connection_refusal_remains_terminal_failed_attempt_not_invented_http_status(self):
        connection = Connection(Response())
        with patch.object(connection, 'getresponse', side_effect=ConnectionRefusedError):
            owner = subject.SetupHTTP(['http://127.0.0.1:32123'], connect=lambda *_args, **_kwargs: connection)
            with self.assertRaises(ConnectionRefusedError):
                owner.open('http://127.0.0.1:32123/a')
        self.assertEqual(owner.active, 0); self.assertIsNone(owner.records[0]['status'])
        self.assertTrue(owner.snapshot()['all_settled']); self.assertEqual(owner.records[0]['outcome'], 'connection-or-header-failure')

    def test_worker_cannot_bypass_outer_owner_birth_gate(self):
        with patch.dict(os.environ, {}, clear=True), self.assertRaisesRegex(subject.Refused, 'outer-owner-required'):
            subject.require_outer_owner()
        with patch.dict(os.environ, {'WEEBDEX_CAPACITY_OWNER': '1:2'}, clear=True), patch.object(subject.os, 'getppid', return_value=1), patch.object(subject, 'process_birth', return_value=3), self.assertRaisesRegex(subject.Refused, 'outer-owner-birth'):
            subject.require_outer_owner()

    def test_cgroup_unknown_and_changed_values_fail_closed(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / 'memory.max').write_text('1000'); (root / 'memory.current').write_text('400')
            self.assertEqual(subject.cgroup_headroom(root), 600)
            for value in ['max', '-1', '200']:
                (root / 'memory.max').write_text(value)
                with self.assertRaises(subject.Refused):
                    subject.cgroup_headroom(root)

    def test_setup_declared_length_short_overrun_split_and_ambiguous_framing(self):
        for length, expected in [('3', 'incomplete'), ('1', 'overrun')]:
            response = Response(b'{}'); response.headers = {'Content-Length': length}
            connection = Connection(response)
            owner = subject.SetupHTTP(['http://127.0.0.1:32123'], connect=lambda *_args, **_kwargs: connection)
            with self.assertRaisesRegex(subject.Refused, 'setup-length-' + expected):
                with owner.open('http://127.0.0.1:32123/a') as actual:
                    actual.read(65536)
            self.assertFalse(owner.records[0]['complete']); self.assertTrue(owner.records[0]['terminal'])
            self.assertEqual(owner.active, 0); self.assertTrue(connection.closed)
        response = Response(b'{}'); response.headers = {'Content-Length': '2'}
        connection = Connection(response)
        owner = subject.SetupHTTP(['http://127.0.0.1:32123'], connect=lambda *_args, **_kwargs: connection)
        with owner.open('http://127.0.0.1:32123/a') as actual:
            self.assertEqual(actual.read(1), b'{'); self.assertFalse(owner.records[0]['complete'])
            self.assertEqual(actual.read(1), b'}')
        self.assertTrue(owner.records[0]['complete'])
        headers = Message(); headers.add_header('Content-Length', '2'); headers.add_header('Content-Length', '2')
        for value in [headers, {'Content-Length': '2', 'Transfer-Encoding': 'chunked'}]:
            response = Response(b'{}'); response.headers = value; connection = Connection(response)
            owner = subject.SetupHTTP(['http://127.0.0.1:32123'], connect=lambda *_args, **_kwargs: connection)
            with self.assertRaises(subject.Refused):
                owner.open('http://127.0.0.1:32123/a')
            self.assertTrue(owner.records[0]['terminal']); self.assertEqual(owner.active, 0)

    def test_admitted_config_snapshot_ignores_original_swap_and_rejects_snapshot_tamper(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            original = root / 'original.json'
            value = {key: '/absolute/' + key for key in subject.FIELDS}
            for key in ('pins_sha256', 'build_association_sha256', 'functional_gate_sha256'):
                value[key] = 'a' * 64
            value['source_commit'] = 'b' * 40
            original.write_text(json.dumps(value))
            args = subject.config_for(original, root)
            snapshot = subject.snapshot_config(args)
            identity = subject.admission_identity(args)
            value['source_commit'] = 'c' * 40; original.write_text(json.dumps(value))
            actual = subject.config_for(snapshot, root, args.admission_config_sha256)
            self.assertEqual(subject.admission_identity(actual), identity)
            report = dict(identity); report['pins_sha256'] = 'd' * 64
            self.assertNotEqual(subject.admission_identity(report), identity)
            snapshot.write_bytes(snapshot.read_bytes() + b' ')
            with self.assertRaisesRegex(subject.Refused, 'snapshot-digest'):
                subject.config_for(snapshot, root, args.admission_config_sha256)

    def test_setup_transfer_encoding_exact_single_chunked_only_without_content_length(self):
        duplicate = Message(); duplicate.add_header('Transfer-Encoding', 'chunked'); duplicate.add_header('Transfer-Encoding', 'chunked')
        invalid = [duplicate, {'Transfer-Encoding': ''}, {'Transfer-Encoding': 'gzip'},
                   {'Transfer-Encoding': 'gzip, chunked'}, {'Transfer-Encoding': 'chunked', 'Content-Length': '2'},
                   {'Transfer-Encoding': '', 'Content-Length': '2'}]
        for headers in invalid:
            response = Response(b'{}'); response.headers = headers; connection = Connection(response)
            owner = subject.SetupHTTP(['http://127.0.0.1:32123'], connect=lambda *_args, **_kwargs: connection)
            with self.subTest(headers=str(headers)), self.assertRaises(subject.Refused):
                owner.open('http://127.0.0.1:32123/a')
            self.assertTrue(connection.closed); self.assertEqual(owner.active, 0)
            self.assertTrue(owner.records[0]['terminal']); self.assertFalse(owner.records[0]['complete'])
        for headers in [{}, {'Transfer-Encoding': 'chunked'}, {'Transfer-Encoding': 'CHUNKED'}]:
            response = Response(b'{}'); response.headers = headers; connection = Connection(response)
            owner = subject.SetupHTTP(['http://127.0.0.1:32123'], connect=lambda *_args, **_kwargs: connection)
            with owner.open('http://127.0.0.1:32123/a') as actual:
                self.assertEqual(actual.read(), b'{}')
            self.assertTrue(owner.records[0]['complete'])

    def test_real_httpresponse_parser_and_guard_agree_about_chunked_and_trailing_ows(self):
        class MemorySocket:
            def __init__(self, wire):
                self.wire = wire

            def makefile(self, *_args):
                return io.BytesIO(self.wire)

        for transfer in [b'chunked', b'CHUNKED', b'chunked ', b'chunked\t']:
            wire = b'HTTP/1.1 200 OK\r\nTransfer-Encoding: ' + transfer + b'\r\n\r\n2\r\n{}\r\n0\r\n\r\n'
            response = HTTPResponse(MemorySocket(wire)); response.begin()
            connection = Connection(response)
            owner = subject.SetupHTTP(['http://127.0.0.1:32123'], connect=lambda *_args, **_kwargs: connection)
            try:
                if transfer.lower() == b'chunked':
                    self.assertTrue(response.chunked)
                    with owner.open('http://127.0.0.1:32123/a') as actual:
                        self.assertEqual(actual.read(), b'{}')
                    self.assertTrue(owner.records[0]['complete']); self.assertEqual(owner.bytes, 2)
                else:
                    self.assertFalse(response.chunked)
                    with self.assertRaisesRegex(subject.Refused, 'setup-transfer-encoding'):
                        owner.open('http://127.0.0.1:32123/a')
                    self.assertFalse(owner.records[0]['complete']); self.assertEqual(owner.bytes, 0)
                self.assertTrue(connection.closed); self.assertEqual(owner.active, 0)
            finally:
                response.close()


if __name__ == '__main__':
    unittest.main()
