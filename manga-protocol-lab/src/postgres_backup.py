"""Offline co-located node backup/restore. Never starts a server.

Requires stopped storage/projection owners and a PostgreSQL DSN supplied in
MANGA_DATABASE_URL. Backups contain private keys: keep their directory private.
Restore only targets a new directory and an empty database, never overwrites one.
"""

from __future__ import annotations
import argparse, contextlib, ctypes, ctypes.util, fcntl, hashlib, json, os, re, select, shutil, stat, subprocess, time
from pathlib import Path
from src.operator_json import save_json, read_json


class BackupError(RuntimeError):
    pass


def sha(path):
    h = hashlib.sha256()
    with Path(path).open("rb") as f:
        for b in iter(lambda: f.read(1024 * 1024), b""):
            h.update(b)
    return h.hexdigest()


def pg_command(pg, name, dsn):
    # Credentials belong in the child's private environment, never process argv.
    command = [str(Path(pg) / name), "--no-password"]
    if name == "pg_restore":
        database = pg_environment(pg, dsn)["PGDATABASE"]
        if "=" in database or "://" in database:
            raise BackupError("plain destination database name required")
        command += ["--dbname", database]
    return command


def pg_environment(pg, dsn):
    class Option(ctypes.Structure):
        _fields_ = [
            (name, ctypes.c_char_p)
            for name in ("keyword", "envvar", "compiled", "val", "label", "dispchar")
        ] + [("dispsize", ctypes.c_int)]

    local = Path(pg).resolve().parent / "lib/libpq.so"
    name = str(local) if local.is_file() else ctypes.util.find_library("pq")
    if not name:
        raise BackupError("installed libpq required to safely parse connection options")
    lib = ctypes.CDLL(name)
    lib.PQconninfoParse.argtypes = [ctypes.c_char_p, ctypes.POINTER(ctypes.c_void_p)]
    lib.PQconninfoParse.restype = ctypes.POINTER(Option)
    lib.PQconninfoFree.argtypes = [ctypes.POINTER(Option)]
    lib.PQfreemem.argtypes = [ctypes.c_void_p]
    error = ctypes.c_void_p()
    options = lib.PQconninfoParse(dsn.encode(), ctypes.byref(error))
    if not options:
        if error.value:
            lib.PQfreemem(error)
        raise BackupError("invalid PostgreSQL connection options")
    env = os.environ.copy()
    env.pop("MANGA_DATABASE_URL", None)
    env.pop("PLATFORM_DATABASE_URL", None)
    env.pop("PGSERVICE", None)
    env.pop("PGSERVICEFILE", None)
    provided = set()
    i = 0
    while options[i].keyword:
        if options[i].envvar:
            env.pop(options[i].envvar.decode(), None)
        i += 1
    try:
        i = 0
        while options[i].keyword:
            option = options[i]
            i += 1
            if option.val is None:
                continue
            provided.add(option.keyword.decode())
            if option.keyword == b"service":
                raise BackupError(
                    "explicit host, database and user required; service indirection is outside this backup profile"
                )
            if not option.envvar:
                if option.keyword == b"fallback_application_name":
                    continue
                raise BackupError(
                    "connection option lacks safe environment mapping; use a libpq service definition"
                )
            env[option.envvar.decode()] = option.val.decode()
    finally:
        lib.PQconninfoFree(options)
    if not (
        {"dbname", "user"} <= provided
        and ("host" in provided or "hostaddr" in provided)
    ):
        raise BackupError("explicit host, database and user required")
    return env


def sql(pg, dsn, query):
    p = subprocess.run(
        pg_command(pg, "psql", dsn)
        + ["-X", "-qAt", "-v", "ON_ERROR_STOP=1", "-c", query],
        capture_output=True,
        text=True,
        env=pg_environment(pg, dsn),
    )
    if p.returncode:
        raise BackupError("PostgreSQL operation failed: " + p.stderr[-1200:])
    return p.stdout.strip()


def absolute(path):
    p = Path(path)
    if not p.is_absolute() or p.is_symlink():
        raise BackupError("explicit absolute non-symlink configured paths required")
    return p.resolve(strict=True)


def child(root, name):
    p = Path(name)
    if p.is_absolute() or ".." in p.parts or not p.parts:
        raise BackupError("unsafe backup path")
    result = root / p
    if not result.resolve().is_relative_to(root.resolve()):
        raise BackupError("backup path escapes directory")
    return result


@contextlib.contextmanager
def owners(config):
    files = []
    try:
        storage = absolute(config["storage"]["path"])
        backend = absolute(config["backend"]["data_dir"])
        for path in [
            storage / ".volume-owner",
            backend / ".projection-owner",
            backend / ".command-lock",
        ]:
            if path.is_symlink() or not path.is_file():
                raise BackupError("initialized owner lock required")
            f = path.open("r+b")
            files.append(f)
            try:
                fcntl.flock(f, fcntl.LOCK_EX | fcntl.LOCK_NB)
            except BlockingIOError:
                raise BackupError(
                    "source still has a live storage/projection owner or subprocess"
                ) from None
        yield
    finally:
        for f in reversed(files):
            f.close()


@contextlib.contextmanager
def snapshot(pg, dsn, log, platform=False):
    marker = "BACKUP_SNAPSHOT_READY"
    with Path(log).open("w") as err:
        p = subprocess.Popen(
            pg_command(pg, "psql", dsn) + ["-X", "-qAt", "-v", "ON_ERROR_STOP=1"],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=err,
            bufsize=0,
            env=pg_environment(pg, dsn),
        )
        try:
            command = (
                "BEGIN ISOLATION LEVEL REPEATABLE READ;SET LOCAL lock_timeout='5s';SET LOCAL idle_in_transaction_session_timeout=0;SELECT pg_advisory_xact_lock(697241113);"
                + ("SELECT pg_advisory_xact_lock(697241114);" if platform else "")
                + "DO $$ DECLARE r record;BEGIN FOR r IN SELECT schemaname,tablename FROM pg_tables WHERE "
                + ("schemaname IN ('public','platform')" if platform else "schemaname='public'")
                + " ORDER BY schemaname,tablename LOOP EXECUTE format('LOCK TABLE %I.%I IN SHARE MODE',r.schemaname,r.tablename);END LOOP;END $$;SELECT 'SNAP:'||pg_export_snapshot();SELECT '"
                + marker
                + "';\n"
            )
            p.stdin.write(command.encode())
            p.stdin.flush()
            snap = None
            deadline = time.monotonic() + 15
            buffer = b""
            ready = False
            while time.monotonic() < deadline and not ready:
                if not select.select(
                    [p.stdout], [], [], max(0, deadline - time.monotonic())
                )[0]:
                    break
                chunk = os.read(p.stdout.fileno(), 65536)
                if not chunk:
                    break
                buffer += chunk
                if len(buffer) > 65536:
                    raise BackupError("snapshot control output budget")
                while b"\n" in buffer:
                    line, buffer = buffer.split(b"\n", 1)
                    line = line.decode().strip()
                    if line.startswith("SNAP:"):
                        snap = line[5:]
                    if line == marker:
                        ready = True
            if not ready or not snap or not re.fullmatch(r"[0-9A-Fa-f-]+", snap):
                raise BackupError(
                    "could not lock/export stopped-node snapshot; inspect private log"
                )
            yield snap
        finally:
            if p.poll() is None:
                try:
                    p.stdin.write(b"ROLLBACK;\n\\q\n")
                    p.stdin.flush()
                    p.wait(timeout=5)
                except (BrokenPipeError, subprocess.TimeoutExpired):
                    p.kill()
                    p.wait()
            p.stdin.close()
            p.stdout.close()


def copy_file(source, destination):
    if source.is_symlink() or not source.is_file():
        raise BackupError("only regular backup files supported")
    destination.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
    with source.open("rb") as src, destination.open("xb") as dst:
        shutil.copyfileobj(src, dst, 1024 * 1024)
        dst.flush()
        os.fsync(dst.fileno())
    os.chmod(destination, stat.S_IMODE(source.stat().st_mode) & 0o700)


def copy_tree(source, destination, exclude_root=()):
    if source.is_symlink() or not source.is_dir():
        raise BackupError("only regular configured directories supported")
    destination.mkdir(parents=True, exist_ok=False, mode=0o700)
    for directory, dirs, files in os.walk(source, followlinks=False):
        dirs.sort()
        files.sort()
        base = Path(directory)
        if base == source:
            dirs[:] = [name for name in dirs if name not in exclude_root]
            files = [name for name in files if name not in exclude_root]
        for name in dirs:
            if (base / name).is_symlink():
                raise BackupError("symlink in backup tree")
            (destination / (base / name).relative_to(source)).mkdir(mode=0o700)
        for name in files:
            copy_file(base / name, destination / (base / name).relative_to(source))


def sync_dirs(root):
    for directory, _, _ in os.walk(root, topdown=False):
        fd = os.open(directory, os.O_RDONLY)
        try:
            os.fsync(fd)
        finally:
            os.close(fd)


def ledger_ok(pg, dsn):
    bad = sql(
        pg,
        dsn,
        "SELECT count(*) FROM capacity_budgets b WHERE allocated_bytes<>(SELECT COALESCE(sum(capacity),0) FROM capacity_grants g WHERE g.budget_key=b.key) OR allocated_bytes>limit_bytes",
    )
    if bad != "0":
        raise BackupError("capacity ledger mismatch; recovery requires review")
    budget = sql(
        pg,
        dsn,
        "SELECT pending_items=(SELECT count(*) FROM sync_items) FROM sync_queue_budget WHERE singleton",
    )
    if budget != "t":
        raise BackupError("synchronization queue accounting mismatch")


def receipt_index(pg, dsn, path):
    query = "SELECT json_build_object('hash',encode(b.hash,'hex'),'generation',encode(b.generation,'hex'),'size',b.size,'state',b.state,'volume',b.volume,'owner',encode(b.owner,'hex'),'quarantined',EXISTS(SELECT 1 FROM quarantined_blobs q WHERE q.hash=b.hash AND q.generation=b.generation),'reclaiming',EXISTS(SELECT 1 FROM blob_reclaims r WHERE r.hash=b.hash AND r.generation=b.generation)) FROM blob_intents b ORDER BY b.hash"
    with path.open("xb") as out:
        result = subprocess.run(
            pg_command(pg, "psql", dsn)
            + ["-X", "-qAt", "-v", "ON_ERROR_STOP=1", "-c", query],
            stdout=out,
            stderr=subprocess.PIPE,
            env=pg_environment(pg, dsn),
        )
        out.flush()
        os.fsync(out.fileno())
    if result.returncode:
        raise BackupError("cannot export receipt inventory")


def verify_receipts(root, config):
    counts = dict(healthy=0, writing=0, quarantined=0, reclaiming=0)
    with (root / "receipts.jsonl").open() as f:
        for line in f:
            item = json.loads(line)
            h = item["hash"]
            g = item["generation"]
            if not re.fullmatch("[0-9a-f]{64}", h) or not re.fullmatch(
                "[0-9a-f]{32}", g
            ):
                raise BackupError("invalid receipt encoding")
            if (
                item["volume"] != config["storage"]["volume"]
                or item["owner"] != config["admin_pubkey"]
            ):
                raise BackupError(
                    "backup profile requires one co-located storage owner/volume"
                )
            if item["reclaiming"]:
                counts["reclaiming"] += 1
                continue
            if item["quarantined"]:
                counts["quarantined"] += 1
                continue
            if item["state"] == "writing":
                counts["writing"] += 1
                continue
            path = child(
                root, "storage/objects/" + h[:2] + "/" + h[2:4] + "/" + h + "." + g
            )
            if (
                path.is_symlink()
                or not path.is_file()
                or path.stat().st_size != item["size"]
                or sha(path) != h
            ):
                raise BackupError("healthy receipt does not match backed-up original")
            counts["healthy"] += 1
    return counts


def signer_public(key):
    # Path-only verification and CLI help do not require the signing dependency.
    from coincurve import PrivateKey, PublicKeyXOnly

    key = absolute(key)
    if key.stat().st_mode & 0o077 or key.stat().st_size > 128:
        raise BackupError("signer key must be protected")
    try:
        return PublicKeyXOnly.from_secret(
            PrivateKey(bytes.fromhex(key.read_text().strip())).secret
        ).format().hex()
    except (ValueError, TypeError):
        raise BackupError("invalid signer key") from None


def backup(config_path, destination, pg, dsn, *, platform=False, copy_extra=None):
    config = read_json(config_path)
    if not config.get("storage") or not config.get("backend"):
        raise BackupError("co-located storage and backend required")
    out = Path(destination).absolute()
    roots = [
        absolute(config["storage"]["path"]),
        absolute(config["backend"]["data_dir"]),
    ]
    if config.get("group_keys_dir"):
        roots.append(absolute(config["group_keys_dir"]))
    if any(out.resolve().is_relative_to(p) for p in roots):
        raise BackupError("backup destination cannot be inside source data")
    key = absolute(config["service_key_file"])
    if key.stat().st_mode & 0o077 or key.stat().st_size > 128:
        raise BackupError("service key must be protected")
    pub = signer_public(key)
    with owners(config):
        out.mkdir(parents=True, exist_ok=False, mode=0o700)
        with snapshot(pg, dsn, out / "snapshot.log", platform=platform) as snap:
            identity = json.loads(
                sql(
                    pg,
                    dsn,
                    "SELECT json_build_object('service',encode(service_key,'hex'),'admin',encode(admin_key,'hex'),'restore_generation',restore_generation) FROM node_identity WHERE singleton",
                )
            )
            if (
                identity["service"] != pub
                or identity["admin"] != config["admin_pubkey"]
            ):
                raise BackupError("config/database identity mismatch")
            ledger_ok(pg, dsn)
            result = subprocess.run(
                pg_command(pg, "pg_dump", dsn)
                + [
                    "--format=custom",
                    "--no-owner",
                    "--no-acl",
                    "--snapshot",
                    snap,
                    "--file",
                    str(out / "database.dump"),
                ],
                capture_output=True,
                env=pg_environment(pg, dsn),
            )
            if result.returncode:
                raise BackupError("database dump failed")
            copy_tree(roots[0], out / "storage", exclude_root=("public",))
            copy_tree(roots[1], out / "backend")
            copy_file(key, out / "service.key")
            if config.get("group_keys_dir"):
                copy_tree(roots[2], out / "groups")
            save_json(out / "source-config.json", config, exclusive=True)
            receipt_index(pg, dsn, out / "receipts.jsonl")
            counts = verify_receipts(out, config)
            schema = int(sql(pg, dsn, "SELECT max(version) FROM schema_migrations"))
            extra = copy_extra(out, config) if copy_extra else {}
        entries = 0
        total = 0
        with (out / "files.jsonl").open("x") as manifest:
            for directory, dirs, files in os.walk(out):
                dirs.sort()
                files.sort()
                for name in files:
                    path = Path(directory) / name
                    if path.name == "files.jsonl" and path.parent == out:
                        continue
                    item = dict(
                        path=path.relative_to(out).as_posix(),
                        bytes=path.stat().st_size,
                        sha256=sha(path),
                    )
                    manifest.write(json.dumps(item, separators=(",", ":")) + "\n")
                    entries += 1
                    total += item["bytes"]
            manifest.flush()
            os.fsync(manifest.fileno())
        complete = dict(
            format=1,
            schema=schema,
            backend_sha256=sha(absolute(config["backend"]["binary"])),
            identity=identity,
            receipts=counts,
            files=entries,
            bytes=total,
            files_sha256=sha(out / "files.jsonl"),
            created_at=int(time.time()),
            contains_private_keys=True,
            derived_public_namespace_excluded=True,
            scope="offline co-located node; no deployment activation",
        )
        complete.update(extra)
        save_json(out / "BACKUP_COMPLETE.json", complete, exclusive=True)
        sync_dirs(out)
    return complete


def backup_entries(source):
    """Read bounded manifest entries and reject unsafe files before external work."""
    with (source / "files.jsonl").open("rb") as manifest:
        while True:
            raw = manifest.readline(16385)
            if not raw:
                break
            if len(raw) > 16384:
                raise BackupError("backup manifest entry budget")
            item = json.loads(raw)
            if not isinstance(item, dict) or set(item) != {"path", "bytes", "sha256"}:
                raise BackupError("invalid backup entry")
            if not isinstance(item["path"], str) or "\\" in item["path"]:
                raise BackupError("unsafe backup path")
            path = Path(item["path"])
            src = child(source, item["path"])
            if path.parts[:2] == ("storage", "public"):
                raise BackupError(
                    "backup includes derived public serving state; create a fresh backup with the current tool"
                )
            if (
                type(item["bytes"]) is not int
                or item["bytes"] < 0
                or not isinstance(item["sha256"], str)
                or not re.fullmatch("[0-9a-f]{64}", item["sha256"])
            ):
                raise BackupError("invalid backup entry")
            if any(
                (source / Path(*path.parts[:n])).is_symlink()
                for n in range(1, len(path.parts) + 1)
            ):
                raise BackupError("symlink in backup path")
            if not src.is_file() or src.stat().st_size != item["bytes"]:
                raise BackupError("backup file mismatch")
            yield item, src


def restore(
    source,
    destination,
    pg,
    dsn,
    source_offline=False,
    bind=None,
    base_url=None,
    backend_binary=None,
    *,
    validate_extra=None,
    restore_extra=None,
    extra_fence_sql="",
):
    if not source_offline:
        raise BackupError(
            "operator must explicitly confirm original service remains offline"
        )
    source = Path(source).resolve(strict=True)
    out = Path(destination).absolute()
    if out.resolve().is_relative_to(source):
        raise BackupError("restore destination cannot be inside backup")
    complete = read_json(source / "BACKUP_COMPLETE.json")
    if (
        complete.get("format") != 1
        or sha(source / "files.jsonl") != complete["files_sha256"]
    ):
        raise BackupError("backup manifest mismatch")
    if sum(1 for _ in backup_entries(source)) != complete["files"]:
        raise BackupError("backup file count mismatch")
    if validate_extra:
        validate_extra(source, complete)
    if sql(pg, dsn, "SELECT count(*) FROM pg_tables WHERE schemaname='public'") != "0":
        raise BackupError("restore requires an empty destination database")
    out.mkdir(parents=True, exist_ok=False, mode=0o700)
    count = 0
    for item, src in backup_entries(source):
        dst = child(out, item["path"])
        copy_file(src, dst)
        if sha(dst) != item["sha256"]:
            raise BackupError("backup file changed or failed verification")
        count += 1
    if count != complete["files"]:
        raise BackupError("backup file count mismatch")
    config = read_json(out / "source-config.json")
    binary = Path(backend_binary or config["backend"]["binary"])
    if (
        not binary.is_absolute()
        or not binary.is_file()
        or sha(binary) != complete["backend_sha256"]
    ):
        raise BackupError("the backup-matched installed strfry binary is required")
    counts = verify_receipts(out, config)
    result = subprocess.run(
        pg_command(pg, "pg_restore", dsn)
        + [
            "--exit-on-error",
            "--single-transaction",
            "--no-owner",
            "--no-acl",
            str(out / "database.dump"),
        ],
        capture_output=True,
        env=pg_environment(pg, dsn),
    )
    if result.returncode:
        (out / "restore-error.log").write_bytes(result.stderr)
        raise BackupError(
            "database restore failed; no server was started; inspect private restore-error.log"
        )
    ledger_ok(pg, dsn)
    sql(
        pg,
        dsn,
        "BEGIN;SELECT pg_advisory_xact_lock(697241113);UPDATE jobs SET state='queued',generation=generation+1,claim_token=NULL,lease_until=NULL,available_at=clock_timestamp(),updated_at=clock_timestamp() WHERE state IN ('queued','leased');DO $$ BEGIN IF to_regclass('public.staging_operations') IS NOT NULL THEN DELETE FROM staging_operations; END IF; IF to_regclass('public.public_export_health_work') IS NOT NULL THEN DELETE FROM public_export_health_work; END IF; IF to_regclass('public.public_export_queue') IS NOT NULL THEN DELETE FROM public_export_queue; DELETE FROM public_export_roles; END IF; IF to_regclass('public.public_export_holds') IS NOT NULL THEN DELETE FROM public_export_holds; END IF; IF to_regclass('public.sync_schedules') IS NOT NULL THEN UPDATE sync_schedules SET enabled=FALSE,revision=revision+1,last_error='paused_after_restore',updated_at=clock_timestamp(); END IF; END $$;UPDATE inventory_snapshot_slots SET session_id=NULL,peer=NULL,peer_epoch=NULL,prefix='',filter='',snapshot=''::bytea,expires_at='-infinity',rounds=0,protocol_bytes=0;UPDATE node_identity SET restore_generation=restore_generation+1,auth_not_before=floor(extract(epoch FROM clock_timestamp()))::bigint+1,restored_at=clock_timestamp() WHERE singleton;"
        + extra_fence_sql
        + "COMMIT",
    )
    config["service_key_file"] = str(out / "service.key")
    config["storage"]["path"] = str(out / "storage")
    config["backend"]["data_dir"] = str(out / "backend")
    config["backend"]["binary"] = str(binary)
    if config.get("group_keys_dir"):
        config["group_keys_dir"] = str(out / "groups")
    if bind:
        config["bind"] = bind
    if base_url:
        config["base_url"] = base_url
    pause_restored_roles(config)
    extra = restore_extra(out, config, complete) if restore_extra else {}
    save_json(out / "restored-config.json", config, exclusive=True)
    result = dict(
        restored=True,
        activated=False,
        config=str(out / "restored-config.json"),
        receipts=counts,
        source_backup_sha256=sha(source / "BACKUP_COMPLETE.json"),
        review_required=[
            "Keep original service offline before activating this identity.",
            "Review restored peer and account/group policy at the backup recovery point.",
            "Re-enable background policies and managed public serving explicitly after review.",
            "Public serving aliases/process state were excluded; rebuild exports from verified restored originals before enabling that role.",
        ],
    )
    result.update(extra)
    save_json(out / "RESTORE_COMPLETE.json", result, exclusive=True)
    sync_dirs(out)
    return result


def pause_restored_roles(config):
    """Disable autonomous work and public images until explicit operator review.

    Keep serving configuration so a later startup can still recover known local
    reader state safely. This is a config rewrite only; it never starts a process.
    """
    config["sync_worker_enabled"] = False
    config["peer_discovery_enabled"] = False
    config["library_reclamation"] = False
    config["publication_reclamation"] = False
    config["staging_reclamation"] = False
    config["storage_audit_interval_seconds"] = 0
    # Private network allowances must be reviewed for the restored host.
    if "discovery_private_overrides" in config:
        config["discovery_private_overrides"] = []
    if config.get("enrollment") is not None:
        config["enrollment"]["enabled"] = False
    if config.get("public_serving") is not None:
        config["public_serving"]["enabled"] = False


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--pg-bin", required=True)
    sub = p.add_subparsers(dest="command", required=True)
    a = sub.add_parser("backup")
    a.add_argument("config")
    a.add_argument("destination")
    a = sub.add_parser("restore")
    a.add_argument("backup")
    a.add_argument("destination")
    a.add_argument("--source-offline", action="store_true")
    a.add_argument("--bind")
    a.add_argument("--base-url")
    a.add_argument("--backend-binary")
    a = p.parse_args()
    dsn = os.environ.get("MANGA_DATABASE_URL")
    if not dsn:
        raise BackupError(
            "MANGA_DATABASE_URL required; it is never saved into the backup"
        )
    result = (
        backup(a.config, a.destination, a.pg_bin, dsn)
        if a.command == "backup"
        else restore(
            a.backup,
            a.destination,
            a.pg_bin,
            dsn,
            a.source_offline,
            a.bind,
            a.base_url,
            a.backend_binary,
        )
    )
    print(json.dumps(result))


if __name__ == "__main__":
    main()
