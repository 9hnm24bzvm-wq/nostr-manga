#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PG_BIN="${PLATFORM_TEST_PG_BIN:-$ROOT/../manga-protocol-lab/vendor/pgsql/bin}"
exec python3 "$ROOT/tests/runtime/auth_fixture_guard.py" "$PG_BIN" "$@"
