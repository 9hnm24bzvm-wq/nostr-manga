"""Real loopback capture/UI tests with synthetic links; no DB/browser/token issuance."""
import ast
import contextlib
import html
import io
import json
import re
import threading
import types
import unittest
from http.client import HTTPConnection
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.parse import parse_qs, urlsplit

SOURCE = (Path(__file__).resolve().parents[2] / 'tests/runtime/browser_fixture.py').read_text()
TREE = ast.parse(SOURCE)
EMAIL = re.compile(r'^[A-Za-z0-9._+\-]{1,64}@example\.invalid$')
RECIPIENT = 'reader@example.invalid'
ORIGIN = 'http://127.0.0.1:43210'
LINK = ORIGIN + '/auth/self-service/verification?token=' + 'a' * 43
NAMESPACE = {'html': html, 're': re, 'EMAIL': EMAIL, 'urlsplit': urlsplit, 'parse_qs': parse_qs,
             'BaseHTTPRequestHandler': BaseHTTPRequestHandler, 'ThreadingHTTPServer': ThreadingHTTPServer,
             'threading': threading, 'json': json}
for node in TREE.body:
    if isinstance(node, ast.FunctionDef) and node.name == 'verification_mailbox_html':
        exec(compile(ast.Module(body=[node], type_ignores=[]), '<actual-mailbox-helper>', 'exec'), NAMESPACE)
fixture_class = next(node for node in TREE.body if isinstance(node, ast.ClassDef) and node.name == 'Fixture')
start = next(node for node in fixture_class.body if isinstance(node, ast.FunctionDef) and node.name == 'start_sink')
exec(compile(ast.Module(body=[start], type_ignores=[]), '<actual-mail-sink>', 'exec'), NAMESPACE)
render = NAMESPACE['verification_mailbox_html']


class VerificationMailbox(unittest.TestCase):
    def test_renderer_empty_and_captured_link(self):
        empty = render(ORIGIN, RECIPIENT).decode()
        self.assertIn('No captured verification message yet', empty)
        self.assertNotIn('<a ', empty)
        body = render(ORIGIN, RECIPIENT, LINK).decode()
        self.assertIn('href="' + html.escape(LINK, quote=True) + '"', body)
        self.assertIn('rel="noreferrer"', body)
        self.assertIn('does not establish external email-provider delivery', body)
        self.assertNotIn('<script', body)

    def test_renderer_refuses_injection_other_origin_and_recovery(self):
        for origin in ['https://example.com', 'http://localhost:43210', ORIGIN + '/', 'http://127.0.0.1:99999']:
            with self.subTest(origin=origin), self.assertRaises(ValueError): render(origin, RECIPIENT, LINK)
        for recipient in ['real@example.com', '<script>@example.invalid', 'reader@example.invalid\n']:
            with self.subTest(recipient=recipient), self.assertRaises(ValueError): render(ORIGIN, recipient, LINK)
        for link in [LINK + '&extra=1', LINK + '#x', LINK.replace('/verification?', '/recovery?'),
                     LINK.replace(':43210', ':43211'), 'javascript:alert(1)', LINK + '" onclick="x']:
            with self.subTest(link=link), self.assertRaises(ValueError): render(ORIGIN, RECIPIENT, link)

    def fixture(self):
        f = types.SimpleNamespace(origin=ORIGIN, mail_origin='', mail_port=0, lock=threading.Lock(),
                                  mail=[], mail_count=0, stop=threading.Event(), maintenance=threading.Event())
        NAMESPACE['start_sink'](f)
        f.mail_origin = 'http://127.0.0.1:' + str(f.sink.server_port)
        def cleanup():
            f.sink.shutdown(); f.sink.server_close(); f.sink_thread.join(timeout=2)
        self.addCleanup(cleanup)
        return f

    def request(self, f, path, method='GET', payload=None, headers=None):
        conn = HTTPConnection('127.0.0.1', f.sink.server_port, timeout=3)
        try:
            conn.request(method, path, json.dumps(payload) if payload is not None else None, headers or {})
            response = conn.getresponse()
            return response.status, dict(response.getheaders()), response.read()
        finally: conn.close()

    def test_actual_sink_capture_clickable_page_no_redemption_or_logging(self):
        f = self.fixture()
        capture = io.StringIO()
        with contextlib.redirect_stderr(capture), contextlib.redirect_stdout(capture):
            status, _, empty = self.request(f, '/mail/inbox?to=' + RECIPIENT + '&kind=verification')
            self.assertEqual(status, 200); self.assertNotIn(b'<a ', empty)
            status, _, _ = self.request(f, '/send', 'POST', {'to': RECIPIENT, 'subject': 'Verify', 'text': 'Verify\n' + LINK})
            self.assertEqual(status, 202)
            status, headers, body = self.request(f, '/mail/inbox?to=' + RECIPIENT + '&kind=verification')
            self.assertEqual(status, 200)
            self.assertIn(LINK.encode(), body)
            self.assertEqual(headers['Cache-Control'], 'no-store')
            self.assertEqual(headers['Referrer-Policy'], 'no-referrer')
            self.assertEqual(headers['X-Content-Type-Options'], 'nosniff')
            self.assertIn("frame-ancestors 'none'", headers['Content-Security-Policy'])
            self.assertNotIn('Access-Control-Allow-Origin', headers)
            self.assertEqual(f.mail_count, 1)
            self.assertEqual(len(f.mail), 1)
            # Reading the view does not consume or mint another message.
            self.assertEqual(self.request(f, '/mail/inbox?to=' + RECIPIENT + '&kind=verification')[0], 200)
            self.assertEqual(f.mail_count, 1)
        self.assertEqual(capture.getvalue(), '')

    def test_query_host_and_lifecycle_guards(self):
        f = self.fixture()
        path = '/mail/inbox?to=' + RECIPIENT + '&kind=verification'
        for suffix in ['&to=other@example.invalid', '&extra=1']:
            self.assertEqual(self.request(f, path + suffix)[0], 400)
        for path_bad in ['/mail/inbox', '/mail/inbox?to=&kind=verification',
                         path.replace('verification', 'recovery'), path.replace(RECIPIENT, 'real@example.com')]:
            self.assertEqual(self.request(f, path_bad)[0], 400)
        self.assertEqual(self.request(f, path, headers={'Host': 'attacker.invalid'})[0], 403)
        f.maintenance.set(); self.assertEqual(self.request(f, path)[0], 503)
        f.maintenance.clear(); f.stop.set(); self.assertEqual(self.request(f, path)[0], 503)

    def test_invalid_captured_link_never_rendered(self):
        f = self.fixture()
        f.mail.append({'to': RECIPIENT, 'kind': 'verification', 'link': 'javascript:alert(1)'})
        status, _, body = self.request(f, '/mail/inbox?to=' + RECIPIENT + '&kind=verification')
        self.assertEqual(status, 503)
        self.assertNotIn(b'javascript:', body)


if __name__ == '__main__': unittest.main()
