#!/usr/bin/env bash
set -euo pipefail
E="$PWD/weebdex-platform/test-evidence/frontend/original-cli-codec-final-20261018"
export PYTHONDONTWRITEBYTECODE=1
export PYTHONPATH="$PWD/weebdex-platform/.build/python-migration-approved-20261018-0556/site-packages:$PWD/manga-protocol-lab:$PWD/manga-protocol-lab/tests"
export WEEBDEX_READONLY_CACHE_ROOTS="$(python3 -c 'import json,sys;print(json.dumps(json.load(open(sys.argv[1]))["roots"]))' "$PWD/weebdex-platform/test-evidence/frontend/complete-discovered-physical-ci-20261018/physical-dependency-root-discovery.json")"
export WEEBDEX_READONLY_AUDIT_DIR="$E/cache-audit"
mkdir -p "$WEEBDEX_READONLY_AUDIT_DIR"
export NODE_OPTIONS="--require=$PWD/weebdex-platform/scripts/semantic-readonly-cache-guard.cjs"
python3 "$E/run-selected.py"
