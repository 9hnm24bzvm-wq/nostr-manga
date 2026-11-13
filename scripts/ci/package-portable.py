"""Package only selected production files; never include tests, secrets or runtime data."""
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import subprocess

root = Path('dist')
root.mkdir(exist_ok=False)
suffix = '.exe' if os.name == 'nt' else ''
cases = [
    ('mangapub-core', 'manga-protocol-lab/server', 'manga-node', [], 'usage: manga-node'),
    ('mangapub-core', 'manga-protocol-lab/server', 'manga-key-tool', [], 'usage: manga-key-tool'),
    ('weebdex-platform', 'weebdex-platform/backend', 'weebdex-platform', ['invalid-action'], 'unknown platform action'),
    ('weebdex-platform', 'weebdex-platform/backend', 'weebdex-admin', [], 'Usage: weebdex-admin'),
]
for package, crate, name, args, expected in cases:
    destination = root / package
    destination.mkdir(exist_ok=True)
    binary = Path(os.environ['CARGO_TARGET_DIR']) / 'release' / (name + suffix)
    copied = destination / binary.name
    shutil.copy2(binary, copied)
    result = subprocess.run([str(copied.resolve()), *args], capture_output=True, text=True, timeout=15)
    if result.returncode == 0 or expected not in result.stderr:
        raise RuntimeError(f'{name}: native executable smoke failed: {result.returncode}: {result.stderr}')
    print(f'{name}: native loader/startup and safe argument rejection passed')
for package, crate in [('mangapub-core', 'manga-protocol-lab/server'), ('weebdex-platform', 'weebdex-platform/backend')]:
    target = root / package / 'migrations'
    target.mkdir()
    for source in sorted((Path(crate) / 'migrations').glob('*.sql')):
        shutil.copy2(source, target / source.name)
# These documentation files describe limitations, not source or fixture contents.
for name in ('NATIVE-BACKEND.md', 'STORAGE-PORTABILITY.md'):
    source = Path('manga-protocol-lab/server') / name
    if source.exists():
        shutil.copy2(source, root / 'mangapub-core' / name)
source = Path('weebdex-platform/deployment/NATIVE-PLATFORMS.md')
if source.exists():
    shutil.copy2(source, root / 'weebdex-platform' / source.name)
(root / 'BUILD-INFO.json').write_text(json.dumps({
    'commit': os.environ['GITHUB_SHA'], 'run_id': os.environ['GITHUB_RUN_ID'],
    'run_attempt': os.environ['GITHUB_RUN_ATTEMPT'], 'os': platform.platform(),
    'architecture': platform.machine(), 'rust': '1.90.0',
    'scope': 'Native hosted-runner tests and executable argument-rejection smoke. Not clean-machine, production, power-loss or throughput acceptance.',
    'windows_note': 'Hosted Windows runner is elevated. Storage volume flush requires elevated access. Restricted service-account behavior is a separate acceptance test.',
    'external_requirements': 'PostgreSQL and valid private runtime configuration; frontend assets are not included in the backend binary package. Inspect final executable dependencies for target-machine runtime requirements.'
}, indent=2) + '\n')
files = sorted(p for p in root.rglob('*') if p.is_file())
(root / 'SHA256SUMS').write_text(''.join(hashlib.sha256(p.read_bytes()).hexdigest() + '  ' + p.relative_to(root).as_posix() + '\n' for p in files))
