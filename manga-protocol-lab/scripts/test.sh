#!/usr/bin/env bash
# Canonical current-runtime acceptance. Historical SQLite experiments are retired.
set -euo pipefail
cd "$(dirname "$0")/.."
mkdir -p results
export PYTHONPATH="$PWD/vendor/pydeps:$PWD${PYTHONPATH:+:$PYTHONPATH}"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$PWD/prototype/target}"
export CARGO_INCREMENTAL="${CARGO_INCREMENTAL:-0}"
python3 -m unittest discover -s tests
python3 -m unittest discover -s scripts/tests
(cd browser-client && npm run build && npm test)
bash scripts/test-postgres.sh
