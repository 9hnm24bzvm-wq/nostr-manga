#!/usr/bin/env python3
"""Disposable real CLI/HTTP/PostgreSQL acceptance; never accepts an operator DSN.

No installations, browser phases, production services or external writes. Binaries
must be built from the checkout beforehand. Private fixture logs stay test-runtime.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import signal
import socket
import subprocess
import tempfile
import time
import urllib.error
import urllib.request

ROOT = Path(__file__).resolve().parents[1]
OWNER = "fixture_owner"
RUNTIME = "fixture_runtime"


def require(value, message):
    if not value:
        raise RuntimeError(message)


def free_port():
    with socket.socket() as sock:
        sock.bind(("127.0.0.1", 0))
        return sock.getsockname()[1]


class Fixture:
    def __init__(self, args, run):
        self.args, self.run = args, run
        self.pg_port, self.http_port = free_port(), free_port()
        self.pg = None
        self.server = None
        self.concurrent = []
        self.database = "weebdex_privileges_test"
        self.checks = []
        self.pg_log = run / "postgres.log"
        ui = run / "ui"
        ui.mkdir()
        (ui / "index.html").write_text('<script>const __sveltekit_fixture = {}; import("/_app/start.js").then(x => x.start());</script>')
        self.config = run / "platform.json"
        self.config.write_text(json.dumps({
            "bind": f"127.0.0.1:{self.http_port}",
            "public_origin": f"http://127.0.0.1:{self.http_port}",
            "ui_dir": str(ui), "uploads_dir": str(run / "uploads"),
            "database_plaintext_local": True, "database_connections": 4,
            "registration_enabled": False, "shutdown_grace_seconds": 15,
        }))
        self.node_config = run / "node.json"
        self.node_config.write_text(json.dumps({
            "bind": "127.0.0.1:1", "base_url": "http://127.0.0.1:1",
            "service_key_file": str(run / "unused-key"),
            "admin_pubkey": "00" * 32, "database_plaintext_local": True,
            "database_connections": 4,
        }))

    def passed(self, name):
        self.checks.append(name)
        print(f"PASS {name}", flush=True)

    def env(self, role):
        env = os.environ.copy()
        for key in ["PLATFORM_DATABASE_URL", "MANGA_DATABASE_URL", "PLATFORM_CONFIG",
                    "PLATFORM_BIND", "PLATFORM_PUBLIC_ORIGIN", "PLATFORM_UI_DIR",
                    "PLATFORM_UI_FALLBACK", "PLATFORM_PROTOCOL_ORIGIN", "PLATFORM_PRODUCTION",
                    "PLATFORM_UPLOADS_DIR", "PGHOST", "PGPORT", "PGUSER", "PGDATABASE",
                    "PGPASSWORD", "PGSERVICE", "PGSERVICEFILE", "PGOPTIONS"]:
            env.pop(key, None)
        env.update(PLATFORM_DATABASE_URL=self.dsn(role), MANGA_DATABASE_URL=self.dsn(role),
                   PLATFORM_CONFIG=str(self.config), RUST_LOG="info")
        return env

    def dsn(self, role, database=None):
        return f"host=127.0.0.1 port={self.pg_port} user={role} dbname={database or self.database}"

    def command(self, command, env=None, ok=True, timeout=30):
        result = subprocess.run(command, env=env, text=True, capture_output=True, timeout=timeout)
        with (self.run / "commands.log").open("a") as log:
            log.write(result.stdout + result.stderr)
        require((result.returncode == 0) == ok, "public-seam command returned an unexpected exit status; inspect private fixture log")
        return result

    def sql(self, sql, role=OWNER, ok=True, database=None):
        result = self.command([str(self.args.pg_bin / "psql"), "-X", "-A", "-t", "-v", "ON_ERROR_STOP=1",
                               self.dsn(role, database), "-c", sql], env=self.env(role), ok=ok)
        return result.stdout.strip()

    def action(self, action, role=RUNTIME, ok=True):
        return self.command([str(self.args.platform_bin), action], env=self.env(role), ok=ok)

    def prepare(self):
        self.action("migrate", OWNER)

    def grants(self):
        self.sql(f"""
            REVOKE ALL ON SCHEMA public,platform FROM PUBLIC;
            GRANT USAGE ON SCHEMA public,platform TO {RUNTIME};
            GRANT SELECT ON ALL TABLES IN SCHEMA public,platform TO {RUNTIME};
            GRANT INSERT,UPDATE,DELETE ON ALL TABLES IN SCHEMA platform TO {RUNTIME};
            REVOKE INSERT,UPDATE,DELETE ON platform.schema_migrations FROM {RUNTIME};
            DO $guard$ BEGIN IF to_regclass('platform.protocol_visibility_policy') IS NOT NULL THEN
                EXECUTE 'REVOKE INSERT,UPDATE,DELETE ON platform.protocol_visibility_policy FROM {RUNTIME}';
            END IF; END $guard$;
            GRANT USAGE,SELECT ON ALL SEQUENCES IN SCHEMA platform TO {RUNTIME};
            REVOKE ALL ON ALL FUNCTIONS IN SCHEMA platform FROM PUBLIC;
            GRANT EXECUTE ON ALL FUNCTIONS IN SCHEMA platform TO {RUNTIME};
        """)

    def start(self):
        self.server = subprocess.Popen([str(self.args.platform_bin), "serve"], env=self.env(RUNTIME),
                                       stdout=(self.run / "server.log").open("ab"), stderr=subprocess.STDOUT)
        for _ in range(100):
            require(self.server.poll() is None, "restricted serve exited before readiness; inspect private fixture log")
            try:
                if self.http("/readyz")[0] == 200:
                    return
            except (OSError, urllib.error.URLError):
                pass
            time.sleep(0.1)
        raise RuntimeError("restricted serve never became ready")

    def stop(self):
        if self.server is not None:
            if self.server.poll() is None:
                self.server.send_signal(signal.SIGTERM)
            require(self.server.wait(timeout=20) == 0, "platform did not terminate gracefully")
            self.server = None

    def http(self, path, method="GET", payload=None, cookie=None):
        headers = {"Origin": f"http://127.0.0.1:{self.http_port}"}
        if cookie:
            headers["Cookie"] = cookie
        if payload is not None:
            headers["Content-Type"] = "application/json"
        request = urllib.request.Request(f"http://127.0.0.1:{self.http_port}{path}", method=method,
                                         headers=headers, data=None if payload is None else json.dumps(payload).encode())
        try:
            response = urllib.request.urlopen(request, timeout=3)
        except urllib.error.HTTPError as response:
            return response.code, response.read()
        with response:
            return response.status, response.read()

    def rejects_readiness(self, label):
        self.action("verify", ok=False)
        self.action("serve", ok=False)
        with socket.socket() as sock:
            require(sock.connect_ex(("127.0.0.1", self.http_port)) != 0, "rejected policy still opened a listener")
        self.passed(label)

    def cleanup(self):
        clean = True
        for process in [self.server] + self.concurrent + [self.pg]:
            if process is None:
                continue
            if process.poll() is None:
                process.send_signal(signal.SIGTERM if process is self.server else signal.SIGINT)
            try:
                process.wait(timeout=20)
            except subprocess.TimeoutExpired:
                clean = False
                process.kill()
                process.wait(timeout=5)
        (self.run / "cleanup.json").write_text(json.dumps({"owned_processes_stopped": True, "graceful": clean}))
        require(clean, "fixture cleanup required forced termination")

    def execute(self):
        self.command([str(self.args.pg_bin / "initdb"), "-D", str(self.run / "data"),
                      "--username=fixture_admin", "--auth-local=reject", "--auth-host=trust",
                      "--no-locale", "--encoding=UTF8"], env=self.env(OWNER))
        self.pg = subprocess.Popen([str(self.args.pg_bin / "postgres"), "-D", str(self.run / "data"),
                                    "-k", "", "-h", "127.0.0.1", "-p", str(self.pg_port),
                                    "-c", "max_connections=16", "-c", "shared_buffers=16MB",
                                    "-c", "log_statement=ddl", "-c", "log_line_prefix=%u "],
                                   stdout=self.pg_log.open("wb"), stderr=subprocess.STDOUT, env=self.env(OWNER))
        for _ in range(100):
            require(self.pg.poll() is None, "owned PostgreSQL exited before readiness")
            ready = subprocess.run([str(self.args.pg_bin / "pg_isready"), "-h", "127.0.0.1", "-p", str(self.pg_port)],
                                   stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
            if ready.returncode == 0:
                break
            time.sleep(0.1)
        else:
            raise RuntimeError("owned PostgreSQL never became ready")
        self.sql(f"CREATE ROLE {OWNER} LOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE; CREATE ROLE {RUNTIME} LOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE NOINHERIT;", "fixture_admin", database="postgres")
        self.sql(f"CREATE DATABASE {self.database} OWNER {OWNER}", "fixture_admin", database="postgres")
        self.sql(f"REVOKE ALL ON DATABASE {self.database} FROM PUBLIC; GRANT CONNECT ON DATABASE {self.database} TO {RUNTIME}")

        # Supported baseline upgrade through PostgreSQL setup, not private Rust methods.
        manifest = (ROOT / "backend/src/domains/mod.rs").read_text()
        migrations = re.findall(r'version:\s*(\d+),\s*name:\s*"([^"]+)",\s*sql:\s*include_str!\("../../migrations/([^"]+)"\)', manifest)
        require(migrations and int(migrations[-1][0]) == 86, "unexpected migration manifest")
        self.sql("CREATE SCHEMA platform; CREATE TABLE platform.schema_migrations(version BIGINT PRIMARY KEY,name TEXT NOT NULL,checksum BYTEA NOT NULL,applied_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp());")
        for version, name, filename in migrations[:-1]:
            source = (ROOT / "backend/migrations" / filename).read_bytes()
            self.sql("BEGIN;" + source.decode() + f"INSERT INTO platform.schema_migrations VALUES({version},'{name}',decode('{hashlib.sha256(source).hexdigest()}','hex'),clock_timestamp()); COMMIT;")
        self.grants()
        self.rejects_readiness("unprepared historical baseline rejects runtime")
        self.prepare()
        self.grants()
        self.action("verify")
        require(self.sql("SELECT platform.protocol_source_is_visible(NULL),platform.protocol_source_is_visible(repeat('a',64))", RUNTIME) == "t|f", "native-only policy exposed source-linked data")
        self.passed("supported upgrade prepares fail-closed native-only policy")

        # Establish real original schema through its public privileged CLI.
        self.command([str(self.args.manga_node_bin), "migrate", str(self.node_config)], env=self.env(OWNER))
        self.grants()
        self.rejects_readiness("native-only policy refuses newly available original schema")
        self.prepare()
        self.action("verify")
        self.passed("owner prepares canonical policy; restricted verify passes")

        history = self.sql("SELECT version,name,encode(checksum,'hex') FROM platform.schema_migrations ORDER BY version")
        for statement in [
            "CREATE OR REPLACE FUNCTION platform.protocol_source_is_visible(source_event TEXT) RETURNS BOOLEAN LANGUAGE SQL AS $$ SELECT TRUE $$",
            "CREATE TABLE platform.runtime_ddl_forbidden(id INTEGER)",
            "UPDATE platform.schema_migrations SET name='tampered' WHERE version=1",
            "DELETE FROM public.schema_migrations WHERE version=1",
            "UPDATE platform.protocol_visibility_policy SET version=2",
        ]:
            self.sql(statement, RUNTIME, ok=False)
        require(self.sql("SELECT version,name,encode(checksum,'hex') FROM platform.schema_migrations ORDER BY version") == history, "restricted role changed migration history")
        self.passed("runtime cannot replace functions, create schema objects or mutate either history/policy")

        offset = self.pg_log.stat().st_size
        self.action("verify")
        self.start()
        require(self.http("/livez")[0] == 200, "restricted listener is not live")
        token = "fixture-runtime-session"
        digest = hashlib.sha256(token.encode()).hexdigest()
        self.sql(f"INSERT INTO platform.users(id,username,email,password_hash,verified,roles,permissions) VALUES('RtOwner001','RuntimeOwner','runtime@fixture.invalid','fixture-no-password',TRUE,'[\"staff\"]','[\"author.create\"]'); INSERT INTO platform.sessions(id,user_id,token_hash,expires_at) VALUES('RtOwner001','RtOwner001',decode('{digest}','hex'),clock_timestamp()+interval '1 hour');")
        status, body = self.http("/api/author", "POST", {"name": "Runtime author"}, "weebdex_session=" + token)
        require(status == 201 and json.loads(body)["name"] == "Runtime author", "normal restricted HTTP write failed")
        self.stop()
        require("fixture_runtime LOG:  statement:" not in self.pg_log.read_bytes()[offset:].decode(), "runtime verify/serve issued DDL")
        self.passed("restricted real HTTP readiness/write/drain succeeds without startup DDL")

        event = "ab" * 32
        self.sql(f"INSERT INTO public.events(id,publisher,kind,created_at,raw_event,state) VALUES(decode('{event}','hex'),decode(repeat('cd',32),'hex'),1002,1,'{{}}','hosted')")
        require(self.sql(f"SELECT platform.protocol_source_is_visible('{event}')", RUNTIME) == "t", "canonical hosted publication invisible")
        self.sql(f"INSERT INTO public.backend_outbox(event_id,action,payload) VALUES(decode('{event}','hex'),'import','{{}}')")
        require(self.sql(f"SELECT platform.protocol_source_is_visible('{event}')", RUNTIME) == "f", "pending projection exposed")
        self.sql("UPDATE public.backend_outbox SET completed_at=clock_timestamp()")
        require(self.sql(f"SELECT platform.protocol_source_is_visible('{event}')", RUNTIME) == "t", "completed projection invisible")
        self.sql(f"UPDATE public.events SET state='removed' WHERE id=decode('{event}','hex')")
        require(self.sql(f"SELECT platform.protocol_source_is_visible('{event}')", RUNTIME) == "f", "removed publication exposed")
        self.passed("canonical hosted/pending-projection/removal policy survives role separation")

        for change, label in [
            ("DELETE FROM platform.protocol_visibility_policy", "missing policy attestation rejects readiness"),
            ("UPDATE platform.protocol_visibility_policy SET version=2", "wrong policy version rejects readiness"),
            ("CREATE OR REPLACE FUNCTION platform.protocol_source_is_visible(source_event TEXT) RETURNS BOOLEAN LANGUAGE SQL STABLE AS $$ SELECT TRUE $$", "mismatched function body rejects readiness"),
            ("ALTER FUNCTION platform.protocol_source_is_visible(TEXT) SECURITY DEFINER", "mismatched function security attributes reject readiness"),
        ]:
            self.sql(change)
            self.rejects_readiness(label)
            self.prepare()
        self.sql("ALTER FUNCTION platform.protocol_source_is_visible(TEXT) RENAME TO missing_policy")
        self.rejects_readiness("missing visibility function rejects readiness")
        self.sql("ALTER FUNCTION platform.missing_policy(TEXT) RENAME TO protocol_source_is_visible")
        self.prepare()

        # Preparation DDL and attestation are one transaction, including failures.
        self.sql("ALTER FUNCTION platform.protocol_source_is_visible(TEXT) SECURITY DEFINER; CREATE FUNCTION platform.reject_policy_update() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'fixture preparation rollback'; END $$; CREATE TRIGGER reject_policy_update BEFORE INSERT OR UPDATE ON platform.protocol_visibility_policy FOR EACH ROW EXECUTE FUNCTION platform.reject_policy_update();")
        self.action("migrate", OWNER, ok=False)
        require(self.sql("SELECT prosecdef FROM pg_proc WHERE oid='platform.protocol_source_is_visible(text)'::regprocedure") == "t", "failed preparation committed replacement DDL")
        self.sql("DROP TRIGGER reject_policy_update ON platform.protocol_visibility_policy; DROP FUNCTION platform.reject_policy_update()")
        self.prepare()
        self.passed("failed privileged preparation rolls back function replacement")

        self.concurrent = [subprocess.Popen([str(self.args.platform_bin), "migrate"], env=self.env(OWNER), stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL) for _ in range(2)]
        require(all(process.wait(timeout=30) == 0 for process in self.concurrent), "concurrent preparation failed")
        require(self.sql("SELECT version,name,encode(checksum,'hex') FROM platform.schema_migrations ORDER BY version") == history, "concurrent preparation changed history")
        self.action("verify")
        self.passed("concurrent privileged migrations/preparation serialize without history drift")

        expected_checksum = hashlib.sha256((ROOT / "backend/migrations/0001_catalog_people.sql").read_bytes()).hexdigest()
        self.sql("UPDATE platform.schema_migrations SET checksum=decode(repeat('00',32),'hex') WHERE version=1")
        self.rejects_readiness("platform schema checksum drift rejects runtime")
        self.action("migrate", OWNER, ok=False)
        self.sql(f"UPDATE platform.schema_migrations SET checksum=decode('{expected_checksum}','hex') WHERE version=1")
        self.sql("INSERT INTO platform.schema_migrations(version,name,checksum) VALUES(999,'future-fixture',decode(repeat('00',32),'hex'))")
        self.rejects_readiness("newer platform migration history rejects runtime")
        self.action("migrate", OWNER, ok=False)
        self.sql("DELETE FROM platform.schema_migrations WHERE version=999")
        self.action("verify")

        self.sql("UPDATE public.schema_migrations SET checksum=decode(repeat('00',32),'hex') WHERE version=1")
        self.rejects_readiness("canonical policy refuses original schema checksum drift")
        self.prepare()
        self.action("verify")
        require(self.sql(f"SELECT platform.protocol_source_is_visible('{event}')", RUNTIME) == "f", "native-only reset exposed source data")
        self.passed("deliberate reset after original drift retains native-only fail-closed behavior")

        # Fresh platform install with original schema is independently prepared.
        self.database = "weebdex_privileges_fresh"
        self.sql(f"CREATE DATABASE {self.database} OWNER {OWNER}", "fixture_admin", database="postgres")
        self.command([str(self.args.manga_node_bin), "migrate", str(self.node_config)], env=self.env(OWNER))
        self.prepare()
        self.grants()
        self.sql(f"REVOKE ALL ON DATABASE {self.database} FROM PUBLIC; GRANT CONNECT ON DATABASE {self.database} TO {RUNTIME}")
        self.action("verify")
        self.start()
        self.stop()
        self.passed("fresh original/platform install starts with restricted runtime")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--pg-bin", required=True, type=Path)
    parser.add_argument("--platform-bin", required=True, type=Path)
    parser.add_argument("--manga-node-bin", required=True, type=Path)
    args = parser.parse_args()
    args.pg_bin = args.pg_bin.resolve()
    args.platform_bin = args.platform_bin.resolve()
    args.manga_node_bin = args.manga_node_bin.resolve()
    for binary in [args.platform_bin, args.manga_node_bin] + [args.pg_bin / name for name in ["initdb", "postgres", "psql", "pg_isready"]]:
        require(binary.is_file() and os.access(binary, os.X_OK), f"required prepared executable unavailable: {binary}")
    require(os.geteuid() != 0, "PostgreSQL fixture requires an unprivileged user")
    runtime = ROOT / "test-runtime"
    runtime.mkdir(exist_ok=True)
    run = Path(tempfile.mkdtemp(prefix="runtime-privileges-", dir=runtime))
    run.chmod(0o700)
    fixture = Fixture(args, run)
    status = "failed"
    try:
        fixture.execute()
        status = "passed"
    finally:
        fixture.cleanup()
        (run / "summary.json").write_text(json.dumps({"status": status, "checks": fixture.checks,
            "seams": ["public CLI", "real HTTP", "real PostgreSQL distinct roles"],
            "browser_or_load_tests": False}, indent=2) + "\n")
    print(f"All {len(fixture.checks)} public-seam checks passed. Private fixture evidence: {run}")


if __name__ == "__main__":
    main()
