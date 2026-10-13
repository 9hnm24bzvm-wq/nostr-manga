#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/../.."
PG=$PWD/vendor/pgsql/bin
export MIXED_DIAGNOSTIC_MAX_WAL_MB=${MIXED_DIAGNOSTIC_MAX_WAL_MB:-512}
case "$MIXED_DIAGNOSTIC_MAX_WAL_MB" in
    512|1024) ;;
    *) echo 'Disposable diagnostic fixture max_wal_size must be 512 or 1024MB.' >&2; exit 2 ;;
esac
RUNTIME=$(mktemp -d /tmp/mixed-diagnostic-XXXXXX)
cleanup() {
    status=$?
    owned_pid=''
    if [ -e "$RUNTIME/data/postmaster.pid" ]; then read -r owned_pid < "$RUNTIME/data/postmaster.pid"; fi
    "$PG/pg_ctl" -D "$RUNTIME/data" -m fast -w stop >/dev/null 2>&1 || true
    if [ ! -e "$RUNTIME/data/postmaster.pid" ] && { [ -z "$owned_pid" ] || ! kill -0 "$owned_pid" 2>/dev/null; }; then
        echo 'Disposable PostgreSQL fixture stopped; no owned postmaster remains.'
        rm -rf "$RUNTIME"
    else
        echo "Disposable PostgreSQL fixture could not be stopped: $RUNTIME" >&2
        status=1
    fi
    exit "$status"
}
trap cleanup EXIT
"$PG/initdb" -D "$RUNTIME/data" --auth=trust --no-locale --encoding=UTF8 >"$RUNTIME/init.log" 2>&1
printf "\nlisten_addresses='127.0.0.1'\nport=55437\nunix_socket_directories=''\nshared_buffers='16MB'\nmax_wal_size='%sMB'\n" "$MIXED_DIAGNOSTIC_MAX_WAL_MB" >>"$RUNTIME/data/postgresql.conf"
if ! "$PG/pg_ctl" -D "$RUNTIME/data" -l "$RUNTIME/server.log" -w start; then
    cat "$RUNTIME/server.log"
    exit 1
fi
export PGHOST=127.0.0.1 PGPORT=55437 PGUSER=$(id -un) PGDATABASE=mixed_diagnostic MIXED_DIAGNOSTIC_TEST_DB=mixed_diagnostic MIXED_PSQL="$PG/psql" MIXED_DIAGNOSTIC_OWNED_RUNTIME="$RUNTIME"
"$PG/createdb" mixed_diagnostic
{ echo BEGIN\;; cat server/migrations/*.sql; echo COMMIT\;; } | "$PG/psql" -X -q -v ON_ERROR_STOP=1 >/dev/null
"$PG/psql" -X -q -v ON_ERROR_STOP=1 <<'SQL'
CREATE TABLE diagnostic_wait_fixture (value integer);
INSERT INTO events(id,publisher,kind,created_at,raw_event,state,retired_at) VALUES
(decode(repeat('11',32),'hex'),decode(repeat('22',32),'hex'),1,1,'{}','hosted',NULL),
(decode(repeat('33',32),'hex'),decode(repeat('22',32),'hex'),11999,1,'{}','removed',clock_timestamp()-interval '6 minutes');
INSERT INTO backend_outbox(event_id,action,payload,created_at) VALUES
(decode(repeat('11',32),'hex'),'import','{}',clock_timestamp()-interval '1 minute'),
(decode(repeat('11',32),'hex'),'import','{}',clock_timestamp()),
(decode(repeat('33',32),'hex'),'delete','{}',clock_timestamp());
SQL
PYTHONPATH="$PWD/vendor/pydeps:$PWD/bench/library-node:$PWD" python3 -m unittest discover -s bench/library-node -p test_pg_diagnostics.py -v
