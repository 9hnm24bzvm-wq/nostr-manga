"""Private offline full-publisher recovery. Never activates a process.

Use trusted backups only. Keys and user data must never enter public checkpoint,
source-control or CI artifacts. Connection/mail credentials remain external.
"""
from __future__ import annotations

import argparse
import contextlib
import copy
import fcntl
import json
import os
from pathlib import Path
import re
import stat
import subprocess
import tempfile
from urllib.parse import urlsplit

from src import postgres_backup as node
from src.operator_json import read_json, save_json

BackupError = node.BackupError
PROFILE = "offline-full-publisher-v1"
FENCE_SQL = """
SELECT pg_advisory_xact_lock(697241114);
UPDATE platform.sessions SET revoked_at=COALESCE(revoked_at,clock_timestamp());
UPDATE platform.api_clients SET revoked_at=COALESCE(revoked_at,clock_timestamp()),
 version=version+1,updated_at=clock_timestamp();
UPDATE platform.auth_flows SET used_at=COALESCE(used_at,clock_timestamp());
UPDATE platform.auth_email_tokens SET used_at=COALESCE(used_at,clock_timestamp()),
 delivery_state=CASE WHEN delivery_state='pending' THEN 'failed' ELSE delivery_state END;
"""
HEX = re.compile(r"[0-9a-f]{64}")


def configured(path, *, exists=True):
    path = Path(path)
    if not path.is_absolute() or ".." in path.parts:
        raise BackupError("explicit absolute normalized configured paths required")
    for candidate in [path, *path.parents]:
        if candidate.is_symlink():
            raise BackupError("symlink in configured path")
    if exists:
        return path.resolve(strict=True)
    if path.exists() or not path.parent.is_dir():
        raise BackupError("new destination with an existing parent required")
    return path


def credential_free(value):
    """Fail closed on accidentally embedded credentials instead of archiving them."""
    forbidden = {"dsn", "database_url", "database_dsn", "password", "api_key",
                 "bearer_token", "access_token", "secret", "smtp_password"}
    if isinstance(value, dict):
        for key, child in value.items():
            if key.lower() in forbidden:
                raise BackupError("credentials must remain in external environment files")
            credential_free(child)
    elif isinstance(value, list):
        for child in value:
            credential_free(child)
    elif isinstance(value, str) and "://" in value:
        parsed = urlsplit(value)
        if parsed.username or parsed.password:
            raise BackupError("credential-bearing configuration URL rejected")
    elif isinstance(value, str) and re.search(r"(?i)\b(?:password|passfile)\s*=", value):
        raise BackupError("credential-bearing connection configuration rejected")


def effective_node(platform):
    credential_free(platform)
    publication = platform.get("protocol_publication")
    if platform.get("protocol_read_only") or not isinstance(publication, dict):
        raise BackupError("full publisher configuration required")
    original = copy.deepcopy(publication.get("native_node"))
    if not isinstance(original, dict) or not original.get("backend"):
        raise BackupError("supervised co-located original node/backend required")
    if publication.get("backend") is not None:
        raise BackupError("retired separate publication backend is unsupported")
    storage = publication.get("storage")
    if not isinstance(storage, dict):
        raise BackupError("original publication storage required")
    # The listener receives the already-owned UploadStore. A null field in its
    # serialized NodeConfig is intentional, not missing original storage.
    if original.get("storage") not in (None, storage):
        raise BackupError("embedded original storage differs from injected storage")
    original["storage"] = copy.deepcopy(storage)
    if original.get("group_keys_dir") != publication.get("group_keys_dir"):
        raise BackupError("platform and original group-key roots must match")
    if original.get("admin_pubkey") != publication.get("storage_owner"):
        raise BackupError("original administrator/storage owner mismatch")
    for name in ("expected_publisher", "storage_owner", "storage_holder"):
        if not isinstance(publication.get(name), str) or not HEX.fullmatch(publication[name]):
            raise BackupError("explicit valid public signer identities required")
    mail = platform.get("auth_mail")
    if mail is not None:
        if not isinstance(mail, dict) or set(mail) - {"endpoint", "from", "bearer_token_env", "allow_loopback_test_sink"}:
            raise BackupError("only external-credential mail configuration is supported")
        parsed = urlsplit(mail.get("endpoint", ""))
        if parsed.query or parsed.fragment:
            raise BackupError("mail endpoint cannot contain credential-bearing parameters")
    return original


@contextlib.contextmanager
def native_owner(root):
    root = configured(root)
    path = root / ".platform-owner"
    if not path.is_file() or path.is_symlink():
        raise BackupError("initialized native platform owner lock required; start/stop the updated service first")
    fd = os.open(path, os.O_RDWR | os.O_NOFOLLOW)
    with os.fdopen(fd, "r+b") as owner:
        try:
            fcntl.flock(owner, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError:
            raise BackupError("source still has a live native platform owner") from None
        held, named = os.fstat(owner.fileno()), path.lstat()
        if not stat.S_ISREG(named.st_mode) or (held.st_dev, held.st_ino) != (named.st_dev, named.st_ino):
            raise BackupError("native platform owner lock changed")
        yield


def export_media(pg, dsn, path):
    query = "SELECT json_build_object('id',id,'key',storage_key,'digest',digest,'bytes',bytes) FROM platform.media_objects ORDER BY id"
    with path.open("xb") as out:
        result = subprocess.run(node.pg_command(pg, "psql", dsn) +
            ["-X", "-qAt", "-v", "ON_ERROR_STOP=1", "-c", query], stdout=out,
            stderr=subprocess.PIPE, env=node.pg_environment(pg, dsn))
        out.flush()
        os.fsync(out.fileno())
    if result.returncode:
        raise BackupError("cannot export native media inventory")


def verify_media(root):
    count = 0
    seen = set()
    with (root / "native-media.jsonl").open("rb") as source:
        while raw := source.readline(16385):
            if len(raw) > 16384:
                raise BackupError("native media inventory entry budget")
            item = json.loads(raw)
            if not isinstance(item, dict) or set(item) != {"id", "key", "digest", "bytes"}:
                raise BackupError("invalid native media inventory")
            if any(not isinstance(item[k], str) or not HEX.fullmatch(item[k]) for k in ("id", "key", "digest")) or type(item["bytes"]) is not int or item["bytes"] <= 0:
                raise BackupError("invalid native media identity/size")
            if item["key"] in seen:
                raise BackupError("duplicate native media inventory")
            seen.add(item["key"])
            relative = "native/objects/" + item["key"][:2] + "/" + item["key"]
            path = node.child(root, relative)
            if any((root / Path(*Path(relative).parts[:n])).is_symlink() for n in range(1, len(Path(relative).parts)+1)):
                raise BackupError("symlink in native media path")
            if not path.is_file() or path.is_symlink() or path.stat().st_size != item["bytes"] or node.sha(path) != item["digest"]:
                raise BackupError("referenced native original/derivative missing or hash/size mismatch")
            count += 1
    return count


def validate_signers(root, platform, original):
    publication = platform["protocol_publication"]
    if node.signer_public(root / "operator.key") != publication["expected_publisher"]:
        raise BackupError("operator signer identity mismatch")
    if node.signer_public(root / "service.key") != publication["storage_holder"]:
        raise BackupError("original service/holder signer identity mismatch")
    if original["admin_pubkey"] != publication["storage_owner"]:
        raise BackupError("original storage owner identity mismatch")


def backup(config_path, destination, pg, dsn):
    platform = read_json(configured(config_path))
    original = effective_node(platform)
    roots = [configured(platform["uploads_dir"]), configured(original["storage"]["path"]),
             configured(original["backend"]["data_dir"])]
    if any(a.is_relative_to(b) or b.is_relative_to(a) for i, a in enumerate(roots) for b in roots[i+1:]):
        raise BackupError("native/original/projection roots must be disjoint")
    out = configured(destination, exists=False)
    if any(out.is_relative_to(root) for root in roots):
        raise BackupError("backup destination cannot be inside source data")
    publication = platform["protocol_publication"]
    operator = configured(publication["operator_key_file"])
    if node.signer_public(operator) != publication["expected_publisher"]:
        raise BackupError("operator signer identity mismatch")
    service = configured(original["service_key_file"])
    configured(original["backend"]["binary"])
    if node.signer_public(service) != publication["storage_holder"]:
        raise BackupError("original service/holder signer identity mismatch")
    if publication.get("group_keys_dir"):
        configured(publication["group_keys_dir"])
    def extra(root, normalized):
        schema = int(node.sql(pg, dsn, "SELECT max(version) FROM platform.schema_migrations"))
        namespace = node.sql(pg, dsn, "SELECT namespace::text FROM platform.catalogue_namespace WHERE singleton")
        if not re.fullmatch(r"[0-9a-f-]{36}", namespace):
            raise BackupError("platform catalogue namespace required")
        node.copy_tree(roots[0], root / "native")
        node.copy_file(operator, root / "operator.key")
        save_json(root / "platform-source-config.json", platform, exclusive=True)
        export_media(pg, dsn, root / "native-media.jsonl")
        count = verify_media(root)
        validate_signers(root, platform, normalized)
        return {"profile": PROFILE, "platform_schema": schema, "catalogue_namespace": namespace,
                "native_media": count, "publisher": publication["expected_publisher"],
                "scope": "private offline full publisher; no deployment activation"}
    with native_owner(roots[0]), tempfile.TemporaryDirectory(prefix="publisher-backup-config-") as temporary:
        config = Path(temporary) / "node.json"
        save_json(config, original, exclusive=True)
        return node.backup(config, out, pg, dsn, platform=True, copy_extra=extra)


def validate_archive(source, complete):
    if complete.get("profile") != PROFILE:
        raise BackupError("full publisher backup completion marker required")
    listed = set()
    for item, path in node.backup_entries(source):
        name = item["path"]
        if name in listed or Path(name).as_posix() != name:
            raise BackupError("duplicate/noncanonical backup manifest path")
        listed.add(name)
        if node.sha(path) != item["sha256"]:
            raise BackupError("backup file hash mismatch")
    actual = set()
    for directory, dirs, files in os.walk(source, followlinks=False):
        for name in [*dirs, *files]:
            path = Path(directory) / name
            if path.is_symlink():
                raise BackupError("symlink in publisher backup")
        actual.update((Path(directory) / name).relative_to(source).as_posix() for name in files)
    if actual - {"files.jsonl", "BACKUP_COMPLETE.json"} != listed:
        raise BackupError("publisher backup manifest is incomplete")
    required = {"database.dump", "source-config.json", "platform-source-config.json", "operator.key",
                "service.key", "receipts.jsonl", "native-media.jsonl", "native/.platform-owner"}
    if not required <= listed:
        raise BackupError("publisher backup required files missing")
    platform = read_json(source / "platform-source-config.json")
    original = effective_node(platform)
    if original != read_json(source / "source-config.json"):
        raise BackupError("publisher/original config snapshot mismatch")
    if complete.get("publisher") != platform["protocol_publication"]["expected_publisher"]:
        raise BackupError("publisher completion identity mismatch")
    validate_signers(source, platform, original)
    if complete["identity"]["service"] != platform["protocol_publication"]["storage_holder"] or complete["identity"]["admin"] != original["admin_pubkey"]:
        raise BackupError("database completion identity mismatch")
    if verify_media(source) != complete.get("native_media"):
        raise BackupError("native media inventory count mismatch")


def empty_database(pg, dsn):
    # Tables alone are insufficient: functions, views, sequences, custom types
    # or a pre-created application schema make a target nonempty too.
    query = """SELECT (NOT EXISTS(SELECT 1 FROM pg_namespace WHERE nspname NOT IN ('public','pg_catalog','information_schema') AND nspname NOT LIKE 'pg_toast%' AND nspname NOT LIKE 'pg_temp%') AND NOT EXISTS(SELECT 1 FROM pg_class c JOIN pg_namespace n ON n.oid=c.relnamespace WHERE n.nspname='public') AND NOT EXISTS(SELECT 1 FROM pg_proc p JOIN pg_namespace n ON n.oid=p.pronamespace WHERE n.nspname='public') AND NOT EXISTS(SELECT 1 FROM pg_type t JOIN pg_namespace n ON n.oid=t.typnamespace WHERE n.nspname='public'))"""
    if node.sql(pg, dsn, query) != "t":
        raise BackupError("full publisher restore requires an empty destination database")


def restore(source, destination, pg, dsn, *, source_offline=False, backend_binary=None, ui_dir=None):
    if not source_offline:
        raise BackupError("operator must explicitly confirm all original/native source owners remain offline")
    source = configured(source)
    out = configured(destination, exists=False)
    for name in ("files.jsonl", "BACKUP_COMPLETE.json"):
        if (source / name).is_symlink() or not (source / name).is_file():
            raise BackupError("regular completed publisher backup controls required")
    def validate(root, complete):
        validate_archive(root, complete)
        empty_database(pg, dsn)
    def rewrite(root, original, complete):
        # Compare the actual restored DB, not only its ancillary manifest. A
        # wrong/missing inventory must never produce a usable restored config.
        with tempfile.TemporaryDirectory(prefix="publisher-restore-check-") as temporary:
            inventory = Path(temporary) / "media.jsonl"
            export_media(pg, dsn, inventory)
            if node.sha(inventory) != node.sha(root / "native-media.jsonl"):
                raise BackupError("restored database/native media inventory mismatch")
        namespace = node.sql(pg, dsn, "SELECT namespace::text FROM platform.catalogue_namespace WHERE singleton")
        if namespace != complete["catalogue_namespace"]:
            raise BackupError("restored database catalogue namespace mismatch")
        identity = json.loads(node.sql(pg, dsn, "SELECT json_build_object('service',encode(service_key,'hex'),'admin',encode(admin_key,'hex')) FROM node_identity WHERE singleton"))
        if any(identity[name] != complete["identity"][name] for name in ("service", "admin")):
            raise BackupError("restored database signing identity mismatch")
        if int(node.sql(pg, dsn, "SELECT max(version) FROM platform.schema_migrations")) != complete["platform_schema"]:
            raise BackupError("restored platform schema mismatch")
        platform = read_json(root / "platform-source-config.json")
        publication = platform["protocol_publication"]
        publication["operator_key_file"] = str(root / "operator.key")
        publication["storage"] = copy.deepcopy(original["storage"])
        publication["group_keys_dir"] = original.get("group_keys_dir")
        publication["native_node"] = copy.deepcopy(original)
        publication["native_node"]["storage"] = None  # injected owned store
        platform["uploads_dir"] = str(root / "native")
        platform["restore_review_required"] = True
        platform["registration_enabled"] = False
        if ui_dir:
            platform["ui_dir"] = str(configured(ui_dir))
        path = root / "restored-platform-config.json"
        save_json(path, platform, exclusive=True)
        return {"profile": PROFILE, "config": str(path),
                "original_config": str(root / "restored-config.json"),
                "restore_review_required": True, "platform_auth_invalidated": True,
                "native_media": complete["native_media"], "catalogue_namespace": complete["catalogue_namespace"],
                "review_required": [
                    "Keep every source owner and signing identity offline. This helper never starts a process.",
                    "Review accounts/passwords, signers, groups, peers, moderation and mail policy at the recovery point.",
                    "Reconcile exact preserved pending publication/deletion inputs; never invent replacement commands.",
                    "Use the platform config for integrated ownership. The original config is for offline review, not a second node.",
                    "After offline review explicitly set restore_review_required=false. Ordinary startup workers then resume; pending user publication still requires an authenticated exact retry.",
                    "Supply DB/mail credentials externally and the matched installed release/UI. Re-enable paused original roles deliberately."]}
    return node.restore(source, out, pg, dsn, source_offline=True, backend_binary=backend_binary,
                        validate_extra=validate, restore_extra=rewrite, extra_fence_sql=FENCE_SQL)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--pg-bin", required=True)
    commands = parser.add_subparsers(dest="command", required=True)
    action = commands.add_parser("backup")
    action.add_argument("config")
    action.add_argument("destination")
    action = commands.add_parser("restore")
    action.add_argument("backup")
    action.add_argument("destination")
    action.add_argument("--source-offline", action="store_true")
    action.add_argument("--backend-binary")
    action.add_argument("--ui-dir")
    args = parser.parse_args()
    dsn = os.environ.get("PLATFORM_DATABASE_URL") or os.environ.get("MANGA_DATABASE_URL")
    if not dsn:
        raise BackupError("PLATFORM_DATABASE_URL required; credentials are never archived")
    if args.command == "backup":
        result = backup(args.config, args.destination, args.pg_bin, dsn)
    else:
        result = restore(args.backup, args.destination, args.pg_bin, dsn,
                         source_offline=args.source_offline, backend_binary=args.backend_binary, ui_dir=args.ui_dir)
    print(json.dumps(result))


if __name__ == "__main__":
    main()
