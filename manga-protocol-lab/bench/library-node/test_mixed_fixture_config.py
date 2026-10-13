"""Exercise the exact fixture-config CLI invoked by the launcher, without app traffic."""
import json, os, pathlib, subprocess, sys, tempfile, unittest

ROOT = pathlib.Path(__file__).resolve().parents[2]
CONFIG = ROOT / 'bench/library-node/fixture_config.py'

class FixtureConfig(unittest.TestCase):
    def generate(self, active=None, connections='16', waiting=None, wal=None):
        with tempfile.TemporaryDirectory(prefix='mixed-config-test-') as tmp:
            root = pathlib.Path(tmp)
            runtime = root / 'runtime'; runtime.mkdir()
            results = root / 'results'; results.mkdir()
            env = {k: v for k, v in os.environ.items() if not k.startswith(('NODE_', 'MIXED_'))}
            env.update(PYTHONPATH=str(ROOT/'vendor/pydeps'), NODE_RESULTS=str(results), NODE_DB_CONNECTIONS=connections,
                       MIXED_SELECTED_NGINX='/synthetic/nginx', MIXED_SELECTED_STRFRY='/synthetic/strfry')
            if active is not None: env['NODE_READ_CATALOGUE_ACTIVE'] = active
            if waiting is not None: env['NODE_READ_CATALOGUE_WAITING'] = waiting
            if wal is not None: env['MIXED_PG_MAX_WAL_MB'] = wal
            result = subprocess.run([sys.executable, str(CONFIG), str(runtime), str(ROOT)], env=env, capture_output=True, text=True)
            if result.returncode: return result, None
            self.pg_config = (runtime/'postgresql-fixture.conf').read_text()
            configs = json.loads((results/'node-limits.json').read_text())
            for c in configs:
                c['storage']['path'] = '<owned-runtime>'
                c['backend']['data_dir'] = '<owned-runtime>'
            return result, configs

    def test_default_omits_override_and_three_changes_only_catalogue(self):
        default, baseline = self.generate()
        changed, trial = self.generate('3')
        self.assertEqual(default.returncode, 0, default.stderr)
        self.assertEqual(changed.returncode, 0, changed.stderr)
        for before, after in zip(baseline, trial):
            self.assertNotIn('read_admission', before)
            self.assertEqual(after.pop('read_admission'), dict(overall=4, wait_ms=200,
                catalogue=dict(active=3, waiting=8), library_head=dict(active=2, waiting=8), live=dict(active=2, waiting=8)))
            self.assertEqual(before, after)

    def test_rejects_nonpositive_nonnumeric_and_overall_overflow(self):
        for value in ('0', '-1', '5', '1.5', 'nan', 'inf', ''):
            with self.subTest(value=value):
                result, _ = self.generate(value)
                self.assertNotEqual(result.returncode, 0)

    def test_obeys_small_database_default_limit(self):
        invalid, _ = self.generate('3', connections='2')
        self.assertNotEqual(invalid.returncode, 0)
        valid, configs = self.generate('2', connections='2')
        self.assertEqual(valid.returncode, 0, valid.stderr)
        for config in configs:
            self.assertEqual(config['read_admission'], dict(overall=2, wait_ms=200,
                catalogue=dict(active=2, waiting=8), library_head=dict(active=2, waiting=8), live=dict(active=2, waiting=8)))

    def test_waiting_override_changes_only_waiting(self):
        for active in (None, '3'):
            before_result, before = self.generate(active)
            after_result, after = self.generate(active, waiting='32')
            self.assertEqual(before_result.returncode, 0, before_result.stderr)
            self.assertEqual(after_result.returncode, 0, after_result.stderr)
            for base, trial in zip(before, after):
                if active is None:
                    base['read_admission'] = dict(overall=4, wait_ms=200,
                        catalogue=dict(active=2, waiting=8), library_head=dict(active=2, waiting=8), live=dict(active=2, waiting=8))
                self.assertEqual(trial['read_admission']['catalogue']['waiting'], 32)
                trial['read_admission']['catalogue']['waiting'] = 8
                self.assertEqual(base, trial)

    def test_waiting_is_finite_bounded_integer(self):
        for value in ('-1', '1025', '1.5', 'nan', 'inf', ''):
            with self.subTest(value=value):
                result, _ = self.generate('3', waiting=value)
                self.assertNotEqual(result.returncode, 0)
        result, configs = self.generate('3', waiting='0')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertTrue(all(c['read_admission']['catalogue']['waiting'] == 0 for c in configs))

    def test_wal_override_changes_only_real_launcher_configuration_factor(self):
        result, baseline = self.generate('3', waiting='32')
        self.assertEqual(result.returncode, 0, result.stderr)
        before = self.pg_config
        self.assertEqual(before, "listen_addresses='127.0.0.1'\nport=55441\nunix_socket_directories=''\nshared_buffers='128MB'\nmax_connections=100\nmax_wal_size='512MB'\n")
        result, trial = self.generate('3', waiting='32', wal='1024')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(baseline, trial)
        self.assertEqual(self.pg_config, before.replace("max_wal_size='512MB'", "max_wal_size='1024MB'"))

    def test_wal_override_is_finite_bounded_integer(self):
        for value in ('0', '-1', '63', '16385', '1.5', 'nan', 'inf', '', "1024MB", "1024\nfsync=off"):
            with self.subTest(value=value):
                result, _ = self.generate(wal=value)
                self.assertNotEqual(result.returncode, 0)

if __name__ == '__main__': unittest.main()
