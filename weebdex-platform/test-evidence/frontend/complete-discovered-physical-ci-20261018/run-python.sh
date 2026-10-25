#!/usr/bin/env bash
set -euo pipefail
E="$PWD/weebdex-platform/test-evidence/frontend/complete-discovered-physical-ci-20261018"
export WEEBDEX_READONLY_CACHE_ROOTS="$(python3 -c 'import json,sys;print(json.dumps(json.load(open(sys.argv[1]))["roots"]))' "$E/physical-dependency-root-discovery.json")"
export WEEBDEX_READONLY_AUDIT_DIR="$E/cache-audit/python-final"
mkdir -p "$WEEBDEX_READONLY_AUDIT_DIR"
export NODE_OPTIONS="--require=$PWD/weebdex-platform/scripts/semantic-readonly-cache-guard.cjs"
python3 "$E/run-python.py"
