#!/usr/bin/env bash
set -euo pipefail
E="$PWD/weebdex-platform/test-evidence/frontend/complete-discovered-physical-ci-20261018"
export WEEBDEX_READONLY_CACHE_ROOTS="$(python3 -c 'import json;print(json.dumps(json.load(open("'"$E"'/physical-dependency-root-discovery.json"))["roots"]))')"
export WEEBDEX_READONLY_AUDIT_DIR="$E/cache-audit/original-browser-models"
mkdir -p "$WEEBDEX_READONLY_AUDIT_DIR"
export NODE_OPTIONS="--require=$PWD/weebdex-platform/scripts/semantic-readonly-cache-guard.cjs"
mkdir -p manga-protocol-lab/results/rebuild-client
node --experimental-vm-modules --test --test-concurrency=1 manga-protocol-lab/browser-client/tests/*.test.mjs
python3 -m unittest discover -s manga-protocol-lab/scripts/tests -p test_package_cli.py -v
(cd manga-protocol-lab && PYTHONPATH="$PWD:$PWD/tests" python3 -m unittest -v test_api_index_cli test_local_library_migration)
(cd manga-protocol-lab/bench/library-node && env -u MIXED_DIAGNOSTIC_TEST_DB -u MIXED_DIAGNOSTIC_OWNED_RUNTIME python3 -m unittest -v test_process_metrics test_diagnostic_process test_scheduler_probe test_pg_diagnostics)
