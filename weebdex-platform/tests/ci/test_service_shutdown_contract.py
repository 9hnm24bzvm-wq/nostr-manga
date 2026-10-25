"""Static shipped-template contracts only; no application or systemd manager runs."""
import configparser
import json
from pathlib import Path
import unittest


ROOT = Path(__file__).resolve().parents[2]


class ServiceShutdownContract(unittest.TestCase):
    def setUp(self):
        # The template uses single-line values and repeated Environment entries.
        # This reads its relevant directives, not a substitute systemd parser.
        unit = configparser.ConfigParser(interpolation=None, strict=False)
        unit.optionxform = str
        unit.read_string((ROOT / 'deployment/weebdex.service.example').read_text())
        self.service = unit['Service']

    def test_main_only_initial_signal_preserves_final_group_cleanup(self):
        self.assertEqual(self.service.get('KillMode'), 'mixed')
        self.assertEqual(self.service.get('KillSignal'), 'SIGTERM')
        self.assertEqual(self.service.get('RestartKillSignal', 'SIGTERM'), 'SIGTERM')
        self.assertFalse(self.service.getboolean('SendSIGHUP', fallback=False))
        self.assertTrue(self.service.getboolean('SendSIGKILL', fallback=True))
        self.assertEqual(self.service.get('FinalKillSignal', 'SIGKILL'), 'SIGKILL')

    def test_exec_wrapper_preserves_application_as_manager_main_process(self):
        self.assertEqual(self.service.get('Type'), 'simple')
        self.assertEqual(self.service.get('ExecStart'),
                         '/opt/weebdex/weebdex-platform/scripts/run-platform.sh serve')
        self.assertNotIn('ExecStop', self.service)
        self.assertNotIn('PIDFile', self.service)
        wrapper = (ROOT / 'scripts/run-platform.sh').read_text()
        statements = [line.strip() for line in wrapper.splitlines()
                      if line.strip() and not line.lstrip().startswith('#')]
        self.assertEqual(statements[-1], 'exec "$binary" "$action"')

    def test_manager_stop_deadline_exceeds_both_shipped_application_graces(self):
        timeout = self.service.get('TimeoutStopSec', '')
        self.assertRegex(timeout, r'^[0-9]+$')
        for name in ['platform.publisher.example.json', 'platform.read-only.example.json']:
            with self.subTest(config=name):
                config = json.loads((ROOT / 'deployment' / name).read_text())
                grace = config['shutdown_grace_seconds']
                self.assertIs(type(grace), int)
                self.assertGreater(grace, 0)
                self.assertGreater(int(timeout), grace)


if __name__ == '__main__':
    unittest.main()
