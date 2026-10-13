#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
mkdir -p results
PROFILE_ARGS=()
BIN_DIR=debug
case "${MANGA_TEST_PROFILE:-dev}" in
  dev) ;;
  release) PROFILE_ARGS=(--release); BIN_DIR=release ;;
  *) echo 'MANGA_TEST_PROFILE must be dev or release' >&2; exit 2 ;;
esac
# The combined suite includes the browser module/real-backend interoperability fixture.
if [ -z "${MANGA_TEST_FIXTURE:-}" ] || [ "${MANGA_TEST_FIXTURE:-}" = http_browser_client ]; then
  command -v node >/dev/null || { echo 'Node20.19+ is required for the browser client fixture.' >&2; exit 2; }
  [ -f browser-client/node_modules/fake-indexeddb/package.json ] && [ -f browser-client/node_modules/@noble/curves/package.json ] || { echo 'Install the pinned client test dependencies first: (cd browser-client && npm ci --ignore-scripts)' >&2; exit 2; }
fi
export CARGO_INCREMENTAL="${CARGO_INCREMENTAL:-0}"
ROOT="$PWD"; PG="${PG_BIN:-$ROOT/vendor/pgsql/bin}"; RUNTIME=$(mktemp -d "${MANGA_TEST_RUNTIME_ROOT:-/tmp}/manga-pg-tests-XXXXXX")
cleanup(){ status=$?; "$PG/pg_ctl" -D "$RUNTIME/data" -m fast -w stop >/dev/null 2>&1 || true; if [ "$status" -eq 0 ] && [ ! -e "$RUNTIME/data/postmaster.pid" ]; then rm -rf "$RUNTIME"; else echo "Failed fixture retained at $RUNTIME" >&2; fi; }; trap cleanup EXIT
python3 - "$RUNTIME/password" <<'PY'
import os,secrets,sys
fd=os.open(sys.argv[1],os.O_WRONLY|os.O_CREAT|os.O_EXCL,0o600)
with os.fdopen(fd,'w') as f:f.write(secrets.token_hex(24))
PY
"$PG/initdb" -D "$RUNTIME/data" --auth-local=peer --auth-host=scram-sha-256 --pwfile="$RUNTIME/password" --no-locale --encoding=UTF8 >"$RUNTIME/init.log" 2>&1
cat >> "$RUNTIME/data/postgresql.conf" <<'PGCONF'
listen_addresses = '127.0.0.1'
port = 55432
unix_socket_directories = ''
shared_buffers = '32MB'
max_connections = 48
PGCONF
"$PG/pg_ctl" -D "$RUNTIME/data" -l "$RUNTIME/server.log" -o "-k '' -p 55432 -c listen_addresses=127.0.0.1 -c shared_buffers=32MB -c max_connections=48" -w start
export PGPASSWORD=$(cat "$RUNTIME/password")
"$PG/createdb" -h 127.0.0.1 -p 55432 manga_test
export MANGA_TEST_DATABASE_URL="host=127.0.0.1 port=55432 user=$(id -un) password=$PGPASSWORD dbname=manga_test"
export MANGA_TEST_REPORT="$ROOT/results/postgres-foundation.json"
export MANGA_CAPACITY_REPORT="$ROOT/results/postgres-capacity.json"
export MANGA_TEST_BACKEND_DIR="$RUNTIME/backend"
export MANGA_HTTP_NODE_REPORT="$ROOT/results/postgres-http-node.json"
export CARGO_HOME="$ROOT/vendor/cargo" RUSTUP_HOME="$ROOT/vendor/rustup" CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$ROOT/target}"
"$CARGO_HOME/bin/cargo" fmt --manifest-path server/Cargo.toml -- --check
"$CARGO_HOME/bin/cargo" test "${PROFILE_ARGS[@]}" --locked --manifest-path server/Cargo.toml -j2 --lib --bins
# Isolate identity, physical grants and backend target state between executable
# integration fixtures. Never let one fixture's quota settings affect another.
if [ -n "${MANGA_TEST_FIXTURE:-}" ] && [ ! -f "server/tests/$MANGA_TEST_FIXTURE.rs" ]; then
  echo "Unknown test fixture: $MANGA_TEST_FIXTURE" >&2; exit 2
fi
for test_file in server/tests/*.rs; do
  fixture=$(basename "$test_file" .rs)
  # Observations of known gaps are an explicit diagnostic, never a correctness
  # pass. Opt in separately with MANGA_TEST_FIXTURE=audit_gaps.
  if [ "$fixture" = audit_gaps ] && [ -z "${MANGA_TEST_FIXTURE:-}" ]; then
    echo 'Optional audit_gaps characterization excluded from correctness gate'
    continue
  fi
  if [ -n "${MANGA_TEST_FIXTURE:-}" ] && [ "$fixture" != "$MANGA_TEST_FIXTURE" ]; then continue; fi
  "$PG/dropdb" -h 127.0.0.1 -p 55432 --force manga_test
  "$PG/createdb" -h 127.0.0.1 -p 55432 manga_test
  export MANGA_TEST_BACKEND_DIR="$RUNTIME/backend-$fixture"
  "$CARGO_HOME/bin/cargo" test "${PROFILE_ARGS[@]}" --locked --manifest-path server/Cargo.toml -j2 --test "$fixture" -- --include-ignored
done
"$CARGO_HOME/bin/cargo" clippy "${PROFILE_ARGS[@]}" --locked --manifest-path server/Cargo.toml -j2 --all-targets -- -D warnings

"$CARGO_HOME/bin/cargo" build "${PROFILE_ARGS[@]}" --locked --manifest-path server/Cargo.toml --bin pg_fixture_worker -j2
export MANGA_TEST_WORKER="$CARGO_TARGET_DIR/$BIN_DIR/pg_fixture_worker"
export MANGA_TEST_EVENT_PATH="$RUNTIME/event.json"
export MANGA_TEST_PG_CTL="$PG/pg_ctl" MANGA_TEST_PG_DATA="$RUNTIME/data" MANGA_TEST_PG_LOG="$RUNTIME/server.log"
export MANGA_PROCESS_REPORT="$ROOT/results/postgres-multiprocess.json"
python3 server/tests/multiprocess.py
"$PG/psql" -h 127.0.0.1 -p 55432 -d manga_test -v ON_ERROR_STOP=1 -c "SELECT current_setting('fsync'),current_setting('synchronous_commit');"
