#!/usr/bin/env bash
# Private, local demonstration. Known synthetic keys; never expose these nodes.
set -euo pipefail
ROOT=$(cd "$(dirname "$0")" && pwd)
CHECK=false
case "${1:-}" in --check) CHECK=true;; '') ;; *) echo 'Usage: ./run-demo.sh [--check]' >&2; exit 2;; esac
[[ $(id -u) -ne 0 ]] || { echo 'Run as a normal user, not root: PostgreSQL refuses root.' >&2; exit 1; }
[[ $(uname -s) == Linux && $(uname -m) == x86_64 ]] || { echo 'Bundled binaries require Linux x86_64; use the included source on other systems.' >&2; exit 1; }
command -v python3 >/dev/null || { echo 'Python3 is required for local setup (standard library only).' >&2; exit 1; }
export LD_LIBRARY_PATH="$ROOT/postgres/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
for file in "$ROOT/bin/manga-node" "$ROOT/bin/manga-demo" "$ROOT/bin/strfry" "$ROOT/postgres/bin/postgres"; do
  [[ -x "$file" ]] || { echo "Missing executable: $file" >&2; exit 1; }
  if ldd "$file" 2>&1 | grep -q 'not found'; then ldd "$file" >&2; exit 1; fi
done
DATA=$(mktemp -d "${TMPDIR:-/tmp}/manga-mvp.XXXXXXXX")
PIDS=(); PG_STARTED=false; PASSED=false
cleanup(){
  local status=$?
  trap - EXIT INT TERM
  for pid in "${PIDS[@]}"; do kill "$pid" 2>/dev/null || true; done
  for pid in "${PIDS[@]}"; do wait "$pid" 2>/dev/null || true; done
  if $PG_STARTED; then "$ROOT/postgres/bin/pg_ctl" -D "$DATA/postgres" -m fast -t 15 -w stop >/dev/null 2>&1 || { echo "Database cleanup needs review: $DATA" >&2; exit 1; }; fi
  if $CHECK && $PASSED; then rm -rf -- "$DATA"; else echo "Local demo data/logs retained: $DATA" >&2; fi
  exit "$status"
}
trap cleanup EXIT
trap 'exit 130' INT TERM
"$ROOT/bin/manga-demo" --fixture-public-keys > "$DATA/public-keys.json"
python3 - "$ROOT" "$DATA" <<'PY'
import json,os,secrets,socket,sys
from pathlib import Path
root,data=map(Path,sys.argv[1:]);keys=json.loads((data/'public-keys.json').read_text())
sockets=[]
for _ in range(3):
 s=socket.socket();s.bind(('127.0.0.1',0));sockets.append(s)
ports=[s.getsockname()[1] for s in sockets]
for s in sockets:s.close()
(data/'ports').write_text('\n'.join(map(str,ports))+'\n')
fd=os.open(data/'password',os.O_WRONLY|os.O_CREAT|os.O_EXCL,0o600)
with os.fdopen(fd,'w') as f:f.write(secrets.token_hex(24))
origins=[f'http://127.0.0.1:{p}' for p in ports[1:]]
for n,origin in enumerate(origins):
 key=data/f'service-{n}.key';fd=os.open(key,os.O_WRONLY|os.O_CREAT|os.O_EXCL,0o600)
 with os.fdopen(fd,'w') as f:f.write(f'{11+n:064x}')
 cfg={'bind':f'127.0.0.1:{ports[n+1]}','base_url':origin,'service_key_file':str(key),'admin_pubkey':keys['admin'],'database_plaintext_local':True,'database_connections':12,'cache_bytes':8388608,'discovery_private_overrides':origins,'group_keys_dir':str(data/f'groups-{n}'),'enrollment':{'enabled':n==1,'max_new_accounts_per_hour':100,'max_self_enrolled_accounts':1000},'storage':{'path':str(data/f'originals-{n}'),'volume':f'mvp{n}','physical_bytes':67108864,'staging_bytes':33554432,'max_blob_bytes':1048576,'upload_slots':4},'backend':{'binary':str(root/'bin/strfry'),'data_dir':str(data/f'backend-{n}'),'timeout_seconds':10,'map_bytes':67108864}}
 (data/f'node-{n}.json').write_text(json.dumps(cfg,indent=2))
PY
mapfile -t PORTS < "$DATA/ports"
PG="$ROOT/postgres/bin"
"$PG/initdb" -D "$DATA/postgres" --auth-local=peer --auth-host=scram-sha-256 --pwfile="$DATA/password" --no-locale --encoding=UTF8 > "$DATA/initdb.log" 2>&1
"$PG/pg_ctl" -D "$DATA/postgres" -l "$DATA/postgres.log" -o "-k '' -p ${PORTS[0]} -c listen_addresses=127.0.0.1 -c shared_buffers=32MB -c max_connections=64" -w start
PG_STARTED=true
export PGPASSWORD=$(cat "$DATA/password")
for n in 0 1; do
 "$PG/createdb" -h 127.0.0.1 -p "${PORTS[0]}" "manga_mvp_$n"
 DSN="host=127.0.0.1 port=${PORTS[0]} user=$(id -un) password=$PGPASSWORD dbname=manga_mvp_$n"
 MANGA_DATABASE_URL="$DSN" "$ROOT/bin/manga-node" migrate "$DATA/node-$n.json" > "$DATA/migration-$n.log" 2>&1
 MANGA_DATABASE_URL="$DSN" "$ROOT/bin/manga-node" serve "$DATA/node-$n.json" > "$DATA/node-$n.log" 2>&1 &
 PIDS+=("$!")
done
A="http://127.0.0.1:${PORTS[1]}"; B="http://127.0.0.1:${PORTS[2]}"
"$ROOT/bin/manga-demo" "$A" "$B" | tee "$DATA/demo-report.json"
PASSED=true
if [[ -n "${MVP_REPORT:-}" ]]; then cp "$DATA/demo-report.json" "$MVP_REPORT"; fi
if $CHECK; then exit 0; fi
printf '\nBoth local API instances are running:\n  %s\n  %s\nData/logs: %s\nPress Ctrl+C to stop. Never expose these synthetic keys publicly.\n' "$A" "$B" "$DATA"
wait "${PIDS[0]}"
