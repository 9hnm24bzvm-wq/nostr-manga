#!/usr/bin/env bash
# Same disposable database, budgets and cleanup as the retained native HTTP tests.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
exec bash "$ROOT/scripts/run-auth-tests.sh" cargo test --offline --locked --manifest-path "$ROOT/backend/Cargo.toml" --test catalog_feed_projection -- --ignored --test-threads=1 --nocapture
