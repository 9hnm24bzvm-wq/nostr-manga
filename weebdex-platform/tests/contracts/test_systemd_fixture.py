"""Local fixture sanity only. These direct signals are not systemd evidence."""
from contextlib import contextmanager
import ctypes
import importlib.util
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import time
import unittest
from unittest.mock import patch
from types import SimpleNamespace

SCRIPT = Path(__file__).resolve().parents[2] / "scripts/test-systemd-shutdown.py"
SPEC = importlib.util.spec_from_file_location("systemd_fixture", SCRIPT)
FIXTURE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(FIXTURE)


def identity(pid):
    try:
        fields = Path(f"/proc/{pid}/stat").read_text().rsplit(") ", 1)[1].split()
        return {"pid": pid, "session": int(fields[3]), "start": int(fields[19]), "state": fields[0]}
    except (OSError, ValueError, IndexError):
        return None


def session_members(owner):
    result = []
    for path in Path("/proc").iterdir():
        if path.name.isdigit():
            row = identity(int(path.name))
            if row and row["session"] == owner["pid"] and row["start"] >= owner["start"]:
                result.append(row)
    return result


def same_process(saved, current):
    return current and all(current[key] == saved[key] for key in ("pid", "session", "start"))


@contextmanager
def adopt_orphans():
    # Process-local test ownership only; preserve the caller's prior setting.
    libc = ctypes.CDLL(None, use_errno=True)
    previous = ctypes.c_int()
    if libc.prctl(37, ctypes.byref(previous), 0, 0, 0) or libc.prctl(36, 1, 0, 0, 0):
        raise OSError(ctypes.get_errno(), "cannot establish fixture subreaper ownership")
    try:
        yield
    finally:
        if libc.prctl(36, previous.value, 0, 0, 0):
            raise OSError(ctypes.get_errno(), "cannot restore subreaper setting")


def cleanup(owner, saved):
    """Kill/reap only verified members of this newly created session, even after owner exit."""
    remembered = {}
    end = time.monotonic() + 5
    while time.monotonic() < end:
        current = session_members(saved)
        for row in current:
            remembered[(row["pid"], row["start"])] = row
            if row["state"] == "Z":
                continue
            try:
                descriptor = os.pidfd_open(row["pid"])
            except ProcessLookupError:
                continue
            try:
                if same_process(row, identity(row["pid"])):
                    signal.pidfd_send_signal(descriptor, signal.SIGKILL)
            except ProcessLookupError:
                pass
            finally:
                os.close(descriptor)
        owner.poll()  # Reap the direct child using its Popen bookkeeping.
        for row in remembered.values():
            if row["pid"] != owner.pid and same_process(row, identity(row["pid"])):
                try:
                    os.waitpid(row["pid"], os.WNOHANG)
                except ChildProcessError:
                    pass  # Adoption may await the intermediate parent's exit.
        if not session_members(saved):
            owner.wait(timeout=1)
            return
        time.sleep(.02)
    raise AssertionError("exact fixture session did not terminate and reap within cleanup deadline")


class SystemdFixtureSanity(unittest.TestCase):
    def start(self, root, startup_delay=False):
        if startup_delay:
            # The wrapper retains the owner session but stalls owner-ready publication,
            # giving the test a deterministic failed-startup window after child spawn.
            code = '''import importlib.util, pathlib, sys, time
spec = importlib.util.spec_from_file_location("fixture", sys.argv[1])
fixture = importlib.util.module_from_spec(spec); spec.loader.exec_module(fixture)
original = fixture.durable
def durable(path, value):
    if path.name == "owner-ready.json": time.sleep(60)
    original(path, value)
fixture.durable = durable
raise SystemExit(fixture.fixture("owner", pathlib.Path(sys.argv[2]), "graceful"))
'''
            command = [sys.executable, "-c", code, str(SCRIPT), str(root)]
        else:
            command = [sys.executable, str(SCRIPT), "--fixture", "owner", str(root), "graceful"]
        start_floor = int(time.clock_gettime(time.CLOCK_BOOTTIME) * os.sysconf("SC_CLK_TCK"))
        owner = subprocess.Popen(command, start_new_session=True)
        saved = identity(owner.pid) or {"pid": owner.pid, "session": owner.pid, "start": start_floor}
        return owner, saved

    def wait_marker(self, root, marker, owner):
        deadline = time.monotonic() + 5
        while not (root / marker).exists():
            self.assertIsNone(owner.poll(), "owner exited before fixture readiness")
            self.assertLess(time.monotonic(), deadline)
            time.sleep(.02)

    def run_fixture(self, interrupt_child):
        with adopt_orphans(), tempfile.TemporaryDirectory(prefix="wdx-stop-fixture-") as name:
            root = Path(name)
            owner, saved = None, None
            try:
                owner, saved = self.start(root)
                self.wait_marker(root, "owner-ready.json", owner)
                child_pid = json.loads((root / "child-ready.json").read_text())["pid"]
                self.assertTrue((root / "pending.json").exists())
                self.assertFalse((root / "committed.json").exists())
                if interrupt_child:
                    os.kill(child_pid, signal.SIGTERM)
                owner.send_signal(signal.SIGTERM)
                code = owner.wait(timeout=5)
                self.assertTrue((root / "readiness-withdrawn.json").exists())
                if interrupt_child:
                    self.assertEqual(code, 74)
                    self.assertTrue((root / "child-term.json").exists())
                    self.assertTrue((root / "drain-failed.json").exists())
                    self.assertFalse((root / "committed.json").exists())
                else:
                    self.assertEqual(code, 0)
                    self.assertTrue((root / "drained.json").exists())
                    self.assertTrue((root / "committed.json").exists())
                    self.assertFalse((root / "child-term.json").exists())
            finally:
                if owner is not None:
                    cleanup(owner, saved)

    def fault_cleanup(self, startup_delay):
        with adopt_orphans(), tempfile.TemporaryDirectory(prefix="wdx-stop-fault-") as name:
            root = Path(name)
            sibling = subprocess.Popen([sys.executable, "-c", "import time; time.sleep(30)"], start_new_session=True)
            owner, saved = None, None
            try:
                owner, saved = self.start(root, startup_delay)
                self.wait_marker(root, "child-ready.json" if startup_delay else "owner-ready.json", owner)
                if startup_delay:
                    self.assertFalse((root / "owner-ready.json").exists())
                owner.kill(); owner.wait(timeout=3)
                self.assertTrue(session_members(saved), "fault must leave actual owned descendants")
                cleanup(owner, saved)
                self.assertEqual(session_members(saved), [])
                self.assertIsNone(sibling.poll(), "cleanup must preserve unrelated sibling")
                lock = subprocess.run(["flock", "--nonblock", str(root / "command.lock"), "true"], timeout=3)
                self.assertEqual(lock.returncode, 0, "cleanup must release actual owned lock")
                self.assertFalse((root / "committed.json").exists())
            finally:
                try:
                    if owner is not None:
                        cleanup(owner, saved)
                finally:
                    sibling.terminate(); sibling.wait(timeout=3)

    def test_root_user_manager_cannot_bypass_nonroot_check_with_override(self):
        args = SimpleNamespace(manager="user", fixture_uid=1000, fixture_gid=1000)
        with patch.object(FIXTURE.os, "geteuid", return_value=0), patch.object(FIXTURE.os, "getegid", return_value=0):
            with self.assertRaisesRegex(RuntimeError, "actual non-root caller"):
                FIXTURE.validate_identity(args)

    def test_user_manager_rejects_ignored_identity_overrides(self):
        with patch.object(FIXTURE.os, "geteuid", return_value=1000), patch.object(FIXTURE.os, "getegid", return_value=1000):
            for uid, gid in ((1001, 1000), (1000, 1001)):
                with self.subTest(uid=uid, gid=gid), self.assertRaisesRegex(RuntimeError, "actual non-root caller"):
                    FIXTURE.validate_identity(SimpleNamespace(manager="user", fixture_uid=uid, fixture_gid=gid))
            FIXTURE.validate_identity(SimpleNamespace(manager="user", fixture_uid=1000, fixture_gid=1000))

    def test_main_only_signal_allows_owned_work_to_finish(self):
        self.run_fixture(False)

    def test_premature_child_signal_prevents_false_success(self):
        self.run_fixture(True)

    def test_owner_crash_cleans_descendants_without_touching_sibling(self):
        self.fault_cleanup(False)

    def test_failed_startup_before_owner_marker_still_cleans_descendants(self):
        self.fault_cleanup(True)


if __name__ == "__main__":
    unittest.main()
