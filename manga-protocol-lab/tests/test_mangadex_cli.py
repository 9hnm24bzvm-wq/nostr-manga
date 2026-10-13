import json
import stat
from operator_support import OperatorCase


class MigrationCLI(OperatorCase):
    def test_rich_series_artifacts_are_deterministic_and_metadata_changes_identity(
        self,
    ):
        import hashlib
        import cbor2
        from test_migration_plan_cli import bundle

        value = bundle()["series"][0]
        value["attributes"].update(
            {
                "title": {"en": "Synthetic series", "ja": "試験"},
                "altTitles": [{"en": "Alternative"}],
                "originalLanguage": "ja",
                "status": "ongoing",
                "tags": [{"attributes": {"name": {"en": "Action"}}}],
            }
        )
        outputs = []
        for name, wrapped in [
            ("direct", value),
            ("wrapped", {"result": "ok", "data": value}),
        ]:
            out = self.root / name
            self.cli(
                "src.mangadex_migration",
                "series",
                self.write(name + ".json", wrapped),
                out,
            )
            outputs.append((out / "manifest.cbor").read_bytes())
        self.assertEqual(outputs[0], outputs[1])
        decoded = cbor2.loads(outputs[0])
        self.assertEqual(decoded["titles"], {"en": "Synthetic series", "ja": "試験"})
        self.assertEqual(decoded["categories"], ["Action"])
        self.assertEqual(decoded["altTitles"], [{"en": "Alternative"}])
        value["attributes"]["title"]["en"] = "Changed"
        out = self.root / "changed"
        self.cli(
            "src.mangadex_migration", "series", self.write("changed.json", value), out
        )
        changed = (out / "manifest.cbor").read_bytes()
        self.assertNotEqual(
            hashlib.sha256(outputs[0]).digest(), hashlib.sha256(changed).digest()
        )
        self.assertNotIn("previous", cbor2.loads(changed))

    def test_unknown_reading_status_and_invalid_identity_maps_refuse_review_output(
        self,
    ):
        from test_migration_plan_cli import M

        for name, value, mapping in [
            ("status", {"statuses": {M: "invented"}}, {"series": {M: "ab" * 32}}),
            ("title_key", {}, {"series": {"Some manga": "ab" * 32}}),
            ("short_hash", {}, {"series": {M: "abc"}}),
        ]:
            out = self.root / name
            result = self.cli(
                "src.mangadex_migration",
                "library",
                self.write(name + ".json", value),
                out,
                "--mapping",
                self.write(name + "-map.json", mapping),
                check=False,
            )
            self.assertNotEqual(result.returncode, 0, name)
            self.assertFalse(out.exists(), name)

    def test_private_library_packet_under_permissive_umask(self):
        source = self.write(
            "input.json", {"read_markers": [], "statuses": {}, "lists": []}
        )
        out = self.root / "packet"
        self.cli("src.mangadex_migration", "library", source, out, umask=0)
        self.assertEqual(stat.S_IMODE(out.stat().st_mode), 0o700)
        self.assertEqual(stat.S_IMODE((out / "review.json").stat().st_mode), 0o600)
        review = json.loads((out / "review.json").read_text())
        self.assertTrue(review["unsigned_preview_only"])
        self.assertTrue(review["requires_publication_consent"])

    def test_series_and_chapter_artifacts_preserve_labels_and_exact_originals(self):
        import hashlib
        from PIL import Image
        from src.cbor_profile import decode
        from test_migration_plan_cli import bundle, M

        b = bundle()
        source = self.write("series.json", b["series"][0])
        out = self.root / "series"
        self.cli("src.mangadex_migration", "series", source, out, umask=0)
        self.assertEqual((out / "manifest.cbor").stat().st_mode & 0o777, 0o600)
        manifest = decode((out / "manifest.cbor").read_bytes())
        self.assertEqual(manifest["titles"], {"en": "Synthetic import"})
        self.assertNotIn(M.encode(), (out / "manifest.cbor").read_bytes())
        for name, color in [("a.png", "red"), ("b.png", "blue")]:
            Image.new("RGB", (2, 3), color).save(self.root / name)
        source = self.write("chapter.json", b["chapters"][0])
        mapping = self.write("mapping.json", {"series": {M: "ab" * 32}})
        out = self.root / "chapter"
        self.cli(
            "src.mangadex_migration",
            "chapter",
            source,
            out,
            "--mapping",
            mapping,
            "--originals",
            self.root,
        )
        chapter = decode((out / "manifest.cbor").read_bytes())
        self.assertEqual((chapter["chapter"], chapter["volume"]), ("001.5", "0"))
        self.assertEqual(
            [x["sha256"] for x in chapter["pages"]],
            [
                hashlib.sha256((self.root / n).read_bytes()).digest()
                for n in ["b.png", "a.png"]
            ],
        )
        self.assertNotIn("title", chapter)

    def test_bad_originals_and_labels_fail_without_packet(self):
        from PIL import Image
        from test_migration_plan_cli import bundle, M

        originals = self.root / "originals"
        originals.mkdir()
        Image.new("RGB", (2, 3), "red").save(originals / "a.png")
        (originals / "bad.png").write_bytes(b"not an image")
        outside = self.root / "outside.png"
        outside.write_bytes((originals / "a.png").read_bytes())
        (originals / "link.png").symlink_to(outside)
        mapping = self.write("mapping.json", {"series": {M: "ab" * 32}})
        control = bundle()["chapters"][0]
        control["ordered_originals"] = ["a.png", "a.png"]
        self.cli(
            "src.mangadex_migration",
            "chapter",
            self.write("control.json", control),
            self.root / "control",
            "--mapping",
            mapping,
            "--originals",
            originals,
        )
        for label, names, updates in [
            ("count", ["a.png"], {}),
            ("traversal", ["../outside.png", "a.png"], {}),
            ("symlink", ["link.png", "a.png"], {}),
            ("decode", ["bad.png", "a.png"], {}),
            ("number", ["a.png", "a.png"], {"chapter": 1.5}),
            (
                "external",
                ["a.png", "a.png"],
                {"externalUrl": "https://example.invalid/x"},
            ),
        ]:
            b = bundle()["chapters"][0]
            b["ordered_originals"] = names
            b["chapter"]["attributes"].update(updates)
            source = self.write("chapter.json", b)
            out = self.root / label
            result = self.cli(
                "src.mangadex_migration",
                "chapter",
                source,
                out,
                "--mapping",
                mapping,
                "--originals",
                originals,
                check=False,
            )
            self.assertNotEqual(result.returncode, 0, label)
            self.assertFalse(out.exists())
            if label in ("traversal", "symlink"):
                self.assertRegex(
                    result.stderr,
                    "unsafe original path|original escapes selected directory",
                )

    def test_library_review_preserves_privacy_and_unmapped_markers(self):
        from test_local_library_migration import source, MAP, CHAPTER

        value = source()
        value["lists"][0]["attributes"]["visibility"] = "private"
        value["read_markers"].append("ffffffff-ffff-ffff-ffff-ffffffffffff")
        src = self.write("library.json", value)
        mapping = self.write("mapping.json", MAP)
        out = self.root / "packet"
        self.cli("src.mangadex_migration", "library", src, out, "--mapping", mapping)
        review = json.loads((out / "review.json").read_text())
        self.assertEqual(review["experimental_library_fields"]["lists"], [])
        self.assertEqual(len(review["private_lists_excluded_pending_review"]), 1)
        self.assertEqual(len(review["unmapped"]), 1)
        self.assertEqual(
            review["experimental_library_fields"]["completed"], ["bb" * 32]
        )

    def test_duplicate_input_and_existing_output_are_refused(self):
        src = self.root / "bad.json"
        src.write_text('{"lists":[],"lists":[]}')
        out = self.root / "packet"
        result = self.cli("src.mangadex_migration", "library", src, out, check=False)
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(out.exists())
        src = self.write("okay.json", {})
        self.cli("src.mangadex_migration", "library", src, out)
        before = (out / "review.json").read_bytes()
        result = self.cli("src.mangadex_migration", "library", src, out, check=False)
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual((out / "review.json").read_bytes(), before)

    def test_credit_names_and_missing_attribution_remain_reviewable(self):
        from PIL import Image
        from src.cbor_profile import decode
        from test_migration_plan_cli import bundle, M

        Image.new("RGB", (2, 2), "red").save(self.root / "a.png")
        value = bundle()["chapters"][0]
        value["ordered_originals"] = ["a.png", "a.png"]
        value["chapter"]["attributes"]["chapter"] = None
        value["chapter"]["relationships"] += [
            {
                "id": "11111111-1111-4111-8111-111111111111",
                "type": "scanlation_group",
                "attributes": {"name": "Synthetic group"},
            },
            {"id": "22222222-2222-4222-8222-222222222222", "type": "scanlation_group"},
        ]
        source = self.write("chapter.json", value)
        mapping = self.write("map.json", {"series": {M: "ab" * 32}})
        out = self.root / "packet"
        self.cli(
            "src.mangadex_migration",
            "chapter",
            source,
            out,
            "--mapping",
            mapping,
            "--originals",
            self.root,
        )
        manifest = decode((out / "manifest.cbor").read_bytes())
        review = json.loads((out / "review.json").read_text())
        self.assertEqual(
            manifest["credits"], [{"role": "scanlation", "name": "Synthetic group"}]
        )
        self.assertNotIn("chapter", manifest)
        self.assertEqual(
            review["unresolved_credit_relationships"],
            [
                {
                    "type": "scanlation_group",
                    "source_id": "22222222-2222-4222-8222-222222222222",
                }
            ],
        )

    def test_large_library_review_does_not_truncate_or_invent_reads(self):
        import uuid
        from test_local_library_migration import SERIES

        ids = [str(uuid.UUID(int=i + 1)) for i in range(16000)]
        mapping = {
            "series": {SERIES: "aa" * 32},
            "chapters": {k: f"{i:064x}" for i, k in enumerate(ids)},
        }
        source = self.write(
            "library.json", {"read_markers": ids, "statuses": {SERIES: "completed"}}
        )
        maps = self.write("map.json", mapping)
        out = self.root / "packet"
        self.cli("src.mangadex_migration", "library", source, out, "--mapping", maps)
        review = json.loads((out / "review.json").read_text())
        self.assertEqual(len(review["experimental_library_fields"]["completed"]), 16000)
        self.assertTrue(review["exceeds_default_event_budget"])
        source = self.write("empty.json", {"statuses": {SERIES: "completed"}})
        out = self.root / "status-only"
        self.cli("src.mangadex_migration", "library", source, out, "--mapping", maps)
        self.assertEqual(
            json.loads((out / "review.json").read_text())[
                "experimental_library_fields"
            ]["completed"],
            [],
        )
