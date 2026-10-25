"""Finite extraction of verified archives. Does not execute browser binaries."""
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import stat
import sys
import zipfile

def digest(path):
    value = hashlib.sha256()
    with path.open('rb') as source:
        while chunk := source.read(262144):
            value.update(chunk)
    return value.hexdigest()

def extract(root):
    rows = json.loads((root / 'download-manifest.json').read_text())
    assert len(rows) == 2
    result = []
    for row in rows:
        archive = root / (row['version'] + '.zip')
        assert archive.stat().st_size == row['size'] and digest(archive) == row['sha256']
        destination = root / row['version']
        destination.mkdir()
        with zipfile.ZipFile(archive) as source:
            entries = source.infolist()
            assert len(entries) <= 2048 and sum(e.file_size for e in entries) <= 1024 ** 3
            names = set()
            for entry in entries:
                relative = PurePosixPath(entry.filename)
                mode = entry.external_attr >> 16
                assert not relative.is_absolute() and relative.parts[0] == 'chrome-linux64'
                assert '..' not in relative.parts and '\\' not in entry.filename
                assert not stat.S_ISLNK(mode) and not (mode & (stat.S_ISUID | stat.S_ISGID))
                assert entry.filename not in names and entry.file_size <= 512 * 1024 ** 2
                names.add(entry.filename)
                target = destination.joinpath(*relative.parts)
                assert target.resolve().is_relative_to(destination.resolve())
                if entry.is_dir():
                    target.mkdir(parents=True, exist_ok=True)
                    continue
                target.parent.mkdir(parents=True, exist_ok=True)
                size = 0
                with source.open(entry) as incoming, target.open('xb') as outgoing:
                    while chunk := incoming.read(262144):
                        size += len(chunk)
                        assert size <= entry.file_size
                        outgoing.write(chunk)
                assert size == entry.file_size
                target.chmod(0o755 if mode & 0o111 else 0o644)
        executable = destination / 'chrome-linux64/chrome'
        assert executable.is_file() and os.access(executable, os.X_OK)
        result.append({**row, 'executable_sha256': digest(executable)})
    with (root / 'acquisition-manifest.json').open('x') as output:
        json.dump(result, output)
        output.write('\n')
    (root / 'acquisition-manifest.json').chmod(0o444)

if __name__ == '__main__':
    try:
        assert len(sys.argv) == 2
        extract(Path(sys.argv[1]).resolve())
        print('Verified browser archives extracted')
    except Exception:
        print('Browser archive verification failed', file=sys.stderr)
        sys.exit(1)
