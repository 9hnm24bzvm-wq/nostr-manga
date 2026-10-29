"""Tests the actual opt-in fixture helper without launching a DB/browser.

The final UI run must separately prove the one synthetic registered user is
changed; these guard tests are not evidence of organic email verification.
"""
import ast
import json
import re
import tempfile
import threading
import time
import types
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SOURCE = (ROOT/'tests/runtime/browser_fixture.py').read_text()
TREE = ast.parse(SOURCE)
EMAIL = 'synthetic-reader-final@example.invalid'
USER = '12345678-abcd-abcd-abcd-123456789abc'


def actual(name, method=False, writes=None):
    nodes = next(n for n in TREE.body if isinstance(n, ast.ClassDef) and n.name == 'Fixture').body if method else TREE.body
    node = next(n for n in nodes if isinstance(n, ast.FunctionDef) and n.name == name)
    ns = {'READER_VERIFICATION_EMAIL': EMAIL, 'json': json, 're': re, 'time': time,
          'write_json': lambda path, value: writes.append((path.name, json.loads(json.dumps(value)))) if writes is not None else None}
    if method:
        ns['reader_verification_payload'] = actual('reader_verification_payload')
        ns['reader_verification_sql'] = actual('reader_verification_sql')
    exec(compile(ast.Module(body=[node], type_ignores=[]), '<actual-reader-verification-fixture>', 'exec'), ns)
    return ns[name]


class ReaderVerificationFixture(unittest.TestCase):
    def fixture(self, result=None):
        folder = tempfile.TemporaryDirectory()
        self.addCleanup(folder.cleanup)
        run = Path(folder.name)
        (run/'fixture-owned.json').write_text(json.dumps({'fixture_only': True, 'database': 'weebdex_browser_test', 'pg_port': 54321, 'synthetic_images': True}))
        calls = []
        def psql(sql):
            calls.append(sql)
            if isinstance(result, Exception): raise result
            return json.dumps({'id': USER, 'verified': True}) if result is None else result
        f = types.SimpleNamespace(reader_verification_control=True, args=types.SimpleNamespace(ubunchu_noncommercial=False),
            run=run, pg_port=54321, stop=threading.Event(), maintenance=threading.Event(), ready_at=time.time(),
            receipt={'production': False, 'database': 'weebdex_browser_test', 'deadline_unix': time.time()+300},
            started=time.time()-30, lock=threading.Lock(), reader_verification_attempts=0, psql=psql)
        return f, calls

    def test_fixed_email_exact_schema_no_normalization(self):
        check = actual('reader_verification_payload')
        self.assertEqual(check({'email': EMAIL}), {'email': EMAIL})
        for value in [None, [], {}, {'email': EMAIL.upper()}, {'email': ' '+EMAIL}, {'email': 'reader@example.invalid'},
                      {'email': 'fixtureseeder@example.invalid'}, {'email': 'synthetic-upload@example.invalid'},
                      {'email': EMAIL, 'verified': True}, {'email': EMAIL, 'profile': 'staff'}, {'email': False}, {'email': EMAIL, 'token': 'test'}]:
            with self.subTest(value=value), self.assertRaises(ValueError): check(value)

    def test_transaction_has_exact_target_eligibility_and_only_two_mutated_fields(self):
        sql = actual('reader_verification_sql')(1790000000.5)
        self.assertIn('BEGIN;', sql)
        self.assertIn("SET LOCAL statement_timeout='3000ms'", sql)
        self.assertIn("SET LOCAL lock_timeout='1000ms'", sql)
        self.assertIn(f"u.email='{EMAIL}'", sql)
        self.assertIn("current_database()='weebdex_browser_test'", sql)
        self.assertIn('u.created_at >= to_timestamp(1790000000.500000)', sql)
        self.assertIn("NOT u.verified AND NOT u.disabled AND u.roles='[\"user\"]'::jsonb", sql)
        self.assertIn('FOR UPDATE OF u', sql)
        self.assertIn('EXISTS (SELECT 1 FROM platform.sessions', sql)
        self.assertIn('s.revoked_at IS NULL AND s.expires_at>clock_timestamp()', sql)
        self.assertIn('UPDATE platform.users u SET verified=TRUE,version=u.version+1\n FROM candidate', sql)
        self.assertEqual(sql.count('UPDATE platform.'), 1)
        self.assertIn("SELECT jsonb_build_object('id',id,'verified',verified) FROM changed", sql)
        self.assertIn("u.id ~ '^[a-f0-9]{8}-", sql)
        for forbidden in ['password', 'token', 'cookie', 'csrf', 'permissions', 'catalog_group_members', 'INSERT ', 'DELETE ']:
            self.assertNotIn(forbidden.lower(), sql.lower())
        for value in [True, None, '1790000000', -1, float('nan'), float('inf')]:
            with self.assertRaises(ValueError): actual('reader_verification_sql')(value)

    def test_positive_is_single_use_and_receipt_explicitly_simulated(self):
        writes = []
        action = actual('reader_verification', True, writes)
        f, calls = self.fixture()
        receipt = action(f, {'email': EMAIL})
        self.assertEqual(len(calls), 1)
        self.assertEqual(receipt['status'], 'verified_test_precondition')
        self.assertTrue(receipt['simulated_test_precondition'])
        self.assertFalse(receipt['organic_email_verification_completed'])
        self.assertEqual(receipt['account_id'], USER)
        for key in ['roles_changed', 'permissions_changed', 'groups_changed', 'password_or_session_changed']:
            self.assertIs(receipt[key], False)
        self.assertEqual(writes[0][1]['status'], 'attempt_reserved')
        self.assertEqual(writes[-2][1]['status'], 'verified_test_precondition')
        with self.assertRaises(ValueError): action(f, {'email': EMAIL})
        self.assertEqual(len(calls), 1)

    def test_disabled_licensed_not_ready_draining_deadline_refuse_before_sql(self):
        changes = [lambda f: setattr(f, 'reader_verification_control', False),
                   lambda f: setattr(f, 'reader_verification_control', 1),
                   lambda f: setattr(f.args, 'ubunchu_noncommercial', True),
                   lambda f: setattr(f, 'ready_at', None), lambda f: f.stop.set(), lambda f: f.maintenance.set(),
                   lambda f: f.receipt.update(deadline_unix=time.time()+29)]
        for change in changes:
            with self.subTest(change=change):
                f, calls = self.fixture(); change(f)
                with self.assertRaises(ValueError): actual('reader_verification', True)(f, {'email': EMAIL})
                self.assertEqual(calls, [])
                self.assertEqual(f.reader_verification_attempts, 0)

    def test_wrong_database_ownership_and_non_synthetic_refuse(self):
        for change in [{'fixture_only': False}, {'database': 'production'}, {'pg_port': 54322}, {'synthetic_images': False}]:
            f, calls = self.fixture()
            p = f.run/'fixture-owned.json'; data = json.loads(p.read_text()); data.update(change); p.write_text(json.dumps(data))
            with self.assertRaises(ValueError): actual('reader_verification', True)(f, {'email': EMAIL})
            self.assertEqual(calls, [])
        for change in [{'production': True}, {'database': 'other'}]:
            f, calls = self.fixture(); f.receipt.update(change)
            with self.assertRaises(ValueError): actual('reader_verification', True)(f, {'email': EMAIL})
            self.assertEqual(calls, [])

    def test_nonexistent_already_verified_staff_disabled_or_no_session_result_cannot_retry(self):
        # All ineligible rows are excluded by the inspected SQL predicate and
        # produce no RETURNING row. A refusal still consumes the attempt.
        for label in ['nonexistent', 'already_verified', 'staff', 'disabled', 'no_session', 'old_account', 'non_uuid']:
            with self.subTest(label=label):
                f, calls = self.fixture(result='')
                action = actual('reader_verification', True)
                with self.assertRaises(RuntimeError): action(f, {'email': EMAIL})
                self.assertEqual(f.reader_verification_attempts, 1)
                self.assertEqual(f.receipt['reader_verification']['status'], 'refused_or_uncertain')
                with self.assertRaises(ValueError): action(f, {'email': EMAIL})
                self.assertEqual(len(calls), 1)

    def test_uncertain_sql_and_malformed_or_multiple_results_are_not_replayed(self):
        bad_results = [RuntimeError('simulated timeout'), '{}', '{', '[]', 'null',
                       json.dumps({'id': USER, 'verified': False}),
                       json.dumps({'id': 'SeedStaff1', 'verified': True}),
                       json.dumps({'id': USER, 'verified': True, 'extra': True}),
                       json.dumps({'id': USER, 'verified': True})+'\n'+json.dumps({'id': USER, 'verified': True})]
        for result in bad_results:
            f, calls = self.fixture(result=result); action = actual('reader_verification', True)
            with self.assertRaises((RuntimeError, ValueError)): action(f, {'email': EMAIL})
            self.assertEqual(f.receipt['reader_verification']['status'], 'refused_or_uncertain')
            with self.assertRaises(ValueError): action(f, {'email': EMAIL})
            self.assertEqual(len(calls), 1)

    def test_concurrent_attempt_is_reserved_before_sql(self):
        f, _ = self.fixture(); entered = threading.Event(); release = threading.Event(); results = []
        def psql(_):
            entered.set(); release.wait(timeout=2)
            return json.dumps({'id': USER, 'verified': True})
        f.psql = psql; action = actual('reader_verification', True)
        worker = threading.Thread(target=lambda: results.append(action(f, {'email': EMAIL})))
        worker.start(); self.assertTrue(entered.wait(timeout=2))
        try:
            with self.assertRaises(ValueError): action(f, {'email': EMAIL})
        finally: release.set(); worker.join(timeout=3)
        self.assertEqual(len(results), 1)

    def test_cli_default_off_and_sink_only_route(self):
        self.assertIn("parser.add_argument('--reader-verification-control',action='store_true'", SOURCE)
        self.assertIn('if args.reader_verification_control and args.ubunchu_noncommercial:parser.error(', SOURCE)
        self.assertIn("if self.path=='/fixture/reader-verification':", SOURCE)
        self.assertIn("if not fixture.reader_verification_control:return self.respond(404", SOURCE)
        self.assertIn("'reader_verification':self.receipt['reader_verification']", SOURCE)
        self.assertIn("if origin is not None and origin!=fixture.origin: return self.respond(403", SOURCE)
        psql = next(n for n in next(n for n in TREE.body if isinstance(n, ast.ClassDef) and n.name == 'Fixture').body if isinstance(n, ast.FunctionDef) and n.name == 'psql')
        self.assertIn("database='weebdex_browser_test'", ast.unparse(psql))
        self.assertIn("'127.0.0.1'", ast.unparse(psql))
        self.assertIn('timeout=15', ast.unparse(psql))


if __name__ == '__main__': unittest.main()
