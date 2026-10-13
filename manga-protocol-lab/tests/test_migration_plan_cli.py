import base64
import copy
import hashlib
import json
import os
import resource
import socket
import subprocess
import sys
from concurrent.futures import ThreadPoolExecutor
from PIL import Image
import cbor2
from operator_support import OperatorCase, ENV, ROOT

M = "00000000-0000-4000-8000-000000000001"
C = "00000000-0000-4000-8000-000000000002"
L = "00000000-0000-4000-8000-000000000003"
PUBLIC = "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798"


def bundle():
    return {
        "series": [
            {
                "id": M,
                "type": "manga",
                "attributes": {"title": {"en": "Synthetic import"}},
                "relationships": [],
            }
        ],
        "chapters": [
            {
                "chapter": {
                    "id": C,
                    "type": "chapter",
                    "attributes": {
                        "chapter": "001.5",
                        "volume": "0",
                        "title": None,
                        "pages": 2,
                        "translatedLanguage": "en",
                        "externalUrl": None,
                    },
                    "relationships": [{"id": M, "type": "manga"}],
                },
                "ordered_originals": ["b.png", "a.png"],
            }
        ],
        "library": {
            "read_markers": [C],
            "statuses": {M: "reading"},
            "lists": [
                {
                    "id": L,
                    "type": "custom_list",
                    "attributes": {"name": "Private fixture", "visibility": "private"},
                    "relationships": [{"type": "manga", "id": M}],
                }
            ],
        },
    }


class PlanCLI(OperatorCase):
    def setUp(self):
        super().setUp()
        for n, col in [("a.png", "red"), ("b.png", "blue")]:
            Image.new("RGB", (2, 2), col).save(self.root / n)
        self.key = self.root / "key"
        self.key.write_text("01".zfill(64))
        self.key.chmod(0o600)

    def plan(self, value=None, name="plan.json", **options):
        source = self.write("bundle.json", bundle() if value is None else value)
        out = self.root / name
        result = self.cli(
            "src.postgres_migration",
            "plan",
            source,
            out,
            "--originals",
            self.root,
            "--publisher",
            PUBLIC,
            "--timestamp",
            "1000",
            **options,
        )
        return out, json.loads(out.read_text()), result

    def test_plan_ids_exact_mapping_original_order_and_cover(self):
        b = bundle()
        b["covers"] = {M: "a.png"}
        _, p, _ = self.plan(b)
        _, again, _ = self.plan(b, "again.json")
        self.assertEqual(p, again)
        self.assertEqual(p["errors"], [])
        for entry in p["entries"]:
            event = entry["event"]
            raw = json.dumps(
                [
                    0,
                    event["pubkey"],
                    event["created_at"],
                    event["kind"],
                    event["tags"],
                    event["content"],
                ],
                separators=(",", ":"),
                ensure_ascii=False,
            ).encode()
            self.assertEqual(event["id"], hashlib.sha256(raw).hexdigest())
            self.assertNotIn("sig", event)
        self.assertEqual(p["mapping"]["series"][M], p["entries"][0]["event"]["id"])
        self.assertEqual(p["mapping"]["chapters"][C], p["entries"][1]["event"]["id"])
        series = cbor2.loads(base64.b64decode(p["entries"][0]["objects"][-1]["data"]))
        self.assertEqual(
            series["cover"]["sha256"],
            hashlib.sha256((self.root / "a.png").read_bytes()).digest(),
        )
        chapter = cbor2.loads(base64.b64decode(p["entries"][1]["objects"][-1]["data"]))
        self.assertEqual((chapter["chapter"], chapter["volume"]), ("001.5", "0"))
        self.assertNotIn("title", chapter)
        self.assertEqual(
            [x["path"] for x in p["entries"][1]["objects"][:-1]], ["b.png", "a.png"]
        )

    def test_selected_public_library_schema_and_private_exclusion(self):
        _, p, _ = self.plan()
        library = p["library"]
        self.assertEqual(library["private_lists_excluded"], 1)
        chunks = [
            cbor2.loads(base64.b64decode(x["data"]))
            for x in library["objects"]
            if x["type"] == "chunk"
        ]
        self.assertEqual({x["family"] for x in chunks}, {"chapters", "series"})
        chapter = next(x for x in chunks if x["family"] == "chapters")
        self.assertEqual(
            chapter["entries"],
            [[bytes.fromhex(p["mapping"]["chapters"][C]), True, None, 1000]],
        )

    def test_invalid_imports_remain_review_errors(self):
        for kind in ["duplicate", "missing_mapping", "hotlink_cover", "orphan_cover"]:
            b = bundle()
            if kind == "duplicate":
                b["series"] *= 2
            if kind == "missing_mapping":
                b["chapters"][0]["chapter"]["relationships"][0]["id"] = (
                    "f" * 8 + "-ffff-ffff-ffff-" + "f" * 12
                )
            if kind == "hotlink_cover":
                b["covers"] = {M: "https://example.invalid/cover.png"}
            if kind == "orphan_cover":
                b["covers"] = {"11111111-1111-4111-8111-111111111111": "a.png"}
            with self.subTest(kind=kind):
                self.assertTrue(self.plan(b, kind + ".json")[1]["errors"])

    def test_public_list_position_survives_canonical_record_order(self):
        b = bundle()
        b["library"]["lists"][0]["attributes"]["visibility"] = "public"
        _, p, _ = self.plan(b)
        chunks = [
            cbor2.loads(base64.b64decode(x["data"]))
            for x in p["library"]["objects"]
            if x["type"] == "chunk"
        ]
        self.assertEqual(
            next(x for x in chunks if x["family"] == "members")["entries"],
            [
                [
                    bytes.fromhex(L.replace("-", "")),
                    bytes.fromhex(p["mapping"]["series"][M]),
                    0,
                    1000,
                ]
            ],
        )

    def test_apply_preflights_do_not_connect(self):
        for case in ["consent", "tampering", "changed_original", "key_permissions"]:
            path, plan, _ = self.plan(name=case + ".json")
            if case == "tampering":
                plan["entries"][0]["event"]["created_at"] += 1
                path.write_text(json.dumps(plan))
            if case == "changed_original":
                Image.new("RGB", (2, 2), "green").save(self.root / "a.png")
            if case == "key_permissions":
                self.key.chmod(0o644)
            with socket.socket() as witness:
                witness.bind(("127.0.0.1", 0))
                witness.listen()
                witness.settimeout(0.1)
                args = ["--accept-review"] if case != "consent" else []
                result = self.cli(
                    "src.postgres_migration",
                    "apply",
                    path,
                    "--originals",
                    self.root,
                    "--origin",
                    f"http://127.0.0.1:{witness.getsockname()[1]}",
                    "--service-key",
                    PUBLIC,
                    "--key-file",
                    self.key,
                    "--journal",
                    self.root / "journal",
                    *args,
                    check=False,
                )
                self.assertNotEqual(result.returncode, 0)
                with self.assertRaises(TimeoutError):
                    witness.accept()
            self.key.chmod(0o600)
            self.assertFalse((self.root / "journal").exists())

    def test_private_exclusive_plan_and_concurrent_publication(self):
        path, p, _ = self.plan(umask=0)
        self.assertEqual(path.stat().st_mode & 0o777, 0o600)
        before = path.read_bytes()
        self.plan(name="other.json")
        result = self.cli(
            "src.postgres_migration",
            "plan",
            self.root / "bundle.json",
            path,
            "--originals",
            self.root,
            "--publisher",
            PUBLIC,
            check=False,
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(path.read_bytes(), before)
        out = self.root / "race.json"
        args = [
            sys.executable,
            "-m",
            "src.postgres_migration",
            "plan",
            str(self.root / "bundle.json"),
            str(out),
            "--originals",
            str(self.root),
            "--publisher",
            PUBLIC,
            "--timestamp",
            "1000",
        ]
        with ThreadPoolExecutor(2) as executor:
            results = list(
                executor.map(
                    lambda _: subprocess.run(
                        args, cwd=ROOT, env=ENV, capture_output=True
                    ),
                    range(2),
                )
            )
        self.assertEqual(sorted(x.returncode == 0 for x in results), [False, True])
        self.assertEqual(json.loads(out.read_text()), p)
        self.assertFalse(list(self.root.glob("*.tmp")))

    def test_duplicate_json_is_rejected_without_output(self):
        source = self.root / "duplicate.json"
        source.write_text('{"series":[],"series":[]}')
        out = self.root / "out"
        result = self.cli(
            "src.postgres_migration",
            "plan",
            source,
            out,
            "--originals",
            self.root,
            "--publisher",
            PUBLIC,
            check=False,
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("duplicate JSON key", result.stderr)
        self.assertFalse(out.exists())

    def test_interrupted_output_leaves_no_complete_plan(self):
        source = self.write("bundle.json", bundle())
        out = self.root / "interrupted.json"

        def limited():
            resource.setrlimit(resource.RLIMIT_FSIZE, (256, 256))

        result = self.cli(
            "src.postgres_migration",
            "plan",
            source,
            out,
            "--originals",
            self.root,
            "--publisher",
            PUBLIC,
            check=False,
            preexec_fn=limited,
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(out.exists())
        self.assertFalse(list(self.root.glob("*.tmp")))
