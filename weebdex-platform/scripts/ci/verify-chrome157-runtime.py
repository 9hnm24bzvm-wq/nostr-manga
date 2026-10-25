"""Fixed, opt-in runtime from qualified counterfactual 40133173860. No execution/network."""
import base64
import hashlib
import json
from pathlib import Path
import sys
import tarfile

ROOT = Path(__file__).resolve().parents[2]
LOCK_SHA = 'e1da6bd8b8a67849c5e55d257336524a6b9fe4c1764c112efb4e8817ea4f3f35'
ARCHIVES = ('e6c94490f0b6b337225aeb2d2a5034858516b7017356980ba94f5176d8774a50',
            '7802b25411aa5b8aa387cc733a45df6eed1aea8cc046fd4e2dec64d5918b1853')
EXECUTABLE_SHA = 'ec4e78d65a5382c2b50aaf28c6a655a624973b3d3c711a2a4ae34369fd9a3444'
VERSION = '157.0.8103.6'


def insist(value):
    if not value:
        raise ValueError('Qualified browser runtime verification failed')


def regular(path, limit):
    insist(path.resolve() == path and path.is_file() and path.stat().st_size <= limit)
    data = path.read_bytes()
    insist(len(data) == path.stat().st_size and len(data) <= limit)
    return data


def digest(path, limit):
    insist(path.resolve() == path and path.is_file() and path.stat().st_size <= limit)
    value, size = hashlib.sha256(), 0
    with path.open('rb') as source:
        while chunk := source.read(262144):
            size += len(chunk)
            insist(size <= limit)
            value.update(chunk)
    insist(size == path.stat().st_size)
    return value.hexdigest()


def verify_package(archive, installed, integrity):
    """Compare every installed byte with the exact official npm SRI tarball.

    npm ci already verifies SRI; independently verify its content-addressed cache
    and reject missing, extra, linked, or changed installed files before import.
    """
    raw = regular(archive, 32 * 1024 ** 2)
    insist('sha512-' + base64.b64encode(hashlib.sha512(raw).digest()).decode() == integrity)
    expected = {}
    with tarfile.open(archive, 'r:gz') as source:
        members = source.getmembers()
        insist(len(members) <= 4096 and sum(row.size for row in members) <= 64 * 1024 ** 2)
        for row in members:
            parts = Path(row.name).parts
            insist(not Path(row.name).is_absolute() and parts[0] == 'package' and '..' not in parts and '\\' not in row.name)
            insist(row.isdir() or row.isfile())
            if row.isdir():
                continue
            name = '/'.join(parts[1:])
            insist(name and name not in expected and row.size <= 32 * 1024 ** 2)
            expected[name] = source.extractfile(row).read()
    actual, size = {}, 0
    for path in installed.rglob('*'):
        insist(not path.is_symlink())
        if path.is_file():
            actual[path.relative_to(installed).as_posix()] = regular(path, 32 * 1024 ** 2)
            size += path.stat().st_size
            insist(len(actual) <= 4096 and size <= 64 * 1024 ** 2)
        else:
            insist(path.is_dir())
    insist(actual == expected and bool(actual))


def verify_driver():
    tool = ROOT.parent / '.ci/native-upload-counterfactual'
    lock_path = tool / 'package-lock.json'
    insist(digest(lock_path, 65536) == LOCK_SHA)
    lock = json.loads(regular(lock_path, 65536))
    for name in ('playwright', 'playwright-core'):
        pin = lock['packages']['node_modules/' + name]
        insist(pin['version'] == '1.64.1' and pin['resolved'] == f'https://registry.npmjs.org/{name}/-/{name}-1.64.1.tgz')
        content = base64.b64decode(pin['integrity'].removeprefix('sha512-'), validate=True).hex()
        insist(len(content) == 128)
        archive = ROOT / '.cache/hosted-npm/_cacache/content-v2/sha512' / content[:2] / content[2:4] / content[4:]
        verify_package(archive, tool / 'node_modules' / name, pin['integrity'])


def verify_archives(full=False):
    directory = ROOT / '.build/hosted-browser/cft-acquisition'
    pin_path = ROOT.parent / '.ci/native-upload-counterfactual/browser-pins.json'
    insist(digest(pin_path, 65536) == '14d6dcf65a7584123784def783037078f061d750bf554cd67cd34370572230dd')
    pins = json.loads(regular(pin_path, 65536))['browsers']
    rows = json.loads(regular(directory / 'download-manifest.json', 65536))
    insist(len(pins) == len(rows) == 2)
    for index, (pin, row) in enumerate(zip(pins, rows)):
        insist(row == dict(pin, sha256=ARCHIVES[index]))
        archive = directory / (row['version'] + '.zip')
        insist(archive.stat().st_size == row['size'] and digest(archive, 256 * 1024 ** 2) == ARCHIVES[index])
    insist(rows[1]['version'] == VERSION and rows[1]['generation'] == '1792024099984701')
    if full:
        acquired = json.loads(regular(directory / 'acquisition-manifest.json', 65536))
        insist(len(acquired) == 2 and acquired[1] == dict(rows[1], executable_sha256=EXECUTABLE_SHA))
        insist(digest(directory / VERSION / 'chrome-linux64/chrome', 512 * 1024 ** 2) == EXECUTABLE_SHA)


if __name__ == '__main__':
    try:
        insist(len(sys.argv) == 2 and sys.argv[1] in ('--driver-only', '--archives-only', '--full'))
        if sys.argv[1] != '--archives-only':
            verify_driver()
        if sys.argv[1] != '--driver-only':
            verify_archives(sys.argv[1] == '--full')
    except Exception:
        print('Qualified browser runtime verification failed', file=sys.stderr)
        sys.exit(1)
