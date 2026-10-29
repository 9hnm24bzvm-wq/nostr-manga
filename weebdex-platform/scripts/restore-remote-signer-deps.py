#!/usr/bin/env python3
"""Restore the exact official NIP-44 module and its used runtime dependencies.
No npm/package hooks, key generation, relay contact, or original dependency edits.
"""
from pathlib import Path, PurePosixPath
import base64, hashlib, io, json, tarfile, urllib.request, argparse
ROOT = Path(__file__).resolve().parents[1]
LOCK = ROOT / 'platform-integrations/local-library/remote-signing-dependencies/package-lock.json'
EXPECTED_LOCK = '0f2ab48d9c6e4f486c0672f384ed6cb4736bf1f20cd876c83b46466075066160'
STAGE = ROOT / '.build/remote-signer-nostr-tools-2.25.2'
VERSIONS = {'nostr-tools': '2.25.2', '@noble/ciphers': '2.1.1', '@noble/curves': '2.0.1', '@noble/hashes': '2.0.1', '@scure/base': '2.0.0'}
if hashlib.sha256(LOCK.read_bytes()).hexdigest() != EXPECTED_LOCK:
    raise SystemExit('Unreviewed remote signer dependency lock')
parser = argparse.ArgumentParser()
parser.add_argument('--verify-existing-cache', type=Path, help='Verify an existing stage against exact sha512-addressed npm cache archives, without network or writes to package files')
args = parser.parse_args()
if STAGE.exists() and args.verify_existing_cache is None:
    raise SystemExit('Refusing to reuse an uncertain remote signer dependency stage')
if args.verify_existing_cache is not None and not STAGE.is_dir():
    raise SystemExit('No existing stage to verify')
lock = json.loads(LOCK.read_text())
manifest = {'source_lock_sha256': EXPECTED_LOCK, 'package_hooks_executed': False, 'network_used': args.verify_existing_cache is None, 'verification_mode': 'sha512-npm-cache' if args.verify_existing_cache else 'official-archive-download', 'scope': 'nostr-tools/nip44 only; no pool, NIP04, mnemonic, WASM or remote transport library', 'packages': []}
for name, version in VERSIONS.items():
    row = lock['packages']['node_modules/' + name]
    short = name.rsplit('/', 1)[-1]
    url = f'https://registry.npmjs.org/{name}/-/{short}-{version}.tgz'
    if row['version'] != version or row['resolved'] != url or row.get('hasInstallScript'):
        raise SystemExit('Unexpected official package identity')
    if args.verify_existing_cache is not None:
        algorithm, encoded = row['integrity'].split('-', 1)
        if algorithm != 'sha512':
            raise SystemExit('Unexpected package integrity algorithm')
        digest = base64.b64decode(encoded, validate=True).hex()
        cached = args.verify_existing_cache / '_cacache/content-v2/sha512' / digest[:2] / digest[2:4] / digest[4:]
        if cached.stat().st_size > 8 * 1024 * 1024:
            raise SystemExit('Cached package archive byte budget exceeded')
        data = cached.read_bytes()
    else:
        with urllib.request.urlopen(urllib.request.Request(url, headers={'User-Agent': 'weebdex-owned-dependency-restore/1'}), timeout=15) as response:
            if response.status != 200 or response.geturl() != url:
                raise SystemExit('Official archive download redirected or failed')
            data = response.read(8 * 1024 * 1024 + 1)
    if len(data) > 8 * 1024 * 1024:
        raise SystemExit('Package archive byte budget exceeded')
    if row['integrity'] != 'sha512-' + base64.b64encode(hashlib.sha512(data).digest()).decode():
        raise SystemExit('Official archive integrity mismatch')
    target = STAGE / 'node_modules' / name
    target.mkdir(parents=True, exist_ok=True)
    (STAGE / f'{short}-{version}.tgz').write_bytes(data)
    files, total = [], 0
    with tarfile.open(fileobj=io.BytesIO(data), mode='r:gz') as archive:
        members = archive.getmembers()
        if len(members) > 4096:
            raise SystemExit('Package member budget exceeded')
        for member in members:
            source = PurePosixPath(member.name)
            if source.is_absolute() or '..' in source.parts or not source.parts or source.parts[0] != 'package' or not (member.isfile() or member.isdir()):
                raise SystemExit('Unsafe archive member')
            destination = target / Path(*source.parts[1:])
            if member.isdir():
                destination.mkdir(parents=True, exist_ok=True)
                continue
            total += member.size
            if total > 64 * 1024 * 1024:
                raise SystemExit('Expanded package budget exceeded')
            content = archive.extractfile(member).read()
            destination.parent.mkdir(parents=True, exist_ok=True)
            if args.verify_existing_cache is not None:
                if destination.read_bytes() != content:
                    raise SystemExit('Existing package bytes differ from the exact archive: ' + str(destination))
            else:
                destination.write_bytes(content)
            files.append({'path': str(destination.relative_to(target)), 'bytes': len(content), 'sha256': hashlib.sha256(content).hexdigest()})
    package = json.loads((target / 'package.json').read_text())
    if package['name'] != name or package['version'] != version:
        raise SystemExit('Extracted package identity mismatch')
    manifest['packages'].append({'name': name, 'version': version, 'url': url, 'integrity': row['integrity'], 'archive_sha256': hashlib.sha256(data).hexdigest(), 'files': files})
evidence = ROOT / 'test-evidence/frontend/remote-signer-deps/package-provenance.json'
evidence.parent.mkdir(parents=True, exist_ok=True)
evidence.write_text(json.dumps(manifest, indent=2) + '\n')
print(json.dumps({'status': 'verified-existing' if args.verify_existing_cache else 'restored', 'packages': len(VERSIONS), 'package_hooks_executed': False, 'lock_sha256': EXPECTED_LOCK}))
