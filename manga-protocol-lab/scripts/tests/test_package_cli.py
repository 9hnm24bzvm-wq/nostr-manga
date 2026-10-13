"""Packaging is accepted through its executable interface and reopened archive."""

import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
import zipfile

SCRIPT = Path(__file__).resolve().parents[1] / "package.py"


class PackageCLI(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix="manga-package-")
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name) / "source"
        self.root.mkdir()
        self.out = Path(self.tmp.name) / "source.zip"

    def put(self, path, content=b"synthetic fixture\n", mode=0o644):
        p = self.root / path
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_bytes(content)
        p.chmod(mode)
        return p

    def cli(self, *extra):
        return subprocess.run(
            [
                sys.executable,
                str(SCRIPT),
                "--root",
                str(self.root),
                "--output",
                str(self.out),
                *map(str, extra),
            ],
            capture_output=True,
            text=True,
            timeout=20,
        )

    def test_current_allowlist_manifest_modes_and_explicit_evidence(self):
        for p in [
            "README.md",
            "src/tool.py",
            "server/src/lib.rs",
            "browser-client/src/store.js",
            "docs/rebuild/STATUS.md",
        ]:
            self.put(p)
        self.put("scripts/tool.sh", b"#!/bin/sh\nexit 0\n", 0o755)
        self.put("runtime/run-demo.sh", b"#!/bin/sh\n", 0o755)
        for p in [
            "vendor/secret",
            "src/__pycache__/old.pyc",
            "target/binary",
            ".runtime/private.key",
            "retired/old.py",
            "docs/history/raw.md",
            "browser-client/node_modules/secret",
            "browser-client/public/app.js",
            "results/raw.log",
            "runtime/service.key",
        ]:
            self.put(p, b"excluded")
        evidence = self.put("results/rebuild-check.json", b'{"passed":true}\n')
        result = self.cli("--evidence", "results/rebuild-check.json")
        self.assertEqual(result.returncode, 0, result.stderr)
        receipt = json.loads(result.stdout)
        self.assertEqual(
            receipt["sha256"], hashlib.sha256(self.out.read_bytes()).hexdigest()
        )
        with zipfile.ZipFile(self.out) as z:
            self.assertIsNone(z.testzip())
            prefix = "manga-protocol-lab/"
            manifest = json.loads(z.read(prefix + "SHA256SUMS.json"))
            self.assertEqual(
                set(manifest),
                {
                    "README.md",
                    "src/tool.py",
                    "server/src/lib.rs",
                    "browser-client/src/store.js",
                    "docs/rebuild/STATUS.md",
                    "scripts/tool.sh",
                    "runtime/run-demo.sh",
                    "results/rebuild-check.json",
                },
            )
            for name, digest in manifest.items():
                self.assertEqual(
                    hashlib.sha256(z.read(prefix + name)).hexdigest(), digest
                )
            self.assertEqual(
                (z.getinfo(prefix + "scripts/tool.sh").external_attr >> 16) & 0o777,
                0o755,
            )

    def test_source_symlinks_are_refused_without_replacing_prior_archive(self):
        self.put("README.md")
        outside = Path(self.tmp.name) / "private"
        outside.write_text("private fixture")
        for directory in [False, True]:
            link = self.root / "src"
            target = Path(self.tmp.name) / "outside"
            if directory:
                target.mkdir(exist_ok=True)
                (target / "leak.py").write_text("excluded")
            link.symlink_to(
                target if directory else outside, target_is_directory=directory
            )
            self.out.write_bytes(b"prior archive")
            result = self.cli()
            self.assertNotEqual(result.returncode, 0)
            self.assertEqual(self.out.read_bytes(), b"prior archive")
            self.assertFalse(list(self.out.parent.glob(".source.zip.*.tmp")))
            link.unlink()

    def test_input_failure_preserves_prior_archive_and_cleans_temporary(self):
        self.put("README.md")
        blocked = self.put("docs/blocked.md", mode=0)
        self.out.write_bytes(b"prior archive")
        try:
            result = self.cli()
            self.assertNotEqual(result.returncode, 0)
            self.assertEqual(self.out.read_bytes(), b"prior archive")
            self.assertFalse(list(self.out.parent.glob(".source.zip.*.tmp")))
        finally:
            blocked.chmod(0o600)

    def test_evidence_escape_and_output_symlink_are_refused(self):
        self.put("README.md")
        secret = Path(self.tmp.name) / "secret"
        secret.write_text("private fixture")
        result = self.cli("--evidence", "../secret")
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(self.out.exists())
        self.out.symlink_to(secret)
        result = self.cli()
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(secret.read_text(), "private fixture")

    def test_symlinked_output_parent_cannot_redirect_archive_replacement(self):
        self.put("README.md")
        outside = Path(self.tmp.name) / "outside"
        outside.mkdir()
        old = outside / "source.zip"
        old.write_bytes(b"prior external archive")
        link = Path(self.tmp.name) / "alias"
        link.symlink_to(outside, target_is_directory=True)
        self.out = link / "source.zip"
        result = self.cli()
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(old.read_bytes(), b"prior external archive")

    def test_explicit_evidence_cannot_include_private_runtime(self):
        self.put("README.md")
        self.put("results/.runtime/state.json", b'{"private":"fixture"}')
        result = self.cli("--evidence", "results/.runtime/state.json")
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(self.out.exists())

    def canonical(self, files):
        path = Path(self.tmp.name) / "canonical.json"
        path.write_text(json.dumps({"files": files}))
        return path

    def test_canonical_manifest_covers_required_historical_source(self):
        normal = self.put("server/src/lib.rs", b"public source\n")
        historical = self.put("server/diagnostics/rebuild-oct8/retention_index.rs", b"historical source\n")
        self.put("server/diagnostics/unreviewed.txt", b"not selected")
        files = {str(p.relative_to(self.root)): hashlib.sha256(p.read_bytes()).hexdigest()
                 for p in (normal, historical)}
        result = self.cli("--canonical-input-manifest", self.canonical(files))
        self.assertEqual(result.returncode, 0, result.stderr)
        coverage = json.loads(result.stdout)["canonical_coverage"]
        self.assertTrue(coverage["checked"])
        self.assertEqual((coverage["required"], coverage["matched"]), (2, 2))
        with zipfile.ZipFile(self.out) as archive:
            manifest = json.loads(archive.read("manga-protocol-lab/SHA256SUMS.json"))
            for name, digest in files.items():
                self.assertEqual(manifest[name], digest)
                self.assertEqual(hashlib.sha256(archive.read("manga-protocol-lab/" + name)).hexdigest(), digest)
            self.assertNotIn("server/diagnostics/unreviewed.txt", manifest)

    def test_missing_canonical_input_preserves_prior_archive(self):
        self.put("README.md")
        self.out.write_bytes(b"prior archive")
        result = self.cli("--canonical-input-manifest", self.canonical({"server/src/missing.rs": "0" * 64}))
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("canonical input omitted: server/src/missing.rs", result.stderr)
        self.assertEqual(self.out.read_bytes(), b"prior archive")
        self.assertFalse(list(self.out.parent.glob(".source.zip.*.tmp")))

    def test_canonical_manifest_covers_only_reviewed_generated_browser_assets(self):
        name = "browser-client/public/vendor/hashes/sha2.js"
        self.put(name, b"reviewed public module\n")
        self.put("browser-client/public/unreviewed.js", b"not selected")
        files = {name: hashlib.sha256(b"reviewed public module\n").hexdigest()}
        result = self.cli("--canonical-input-manifest", self.canonical(files))
        self.assertEqual(result.returncode, 0, result.stderr)
        with zipfile.ZipFile(self.out) as archive:
            manifest = json.loads(archive.read("manga-protocol-lab/SHA256SUMS.json"))
            self.assertEqual(manifest, files)
            self.assertEqual(archive.read("manga-protocol-lab/" + name), b"reviewed public module\n")

    def test_altered_canonical_input_preserves_prior_archive(self):
        self.put("server/src/lib.rs", b"altered source\n")
        self.out.write_bytes(b"prior archive")
        result = self.cli("--canonical-input-manifest", self.canonical({"server/src/lib.rs": hashlib.sha256(b"accepted source\n").hexdigest()}))
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("canonical input hash mismatch: server/src/lib.rs", result.stderr)
        self.assertEqual(self.out.read_bytes(), b"prior archive")
        self.assertFalse(list(self.out.parent.glob(".source.zip.*.tmp")))

    def test_canonical_manifest_cannot_authorize_private_or_excluded_paths(self):
        self.put("README.md")
        for name in ("results/.runtime/state.json", "runtime/node.json", "server/diagnostics/.credentials/token.txt",
                     "server/diagnostics/secret.key", "server/diagnostics/.env.production", "vendor/private.txt",
                     "browser-client/public/vendor/.env", "browser-client/public/private.key"):
            with self.subTest(name=name):
                self.put(name, b"private synthetic sentinel")
                self.out.write_bytes(b"prior archive")
                result = self.cli("--canonical-input-manifest", self.canonical({name: hashlib.sha256(b"private synthetic sentinel").hexdigest()}))
                self.assertNotEqual(result.returncode, 0)
                self.assertIn("unsafe canonical input path", result.stderr)
                self.assertEqual(self.out.read_bytes(), b"prior archive")
                self.assertFalse(list(self.out.parent.glob(".source.zip.*.tmp")))

    def test_canonical_manifest_rejects_escape_bad_hash_and_duplicate_paths(self):
        self.put("README.md")
        manifest = Path(self.tmp.name) / "canonical.json"
        for content in (json.dumps({"files": {"../outside": "0" * 64}}),
                        json.dumps({"files": {"/absolute": "0" * 64}}),
                        json.dumps({"files": {"server/./src/lib.rs": "0" * 64}}),
                        json.dumps({"files": {"server/src/lib.rs": "bad"}}),
                        '{"files":{"README.md":"' + "0" * 64 + '","README.md":"' + "0" * 64 + '"}}',
                        '{"files":{}}'):
            with self.subTest(content=content):
                manifest.write_text(content)
                result = self.cli("--canonical-input-manifest", manifest)
                self.assertNotEqual(result.returncode, 0)
                self.assertIn("canonical", result.stderr.lower())
                self.assertNotIn("unrecognized arguments", result.stderr)
                self.assertFalse(self.out.exists())
