import hashlib
import json
import os
from operator_support import OperatorCase


class BackupCLI(OperatorCase):
    def backup_fixture(
        self, path="storage/public/.serving/state.json", raw=b"old process state"
    ):
        source = self.root / "backup"
        item = source / path
        item.parent.mkdir(parents=True)
        item.write_bytes(raw)
        line = (
            json.dumps(
                {
                    "path": path,
                    "bytes": len(raw),
                    "sha256": hashlib.sha256(raw).hexdigest(),
                }
            )
            + "\n"
        )
        manifest = source / "files.jsonl"
        manifest.write_text(line)
        self.write(
            "backup/BACKUP_COMPLETE.json",
            {
                "format": 1,
                "files": 1,
                "files_sha256": hashlib.sha256(line.encode()).hexdigest(),
            },
        )
        return source

    def pg_witness(self):
        pg = self.root / "pg"
        pg.mkdir()
        psql = pg / "psql"
        marker = self.root / "database-contacted"
        psql.write_text('#!/bin/sh\nprintf called > "' + str(marker) + '"\nexit 1\n')
        psql.chmod(0o700)
        return pg, marker

    def restore(self, source, *extra):
        pg, marker = self.pg_witness()
        out = self.root / "restored"
        result = self.cli(
            "src.postgres_backup",
            "--pg-bin",
            pg,
            "restore",
            source,
            out,
            *extra,
            check=False,
            env={"MANGA_DATABASE_URL": "host=127.0.0.1 dbname=synthetic user=fixture"}
        )
        return result, out, marker

    def test_legacy_serving_state_rejected_before_database_or_output(self):
        result, out, contacted = self.restore(self.backup_fixture(), "--source-offline")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("derived public serving state", result.stderr)
        self.assertFalse(contacted.exists())
        self.assertFalse(out.exists())

    def test_offline_consent_and_manifest_integrity_before_database(self):
        source = self.backup_fixture("storage/object")
        result, out, contacted = self.restore(source)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("explicitly confirm", result.stderr)
        self.assertFalse(out.exists())
        self.assertFalse(contacted.exists())
        (source / "files.jsonl").write_text("tampered")
        # Reuse public CLI with the already arranged external witness.
        result = self.cli(
            "src.postgres_backup",
            "--pg-bin",
            self.root / "pg",
            "restore",
            source,
            out,
            "--source-offline",
            check=False,
            env={"MANGA_DATABASE_URL": "host=127.0.0.1 dbname=synthetic user=fixture"},
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("manifest mismatch", result.stderr)
        self.assertFalse(contacted.exists())
        self.assertFalse(out.exists())

    def test_unsafe_manifest_entries_fail_before_database(self):
        source = self.backup_fixture("storage/object")
        pg, contacted = self.pg_witness()
        out = self.root / "restored"
        for kind in ["traversal", "absolute", "missing", "symlink", "oversize", "size"]:
            item = {"path": "storage/object", "bytes": 17, "sha256": "a" * 64}
            if kind == "traversal":
                item["path"] = "../outside"
            if kind == "absolute":
                item["path"] = "/etc/passwd"
            if kind == "missing":
                item["path"] = "storage/missing"
            if kind == "size":
                item["bytes"] = 100
            if kind == "symlink":
                (source / "storage/link").symlink_to(source / "storage/object")
                item["path"] = "storage/link"
            raw = (
                json.dumps(item) + (" " * 17000 if kind == "oversize" else "") + "\n"
            ).encode()
            (source / "files.jsonl").write_bytes(raw)
            self.write(
                "backup/BACKUP_COMPLETE.json",
                {
                    "format": 1,
                    "files": 1,
                    "files_sha256": hashlib.sha256(raw).hexdigest(),
                },
            )
            result = self.cli(
                "src.postgres_backup",
                "--pg-bin",
                pg,
                "restore",
                source,
                out,
                "--source-offline",
                check=False,
                env={
                    "MANGA_DATABASE_URL": "host=127.0.0.1 dbname=synthetic user=fixture"
                },
            )
            self.assertNotEqual(result.returncode, 0, kind)
            self.assertFalse(contacted.exists(), kind)
            self.assertFalse(out.exists(), kind)

    def test_live_owner_refuses_backup_without_database(self):
        import fcntl
        from test_migration_plan_cli import PUBLIC

        storage = self.root / "storage"
        backend = self.root / "backend"
        storage.mkdir()
        backend.mkdir()
        for path in [
            storage / ".volume-owner",
            backend / ".projection-owner",
            backend / ".command-lock",
        ]:
            path.touch()
        key = self.root / "service.key"
        key.write_text("01".zfill(64))
        key.chmod(0o600)
        config = self.write(
            "config.json",
            {
                "storage": {"path": str(storage)},
                "backend": {"data_dir": str(backend)},
                "service_key_file": str(key),
                "admin_pubkey": PUBLIC,
            },
        )
        pg, contacted = self.pg_witness()
        out = self.root / "backup"
        with (storage / ".volume-owner").open("rb") as lock:
            fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
            result = self.cli(
                "src.postgres_backup",
                "--pg-bin",
                pg,
                "backup",
                config,
                out,
                check=False,
                env={
                    "MANGA_DATABASE_URL": "host=127.0.0.1 dbname=synthetic user=fixture"
                },
            )
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("live storage/projection owner", result.stderr)
        self.assertFalse(out.exists())
        self.assertFalse(contacted.exists())

    def test_database_credentials_use_environment_not_argv(self):
        source = self.backup_fixture("storage/object")
        pg, marker = self.pg_witness()
        witness = pg / "psql"
        witness.write_text(
            "#!/usr/bin/env python3\nimport json,os,sys\nfrom pathlib import Path\nPath("
            + repr(str(marker))
            + ').write_text(json.dumps({"argv":sys.argv,"database":os.environ.get("PGDATABASE"),"password":os.environ.get("PGPASSWORD"),"dsn":os.environ.get("MANGA_DATABASE_URL")}))\nsys.exit(1)\n'
        )
        result = self.cli(
            "src.postgres_backup",
            "--pg-bin",
            pg,
            "restore",
            source,
            self.root / "restored",
            "--source-offline",
            check=False,
            env={
                "MANGA_DATABASE_URL": "host=127.0.0.1 dbname=synthetic user=fixture password='synthetic password'"
            },
        )
        self.assertNotEqual(result.returncode, 0)
        observed = json.loads(marker.read_text())
        self.assertNotIn("synthetic password", " ".join(observed["argv"]))
        self.assertEqual(observed["password"], "synthetic password")
        self.assertEqual(observed["database"], "synthetic")
        self.assertIsNone(observed["dsn"])
