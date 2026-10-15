#!/usr/bin/env python3
"""Restore the exact official NIP-19 encoding package, with no package hooks."""
from pathlib import Path, PurePosixPath
import base64
import hashlib
import io
import json
import tarfile
import urllib.request

ROOT = Path(__file__).resolve().parents[1]
LOCK = ROOT / 'platform-integrations/local-library/signing-dependencies/package-lock.json'
STAGE = ROOT / '.build/local-signing-base-2.0.0'
NAME = '@scure/base'
URL = 'https://registry.npmjs.org/@scure/base/-/base-2.0.0.tgz'
lock = json.loads(LOCK.read_text())
if set(lock['packages']) != {'', 'node_modules/' + NAME}:
    raise SystemExit('Unexpected local signing dependency membership')
row = lock['packages']['node_modules/' + NAME]
if row['version'] != '2.0.0' or row['resolved'] != URL or row.get('hasInstallScript'):
    raise SystemExit('Exact official local signing package contract differs')
if STAGE.exists():
    raise SystemExit('Refusing to reuse an uncertain local signing dependency stage')
with urllib.request.urlopen(urllib.request.Request(URL, headers={'User-Agent': 'weebdex-owned-dependency-restore/1'}), timeout=15) as response:
    if response.status != 200 or response.geturl() != URL:
        raise SystemExit('Official package download was not the exact successful endpoint')
    data = response.read(1024 * 1024 + 1)
if len(data) > 1024 * 1024:
    raise SystemExit('Local signing package archive size limit exceeded')
algorithm, encoded = row['integrity'].split('-', 1)
if algorithm != 'sha512' or base64.b64encode(hashlib.sha512(data).digest()).decode() != encoded:
    raise SystemExit('Local signing package lock integrity mismatch')
target = STAGE / 'node_modules' / NAME
target.mkdir(parents=True)
(STAGE / 'base-2.0.0.tgz').write_bytes(data)
files = []
total = 0
with tarfile.open(fileobj=io.BytesIO(data), mode='r:gz') as archive:
    members = archive.getmembers()
    if len(members) > 64:
        raise SystemExit('Local signing package member limit exceeded')
    for member in members:
        name = PurePosixPath(member.name)
        if name.is_absolute() or '..' in name.parts or not name.parts or name.parts[0] != 'package' or not (member.isdir() or member.isfile()):
            raise SystemExit('Unsafe local signing package member')
        relative = Path(*name.parts[1:])
        destination = target / relative
        if member.isdir():
            destination.mkdir(parents=True, exist_ok=True)
            continue
        total += member.size
        if total > 2 * 1024 * 1024:
            raise SystemExit('Local signing expanded package byte limit exceeded')
        content = archive.extractfile(member).read()
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_bytes(content)
        files.append({'path': str(relative), 'bytes': len(content), 'sha256': hashlib.sha256(content).hexdigest()})
package = json.loads((target / 'package.json').read_text())
if package['name'] != NAME or package['version'] != '2.0.0' or package.get('dependencies') or package.get('scripts', {}).get('install'):
    raise SystemExit('Admitted local signing package identity/dependency mismatch')
manifest = {'source_lock': str(LOCK.relative_to(ROOT.parent)), 'source_lock_sha256': hashlib.sha256(LOCK.read_bytes()).hexdigest(), 'package_hooks_executed': False, 'original_node_modules_used': False, 'packages': [{'name': NAME, 'version': '2.0.0', 'url': URL, 'lock_integrity': row['integrity'], 'archive_sha256': hashlib.sha256(data).hexdigest(), 'archive_bytes': len(data), 'expanded_bytes': total, 'files': files}]}
evidence = ROOT / 'test-evidence/frontend/local-signing-base-restore/package-provenance.json'
evidence.parent.mkdir(parents=True, exist_ok=True)
evidence.write_text(json.dumps(manifest, indent=2) + '\n')
print(json.dumps({'package': NAME, 'version': '2.0.0', 'official_get_status': 200, 'lock_integrity_verified': True, 'files': len(files)}))
