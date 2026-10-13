"""On-disk lifecycle seams for the exact offline recovery-test projection."""
import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]
SCRIPTS = ROOT / "weebdex-platform/scripts"
SPEC = importlib.util.spec_from_file_location("backup_runner", SCRIPTS / "run-backup-tests.py")
RUNNER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(RUNNER)
ONE_PASS = ("running 1 test\n"
            "test " + RUNNER.TEST + " ... ok\n"
            "test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out\n")


class BackupRunnerLifecycle(unittest.TestCase):
    def bounded(self, leak=False):
        with tempfile.TemporaryDirectory(prefix="synthetic-backup-lifecycle-") as directory:
            root = Path(directory)
            # The pipe proves the owned descendant is alive before its parent
            # emits the exact passing result and exits. No unrelated PID is used.
            code = "import os,time\n"
            if leak:
                code += ("r,w=os.pipe()\n"
                         "if os.fork()==0:\n"
                         " os.close(r); os.write(w,b'1'); os.close(w); time.sleep(60); os._exit(0)\n"
                         "os.close(w); assert os.read(r,1)==b'1'; os.close(r)\n")
            code += "print(" + repr(ONE_PASS) + ",end='',flush=True)\n"
            result = subprocess.run([sys.executable, str(SCRIPTS / "run-bounded.py"),
                "--cwd", str(root), "--deadline", "10", "--receipt", str(root / "lifecycle.json"),
                "--log", str(root / "test.log"), "--", sys.executable, "-c", code],
                stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, timeout=20)
            self.assertEqual(result.returncode, 0, result.stderr)
            raw = json.loads((root / "lifecycle.json").read_text())
            log = (root / "test.log").read_text()
            return raw, RUNNER.project_summary(raw, log, result.returncode)

    def test_natural_one_pass_lifecycle_projects_passed(self):
        raw, summary = self.bounded()
        self.assertEqual(raw["cleanup_signals"], [])
        self.assertEqual(summary["status"], "passed")
        self.assertEqual(summary["executed_tests"], 1)

    def test_exact_one_pass_with_forced_descendant_cleanup_projects_failed(self):
        raw, summary = self.bounded(leak=True)
        # Reproduce the supervisor's exit-zero/clean-after-kill false-pass seam.
        self.assertEqual(raw["process_exit_code"], 0)
        self.assertTrue(raw["cleanup_verified"])
        self.assertEqual(raw["surviving_owned_process_ids"], [])
        self.assertTrue(raw["cleanup_signals"])
        self.assertEqual(summary["executed_tests"], 1)
        self.assertEqual(summary["status"], "failed")
        self.assertFalse(summary["cleanup_verified"])

    def test_incomplete_or_untrusted_lifecycle_cannot_project_passed(self):
        raw, _ = self.bounded()
        for field, value in (("terminal_receipt", False), ("cleanup_signals", ["SIGKILL"]),
                             ("surviving_owned_process_ids", [123]), ("cleanup_verified", False),
                             ("deadline_exceeded", True), ("prior_or_unrelated_processes_touched", True),
                             ("process_exit_code", True)):
            with self.subTest(field=field):
                self.assertEqual(RUNNER.project_summary({**raw, field:value}, ONE_PASS, 0)["status"], "failed")
        for field in ("terminal_receipt", "cleanup_signals", "surviving_owned_process_ids", "deadline_exceeded"):
            incomplete = dict(raw)
            incomplete.pop(field)
            self.assertEqual(RUNNER.project_summary(incomplete, ONE_PASS, 0)["status"], "failed")

    def test_zero_or_duplicate_results_cannot_project_passed(self):
        raw, _ = self.bounded()
        for log in ("running 0 tests\ntest result: ok. 0 passed; 0 failed; 0 ignored;\n", ONE_PASS * 2):
            summary = RUNNER.project_summary(raw, log, 0)
            self.assertEqual(summary["status"], "failed")
            self.assertEqual(summary["executed_tests"], 0)
