"""Template/config regression checks, not installed service-stop acceptance."""
import importlib.util
import json
import re
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("shutdown_contract", ROOT / "scripts/check-service-shutdown.py")
CONTRACT = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CONTRACT)
UNIT = (ROOT / "deployment/weebdex.service.example").read_text()


class ServiceShutdownContract(unittest.TestCase):
    def test_example_configs_and_source_default_fit_manager_budget(self):
        defaults = re.findall(r"shutdown_grace_seconds:\s*(\d+)\s*,", (ROOT / "backend/src/config.rs").read_text())
        self.assertEqual(defaults, ["210"])
        for config in ROOT.glob("deployment/platform.*.example.json"):
            with self.subTest(config=config.name):
                self.assertEqual(CONTRACT.check(UNIT, json.loads(config.read_text()), 210)["manager_stop_seconds"], 225)
        self.assertEqual(CONTRACT.check(UNIT, {}, 210)["application_grace_seconds"], 210)

    def test_missing_or_wrong_kill_mode_fails(self):
        for mutation in ("", "KillMode=control-group", "KillMode=process", "KillMode=none"):
            with self.subTest(mutation=mutation), self.assertRaises(ValueError):
                CONTRACT.check(UNIT.replace("KillMode=mixed", mutation), {}, 210)

    def test_actual_config_override_must_fit_not_just_default(self):
        for grace in (225, 226, 900, True, "210", 0):
            with self.subTest(grace=grace), self.assertRaises(ValueError):
                CONTRACT.check(UNIT, {"shutdown_grace_seconds": grace}, 210)
        self.assertEqual(CONTRACT.check(UNIT, {"shutdown_grace_seconds": 224}, 210)["application_grace_seconds"], 224)

    def test_manager_deadline_must_be_finite_and_strictly_longer(self):
        for stop in ("210", "209s", "infinity", "", "225junk", "9" * 400):
            with self.subTest(stop=stop), self.assertRaises(ValueError):
                CONTRACT.check(UNIT.replace("TimeoutStopSec=225", "TimeoutStopSec=" + stop), {}, 210)
        self.assertEqual(CONTRACT.seconds("3min 45s"), 225)

    def test_final_cgroup_cleanup_cannot_be_disabled(self):
        for directive in ("SendSIGKILL=no", "FinalKillSignal=SIGTERM", "KillSignal=SIGINT"):
            text = UNIT.replace("KillSignal=SIGTERM", directive) if directive.startswith("KillSignal=") else UNIT + "\n[Service]\n" + directive
            with self.subTest(directive=directive), self.assertRaises(ValueError):
                CONTRACT.check(text, {}, 210)

    def test_stop_and_restart_cannot_interrupt_owned_children(self):
        for directive in ("SendSIGHUP=yes", "RestartKillSignal=SIGKILL", "ExecStop=/usr/bin/killall strfry"):
            with self.subTest(directive=directive), self.assertRaises(ValueError):
                CONTRACT.check(UNIT + "\n[Service]\n" + directive, {}, 210)

    def test_main_owner_is_exec_not_extra_wrapper(self):
        self.assertIn('exec "$binary" "$action"', (ROOT / "scripts/run-platform.sh").read_text())
        with self.assertRaises(ValueError):
            CONTRACT.check(UNIT.replace("/scripts/run-platform.sh serve", "/scripts/other-wrapper.sh serve"), {}, 210)

    def test_duplicate_shutdown_directives_are_ambiguous(self):
        with self.assertRaises(ValueError):
            CONTRACT.check(UNIT + "\n[Service]\nKillMode=control-group\n", {}, 210)


if __name__ == "__main__":
    unittest.main()
