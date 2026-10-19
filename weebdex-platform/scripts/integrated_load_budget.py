"""Finite single-process HTTP accounting for an owned loopback load fixture.

No workload runs on import. This is a transport primitive, not a browser/device
progress simulator, endpoint fan-out model, production load or capacity result.
"""
import asyncio
from collections import Counter
import math
import time
from urllib.parse import urlsplit

import aiohttp
from yarl import URL

LANES = frozenset({'setup', 'catalogue', 'reader', 'media', 'analytics', 'public_sync',
                   'upload', 'diagnostics', 'verification'})
METHODS = frozenset({'GET', 'HEAD', 'POST', 'PUT', 'PATCH', 'DELETE', 'OPTIONS'})
FORBIDDEN_HEADERS = frozenset({'host', 'content-length', 'transfer-encoding', 'connection', 'expect'})


class BudgetRefused(RuntimeError):
    def __init__(self, reason):
        super().__init__('Owned HTTP budget refused: ' + reason)
        self.reason = reason


class TransportFailure(RuntimeError):
    """Fixed error, with no URL, header, body or private underlying exception."""


class VerificationRejected(RuntimeError):
    pass


def origin(value):
    if type(value) is not str or not value.isascii() or any(ord(c) < 33 or c == '\\' for c in value):
        raise ValueError('An exact numeric-loopback HTTP URL is required')
    parsed = urlsplit(value)
    try:
        port = parsed.port
    except ValueError:
        raise ValueError('An exact numeric-loopback HTTP URL is required') from None
    if (parsed.scheme != 'http' or parsed.hostname not in {'127.0.0.1', '::1'} or
            not port or parsed.username is not None or parsed.password is not None or '#' in value):
        raise ValueError('An exact numeric-loopback HTTP URL is required')
    host = '[::1]' if parsed.hostname == '::1' else '127.0.0.1'
    return f'http://{host}:{port}', parsed


class SharedBudget:
    """One event-loop owner's counters; all lanes share the same credits.

    Response credits cap bytes consumed by the application, before each read.
    Parser-prefetched encoded payload bytes are measured separately and may
    exceed that reader cap. Neither counter includes headers/chunk framing or
    proves NIC/OS-buffer traffic. Upload bytes are conservatively reserved for
    each admitted serialized body, including uncertain/failed attempts.
    """
    def __init__(self, *, max_http_attempts=2500, max_response_body_bytes=512 * 1024**2,
                 max_upload_body_bytes=32 * 1024**2, duration_s=180, max_inflight=32):
        bounds = [(max_http_attempts, 2500), (max_response_body_bytes, 512 * 1024**2),
                  (max_upload_body_bytes, 32 * 1024**2)]
        if any(type(value) is not int or not 0 <= value <= high for value, high in bounds):
            raise ValueError('Budget counters must be bounded nonnegative integers')
        if (type(duration_s) not in (int, float) or not math.isfinite(duration_s) or
                not 0 < duration_s <= 180 or type(max_inflight) is not int or not 1 <= max_inflight <= 32):
            raise ValueError('Budget duration/inflight bounds exceeded')
        self.limits = dict(http_attempts=max_http_attempts, response_body_bytes=max_response_body_bytes,
                           upload_body_bytes=max_upload_body_bytes, duration_s=duration_s, inflight=max_inflight)
        self.start = time.monotonic()
        self.deadline = self.start + duration_s
        self.records = []
        self.upload_reserved = self.consumed = self.read_reserved = self.inflight = self.peak_inflight = 0
        self.refusals = Counter()
        self._loop = None
        self._changed = asyncio.Event()

    def _owner(self):
        loop = asyncio.get_running_loop()
        if self._loop is None:
            self._loop = loop
        if self._loop is not loop:
            raise RuntimeError('A shared budget belongs to exactly one event loop')

    def _refuse(self, reason):
        self.refusals[reason] += 1
        raise BudgetRefused(reason)

    def _begin(self, lane, method, upload_bytes):
        self._owner()
        if time.monotonic() >= self.deadline:
            self._refuse('deadline')
        if len(self.records) >= self.limits['http_attempts']:
            self._refuse('request-limit')
        if self.consumed >= self.limits['response_body_bytes']:
            self._refuse('response-byte-limit')
        if self.upload_reserved + upload_bytes > self.limits['upload_body_bytes']:
            self._refuse('upload-byte-limit')
        if self.inflight >= self.limits['inflight']:
            self._refuse('inflight-limit')
        now = time.monotonic()
        record = dict(attempt=len(self.records) + 1, lane=lane, method=method,
                      started_s=now - self.start, status=None, outcome='started',
                      upload_body_bytes_reserved=upload_bytes, response_body_bytes_consumed=0,
                      parser_received_payload_bytes=None, response_headers_ms=None,
                      first_body_ms=None, complete_body_ms=None, semantic_verified=False)
        self.records.append(record)
        self.upload_reserved += upload_bytes
        self.inflight += 1
        self.peak_inflight = max(self.peak_inflight, self.inflight)
        return record

    async def _read_credit(self, maximum, deadline):
        self._owner()
        while True:
            if time.monotonic() >= deadline:
                raise TimeoutError
            changed = self._changed
            available = self.limits['response_body_bytes'] - self.consumed - self.read_reserved
            if available:
                credit = min(maximum, available)
                self.read_reserved += credit
                return credit
            if not self.read_reserved:
                return 0
            async with asyncio.timeout_at(deadline):
                await changed.wait()

    def _settle_read(self, credit, consumed):
        self.read_reserved -= credit
        self.consumed += consumed
        assert 0 <= consumed <= credit and self.read_reserved >= 0
        changed = self._changed
        self._changed = asyncio.Event()
        changed.set()

    def snapshot(self):
        lanes = {}
        for record in self.records:
            lane = lanes.setdefault(record['lane'], dict(http_attempts=0, responses=0,
                response_body_bytes_consumed=0, parser_received_payload_bytes=0, missing_byte_observations=0, http_statuses={}))
            lane['http_attempts'] += 1
            lane['response_body_bytes_consumed'] += record['response_body_bytes_consumed']
            if record['parser_received_payload_bytes'] is None:
                lane['missing_byte_observations'] += 1
            else:
                lane['parser_received_payload_bytes'] += record['parser_received_payload_bytes']
            if record['status'] is not None:
                lane['responses'] += 1
                status = str(record['status'])
                lane['http_statuses'][status] = lane['http_statuses'].get(status, 0) + 1
        return dict(limits=dict(self.limits), http_attempts=len(self.records),
            upload_body_bytes_reserved=self.upload_reserved, response_body_bytes_consumed=self.consumed,
            response_read_bytes_reserved=self.read_reserved,
            parser_received_payload_bytes=sum(r['parser_received_payload_bytes'] for r in self.records if r['parser_received_payload_bytes'] is not None),
            missing_byte_observations=sum(r['parser_received_payload_bytes'] is None for r in self.records),
            inflight=self.inflight, peak_inflight=self.peak_inflight, refusals=dict(self.refusals),
            lanes=lanes, attempts=[dict(r) for r in self.records],
            device_progress=dict(measured=False, logical_saves_observed=None, expected_http_writes=0,
                                 backend_substitution_allowed=False),
            capacity_qualified=False)


class BudgetedHTTP:
    """Sole owned-session route: exact allowlisted origins, no redirect or retry.

    aiohttp may internally retry an idempotent request after a disconnected
    server. Public middleware converts those transport exceptions before its
    retry loop; a second middleware entry is also refused before any send.
    """
    def __init__(self, budget, origins, *, request_timeout_s=30, chunk_bytes=65536):
        if (type(request_timeout_s) not in (int, float) or not math.isfinite(request_timeout_s) or
                not 0 < request_timeout_s <= 30 or type(chunk_bytes) is not int or not 1 <= chunk_bytes <= 65536):
            raise ValueError('Bounded request timeout/chunk size required')
        self.budget = budget
        self.origins = set()
        for value in origins:
            exact, parsed = origin(value)
            if value != exact or parsed.path or parsed.query:
                raise ValueError('Origin allowlist must contain exact origins')
            self.origins.add(exact)
        if not self.origins:
            raise ValueError('At least one owned loopback origin is required')
        self.timeout = request_timeout_s
        self.chunk_bytes = chunk_bytes
        self.__session = None
        self._tasks = set()
        self._closing = False

    async def __aenter__(self):
        if self.__session is not None or self._closing:
            raise RuntimeError('Owned transport cannot be reopened')
        self.__session = aiohttp.ClientSession(auto_decompress=False, trust_env=False,
            cookie_jar=aiohttp.DummyCookieJar(), connector=aiohttp.TCPConnector(force_close=True),
            read_bufsize=self.chunk_bytes)
        return self

    async def __aexit__(self, *args):
        self._closing = True
        tasks = self._tasks - {asyncio.current_task()}
        for task in tasks:
            task.cancel()
        if tasks:
            await asyncio.gather(*tasks, return_exceptions=True)
        if self.__session is not None:
            await self.__session.close()

    async def request(self, lane, method, url, *, body=b'', headers=None, capture_limit=0, verify=None):
        exact, parsed = origin(url)
        if not parsed.path.startswith('/') or str(URL(url, encoded=True)) != url:
            raise ValueError('An exact encoded request target with an explicit path is required')
        if (type(lane) is not str or type(method) is not str or lane not in LANES or method not in METHODS or exact not in self.origins or type(body) is not bytes or
                type(capture_limit) is not int or not 0 <= capture_limit <= 1024**2):
            raise ValueError('Unsupported owned HTTP request')
        if headers is not None and (type(headers) is not dict or any(type(k) is not str or type(v) is not str or
                k.lower() in FORBIDDEN_HEADERS for k, v in headers.items())):
            raise ValueError('Unsupported owned HTTP headers')
        if self.__session is None or self._closing:
            raise RuntimeError('Owned transport is not open')
        task = asyncio.current_task()
        if task in self._tasks:
            raise RuntimeError('Reentrant transport requests are unsupported')
        self._tasks.add(task)
        record = response = None
        began = None
        deadline = min(self.budget.deadline, time.monotonic() + self.timeout)

        async def one_attempt(request, handler):
            nonlocal record, began
            if record is not None:
                self.budget._refuse('internal-retry-blocked')
            record = self.budget._begin(lane, method, len(body))
            began = time.monotonic()
            try:
                return await handler(request)
            except (aiohttp.ClientOSError, aiohttp.ServerDisconnectedError):
                raise TransportFailure('Owned HTTP connection failed; not retried') from None

        try:
            async with asyncio.timeout_at(deadline):
                response = await self.__session.request(method, URL(url, encoded=True), data=body, headers=headers,
                    allow_redirects=False, middlewares=(one_attempt,), timeout=aiohttp.ClientTimeout(total=None))
                record['status'] = response.status
                record['response_headers_ms'] = (time.monotonic() - began) * 1000
                collected = []
                size = 0
                while True:
                    if response.content.at_eof():
                        break
                    credit = await self.budget._read_credit(self.chunk_bytes, deadline)
                    if not credit:
                        # An exact-limit chunked response may deliver its framing
                        # EOF later. Do not consume an uncharged lookahead byte.
                        if response.content.total_bytes == size:
                            await response.content.wait_eof()
                        if response.content.at_eof():
                            break
                        self.budget._refuse('response-byte-limit')
                    chunk = b''
                    try:
                        chunk = await response.content.read(credit)
                    finally:
                        self.budget._settle_read(credit, len(chunk))
                    if not chunk:
                        break
                    if record['first_body_ms'] is None:
                        record['first_body_ms'] = (time.monotonic() - began) * 1000
                    size += len(chunk)
                    record['response_body_bytes_consumed'] = size
                    if capture_limit:
                        if size > capture_limit:
                            self.budget._refuse('capture-limit')
                        collected.append(chunk)
                record['complete_body_ms'] = (time.monotonic() - began) * 1000
                result = dict(status=response.status, headers=dict(response.headers),
                              body=b''.join(collected) if capture_limit else None, body_bytes=size)
                if verify is not None:
                    try:
                        valid = verify(result)
                        if hasattr(valid, '__await__'):
                            valid = await valid
                        if valid is not True:
                            raise VerificationRejected('Response verification rejected')
                    except Exception:
                        raise VerificationRejected('Response verification rejected') from None
                    record['semantic_verified'] = True
                if time.monotonic() >= deadline:
                    raise TimeoutError
                record['outcome'] = 'complete-transport'
                return result
        except BudgetRefused:
            if record:
                record['outcome'] = 'budget-refused'
            raise
        except VerificationRejected:
            record['outcome'] = 'verification-rejected'
            raise
        except asyncio.CancelledError:
            if record:
                record['outcome'] = 'cancelled'
            raise
        except TimeoutError:
            if record:
                record['outcome'] = 'timeout'
            raise TransportFailure('Owned HTTP deadline expired') from None
        except Exception:
            if record:
                record['outcome'] = 'transport-error'
            raise TransportFailure('Owned HTTP transport failed; not retried') from None
        finally:
            try:
                if response is not None:
                    response.close()
                    record['parser_received_payload_bytes'] = response.content.total_bytes
                    try:
                        async with asyncio.timeout(1):
                            await response.wait_for_close()
                        record['response_close_settled'] = True
                    except Exception:
                        record['response_close_settled'] = False
                        if record['outcome'] == 'complete-transport':
                            record['outcome'] = 'transport-error'
                            raise TransportFailure('Owned HTTP close did not settle') from None
            finally:
                if record is not None:
                    record['finished_s'] = time.monotonic() - self.budget.start
                    self.budget.inflight -= 1
                self._tasks.discard(task)
