#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/../.."
ROOT="$PWD"; PG="$ROOT/vendor/pgsql/bin"; mkdir -p .bench-runtime
RUNTIME=$(mktemp -d "$ROOT/.bench-runtime/library-node-XXXXXX")
NAME="${1:-serial}"
[[ "$NAME" =~ ^[a-zA-Z0-9][a-zA-Z0-9._-]*$ ]] || { echo "Use a simple unique result name" >&2; exit 2; }
export NODE_RESULTS="$ROOT/results/library-node/$NAME"
[ ! -e "$NODE_RESULTS" ] || { echo "Result path exists; choose a new result name: $NODE_RESULTS" >&2; exit 2; }
mkdir -p "$NODE_RESULTS"
cleanup(){ code=$?; if [ -n "${SERVER:-}" ];then kill "$SERVER" 2>/dev/null || true;wait "$SERVER" 2>/dev/null || true;fi; "$PG/pg_ctl" -D "$RUNTIME/data" -m fast -w stop >/dev/null 2>&1 || true; cp "$RUNTIME/pg.log" "$NODE_RESULTS/postgres.log" 2>/dev/null || true; if [ "$code" -eq 0 ];then rm -rf "$RUNTIME";else echo "Failure fixture retained: $RUNTIME";fi; };trap cleanup EXIT
export PYTHONPATH="$ROOT/vendor/pydeps"
python3 - "$RUNTIME" "$ROOT" <<'PY'
import json,os,secrets,sys
from pathlib import Path
from coincurve import PrivateKey,PublicKeyXOnly
r=Path(sys.argv[1]);root=Path(sys.argv[2])
for name,value in [('password',secrets.token_hex(24)),('service.key',f'{11:064x}')]:
 fd=os.open(r/name,os.O_WRONLY|os.O_CREAT|os.O_EXCL,0o600)
 with os.fdopen(fd,'w') as f:f.write(value)
admin=PublicKeyXOnly.from_secret(PrivateKey.from_int(1).secret).format().hex()
config=dict(bind='127.0.0.1:58200',base_url='http://127.0.0.1:58200',service_key_file=str(r/'service.key'),admin_pubkey=admin,database_plaintext_local=True,library_reclamation=os.environ.get('NODE_GC','0')=='1',database_connections=int(os.environ.get("NODE_DB_CONNECTIONS","16")),library_concurrency=dict(request_slots=int(os.environ.get("NODE_LIBRARY_REQUESTS","128")),processing_slots=int(os.environ.get("NODE_LIBRARY_WORK","8")),commit_slots=int(os.environ.get("NODE_LIBRARY_COMMITS","4"))),database_wait_ms=2000,cache_bytes=67108864,sync_worker_enabled=False,storage=dict(path=str(r/'objects'),volume='nodebench',physical_bytes=3000000000,staging_bytes=3000000000,max_blob_bytes=1000000,upload_slots=int(os.environ.get("NODE_STORAGE_SLOTS","4")),credit_lanes=int(os.environ.get("NODE_CREDIT_LANES","16")),reclamation_slots=int(os.environ.get("NODE_RECLAMATION_SLOTS","4"))),backend=dict(binary=str(root/'vendor/strfry/strfry'),data_dir=str(r/'backend'),map_bytes=1073741824))
(r/'node.json').write_text(json.dumps(config))
(Path(os.environ['NODE_RESULTS'])/'node-limits.json').write_text(json.dumps(dict(library_reclamation=config['library_reclamation'],database_connections=config['database_connections'],database_wait_ms=config['database_wait_ms'],cache_bytes=config['cache_bytes'],library_concurrency=config['library_concurrency'],storage_slots=config['storage']['upload_slots'],credit_lanes=config['storage']['credit_lanes'],reclamation_slots=config['storage']['reclamation_slots']),indent=2))
PY
"$PG/initdb" -D "$RUNTIME/data" --auth-local=peer --auth-host=scram-sha-256 --pwfile="$RUNTIME/password" --no-locale --encoding=UTF8 >"$RUNTIME/init.log" 2>&1
cat >>"$RUNTIME/data/postgresql.conf" <<'CONF'
listen_addresses='127.0.0.1'
port=55441
unix_socket_directories=''
shared_buffers='128MB'
max_connections=48
max_wal_size='512MB'
CONF
"$PG/pg_ctl" -D "$RUNTIME/data" -l "$RUNTIME/pg.log" -w start >/dev/null
export PGPASSWORD=$(cat "$RUNTIME/password")
"$PG/createdb" -h127.0.0.1 -p55441 manga_node_bench
export MANGA_DATABASE_URL="host=127.0.0.1 port=55441 user=$(id -un) password=$PGPASSWORD dbname=manga_node_bench"
BIN="$ROOT/prototype/target/release/manga-node"
"$BIN" migrate "$RUNTIME/node.json" >"$NODE_RESULTS/migration.log" 2>&1
"$BIN" serve "$RUNTIME/node.json" >"$NODE_RESULTS/server.log" 2>&1 & SERVER=$!
export NODE_PID="$SERVER" NODE_OBJECT_DIR="$RUNTIME/objects"
python3 - <<'PY'
import time,urllib.request
for i in range(100):
 try:urllib.request.urlopen('http://127.0.0.1:58200');break
 except Exception:time.sleep(.1)
else:raise RuntimeError('node startup timeout')
PY
sha256sum "$BIN" bench/library-node/run.py bench/library-node/load-fixture.sh bench/library-node/load.py bench/library-node/process_metrics.py >"$NODE_RESULTS/hashes.txt"
df -h "$RUNTIME" >"$NODE_RESULTS/filesystems.txt";stat -f -c '%T' "$RUNTIME" >"$NODE_RESULTS/filesystem-type.txt"
python3 bench/library-node/load.py | tee "$NODE_RESULTS/client.log"
"$PG/psql" -h127.0.0.1 -p55441 -d manga_node_bench -At -c "SELECT json_build_object('fsync',current_setting('fsync'),'synchronous_commit',current_setting('synchronous_commit'),'shared_buffers',current_setting('shared_buffers'),'pending',(SELECT count(*) FROM backend_outbox WHERE completed_at IS NULL),'events',(SELECT count(*) FROM events));" >"$NODE_RESULTS/postgres-final.json"
du -sb "$RUNTIME/data" "$RUNTIME/backend" "$RUNTIME/objects" >"$NODE_RESULTS/disk-final.txt"
