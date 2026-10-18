"""Pure producer-receipt controls; no worker, browser, keys or runtime started."""
import hashlib
import importlib.util
import contextlib
import io
import json
import os
import shutil
import subprocess
import sys
import tempfile
from types import SimpleNamespace
from pathlib import Path
import unittest
from unittest import mock

spec = importlib.util.spec_from_file_location('sealed_owner', Path(__file__).parents[2] / 'scripts/ci/run-sandbox-preflight.py')
owner = importlib.util.module_from_spec(spec)
spec.loader.exec_module(owner)


class SealReadyTests(unittest.TestCase):
    def clean(self):
        return dict(verified_stopped=True, forced=False, stdout_eof=True,
                    remaining_owned=0, unknown_births=0, tracking_errors=0)

    def test_receipt_binds_exact_public_and_full_log_bytes(self):
        public, log = b'{"synthetic":true}\n', b'private synthetic startup\x00\xff'
        value = owner.seal_ready_receipt(public, log, self.clean(), False)
        self.assertEqual(value, dict(schema=1, public_record_sha256=hashlib.sha256(public).hexdigest(),
                                    log_sha256=hashlib.sha256(log).hexdigest(), log_bytes=len(log)))
        self.assertNotIn('private', str(value))

    def test_eof_and_normal_cleanup_are_required(self):
        for key, value in [('verified_stopped', False), ('forced', True), ('stdout_eof', False),
                           ('remaining_owned', 1), ('unknown_births', 1), ('tracking_errors', 1)]:
            with self.subTest(key=key):
                clean = self.clean(); clean[key] = value
                self.assertIsNone(owner.seal_ready_receipt(b'{}', b'log', clean, False))

    def test_missing_null_and_bool_as_integer_cleanup_are_refused(self):
        for key in self.clean():
            for value in [None, '0']:
                clean = self.clean(); clean[key] = value
                self.assertIsNone(owner.seal_ready_receipt(b'{}', b'log', clean, False))
            clean = self.clean(); del clean[key]
            self.assertIsNone(owner.seal_ready_receipt(b'{}', b'log', clean, False))
        clean = self.clean(); clean['remaining_owned'] = False
        self.assertIsNone(owner.seal_ready_receipt(b'{}', b'log', clean, False))

    def test_overflow_and_oversized_input_refuse_instead_of_truncating(self):
        self.assertIsNone(owner.seal_ready_receipt(b'{}', b'log', self.clean(), True))
        self.assertIsNone(owner.seal_ready_receipt(b'x' * (owner.MAX_JSON + 1), b'log', self.clean(), False))
        self.assertIsNone(owner.seal_ready_receipt(b'{}', b'x' * (owner.MAX_LOG + 1), self.clean(), False))

    def test_exact_reader_limits_are_preserved_for_later_total_envelope_limit(self):
        log = b'x' * owner.MAX_LOG
        value = owner.seal_ready_receipt(b'x' * owner.MAX_JSON, log, self.clean(), False)
        self.assertEqual(value['log_bytes'], owner.MAX_LOG)

    def test_nonbytes_are_not_coerced(self):
        for value in [None, 'private', bytearray(b'log'), 1]:
            self.assertIsNone(owner.seal_ready_receipt(value, b'log', self.clean(), False))
            self.assertIsNone(owner.seal_ready_receipt(b'{}', value, self.clean(), False))

    def test_main_emits_private_binding_only_after_new_public_and_normal_eof(self):
        for change in [{}, {'forced': True}, {'stdout_eof': False}, {'overflow': True}]:
            with self.subTest(change=change):
                # Owned synthetic files only; no browser, Node worker or private key.
                root = Path(tempfile.mkdtemp(prefix='sealed-owner-synthetic-')).resolve()
                runtime = root / '.build/hosted-browser/runtime.json'
                runtime.parent.mkdir(parents=True)
                runtime.write_text(json.dumps(dict(schema=1, playwright='1.62.1',
                    chromium_revision='1234', npm_hooks_executed=False, executable_sha256='a' * 64)))
                node = root / 'synthetic-node'; node.write_text('not executed')
                public = root / 'ci-evidence/sandbox-preflight.json'
                clean = dict(self.clean(), **{k: v for k, v in change.items() if k != 'overflow'})
                class StandInOwner:
                    def __init__(self, private, started):
                        self.private = private
                        self.child = SimpleNamespace(returncode=1)
                        self.output_overflow = change.get('overflow', False)
                        self.interrupted = False
                    def launch(self, *_):
                        (self.private / 'worker.log').write_bytes(b'synthetic full log\n')
                    def observe(self):
                        return 'worker-failed'
                    def cleanup(self):
                        if public.exists():
                            raise AssertionError('Public record precedes cleanup')
                        return clean
                stdout = io.StringIO()
                with mock.patch.object(owner, 'ROOT', root), mock.patch.object(owner, 'Owner', StandInOwner), \
                        mock.patch.object(owner.os, 'getuid', return_value=1000), \
                        mock.patch.object(owner.ctypes, 'CDLL', return_value=SimpleNamespace(prctl=lambda *_: 0)), \
                        mock.patch.object(sys, 'argv', ['owner', '--node', str(node)]), \
                        contextlib.redirect_stdout(stdout):
                    self.assertEqual(owner.main(), 1)
                public_bytes = public.read_bytes()
                owner.validate_public(json.loads(public_bytes))
                self.assertTrue(stdout.getvalue().startswith('evidence_sha256=' + hashlib.sha256(public_bytes).hexdigest()))
                receipt = root / '.build/sandbox-preflight/seal-ready.json'
                if change:
                    self.assertFalse(receipt.exists())
                else:
                    self.assertEqual(json.loads(receipt.read_bytes()), owner.seal_ready_receipt(
                        public_bytes, b'synthetic full log\n', clean, False))
                    self.assertNotIn('synthetic full log', receipt.read_text())

    @unittest.skipUnless(hasattr(os, 'mkfifo'), 'POSIX FIFO control')
    def test_nonregular_record_receipt_log_and_public_key_refuse_without_blocking(self):
        node = shutil.which('node')
        self.assertIsNotNone(node, 'An already-installed Node executable is required; no install/fallback')
        helper = Path(__file__).parents[2] / 'scripts/ci/sealed-sandbox-log.mjs'
        public, log = b'{}\n', b'synthetic'
        receipt = dict(schema=1, public_record_sha256=hashlib.sha256(public).hexdigest(),
                       log_sha256=hashlib.sha256(log).hexdigest(), log_bytes=len(log))
        targets = ['ci-evidence/sandbox-preflight.json', '.build/sandbox-preflight/seal-ready.json',
                   '.build/sandbox-preflight/worker.log', '.build/sandbox-preflight-recipient/public.pem']
        for target in targets:
            with self.subTest(target=target):
                root = Path(tempfile.mkdtemp(prefix='sealed-fifo-synthetic-')).resolve()
                files = {targets[0]: public, targets[1]: (json.dumps(receipt, sort_keys=True) + '\n').encode(),
                         targets[2]: log, targets[3]: b'not reached'}
                for name, data in files.items():
                    output = root / name; output.parent.mkdir(parents=True, exist_ok=True)
                    if name == target:
                        os.mkfifo(output, 0o600)
                    else:
                        output.write_bytes(data)
                program = ("import {sealRecordedLog,SealedLogRefused} from " + json.dumps(helper.as_uri()) + ";"
                    "try { sealRecordedLog(" + json.dumps(dict(root=str(root), env={},
                    expectedEvidenceSha256=receipt['public_record_sha256'], expectedRecipientSha256='a' * 64)) +
                    "); process.exitCode=2; } catch(e) {if (!(e instanceof SealedLogRefused)) process.exitCode=3; else console.log('refused');}")
                result = subprocess.run([node, '--input-type=module', '-e', program],
                    stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                    timeout=3, env={'PATH': '/usr/bin:/bin', 'NODE_OPTIONS': '--max-old-space-size=128'})
                self.assertEqual(result.returncode, 0)
                self.assertEqual(result.stdout, b'refused\n')
                self.assertEqual(result.stderr, b'')
                self.assertFalse((root / '.build/sandbox-preflight/sealed-worker-log.json').exists())


if __name__ == '__main__':
    unittest.main()
