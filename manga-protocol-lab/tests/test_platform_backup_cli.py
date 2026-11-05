"""Real filesystem and subprocess CLI safety, without a production DB or keys."""
import fcntl
import hashlib
import json
import os
from pathlib import Path
from operator_support import OperatorCase
from src.platform_backup import BackupError, effective_node, native_owner, verify_media, credential_free
from src.postgres_backup import copy_tree, restore_group_root


class PlatformBackupCLI(OperatorCase):
    def config(self):
        return {"uploads_dir":str(self.root / "native"), "protocol_publication": {
            "storage": {"path":str(self.root / "originals"), "volume":"fixture"},
            "operator_key_file":str(self.root / "operator.key"), "expected_publisher":"11"*32,
            "storage_owner":"22"*32, "storage_holder":"33"*32,
            "group_keys_dir":None, "backend":None,
            "native_node":{"storage":None, "group_keys_dir":None, "admin_pubkey":"22"*32,
                           "backend":{"data_dir":str(self.root / "projection")}}}}

    def archive(self, name="native/objects/file", raw=b"fixture", listed=None):
        source = self.root / "backup"
        path = source / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(raw)
        item = {"path":name, "bytes":len(raw), "sha256":hashlib.sha256(raw).hexdigest()}
        lines = json.dumps(listed or item)+"\n"
        (source / "files.jsonl").write_text(lines)
        self.write("backup/BACKUP_COMPLETE.json", {"format":1,"profile":"offline-full-publisher-v1",
            "files":1,"files_sha256":hashlib.sha256(lines.encode()).hexdigest()})
        return source

    def restore_cli(self, source, offline=True):
        pg = self.root / "pg"
        pg.mkdir(exist_ok=True)
        witness = self.root / "database-contacted"
        (pg / "psql").write_text('#!/bin/sh\nprintf called > "'+str(witness)+'"\nexit 1\n')
        (pg / "psql").chmod(0o700)
        out = self.root / "restored"
        result = self.cli("src.platform_backup", "--pg-bin", pg, "restore", source, out,
            *(["--source-offline"] if offline else []), check=False,
            env={"PLATFORM_DATABASE_URL":"host=127.0.0.1 dbname=synthetic user=fixture password='synthetic-only'"})
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(witness.exists(), result.stderr)
        self.assertFalse(out.exists(), result.stderr)
        return result

    def test_null_injected_storage_normalizes_without_mutating_source(self):
        platform = self.config()
        effective = effective_node(platform)
        self.assertEqual(effective["storage"], platform["protocol_publication"]["storage"])
        self.assertIsNone(platform["protocol_publication"]["native_node"]["storage"])
        platform["protocol_publication"]["native_node"]["storage"] = {"path":"/other"}
        with self.assertRaises(BackupError): effective_node(platform)

    def test_separate_group_roots_and_identity_mismatch_rejected(self):
        for field, value in (("group_keys_dir", "/other"), ("admin_pubkey", "44"*32)):
            platform = self.config()
            platform["protocol_publication"]["native_node"][field] = value
            with self.assertRaises(BackupError): effective_node(platform)

    def test_config_credentials_rejected_but_environment_names_allowed(self):
        credential_free({"auth_mail":{"bearer_token_env":"MAIL_CREDENTIAL"}})
        for value in ({"password":"fixture"}, {"note":"postgres://u:fixture@localhost/db"},
                      {"dsn":"host=localhost user=fixture"}, {"note":"password=fixture"}):
            with self.assertRaises(BackupError): credential_free(value)
        platform = self.config()
        platform["auth_mail"] = {"endpoint":"https://mail.fixture.invalid/send?token=fixture", "from":"fixture@example.invalid"}
        with self.assertRaises(BackupError): effective_node(platform)

    def test_native_live_owner_and_symlink_rejected(self):
        root = self.root / "native"
        root.mkdir()
        path = root / ".platform-owner"
        path.touch()
        with path.open("rb") as lock:
            fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
            with self.assertRaisesRegex(BackupError, "live native"):
                with native_owner(root): pass
        with native_owner(root): pass
        path.unlink()
        target = self.root / "other"
        target.touch()
        path.symlink_to(target)
        with self.assertRaises(BackupError):
            with native_owner(root): pass

    def test_enabled_empty_group_root_restored_privately_without_fabricating_keys(self):
        original = self.root / "original-groups"
        original.mkdir()
        source = self.root / "backup"
        source.mkdir()
        copy_tree(original, source / "groups")
        out = self.root / "restored"
        out.mkdir()
        config = {"group_keys_dir":str(original)}
        restore_group_root(source, out, config)
        self.assertTrue((out / "groups").is_dir())
        self.assertEqual(list((out / "groups").iterdir()), [])
        self.assertEqual((out / "groups").stat().st_mode & 0o777, 0o700)
        (out / "groups").rmdir()
        (source / "groups").rmdir()
        with self.assertRaises(BackupError): restore_group_root(source, out, config)
        (source / "groups").symlink_to(original)
        with self.assertRaises(BackupError): restore_group_root(source, out, config)
        self.assertFalse((out / "groups").exists())

    def test_native_referenced_original_derivative_hash_and_size(self):
        records = []
        for key, raw in (("11"*32,b"original"),("22"*32,b"derivative")):
            path = self.root / "native/objects" / key[:2] / key
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(raw)
            records.append({"id":key,"key":key,"digest":hashlib.sha256(raw).hexdigest(),"bytes":len(raw)})
        index = self.root / "native-media.jsonl"
        index.write_text("".join(json.dumps(item)+"\n" for item in records))
        self.assertEqual(verify_media(self.root), 2)
        path.write_bytes(b"tampered!!")
        with self.assertRaisesRegex(BackupError, "hash/size mismatch"): verify_media(self.root)
        path.unlink()
        with self.assertRaises(BackupError): verify_media(self.root)

    def test_no_consent_or_partial_marker_never_contacts_database(self):
        source = self.archive()
        self.assertIn("explicitly confirm", self.restore_cli(source, offline=False).stderr)
        (source / "BACKUP_COMPLETE.json").unlink()
        self.assertIn("completed publisher", self.restore_cli(source).stderr)

    def test_tamper_or_extra_file_never_contacts_database(self):
        source = self.archive()
        (source / "native/objects/file").write_bytes(b"changed")
        self.assertIn("hash mismatch", self.restore_cli(source).stderr)
        (source / "native/objects/file").write_bytes(b"fixture")
        (source / "rogue").write_bytes(b"unexpected")
        self.assertIn("incomplete", self.restore_cli(source).stderr)

    def test_traversal_symlink_and_duplicate_alias_never_contacts_database(self):
        source = self.archive()
        for name in ("../outside", "/etc/passwd", "native/../native/objects/file", "native//objects/file"):
            raw = json.dumps({"path":name,"bytes":7,"sha256":hashlib.sha256(b"fixture").hexdigest()})+"\n"
            (source / "files.jsonl").write_text(raw)
            self.write("backup/BACKUP_COMPLETE.json", {"format":1,"profile":"offline-full-publisher-v1", "files":1,"files_sha256":hashlib.sha256(raw.encode()).hexdigest()})
            self.restore_cli(source)
        (source / "files.jsonl").unlink()
        (source / "files.jsonl").symlink_to(source / "native/objects/file")
        self.assertIn("regular completed", self.restore_cli(source).stderr)

    def snapshot_tool(self, mode):
        pg = self.root / "snapshot-pg"
        pg.mkdir()
        psql = pg / "psql"
        psql.write_text("#!/usr/bin/env python3\nimport os,sys\nfrom pathlib import Path\n"
            + "Path(" + repr(str(self.root / "exporter.pid")) + ").write_text(str(os.getpid()))\n"
            + "sys.stdin.readline()\nprint('SNAP:00000001-1',flush=True)\nprint('CTRL:123:456:repeatable read',flush=True)\nprint('BACKUP_SNAPSHOT_READY',flush=True)\n"
            + "line=sys.stdin.readline()\n"
            + ("sys.exit(0)\n" if mode == "die" else
               "print('CHECK:123:" + ("789" if mode == "changed" else "456") + ":repeatable read',flush=True)\nprint('BACKUP_SNAPSHOT_STILL_HELD',flush=True)\nsys.stdin.readline()\n"))
        psql.chmod(0o700)
        return pg

    def test_snapshot_exporter_death_after_yield_cannot_complete(self):
        from src.postgres_backup import snapshot
        for mode in ("die", "changed"):
            pg = self.snapshot_tool(mode)
            completion = self.root / "completion.json"
            with self.assertRaisesRegex(BackupError, "snapshot"):
                with snapshot(pg, "host=127.0.0.1 dbname=synthetic user=fixture", self.root / "private-snapshot.log", platform=True):
                    pass
                completion.write_text("must not be reached")
            self.assertFalse(completion.exists())
            import shutil
            shutil.rmtree(pg)

    def test_snapshot_same_transaction_round_trip_allows_completion(self):
        from src.postgres_backup import snapshot
        pg = self.snapshot_tool("live")
        with snapshot(pg, "host=127.0.0.1 dbname=synthetic user=fixture", self.root / "private-snapshot.log", platform=True) as value:
            self.assertEqual(value, "00000001-1")
