#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PG_BIN="$ROOT/../manga-protocol-lab/vendor/pgsql/bin"
mkdir -p "$ROOT/test-runtime"
RUN="$(mktemp -d "$ROOT/test-runtime/auth-XXXXXX")"
chmod 700 "$RUN"
PORT="$(python3 -c 'import socket; s=socket.socket(); s.bind(("127.0.0.1",0)); print(s.getsockname()[1]); s.close()')"
PG_PID=""
cleanup() {
    status=$?
    trap - EXIT INT TERM
    if [[ -n "$PG_PID" ]] && kill -0 "$PG_PID" 2>/dev/null; then
        kill -INT "$PG_PID" 2>/dev/null || true
        wait "$PG_PID" || true
    fi
    if [[ -n "$PG_PID" ]] && kill -0 "$PG_PID" 2>/dev/null; then
        printf '{"postgres_cleanup":"failed","exit":%s}\n' "$status" > "$RUN/cleanup.json"
        exit 71
    fi
    printf '{"postgres_cleanup":"verified_stopped","exit":%s}\n' "$status" > "$RUN/cleanup.json"
    exit "$status"
}
trap cleanup EXIT INT TERM
"$PG_BIN/initdb" -D "$RUN/data" --auth-local=reject --auth-host=trust --no-locale --encoding=UTF8 > "$RUN/initdb.log" 2>&1
"$PG_BIN/postgres" -D "$RUN/data" -k '' -h 127.0.0.1 -p "$PORT" -c max_connections=12 -c shared_buffers=16MB -c fsync=off -c full_page_writes=off > "$RUN/postgres.log" 2>&1 &
PG_PID=$!
for attempt in $(seq 1 50); do
    if "$PG_BIN/pg_isready" -h 127.0.0.1 -p "$PORT" -d postgres >/dev/null 2>&1; then break; fi
    kill -0 "$PG_PID" || { echo "Isolated postgres exited before readiness" >&2; exit 72; }
    sleep 0.1
done
"$PG_BIN/createdb" -h 127.0.0.1 -p "$PORT" weebdex_platform_test
export PLATFORM_TEST_RUNTIME_DIR="$RUN"
export PLATFORM_AUTH_TEST_DATABASE_URL="host=127.0.0.1 port=$PORT dbname=weebdex_platform_test"
printf '{"database":"weebdex_platform_test","transport":"private loopback TCP","fixture_only":true}\n' > "$RUN/fixture.json"
if [[ $# -gt 0 ]]; then
    "$@"
else
    cargo test --manifest-path "$ROOT/backend/Cargo.toml" --test auth_http -- --ignored --test-threads=1
fi
