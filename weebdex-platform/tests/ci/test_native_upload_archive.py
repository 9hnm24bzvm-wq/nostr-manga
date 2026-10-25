"""Actual extractor subprocess negatives, with tiny synthetic archives only.

No browser binary, socket, package install, or download is used. The harmless
synthetic file named chrome is never executed.
"""
import hashlib
import json
from pathlib import Path
import stat
import struct
import subprocess
import sys
import tempfile
import unittest
import warnings
import zipfile

SCRIPT = Path(__file__).resolve().parents[2] / 'scripts/ci/extract-native-upload-browsers.py'
VERSIONS = ['156.0.8078.47', '157.0.8103.6']
SYNTHETIC = b'Synthetic archive fixture. Not executable code.\n'

def make_zip(path, attack=None):
    with warnings.catch_warnings():
        warnings.simplefilter('ignore', UserWarning)
        with zipfile.ZipFile(path, 'w', compression=zipfile.ZIP_STORED) as archive:
            item = zipfile.ZipInfo('chrome-linux64/chrome')
            item.external_attr = (stat.S_IFREG | 0o755) << 16
            archive.writestr(item, SYNTHETIC)
            if attack == 'traversal':
                archive.writestr('chrome-linux64/../../../escaped', b'forbidden')
            elif attack == 'duplicate':
                duplicate = zipfile.ZipInfo('chrome-linux64/chrome')
                duplicate.external_attr = (stat.S_IFREG | 0o755) << 16
                archive.writestr(duplicate, b'duplicate')
            elif attack == 'symlink':
                link = zipfile.ZipInfo('chrome-linux64/link')
                link.external_attr = (stat.S_IFLNK | 0o777) << 16
                archive.writestr(link, '../../../escaped')
    if attack == 'oversized':
        data = bytearray(path.read_bytes())
        central = data.index(b'PK\x01\x02')
        struct.pack_into('<I', data, central + 24, 1024 ** 3 + 1)
        path.write_bytes(data)
    assert path.stat().st_size < 4096

class ArchiveControls(unittest.TestCase):
    def run_fixture(self, attack=None):
        with tempfile.TemporaryDirectory(prefix='counterfactual-zip-') as directory:
            outer = Path(directory)
            root = outer / 'archives'
            root.mkdir()
            sentinel = outer / 'escaped'
            sentinel.write_bytes(b'unchanged')
            rows = []
            for index, version in enumerate(VERSIONS):
                archive = root / (version + '.zip')
                make_zip(archive, attack if index == 0 else None)
                data = archive.read_bytes()
                rows.append({'version': version, 'size': len(data), 'sha256': '0' * 64 if attack == 'hash-mismatch' and index == 0 else hashlib.sha256(data).hexdigest()})
            (root / 'download-manifest.json').write_text(json.dumps(rows))
            result = subprocess.run([sys.executable, '-B', str(SCRIPT), str(root)], capture_output=True, timeout=3, check=False)
            self.assertLess(len(result.stdout) + len(result.stderr), 256)
            self.assertEqual(sentinel.read_bytes(), b'unchanged')
            witness = root / 'acquisition-manifest.json'
            if attack is None:
                self.assertEqual(result.returncode, 0)
                values = json.loads(witness.read_text())
                self.assertEqual(len(values), 2)
                self.assertTrue(all(row['executable_sha256'] == hashlib.sha256(SYNTHETIC).hexdigest() for row in values))
            else:
                self.assertEqual(result.returncode, 1)
                self.assertFalse(witness.exists())
                self.assertEqual(result.stderr, b'Browser archive verification failed\n')

    def test_finite_valid_control(self): self.run_fixture()
    def test_traversal_rejected(self): self.run_fixture('traversal')
    def test_duplicate_rejected(self): self.run_fixture('duplicate')
    def test_symlink_rejected(self): self.run_fixture('symlink')
    def test_oversized_declaration_rejected(self): self.run_fixture('oversized')
    def test_hash_mismatch_rejected(self): self.run_fixture('hash-mismatch')

if __name__ == '__main__':
    unittest.main()
