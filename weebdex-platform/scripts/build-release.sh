#!/usr/bin/env bash
# Run deliberately after final validation. Lockfiles resolve dependencies;
# this script does not install software or silently repair source/lockfiles.
set -euo pipefail
root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"
command -v cargo >/dev/null && command -v node >/dev/null && command -v python3 >/dev/null
node -e 'const [a,b]=process.versions.node.split(".").map(Number);if(a<22||(a===22&&b<12))throw Error("Node 22.12+ required")'
frontend="${WEEBDEX_FRONTEND_ROOT:-$root/frontend}"
[[ -f "$frontend/node_modules/vite/bin/vite.js" ]] || { printf 'Restore frontend dependencies with its unchanged Yarn lockfile before building\n' >&2; exit 69; }
export WEEBDEX_FRONTEND_ROOT="$frontend"
# Same-origin output is required by production CSP and browser auth policy.
export PUBLIC_API_URL=/api PUBLIC_KRATOS_PUBLIC_API=/auth PUBLIC_COVER_URL=/covers PUBLIC_DATA_URL=/data PUBLIC_UPLOAD_URL=/uploads PUBLIC_PROXY_URL=/proxy PUBLIC_OPENGRAPH_URL=/og-image
export PUBLIC_SITE_NAME="${PUBLIC_SITE_NAME:-WeebDex}"
node scripts/build-frontend.mjs
cargo build --manifest-path backend/Cargo.toml --locked --release --bin weebdex-platform
mkdir -p artifacts/backend
cp "${CARGO_TARGET_DIR:-$root/backend/target}/release/weebdex-platform" artifacts/backend/weebdex-platform
python3 - <<'PY'
from pathlib import Path
import hashlib,json,subprocess
root=Path.cwd()
files=[root/'artifacts/backend/weebdex-platform', *sorted((root/'artifacts/frontend').rglob('*'))]
files=[p for p in files if p.is_file()]
def sha(path):
    h=hashlib.sha256()
    with path.open('rb') as f:
        for chunk in iter(lambda:f.read(1024*1024),b''): h.update(chunk)
    return h.hexdigest()
manifest={'format':1,'validation':'Build completed; this receipt does not claim tests, deployment or deterministic binary bytes',
          'toolchain':{'rustc':subprocess.check_output(['rustc','--version'],text=True).strip(),'node':subprocess.check_output(['node','--version'],text=True).strip()},
          'lockfiles':{str(p.relative_to(root)):sha(p) for p in [root/'backend/Cargo.lock',root/'frontend/yarn.lock']},
          'artifacts':[{ 'path':str(p.relative_to(root)),'bytes':p.stat().st_size,'sha256':sha(p)} for p in files]}
(root/'artifacts/release-manifest.json').write_text(json.dumps(manifest,indent=2)+'\n')
PY
printf 'Release artifacts and SHA-256 receipt written under artifacts/\n'
