#!/usr/bin/env bash
set -euo pipefail
ROOT="$PWD"
E="$ROOT/weebdex-platform/test-evidence/frontend/controlled-bench-seams-final-20261018"
export PYTHONDONTWRITEBYTECODE=1
export PYTHONPATH="$ROOT/weebdex-platform/.build/python-migration-approved-20261018-0556/site-packages:$ROOT/manga-protocol-lab:$ROOT/manga-protocol-lab/bench/library-node"
export NODE_RESULTS="$ROOT/weebdex-platform/.build/controlled-bench-seams-final-20261018/results"
export TMPDIR="$ROOT/weebdex-platform/.build/controlled-bench-seams-final-20261018/tmp"
export WEEBDEX_READONLY_CACHE_ROOTS="$(python3 -c 'import json,sys;print(json.dumps(json.load(open(sys.argv[1]))["roots"]))' "$ROOT/weebdex-platform/test-evidence/frontend/complete-discovered-physical-ci-20261018/physical-dependency-root-discovery.json")"
export WEEBDEX_READONLY_AUDIT_DIR="$E/cache-audit"
mkdir -p "$WEEBDEX_READONLY_AUDIT_DIR"
export NODE_OPTIONS="--require=$ROOT/weebdex-platform/scripts/semantic-readonly-cache-guard.cjs"
cd manga-protocol-lab
python3 "$E/run-selected.py"
