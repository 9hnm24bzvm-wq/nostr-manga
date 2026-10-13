#!/usr/bin/env bash
# Invoked only inside run-auth-tests.sh's owned disposable loopback cluster.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
[[ $# -gt 0 ]] || { echo 'An explicit precompiled original test command is required' >&2; exit 64; }
[[ "${PLATFORM_TEST_RUNTIME_DIR:-}" == "$ROOT/test-runtime/auth-"* ]] || {
    echo 'Requires the owned disposable fixture runner' >&2; exit 64;
}
[[ "${PLATFORM_AUTH_TEST_DATABASE_URL:-}" =~ ^host=127\.0\.0\.1\ port=[0-9]+\ dbname=weebdex_platform_test$ ]] || {
    echo 'Refusing a non-fixture database connection' >&2; exit 64;
}
"$ROOT/../manga-protocol-lab/vendor/pgsql/bin/createdb" \
    --maintenance-db "$PLATFORM_AUTH_TEST_DATABASE_URL" manga_test
export MANGA_TEST_DATABASE_URL="${PLATFORM_AUTH_TEST_DATABASE_URL/dbname=weebdex_platform_test/dbname=manga_test}"
export STRFRY_BIN="$ROOT/../manga-protocol-lab/vendor/strfry/strfry"
export MANGA_TEST_BACKEND_DIR="$PLATFORM_TEST_RUNTIME_DIR/original-backend"
mkdir -p "$MANGA_TEST_BACKEND_DIR"
exec "$@"
