"""Local fixture sanity only. These direct signals are not systemd evidence."""
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import time
import unittest

SCRIPT = Path(__file__).resolve().parents[2] / "scripts/test-systemd-shutdown.py"


class SystemdFixtureSanity(unittest.TestCase):
    def run_fixture(self, interrupt_child):
        with tempfile.TemporaryDirectory(prefix="wdx-stop-fixture-") as name:
            root = Path(name)
            owner = subprocess.Popen([sys.executable, str(SCRIPT), "--fixture", "owner", name, "graceful"], start_new_session=True)
            timeout_pid = None
            try:
                deadline = time.monotonic() + 5
                while not (root / "owner-ready.json").exists():
                    self.assertIsNone(owner.poll(), "owner exited before fixture readiness")
                    self.assertLess(time.monotonic(), deadline)
                    time.sleep(.02)
                timeout_pid = json.loads((root / "owner-ready.json").read_text())["timeout_pid"]
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
                if owner.poll() is None:
                    if timeout_pid:
                        try:
                            if os.getpgid(timeout_pid) == timeout_pid and os.getsid(timeout_pid) == owner.pid:
                                os.killpg(timeout_pid, signal.SIGKILL)
                        except ProcessLookupError:
                            pass
                    os.killpg(owner.pid, signal.SIGKILL)
                    owner.wait(timeout=5)

    def test_main_only_signal_allows_owned_work_to_finish(self):
        self.run_fixture(False)

    def test_premature_child_signal_prevents_false_success(self):
        self.run_fixture(True)


if __name__ == "__main__":
    unittest.main()
