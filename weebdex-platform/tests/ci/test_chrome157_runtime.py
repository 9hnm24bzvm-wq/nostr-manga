"""Finite synthetic archives and records only. No install, browser, server or network."""
import base64
import hashlib
import importlib.util
import io
import json
from pathlib import Path
import tarfile
import tempfile
import unittest
from unittest import mock

ROOT = Path(__file__).resolve().parents[2]


def load(name, filename):
    spec = importlib.util.spec_from_file_location(name, ROOT / 'scripts/ci' / filename)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


runtime = load('qualified_runtime', 'verify-chrome157-runtime.py')
validator = load('qualified_evidence', 'validate-ci-evidence.py')


class QualifiedRuntime(unittest.TestCase):
    def fixture(self, root, *, name='package/index.js', link=False):
        archive, installed = root / 'package.tgz', root / 'installed'
        installed.mkdir()
        (installed / 'index.js').write_bytes(b'// synthetic only\n')
        with tarfile.open(archive, 'w:gz') as target:
            row = tarfile.TarInfo(name)
            if link:
                row.type, row.linkname = tarfile.SYMTYPE, '/tmp/unapproved'
                target.addfile(row)
            else:
                row.size = len(b'// synthetic only\n')
                target.addfile(row, io.BytesIO(b'// synthetic only\n'))
        integrity = 'sha512-' + base64.b64encode(hashlib.sha512(archive.read_bytes()).digest()).decode()
        return archive, installed, integrity

    def test_exact_official_content_and_complete_installed_inventory(self):
        with tempfile.TemporaryDirectory() as name:
            runtime.verify_package(*self.fixture(Path(name)))

    def test_changed_extra_missing_and_linked_installed_bytes_refused(self):
        for change in ('changed', 'extra', 'missing', 'linked'):
            with self.subTest(change=change), tempfile.TemporaryDirectory() as name:
                archive, installed, integrity = self.fixture(Path(name))
                if change == 'changed':
                    (installed / 'index.js').write_bytes(b'not reviewed')
                elif change == 'extra':
                    (installed / 'extra.js').write_bytes(b'not reviewed')
                else:
                    (installed / 'index.js').unlink()
                    if change == 'linked':
                        (installed / 'index.js').symlink_to(archive)
                with self.assertRaises(ValueError):
                    runtime.verify_package(archive, installed, integrity)

    def test_archive_sri_change_refused_before_parsing(self):
        with tempfile.TemporaryDirectory() as name:
            archive, installed, integrity = self.fixture(Path(name))
            archive.write_bytes(archive.read_bytes() + b'changed')
            with self.assertRaises(ValueError):
                runtime.verify_package(archive, installed, integrity)

    def test_traversal_absolute_links_and_wrong_root_refused(self):
        for member, link in [('package/../index.js', False), ('/package/index.js', False),
                             ('other/index.js', False), ('package/index.js', True)]:
            with self.subTest(member=member, link=link), tempfile.TemporaryDirectory() as name:
                with self.assertRaises(ValueError):
                    runtime.verify_package(*self.fixture(Path(name), name=member, link=link))

    def prepared(self):
        return dict(schema=1, status='prepared', browser_phases_started=False, stage='complete',
                    source_files_checked=676, frontend_preserved=True, artifacts_uploaded=False,
                    fresh_build=True, historical_binaries_reused=False, playwright='1.64.1',
                    chromium_revision='cft-157.0.8103.6', native_feed_regression=validator.native_feed.not_run(),
                    **{key: '0' * 64 for key in ('source_snapshot_sha256', 'frontend_manifest_sha256',
                       'frontend_index_sha256', 'normal_binary_sha256', 'provisioner_sha256')},
                    chromium_binary_sha256=runtime.EXECUTABLE_SHA)

    def test_only_exact_qualified_identity_or_legacy_pair_is_admitted(self):
        baseline = self.prepared()
        validator.preparation(baseline)
        validator.preparation(dict(baseline, playwright='1.62.1', chromium_revision='1234'))
        for changes in (dict(playwright='1.64.2'), dict(chromium_revision='157'),
                        dict(playwright='1.62.1'), dict(chromium_revision='1234'),
                        dict(chromium_binary_sha256='0' * 64), dict(extra='private')):
            with self.subTest(changes=changes), self.assertRaises(ValueError):
                validator.preparation(dict(baseline, **changes))

    def test_checked_in_qualified_lock_and_browser_provenance_remain_exact(self):
        path = ROOT.parent / '.ci/native-upload-counterfactual/package-lock.json'
        self.assertEqual(hashlib.sha256(path.read_bytes()).hexdigest(), runtime.LOCK_SHA)
        pins = json.loads((path.parent / 'browser-pins.json').read_text())
        self.assertEqual(pins['driver']['version'], '1.64.1')
        self.assertEqual(pins['browsers'][1]['version'], runtime.VERSION)
        self.assertEqual(pins['browsers'][1]['generation'], '1792024099984701')
        self.assertEqual(pins['browsers'][1]['download'], 'https://storage.googleapis.com/chrome-for-testing-public/157.0.8103.6/linux64/chrome-linux64.zip')

    def test_driver_resolves_exact_sri_cache_and_refuses_wrong_lock(self):
        with tempfile.TemporaryDirectory() as name:
            repo = Path(name)
            root = repo / 'weebdex-platform'
            tool = repo / '.ci/native-upload-counterfactual'
            tool.mkdir(parents=True)
            lock = {'packages': {}}
            for package in ('playwright', 'playwright-core'):
                fixture = repo / package
                fixture.mkdir()
                archive, installed, integrity = self.fixture(fixture)
                content = base64.b64decode(integrity[7:]).hex()
                cached = root / '.cache/hosted-npm/_cacache/content-v2/sha512' / content[:2] / content[2:4] / content[4:]
                cached.parent.mkdir(parents=True, exist_ok=True)
                cached.write_bytes(archive.read_bytes())
                (tool / 'node_modules').mkdir(exist_ok=True)
                installed.rename(tool / 'node_modules' / package)
                lock['packages']['node_modules/' + package] = dict(version='1.64.1',
                    resolved=f'https://registry.npmjs.org/{package}/-/{package}-1.64.1.tgz', integrity=integrity)
            lock_path = tool / 'package-lock.json'
            lock_path.write_text(json.dumps(lock))
            with mock.patch.object(runtime, 'ROOT', root), mock.patch.object(runtime, 'LOCK_SHA', hashlib.sha256(lock_path.read_bytes()).hexdigest()):
                runtime.verify_driver()
                lock_path.write_text(lock_path.read_text() + ' ')
                with self.assertRaises(ValueError):
                    runtime.verify_driver()


if __name__ == '__main__':
    unittest.main()
