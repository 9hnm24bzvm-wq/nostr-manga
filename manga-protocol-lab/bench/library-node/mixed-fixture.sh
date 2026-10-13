#!/usr/bin/env bash
# Disposable two-node/real-nginx fixture. Explicit gate prevents accidental load.
set -euo pipefail
cd "$(dirname "$0")/../.."
if [ "${MIXED_FUNCTIONAL_GATE:-}" != passed ]; then
 echo 'Blocked: finish the canonical functional gate before setting MIXED_FUNCTIONAL_GATE=passed.' >&2; exit 2
fi
NAME=${1:?Provide a new result name}; shift
[[ "$NAME" =~ ^[a-zA-Z0-9][a-zA-Z0-9._-]*$ ]] || exit 2
ROOT=$PWD
PG="${MIXED_PG_BIN_DIR:-$ROOT/vendor/pgsql/bin}"
BIN="${MIXED_NODE_BIN:-${CARGO_TARGET_DIR:-$ROOT/.build-release}/release/manga-node}"
NGINX="${MIXED_NGINX_BIN:-$ROOT/vendor/nginx-1.28.0/objs/nginx}"
STRFRY="${MIXED_STRFRY_BIN:-$ROOT/vendor/strfry/strfry}"
export MIXED_SELECTED_NGINX="$NGINX" MIXED_SELECTED_STRFRY="$STRFRY"
: "${MIXED_EXPECTED_NODE_SHA256:?Provide the canonical-gate-tested node SHA256}"
export NODE_RESULTS="$ROOT/results/library-node-mixed/$NAME"
[ ! -e "$NODE_RESULTS" ] || { echo 'Result path already exists' >&2; exit 2; }
# Argument-only validation does not initialize a database or start traffic.
python3 bench/library-node/mixed.py --plan "$@" | python3 -c 'import json,sys; assert json.load(sys.stdin)["origins"] == dict(api="http://127.0.0.1:58200",peer="http://127.0.0.1:58201",managed_nginx="http://127.0.0.1:58202"), "Launcher requires default origins; direct driver supports custom loopback fixtures"' 
[ -x "$BIN" ] && [ -x "$NGINX" ] && [ -x "$STRFRY" ] && [ -x "$PG/postgres" ]
PG=$(realpath "$PG"); BIN=$(realpath "$BIN"); NGINX=$(realpath "$NGINX"); STRFRY=$(realpath "$STRFRY")
export MIXED_SELECTED_NGINX="$NGINX" MIXED_SELECTED_STRFRY="$STRFRY"
[[ "$MIXED_EXPECTED_NODE_SHA256" =~ ^[0-9a-f]{64}$ ]] || exit 2
[ "$(sha256sum "$BIN" | cut -d' ' -f1)" = "$MIXED_EXPECTED_NODE_SHA256" ] || { echo 'Selected node differs from canonical-gate hash' >&2; exit 2; }
mkdir -p "$NODE_RESULTS" "$ROOT/.runtime-final/tmp" "$ROOT/.bench-runtime"
export TMPDIR="$ROOT/.runtime-final/tmp" PYTHONPATH="$ROOT/vendor/pydeps${PYTHONPATH:+:$PYTHONPATH}"
RUNTIME=$(mktemp -d "$ROOT/.bench-runtime/mixed-XXXXXX")
export MIXED_RUNTIME="$RUNTIME"
cleanup(){
 code=$?; trap - EXIT
 for pid in "${SERVER:-}" "${PEER:-}"; do [ -z "$pid" ] || kill -TERM "$pid" 2>/dev/null || true; done
 for pid in "${SERVER:-}" "${PEER:-}"; do
  [ -z "$pid" ] && continue
  for _ in $(seq 1 600); do kill -0 "$pid" 2>/dev/null || break; sleep .05; done
  if kill -0 "$pid" 2>/dev/null; then kill -KILL "$pid" 2>/dev/null || true; code=2; fi
  wait "$pid" 2>/dev/null || true
 done
 pg_stopped=0
 if [ ! -f "$RUNTIME/data/PG_VERSION" ]; then pg_stopped=1
 else
  "$PG/pg_ctl" -D "$RUNTIME/data" -m fast -w stop >/dev/null 2>&1 || code=2
  set +e
  "$PG/pg_ctl" -D "$RUNTIME/data" status >/dev/null 2>&1; pg_status=$?
  set -e
  if [ "$pg_status" -eq 3 ]; then pg_stopped=1; else code=2; fi
 fi
 cp "$RUNTIME/pg.log" "$NODE_RESULTS/postgres.log" 2>/dev/null || true
 du -sb "$RUNTIME/data" "$RUNTIME/node-0" "$RUNTIME/node-1" >"$NODE_RESULTS/disk-final.txt" 2>/dev/null || true
 # Failure retains owned files for diagnosis. Never remove a still-listening reader.
 if [ "$code" -eq 0 ] && [ "$pg_stopped" -eq 1 ] && python3 - <<'PY'
import socket
for port in (58200,58201,58202,55441):
 try:
  connection=socket.create_connection(('127.0.0.1',port),timeout=.2);connection.close();raise SystemExit(1)
 except OSError:pass
PY
 then rm -rf "$RUNTIME"; else code=2; echo "Fixture retained for diagnosis: $RUNTIME" >&2; fi
 exit "$code"
}; trap cleanup EXIT
python3 bench/library-node/fixture_config.py "$RUNTIME" "$ROOT"
"$PG/initdb" -D "$RUNTIME/data" --auth-local=peer --auth-host=scram-sha-256 --pwfile="$RUNTIME/password" --no-locale --encoding=UTF8 >"$RUNTIME/init.log" 2>&1
cat "$RUNTIME/postgresql-fixture.conf" >>"$RUNTIME/data/postgresql.conf"
"$PG/pg_ctl" -D "$RUNTIME/data" -l "$RUNTIME/pg.log" -w start >/dev/null
export PGPASSWORD=$(cat "$RUNTIME/password")
export PGHOST=127.0.0.1 PGPORT=55441 PGUSER=$(id -un) MIXED_PSQL="$PG/psql"
export NODE_PG_PID=$(head -1 "$RUNTIME/data/postmaster.pid")
for index in 0 1; do
 "$PG/createdb" -h127.0.0.1 -p55441 "manga_mixed_$index"
 dsn="host=127.0.0.1 port=55441 user=$(id -un) password=$PGPASSWORD dbname=manga_mixed_$index"
 MANGA_DATABASE_URL="$dsn" "$BIN" migrate "$RUNTIME/node-$index/node.json" >"$NODE_RESULTS/migration-$index.log" 2>&1
 MANGA_DATABASE_URL="$dsn" "$BIN" serve "$RUNTIME/node-$index/node.json" >"$NODE_RESULTS/server-$index.log" 2>&1 & pid=$!
 if [ "$index" -eq 0 ]; then SERVER=$pid; else PEER=$pid; fi
 done
export NODE_PID="$SERVER" NODE_PEER_PID="$PEER"
python3 - <<'PY'
import time,urllib.request
for port in (58200,58201):
 for _ in range(200):
  try:urllib.request.urlopen(f'http://127.0.0.1:{port}',timeout=.5);break
  except OSError:time.sleep(.1)
 else:raise RuntimeError('node startup deadline')
PY
sha256sum "$BIN" "$NGINX" "$STRFRY" "$PG/postgres" bench/library-node/{mixed.py,mixed_metrics.py,image_shards.py,pg_diagnostics.py,mixed-fixture.sh,fixture_config.py,diagnostic_observer.py,run.py,process_metrics.py} >"$NODE_RESULTS/hashes.txt"
find server/src server/migrations src -type f \( -name '*.rs' -o -name '*.sql' -o -name '*.py' \) -print0 | sort -z | xargs -0 sha256sum >"$NODE_RESULTS/server-source-hashes.txt"
find . -maxdepth 3 -type f \( -name Cargo.toml -o -name Cargo.lock \) -print0 | sort -z | xargs -0 sha256sum >>"$NODE_RESULTS/server-source-hashes.txt"
sha256sum "$NODE_RESULTS/server-source-hashes.txt" >"$NODE_RESULTS/source-identity.txt"
printf '%s\n' "$BIN" >"$NODE_RESULTS/selected-node.txt"
python3 - <<'PY' >"$NODE_RESULTS/generator-environment.json"
import importlib.metadata,json,os,platform
print(json.dumps(dict(python=platform.python_version(),packages={p:importlib.metadata.version(p) for p in ('aiohttp','coincurve','cbor2')},cpu_count=os.cpu_count(),notes='Generator shares this host with both nodes, PostgreSQL and nginx.'),indent=2))
PY
"$PG/psql" -h127.0.0.1 -p55441 -d manga_mixed_0 -At -c "SELECT json_build_object('fsync',current_setting('fsync'),'synchronous_commit',current_setting('synchronous_commit'),'shared_buffers',current_setting('shared_buffers'));" >"$NODE_RESULTS/postgres-settings.json"
df -h "$RUNTIME" >"$NODE_RESULTS/filesystems.txt"; stat -f -c '%T' "$RUNTIME" >"$NODE_RESULTS/filesystem-type.txt"
du -sb "$RUNTIME/data" "$RUNTIME/node-0" "$RUNTIME/node-1" >"$NODE_RESULTS/disk-before-setup.txt"
python3 bench/library-node/mixed.py --execute "$@" | tee "$NODE_RESULTS/client.log"
