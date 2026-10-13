"""Actual operator CLI, inherited pipes, kernel process handles and nginx HTTP."""

import ctypes
import hashlib
import json
import os
from pathlib import Path
import select
import signal
import socket
import subprocess
import sys
import time
import urllib.error
import urllib.request
from operator_support import OperatorCase, ROOT, ENV

NGINX = ROOT / "vendor/nginx-1.28.0/objs/nginx"
OWNER = "ab" * 32


class ServingCLI(OperatorCase):
    @classmethod
    def setUpClass(cls):
        if os.geteuid() == 0 or not NGINX.is_file() or not hasattr(os, "pidfd_open"):
            raise RuntimeError(
                "unprivileged Linux, pidfds and pinned nginx are required; no modeled fallback"
            )
        if ctypes.CDLL(None, use_errno=True).prctl(36, 1, 0, 0, 0) != 0:
            raise RuntimeError("subreaper unavailable for isolated crash-test cleanup")

    def setUp(self):
        super().setUp()
        self.public = self.root / "public"
        self.runtime = self.public / ".serving"
        with socket.socket() as s:
            s.bind(("127.0.0.1", 0))
            self.port = s.getsockname()[1]
        self.children = []
        self.handles = []
        self.addCleanup(self.cleanup)

    def args(self, action="serve", owner=OWNER, runtime=None):
        return [
            sys.executable,
            "-m",
            "src.serving_supervisor",
            action,
            "--public",
            str(self.public),
            "--runtime",
            str(runtime or self.runtime),
            "--nginx",
            str(NGINX),
            "--owner",
            owner,
            "--bind",
            f"127.0.0.1:{self.port}",
            "--shutdown-seconds",
            "1",
        ]

    def start(self, action="serve"):
        child = subprocess.Popen(
            self.args(action),
            cwd=ROOT,
            env=ENV,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        self.children.append(child)
        line = self.line(child)
        self.assertTrue(line["ready"], line)
        return child

    def line(self, child, seconds=12):
        self.assertTrue(
            select.select([child.stdout], [], [], seconds)[0],
            "operator response deadline",
        )
        line = child.stdout.readline()
        if not line:
            child.wait(timeout=3)
            self.fail(child.stderr.read().decode())
        return json.loads(line)

    def rpc(self, child, op):
        nonce = os.urandom(32).hex()
        child.stdin.write((json.dumps({"op": op, "nonce": nonce}) + "\n").encode())
        child.stdin.flush()
        reply = self.line(child)
        self.assertEqual(reply["nonce"], nonce)
        return reply

    def status(self, child):
        reply = self.rpc(child, "status")
        self.assertTrue(reply["ok"], reply)
        return reply["result"]

    def observe(self, child):
        workers = self.status(child)["workers"]
        handles = []
        for worker in workers:
            fd = os.pidfd_open(worker["pid"])
            self.handles.append(fd)
            handles.append(fd)
        self.assertTrue(handles)
        return workers, handles

    def gone(self, handles, seconds=8):
        end = time.monotonic() + seconds
        while time.monotonic() < end:
            if all(select.select([fd], [], [], 0)[0] for fd in handles):
                return
            time.sleep(0.02)
        self.fail("published reader process still alive")

    def http(self, path="/_serving", headers=None):
        return urllib.request.urlopen(
            urllib.request.Request(
                f"http://127.0.0.1:{self.port}" + path, headers=headers or {}
            ),
            timeout=4,
        )

    def cleanup(self):
        for child in self.children:
            if child.stdin and not child.stdin.closed:
                child.stdin.close()
            if child.poll() is None:
                try:
                    child.wait(timeout=8)
                except subprocess.TimeoutExpired:
                    child.kill()
                    child.wait(timeout=3)
            child.stdout.close()
            child.stderr.close()
        if self.runtime.exists():
            result = subprocess.run(
                self.args("recover"), cwd=ROOT, env=ENV, capture_output=True, timeout=12
            )
            self.assertEqual(result.returncode, 0, result.stderr.decode())
        for fd in self.handles:
            os.close(fd)
        while True:
            try:
                pid, _ = os.waitpid(-1, os.WNOHANG)
                if not pid:
                    break
            except ChildProcessError:
                break

    def test_prepared_activation_suspension_and_explicit_stop(self):
        child = self.start("prepare")
        self.assertFalse(self.status(child)["active"])
        self.assertTrue(self.rpc(child, "drain")["result"]["inactive"])
        self.assertTrue(self.rpc(child, "activate")["result"]["active"])
        _, handles = self.observe(child)
        with self.http() as response:
            self.assertEqual(response.status, 204)
        self.assertTrue(self.rpc(child, "suspend")["result"]["inactive"])
        self.gone(handles)
        self.assertFalse(self.status(child)["active"])
        self.assertTrue(self.rpc(child, "activate")["result"]["active"])
        self.assertTrue(self.rpc(child, "stop")["result"]["stopped"])
        child.wait(timeout=8)
        self.assertEqual(child.returncode, 0)

    def test_owner_eof_then_immediate_successor_owns_fresh_family(self):
        child = self.start()
        _, handles = self.observe(child)
        child.stdin.close()
        # Acquire through the same exclusive CLI after EOF; contention may race shutdown.
        end = time.monotonic() + 10
        successor = None
        while time.monotonic() < end:
            p = subprocess.Popen(
                self.args(),
                cwd=ROOT,
                env=ENV,
                stdin=subprocess.PIPE,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
            )
            self.children.append(p)
            self.assertTrue(select.select([p.stdout], [], [], 10)[0])
            line = p.stdout.readline()
            if line:
                self.assertTrue(json.loads(line)["ready"])
                successor = p
                break
            p.wait(timeout=3)
            self.assertIn(b"still owns", p.stderr.read())
            time.sleep(0.03)
        self.assertIsNotNone(successor)
        child.wait(timeout=5)
        self.assertEqual(child.returncode, 0)
        self.gone(handles)
        self.assertTrue(self.status(successor)["active"])

    def test_duplicate_owner_and_alternate_runtime_refused(self):
        child = self.start()
        for args in [self.args(), self.args(runtime=self.root / "alternate")]:
            result = subprocess.run(
                args, cwd=ROOT, env=ENV, capture_output=True, timeout=5
            )
            self.assertNotEqual(result.returncode, 0)
        self.assertTrue(self.status(child)["active"])

    def test_sigkill_owner_recovery_stops_surviving_readers(self):
        child = self.start()
        _, handles = self.observe(child)
        child.kill()
        child.wait(timeout=5)
        self.assertTrue(any(not select.select([fd], [], [], 0)[0] for fd in handles))
        result = subprocess.run(
            self.args("recover"), cwd=ROOT, env=ENV, capture_output=True, timeout=12
        )
        self.assertEqual(result.returncode, 0, result.stderr.decode())
        self.gone(handles)
        successor = self.start()
        self.assertTrue(self.status(successor)["active"])

    def test_wrong_owner_cannot_recover_or_signal_other_readers(self):
        child = self.start()
        _, handles = self.observe(child)
        child.kill()
        child.wait(timeout=5)
        result = subprocess.run(
            self.args("recover", owner="cd" * 32),
            cwd=ROOT,
            env=ENV,
            capture_output=True,
            timeout=6,
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertIn(b"owner/binary mismatch", result.stderr)
        self.assertTrue(any(not select.select([fd], [], [], 0)[0] for fd in handles))

    def test_dead_master_is_not_a_successful_drain_proof(self):
        child = self.start()
        workers, handles = self.observe(child)
        master = next(w["pid"] for w in workers if w["pid"] == w["session"])
        os.kill(master, signal.SIGKILL)
        # pidfd delivery, rather than a sleep, establishes that external fault occurred.
        self.gone(
            [fd for worker, fd in zip(workers, handles) if worker["pid"] == master]
        )
        self.assertFalse(self.status(child)["active"])
        activation = self.rpc(child, "activate")
        self.assertFalse(activation["ok"], "dead master cannot acknowledge activation")
        self.assertIn("master unavailable", activation["error"])
        result = self.rpc(child, "drain")
        self.assertFalse(result["ok"])
        self.assertIn("master unavailable", result["error"])
        self.assertTrue(self.rpc(child, "stop")["ok"])
        child.wait(timeout=8)
        self.gone(handles)

    def test_slow_unlinked_reader_drain_range_and_new_namespace(self):
        child = self.start()
        data = b"synthetic-original" * 600000
        digest = hashlib.sha256(data).hexdigest()
        alias = self.public / digest[:2] / digest[2:4] / (digest + ".png")
        alias.parent.mkdir(parents=True)
        alias.write_bytes(data)
        with self.http("/" + digest + ".png", {"Range": "bytes=8-1031"}) as response:
            self.assertEqual(response.status, 206)
            self.assertEqual(response.read(), data[8:1032])
        slow = socket.socket()
        self.addCleanup(slow.close)
        slow.setsockopt(socket.SOL_SOCKET, socket.SO_RCVBUF, 1024)
        slow.settimeout(5)
        slow.connect(("127.0.0.1", self.port))
        slow.sendall(
            f"GET /{digest}.png HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n".encode()
        )
        prefix = b""
        while b"\r\n\r\n" not in prefix:
            prefix += slow.recv(1)
        self.assertTrue(prefix.startswith(b"HTTP/1.1 200"))
        workers, handles = self.observe(child)
        alias.unlink()
        self.assertFalse(alias.exists())
        self.assertEqual(slow.recv(16), data[:16])
        result = self.rpc(child, "drain")
        self.assertTrue(result["ok"], result)
        self.assertTrue(result["result"]["drained"])
        retired = {x["pid"] for x in result["result"]["workers"]}
        self.gone([fd for w, fd in zip(workers, handles) if w["pid"] in retired])
        self.assertTrue(retired)
        with self.assertRaises(urllib.error.HTTPError) as missing:
            self.http("/" + digest + ".png")
        self.assertEqual(missing.exception.code, 404)
        self.assertTrue(self.status(child)["active"])

    def test_oversized_control_input_exits_and_stops_readers(self):
        child = self.start()
        _, handles = self.observe(child)
        child.stdin.write(b"x" * 16386 + b"\n")
        child.stdin.flush()
        child.wait(timeout=10)
        self.assertNotEqual(child.returncode, 0)
        self.gone(handles)
