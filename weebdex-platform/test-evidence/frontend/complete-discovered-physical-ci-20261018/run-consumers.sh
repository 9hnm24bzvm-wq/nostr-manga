#!/usr/bin/env bash
set -euo pipefail
E="$PWD/test-evidence/frontend/complete-discovered-physical-ci-20261018"
export WEEBDEX_READONLY_CACHE_ROOTS="$(python3 -c 'import json;print(json.dumps(json.load(open("'"$E"'/physical-dependency-root-discovery.json"))["roots"]))')"
export WEEBDEX_READONLY_AUDIT_DIR="$E/cache-audit/semantic-consumers"
mkdir -p "$WEEBDEX_READONLY_AUDIT_DIR"
export NODE_OPTIONS="--max-old-space-size=2048 --require=$PWD/scripts/semantic-readonly-cache-guard.cjs"
for gate in 0 1; do
 generation="$PWD/.build/complete-discovered-strict-physical-$gate-20261018-0335"
 for script in test_composed_semantic_projection test_semantic_validation_fail_closed; do
  node "tests/ci/$script.mjs" "$generation" "$E" > "$E/consumer-$gate-$script.log" 2>&1
  tail -1 "$E/consumer-$gate-$script.log"
 done
done
