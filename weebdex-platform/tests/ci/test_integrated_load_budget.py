"""Actual finite raw loopback HTTP seams; no application, browser or PostgreSQL."""
import asyncio
import gzip
import importlib.util
import json
import time
from pathlib import Path
import unittest
from types import SimpleNamespace
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location('integrated_budget', ROOT / 'scripts/integrated_load_budget.py')
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)
Budget, HTTP = module.SharedBudget, module.BudgetedHTTP


class TransportBudgetSeams(unittest.IsolatedAsyncioTestCase):
    async def asyncSetUp(self):
        self.requests = []
        self.handlers = set()
        self.closed = asyncio.Event()
        self.extra_servers = []
        self.redirect_origin = None
        self.server = await asyncio.start_server(self.handle, '127.0.0.1', 0)
        self.origin = 'http://127.0.0.1:' + str(self.server.sockets[0].getsockname()[1])

    async def asyncTearDown(self):
        self.server.close()
        await self.server.wait_closed()
        for server in self.extra_servers:
            server.close()
            await server.wait_closed()
        for task in self.handlers:
            task.cancel()
        await asyncio.gather(*self.handlers, return_exceptions=True)

    async def handle(self, reader, writer):
        task = asyncio.current_task()
        self.handlers.add(task)
        try:
            raw = await asyncio.wait_for(reader.readuntil(b'\r\n\r\n'), 1)
            lines = raw.decode('ascii').split('\r\n')
            method, path, _ = lines[0].split(' ')
            headers = dict(line.split(': ', 1) for line in lines[1:] if ': ' in line)
            body = await reader.readexactly(int(headers.get('Content-Length', '0')))
            self.requests.append(dict(method=method, path=path, headers=headers, body=body))
            path = path.split('?', 1)[0]
            if path.startswith('/disconnect'):
                return
            if path.startswith('/pending'):
                writer.write(b'HTTP/1.1 200 OK\r\nContent-Length: 10\r\n\r\nab')
                await writer.drain()
                await reader.read()
                self.closed.set()
                return
            if path.startswith('/chunk'):
                payload = b'abc' if path == '/chunk-short' else b'abcd'
                writer.write(b'HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n' + hex(len(payload))[2:].encode() + b'\r\n' + payload + b'\r\n')
                await writer.drain()
                await asyncio.sleep(.03)
                writer.write((b'1\r\ne\r\n' if path == '/chunk-extra' else b'') + b'0\r\n\r\n')
            elif path == '/truncated':
                writer.write(b'HTTP/1.1 200 OK\r\nContent-Length: 10\r\n\r\nabcd')
            elif path == '/no-length':
                writer.write(b'HTTP/1.1 200 OK\r\nConnection: close\r\n\r\nabcde')
            elif path == '/gzip':
                payload = gzip.compress(b'original payload' * 100, mtime=0)
                writer.write(b'HTTP/1.1 200 OK\r\nContent-Encoding: gzip\r\nContent-Length: ' + str(len(payload)).encode() + b'\r\n\r\n' + payload)
            elif path.startswith('/redirect'):
                status = b'302 Found' if path.endswith('302') else b'307 Temporary Redirect'
                target = self.redirect_origin if 'cross' in path else self.origin
                writer.write(b'HTTP/1.1 ' + status + b'\r\nLocation: ' + (target + '/unexpected').encode() + b'\r\nContent-Length: 3\r\n\r\nred')
            else:
                size = int(path.rsplit('/', 1)[-1])
                status = b'503 Service Unavailable' if path.startswith('/status') else b'200 OK'
                writer.write(b'HTTP/1.1 ' + status + b'\r\nContent-Length: ' + str(size).encode() + b'\r\n\r\n')
                if path.startswith('/slow'):
                    await writer.drain()
                    await asyncio.sleep(.02)
                if method != 'HEAD':
                    writer.write(b'x' * size)
            await writer.drain()
        except (ConnectionError, asyncio.IncompleteReadError):
            pass
        finally:
            writer.close()
            await writer.wait_closed()
            self.handlers.discard(task)

    def budget(self, **kwargs):
        return Budget(duration_s=2, **kwargs)

    async def test_shared_last_request_slot_and_status_body_accounting(self):
        budget = self.budget(max_http_attempts=1)
        async with HTTP(budget, [self.origin]) as http:
            results = await asyncio.gather(http.request('reader', 'GET', self.origin + '/status/4'),
                                           http.request('diagnostics', 'GET', self.origin + '/bytes/4'), return_exceptions=True)
            self.assertEqual(sum(isinstance(r, module.BudgetRefused) for r in results), 1)
        value = budget.snapshot()
        self.assertEqual(len(self.requests), 1)
        self.assertEqual(value['http_attempts'], 1)
        self.assertEqual(value['response_body_bytes_consumed'], 4)
        self.assertEqual(value['lanes']['reader']['http_statuses'], {'503': 1})
        self.assertEqual(value['attempts'][0]['outcome'], 'complete-transport')
        self.assertFalse(value['attempts'][0]['semantic_verified'])

    async def test_explicit_retry_spends_a_new_attempt_and_exhaustion_never_sends(self):
        budget = self.budget(max_http_attempts=2)
        async with HTTP(budget, [self.origin]) as http:
            for _ in range(2):
                await http.request('catalogue', 'GET', self.origin + '/status/1')
            with self.assertRaises(module.BudgetRefused) as caught:
                await http.request('verification', 'GET', self.origin + '/bytes/1')
            self.assertEqual(caught.exception.reason, 'request-limit')
        self.assertEqual(len(self.requests), 2)
        self.assertEqual(budget.snapshot()['response_body_bytes_consumed'], 2)

    async def test_disconnected_get_and_put_are_not_retried_inside_aiohttp(self):
        budget = self.budget()
        async with HTTP(budget, [self.origin]) as http:
            for method in ['GET', 'PUT']:
                with self.assertRaises(module.TransportFailure):
                    await http.request('upload', method, self.origin + '/disconnect', body=b'abc')
        self.assertEqual([r['method'] for r in self.requests], ['GET', 'PUT'])
        value = budget.snapshot()
        self.assertEqual(value['http_attempts'], 2)
        self.assertEqual(value['upload_body_bytes_reserved'], 6)
        self.assertEqual(value['inflight'], 0)
        self.assertEqual(value['missing_byte_observations'], 2)

    async def test_redirects_never_forward_authority_body_or_consume_an_extra_attempt(self):
        second = await asyncio.start_server(self.handle, '127.0.0.1', 0)
        self.extra_servers.append(second)
        self.redirect_origin = 'http://127.0.0.1:' + str(second.sockets[0].getsockname()[1])
        budget = self.budget()
        statuses = []
        async with HTTP(budget, [self.origin, self.redirect_origin]) as http:
            for path in ['/redirect302', '/redirect307', '/redirect-cross302', '/redirect-cross307']:
                result = await http.request('upload', 'PUT', self.origin + path, body=b'synthetic-secret', headers={'Authorization': 'synthetic-authority'})
                statuses.append(result['status'])
        self.assertEqual(statuses, [302, 307, 302, 307])
        self.assertEqual(len(self.requests), 4)
        self.assertTrue(all(r['path'].startswith('/redirect') for r in self.requests))
        self.assertEqual(budget.snapshot()['http_attempts'], 4)
        public = json.dumps(budget.snapshot())
        self.assertNotIn('synthetic-secret', public)
        self.assertNotIn('synthetic-authority', public)
        self.assertNotIn(self.origin, public)

    async def test_upload_reservation_is_shared_and_failed_attempts_are_not_refunded(self):
        budget = self.budget(max_upload_body_bytes=3)
        async with HTTP(budget, [self.origin]) as http:
            with self.assertRaises(module.TransportFailure):
                await http.request('upload', 'POST', self.origin + '/disconnect', body=b'abc')
            with self.assertRaises(module.BudgetRefused) as caught:
                await http.request('public_sync', 'POST', self.origin + '/bytes/1', body=b'a')
            self.assertEqual(caught.exception.reason, 'upload-byte-limit')
        self.assertEqual(len(self.requests), 1)
        self.assertEqual(budget.snapshot()['upload_body_bytes_reserved'], 3)

    async def test_exact_chunked_limit_accepts_delayed_eof_without_lookahead(self):
        budget = self.budget(max_response_body_bytes=4)
        async with HTTP(budget, [self.origin], chunk_bytes=4) as http:
            result = await http.request('media', 'GET', self.origin + '/chunk-exact', capture_limit=4)
        self.assertEqual(result['body'], b'abcd')
        self.assertEqual(budget.snapshot()['response_body_bytes_consumed'], 4)
        self.assertEqual(budget.snapshot()['parser_received_payload_bytes'], 4)
        self.assertEqual(budget.snapshot()['response_read_bytes_reserved'], 0)

    async def test_delayed_chunked_overflow_refuses_without_consuming_extra_byte(self):
        budget = self.budget(max_response_body_bytes=4)
        async with HTTP(budget, [self.origin], chunk_bytes=4) as http:
            with self.assertRaises(module.BudgetRefused) as caught:
                await http.request('media', 'GET', self.origin + '/chunk-extra')
            self.assertEqual(caught.exception.reason, 'response-byte-limit')
        value = budget.snapshot()
        self.assertEqual(value['response_body_bytes_consumed'], 4)
        self.assertEqual(value['parser_received_payload_bytes'], 5)
        self.assertEqual(value['attempts'][0]['outcome'], 'budget-refused')

    async def test_concurrent_response_credits_are_shared_across_lanes(self):
        budget = self.budget(max_response_body_bytes=5)
        async with HTTP(budget, [self.origin], chunk_bytes=4) as http:
            result = await asyncio.gather(http.request('reader', 'GET', self.origin + '/slow/3'),
                                          http.request('media', 'GET', self.origin + '/slow/3'), return_exceptions=True)
        self.assertEqual(sum(isinstance(r, module.BudgetRefused) for r in result), 1)
        value = budget.snapshot()
        self.assertEqual(value['http_attempts'], 2)
        self.assertEqual(value['response_body_bytes_consumed'], 5)
        self.assertEqual(value['parser_received_payload_bytes'], 6)
        self.assertEqual(value['response_read_bytes_reserved'], 0)
        self.assertEqual(value['inflight'], 0)

    async def test_no_length_and_truncated_length_never_invent_complete_body(self):
        for cap, path, error in [(4, '/no-length', module.BudgetRefused), (20, '/truncated', module.TransportFailure)]:
            budget = self.budget(max_response_body_bytes=cap)
            async with HTTP(budget, [self.origin], chunk_bytes=4) as http:
                with self.assertRaises(error):
                    await http.request('reader', 'GET', self.origin + path)
            value = budget.snapshot()
            self.assertEqual(value['http_attempts'], 1)
            self.assertLessEqual(value['response_body_bytes_consumed'], cap)
            self.assertEqual(value['parser_received_payload_bytes'], 5 if path == '/no-length' else 4)
            self.assertNotEqual(value['attempts'][0]['outcome'], 'complete-transport')

    async def test_gzip_counts_encoded_bytes_and_verification_failure_keeps_all_costs(self):
        payload = gzip.compress(b'original payload' * 100, mtime=0)
        budget = self.budget(max_response_body_bytes=len(payload) * 2)
        async with HTTP(budget, [self.origin]) as http:
            result = await http.request('reader', 'GET', self.origin + '/gzip', capture_limit=1024, verify=lambda result: result['body'] == payload)
            self.assertEqual(result['body'], payload)
            with self.assertRaises(module.VerificationRejected):
                await http.request('verification', 'GET', self.origin + '/gzip', verify=lambda result: False)
        value = budget.snapshot()
        self.assertEqual(value['response_body_bytes_consumed'], len(payload) * 2)
        self.assertEqual(value['parser_received_payload_bytes'], len(payload) * 2)
        self.assertTrue(value['attempts'][0]['semantic_verified'])
        self.assertEqual(value['attempts'][1]['outcome'], 'verification-rejected')
        self.assertEqual(value['lanes']['verification']['http_statuses'], {'200': 1})

    async def wait_consumed(self, budget):
        async with asyncio.timeout(.5):
            while not budget.records or not budget.records[0]['response_body_bytes_consumed']:
                await asyncio.sleep(.001)

    async def test_cancel_mid_body_releases_credits_closes_socket_and_joins_task(self):
        budget = self.budget()
        async with HTTP(budget, [self.origin]) as http:
            task = asyncio.create_task(http.request('media', 'GET', self.origin + '/pending'))
            await self.wait_consumed(budget)
            task.cancel()
            with self.assertRaises(asyncio.CancelledError):
                await task
            await asyncio.wait_for(self.closed.wait(), .5)
        value = budget.snapshot()
        self.assertEqual(value['attempts'][0]['outcome'], 'cancelled')
        self.assertEqual(value['response_body_bytes_consumed'], 2)
        self.assertEqual(value['response_read_bytes_reserved'], 0)
        self.assertEqual(value['inflight'], 0)
        self.assertTrue(value['attempts'][0]['response_close_settled'])

    async def test_timeout_and_context_exit_own_outstanding_request_cleanup(self):
        budget = self.budget()
        async with HTTP(budget, [self.origin], request_timeout_s=.04) as http:
            with self.assertRaises(module.TransportFailure):
                await http.request('media', 'GET', self.origin + '/pending')
        self.assertEqual(budget.snapshot()['attempts'][0]['outcome'], 'timeout')
        budget = self.budget()
        async with HTTP(budget, [self.origin]) as http:
            task = asyncio.create_task(http.request('media', 'GET', self.origin + '/pending'))
            await self.wait_consumed(budget)
        self.assertTrue(task.done() and task.cancelled())
        self.assertEqual(budget.snapshot()['inflight'], 0)
        self.assertEqual(budget.snapshot()['response_read_bytes_reserved'], 0)

    async def test_connector_refusal_is_one_charged_attempt_with_unknown_response_bytes(self):
        self.server.close()
        await self.server.wait_closed()
        budget = self.budget(max_http_attempts=1)
        async with HTTP(budget, [self.origin]) as http:
            with self.assertRaises(module.TransportFailure):
                await http.request('catalogue', 'GET', self.origin + '/bytes/1')
            with self.assertRaises(module.BudgetRefused):
                await http.request('diagnostics', 'GET', self.origin + '/bytes/1')
        value = budget.snapshot()
        self.assertEqual(value['http_attempts'], 1)
        self.assertIsNone(value['attempts'][0]['status'])
        self.assertIsNone(value['attempts'][0]['parser_received_payload_bytes'])
        self.assertEqual(value['missing_byte_observations'], 1)
        self.assertEqual(self.requests, [])

    async def test_byte_boundaries_and_head_count_actual_payload_not_content_length(self):
        for path, size in [('/bytes/3', 3), ('/bytes/4', 4), ('/chunk-short', 3)]:
            budget = self.budget(max_response_body_bytes=4)
            async with HTTP(budget, [self.origin], chunk_bytes=4) as http:
                result = await http.request('media', 'GET', self.origin + path)
            self.assertEqual(result['body_bytes'], size)
            self.assertEqual(budget.snapshot()['response_body_bytes_consumed'], size)
        budget = self.budget(max_response_body_bytes=4)
        async with HTTP(budget, [self.origin], chunk_bytes=4) as http:
            with self.assertRaises(module.BudgetRefused):
                await http.request('reader', 'GET', self.origin + '/status/5')
        self.assertEqual(budget.snapshot()['response_body_bytes_consumed'], 4)
        self.assertEqual(budget.snapshot()['lanes']['reader']['http_statuses'], {'503': 1})
        budget = self.budget(max_response_body_bytes=1)
        async with HTTP(budget, [self.origin]) as http:
            result = await http.request('media', 'HEAD', self.origin + '/bytes/20')
        self.assertEqual(result['body_bytes'], 0)
        self.assertEqual(budget.snapshot()['parser_received_payload_bytes'], 0)

    async def test_inflight_refusal_and_absolute_deadline_are_shared_across_lanes(self):
        budget = self.budget(max_inflight=1)
        async with HTTP(budget, [self.origin]) as http:
            task = asyncio.create_task(http.request('media', 'GET', self.origin + '/pending'))
            await self.wait_consumed(budget)
            with self.assertRaises(module.BudgetRefused) as caught:
                await http.request('diagnostics', 'GET', self.origin + '/bytes/1')
            self.assertEqual(caught.exception.reason, 'inflight-limit')
            task.cancel()
            await asyncio.gather(task, return_exceptions=True)
            await http.request('verification', 'GET', self.origin + '/bytes/1')
        self.assertEqual(budget.snapshot()['http_attempts'], 2)
        self.assertEqual(budget.snapshot()['peak_inflight'], 1)
        budget = Budget(duration_s=.03)
        async with HTTP(budget, [self.origin]) as http:
            with self.assertRaises(module.TransportFailure):
                await http.request('media', 'GET', self.origin + '/pending')
            with self.assertRaises(module.BudgetRefused) as caught:
                await http.request('diagnostics', 'GET', self.origin + '/bytes/1')
            self.assertEqual(caught.exception.reason, 'deadline')
        self.assertEqual(budget.snapshot()['http_attempts'], 1)

    async def test_late_verification_cannot_turn_a_stalled_loop_into_completion(self):
        # Drive the actual request/middleware/accounting path without asking an
        # overloaded host to finish loopback I/O within 10 ms. Only its clock and
        # response transport are controlled; the original 10 ms deadline and
        # post-verification timeout gate remain unchanged.
        clock = [time.monotonic()]
        consumed, closed, verified = [], [], []
        async def read(credit):
            self.assertGreaterEqual(credit, 1)
            consumed.append(b'x')
            return b'x'
        async def wait_for_close():
            self.assertEqual(closed, [True])
        response = SimpleNamespace(status=200, headers={},
            content=SimpleNamespace(at_eof=lambda: bool(consumed), read=read, total_bytes=1),
            close=lambda: closed.append(True), wait_for_close=wait_for_close)
        async def request(method, url, **options):
            self.assertEqual((method, str(url)), ('GET', self.origin + '/bytes/1'))
            self.assertFalse(options['allow_redirects'])
            self.assertEqual(len(options['middlewares']), 1)
            async def dispatched(_):
                return response
            return await options['middlewares'][0](object(), dispatched)
        def delayed(result):
            self.assertEqual(result['body_bytes'], 1)
            verified.append(True)
            clock[0] += .03
            return True
        with patch.object(module, 'time', SimpleNamespace(monotonic=lambda: clock[0])):
            budget = self.budget()
            async with HTTP(budget, [self.origin], request_timeout_s=.01) as http:
                with patch.object(module.aiohttp.ClientSession, 'request', side_effect=request):
                    with self.assertRaisesRegex(module.TransportFailure, 'deadline expired'):
                        await http.request('verification', 'GET', self.origin + '/bytes/1', verify=delayed)
        result = budget.snapshot()
        self.assertEqual(verified, [True])
        self.assertEqual(result['attempts'][0]['outcome'], 'timeout')
        self.assertTrue(result['attempts'][0]['semantic_verified'])
        self.assertTrue(result['attempts'][0]['response_close_settled'])
        self.assertEqual(result['http_attempts'], 1)
        self.assertEqual(result['response_body_bytes_consumed'], 1)
        self.assertEqual(result['inflight'], 0)

    async def test_invalid_destinations_headers_lanes_and_bounds_never_spend_an_attempt(self):
        budget = self.budget()
        async with HTTP(budget, [self.origin]) as http:
            for url in ['https://example.invalid/', 'http://localhost:1234/', self.origin + '/?x=y#fragment', self.origin + '/bytes/1#',
                        self.origin + '/bytes/1?', self.origin, self.origin.replace('127.0.0.1', 'private@127.0.0.1') + '/bytes/1', self.origin + '/bad path']:
                with self.subTest(url=url), self.assertRaises(ValueError):
                    await http.request('reader', 'GET', url)
            for headers in [{'Host': 'elsewhere.invalid'}, {'Content-Length': '99'}, {'Transfer-Encoding': 'chunked'}]:
                with self.assertRaises(ValueError):
                    await http.request('reader', 'GET', self.origin + '/bytes/1', headers=headers)
            for lane in ['local_progress', 'unknown', [], None]:
                with self.assertRaises(ValueError):
                    await http.request(lane, 'GET', self.origin + '/bytes/1')
        self.assertEqual(budget.snapshot()['http_attempts'], 0)
        self.assertEqual(self.requests, [])
        for kwargs in [dict(max_http_attempts=True), dict(max_response_body_bytes=-1), dict(duration_s=float('nan')),
                       dict(max_inflight=0), dict(max_http_attempts=2501)]:
            with self.assertRaises(ValueError):
                Budget(**kwargs)

    async def test_signed_ascii_request_target_and_serialized_body_are_not_requoted(self):
        budget = self.budget()
        target = '/bytes/1?part=%2f&literal=%2520'
        body = b'{"synthetic":"already serialized"}'
        async with HTTP(budget, [self.origin]) as http:
            await http.request('public_sync', 'POST', self.origin + target, body=body)
        self.assertEqual(self.requests[0]['path'], target)
        self.assertEqual(self.requests[0]['body'], body)
        self.assertEqual(budget.snapshot()['upload_body_bytes_reserved'], len(body))

    async def test_local_progress_is_unmeasured_and_cannot_be_a_network_lane(self):
        budget = self.budget()
        async with HTTP(budget, [self.origin]) as http:
            with self.assertRaises(ValueError):
                await http.request('local_progress', 'POST', self.origin + '/bytes/0')
        self.assertEqual(self.requests, [])
        value = budget.snapshot()
        self.assertEqual(value['http_attempts'], 0)
        self.assertEqual(value['device_progress'], dict(measured=False, logical_saves_observed=None,
            expected_http_writes=0, backend_substitution_allowed=False))
        self.assertFalse(value['capacity_qualified'])


class DeadlineClassificationControls(unittest.IsolatedAsyncioTestCase):
    """Actual budget request path with a controlled transport/clock, no listener."""
    async def exercise(self, duration, request_timeout, advance=0, failure=None):
        clock = [time.monotonic()]
        consumed, closed = [], []
        async def read(credit):
            consumed.append(b'x')
            return b'x'
        async def wait_for_close():
            self.assertEqual(closed, [True])
        response = SimpleNamespace(status=200, headers={},
            content=SimpleNamespace(at_eof=lambda: bool(consumed), read=read, total_bytes=1),
            close=lambda: closed.append(True), wait_for_close=wait_for_close)
        async def request(method, url, **options):
            async def dispatched(_):
                if failure is not None:
                    raise failure
                return response
            return await options['middlewares'][0](object(), dispatched)
        def verify(_):
            clock[0] += advance
            return True
        with patch.object(module, 'time', SimpleNamespace(monotonic=lambda: clock[0])):
            budget = Budget(duration_s=duration)
            async with HTTP(budget, ['http://127.0.0.1:1'], request_timeout_s=request_timeout) as http:
                with patch.object(module.aiohttp.ClientSession, 'request', side_effect=request):
                    try:
                        await http.request('verification', 'GET', 'http://127.0.0.1:1/owned', verify=verify)
                    except BaseException as error:
                        result = error
                    else:
                        self.fail('Expected a bounded stop')
        self.assertEqual(budget.snapshot()['inflight'], 0)
        self.assertEqual(budget.snapshot()['http_attempts'], 1)
        return result, budget.snapshot()

    async def test_selected_shared_earlier_request_and_equal_deadlines(self):
        for duration, request_timeout, advance, shared in [
                (.01, 1, .03, True), (1, .01, .03, False), (1, 1, 1.1, True)]:
            with self.subTest(duration=duration, request_timeout=request_timeout):
                error, snapshot = await self.exercise(duration, request_timeout, advance)
                self.assertIsInstance(error, module.TransportDeadline)
                self.assertIs(error.shared_deadline, shared)
                self.assertEqual(snapshot['attempts'][0]['outcome'], 'timeout')
                self.assertTrue(snapshot['attempts'][0]['response_close_settled'])

    async def test_early_internal_timeout_is_not_selected_deadline_expiry(self):
        error, _ = await self.exercise(1, 1, failure=TimeoutError())
        self.assertIsInstance(error, module.TransportFailure)
        self.assertNotIsInstance(error, module.TransportDeadline)

    async def test_ordinary_transport_failure_is_not_deadline(self):
        error, _ = await self.exercise(1, 1, failure=module.aiohttp.ClientOSError())
        self.assertIsInstance(error, module.TransportFailure)
        self.assertNotIsInstance(error, module.TransportDeadline)

    async def test_cancellation_propagates_without_deadline_relabel(self):
        cancelled = asyncio.CancelledError()
        error, snapshot = await self.exercise(1, 1, failure=cancelled)
        self.assertIs(error, cancelled)
        self.assertEqual(snapshot['attempts'][0]['outcome'], 'cancelled')


if __name__ == '__main__':
    unittest.main()
