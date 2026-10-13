import json
from operator_support import OperatorCase

SERIES = "11111111-1111-1111-1111-111111111111"
CHAPTER = "22222222-2222-2222-2222-222222222222"
LIST = "33333333-3333-3333-3333-333333333333"
MAP = {"series": {SERIES: "aa" * 32}, "chapters": {CHAPTER: "bb" * 32}}


def source():
    return {
        "read_markers": [CHAPTER],
        "statuses": {SERIES: "reading"},
        "lists": [
            {
                "id": LIST,
                "type": "custom_list",
                "attributes": {"name": "My list", "visibility": "public"},
                "relationships": [{"type": "manga", "id": SERIES}],
            }
        ],
    }


class LocalMigrationCLI(OperatorCase):
    def test_exact_local_records_without_signing_or_publication(self):
        src = self.write("library.json", source())
        mapping = self.write("mapping.json", MAP)
        out = self.root / "backup.json"
        self.cli(
            "src.local_library_migration",
            src,
            out,
            "--mapping",
            mapping,
            "--timestamp",
            "123",
        )
        result = json.loads(out.read_text())
        self.assertEqual(result["format"], "manga-local-library")
        self.assertEqual(result["version"], 1)
        self.assertEqual(
            result["records"],
            {
                "chapters": [
                    {
                        "id": "bb" * 32,
                        "completed": True,
                        "page": None,
                        "updated_at": 123,
                    }
                ],
                "series": [{"id": "aa" * 32, "status": "reading", "updated_at": 123}],
                "lists": [
                    {"id": LIST.replace("-", ""), "name": "My list", "updated_at": 123}
                ],
                "members": [
                    {
                        "list": LIST.replace("-", ""),
                        "series": "aa" * 32,
                        "position": 0,
                        "updated_at": 123,
                    }
                ],
            },
        )
        self.assertEqual(out.stat().st_mode & 0o777, 0o600)

    def test_private_lists_require_explicit_local_opt_in(self):
        value = source()
        value["lists"][0]["attributes"]["visibility"] = "private"
        src = self.write("library.json", value)
        mapping = self.write("map.json", MAP)
        for include in [False, True]:
            out = self.root / ("included.json" if include else "excluded.json")
            self.cli(
                "src.local_library_migration",
                src,
                out,
                "--mapping",
                mapping,
                "--timestamp",
                "123",
                *(["--include-private-lists"] if include else []),
            )
            data = json.loads(out.read_text())
            self.assertEqual(len(data["records"]["lists"]), int(include))
            self.assertEqual(len(data["records"]["members"]), int(include))

    def test_unmapped_history_produces_review_only(self):
        value = source()
        value["read_markers"].append("ffffffff-ffff-ffff-ffff-ffffffffffff")
        src = self.write("library.json", value)
        mapping = self.write("map.json", MAP)
        out = self.root / "backup"
        review = self.root / "review"
        result = self.cli(
            "src.local_library_migration",
            src,
            out,
            "--mapping",
            mapping,
            "--review",
            review,
            check=False,
        )
        self.assertEqual(result.returncode, 2, result.stderr)
        self.assertFalse(out.exists())
        data = json.loads(review.read_text())
        self.assertEqual(
            data["unmapped"],
            [{"kind": "chapter", "source_id": "ffffffff-ffff-ffff-ffff-ffffffffffff"}],
        )
        self.assertTrue(data["local_only"])
        self.assertFalse(data["publication"])

    def test_invalid_names_timestamp_and_duplicate_lists_are_refused(self):
        mapping = self.write("map.json", MAP)
        for case in ["name", "duplicate", "timestamp"]:
            value = source()
            if case == "name":
                value["lists"][0]["attributes"]["name"] = "bad\nname"
            if case == "duplicate":
                value["lists"] *= 2
            src = self.write("library.json", value)
            out = self.root / case
            result = self.cli(
                "src.local_library_migration",
                src,
                out,
                "--mapping",
                mapping,
                "--timestamp",
                str(2**53 if case == "timestamp" else 1),
                check=False,
            )
            self.assertNotEqual(result.returncode, 0)
            self.assertFalse(out.exists())

    def test_duplicate_json_and_overwrite_are_refused(self):
        src = self.root / "library.json"
        src.write_text('{"lists":[],"lists":[]}')
        mapping = self.write("map.json", MAP)
        out = self.root / "backup"
        result = self.cli(
            "src.local_library_migration", src, out, "--mapping", mapping, check=False
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(out.exists())
        self.write("library.json", source())
        self.cli("src.local_library_migration", src, out, "--mapping", mapping)
        before = out.read_bytes()
        result = self.cli(
            "src.local_library_migration", src, out, "--mapping", mapping, check=False
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(out.read_bytes(), before)

    def test_backup_imports_through_actual_browser_library_operations(self):
        import subprocess
        from operator_support import ROOT

        src = self.write("library.json", source())
        mapping = self.write("map.json", MAP)
        out = self.root / "backup.json"
        self.cli(
            "src.local_library_migration",
            src,
            out,
            "--mapping",
            mapping,
            "--timestamp",
            "123",
        )
        code = """import 'fake-indexeddb/auto';
import {readFileSync} from 'node:fs';
import {LocalLibrary} from './src/store.js';
const input=JSON.parse(readFileSync(process.argv[1],'utf8'));
const store=await LocalLibrary.open('operator-migration-interop');
try { await store.replace(input,0); const restored=await store.snapshot(); console.log(JSON.stringify(restored.records)); }
finally { store.close(); }
"""
        result = subprocess.run(
            ["node", "--input-type=module", "-e", code, str(out)],
            cwd=ROOT / "browser-client",
            text=True,
            capture_output=True,
            timeout=15,
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(
            json.loads(result.stdout), json.loads(out.read_text())["records"]
        )

    def test_list_name_contract_matches_browser_without_rewriting_valid_unicode(self):
        import subprocess
        from operator_support import ROOT

        names = ["Café", "Cafe\u0301", "猫 🐈", "x\u200dy", "\u00a0", " ", "🐈" * 128]
        value = source()
        value["lists"] = [
            {
                "id": f"00000000-0000-0000-0000-{index:012x}",
                "type": "custom_list",
                "attributes": {"name": name, "visibility": "public"},
                "relationships": [],
            }
            for index, name in enumerate(names)
        ]
        src = self.write("names.json", value)
        mapping = self.write("map.json", MAP)
        backup = self.root / "names-backup.json"
        plan = self.root / "names-plan.json"
        self.cli("src.local_library_migration", src, backup, "--mapping", mapping)
        bundle = self.write("bundle.json", {"library": value})
        publisher = "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798"
        self.cli(
            "src.postgres_migration",
            "plan",
            bundle,
            plan,
            "--mapping",
            mapping,
            "--originals",
            self.root,
            "--publisher",
            publisher,
        )
        # Real CLI artifacts cross the same backup/codec interfaces the browser uses.
        # Compare both returned name lists with independent source literals, not
        # values decoded from the output under test.
        code = """import 'fake-indexeddb/auto';
import {readFileSync} from 'node:fs';
import {LocalLibrary} from './src/store.js';
import {importBackup} from './src/backup.js';
import {readGraph} from './src/codec.js';
const store=await LocalLibrary.open('operator-name-contract');
try {
  await importBackup(store,new Blob([readFileSync(process.argv[1])]),0);
  const local=(await store.list('lists')).map(row=>row.name);
  const plan=JSON.parse(readFileSync(process.argv[2],'utf8'));
  const objects=plan.library.objects;
  const root=objects.find(item=>item.type==='root');
  const bytes=new Map(objects.map(item=>[item.hash,new Uint8Array(Buffer.from(item.data,'base64'))]));
  const wire=readGraph(bytes.get(root.hash),bytes).lists.map(row=>row.name);
  console.log(JSON.stringify({local,wire}));
} finally { store.close(); }
"""
        result = subprocess.run(
            ["node", "--input-type=module", "-e", code, str(backup), str(plan)],
            cwd=ROOT / "browser-client",
            text=True,
            capture_output=True,
            timeout=15,
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(json.loads(result.stdout), {"local": names, "wire": names})

        for codepoint in [0x80, 0x85, 0x9F]:
            with self.subTest(control=f"U+{codepoint:04X}"):
                value = source()
                value["lists"][0]["attributes"]["name"] = (
                    "Before" + chr(codepoint) + "After"
                )
                src = self.write("invalid-name.json", value)
                backup = self.root / f"invalid-{codepoint}.json"
                result = self.cli(
                    "src.local_library_migration",
                    src,
                    backup,
                    "--mapping",
                    mapping,
                    check=False,
                )
                self.assertNotEqual(
                    result.returncode, 0, "unimportable local backup was accepted"
                )
                self.assertFalse(backup.exists())
                bundle = self.write("invalid-bundle.json", {"library": value})
                plan = self.root / f"invalid-plan-{codepoint}.json"
                self.cli(
                    "src.postgres_migration",
                    "plan",
                    bundle,
                    plan,
                    "--mapping",
                    mapping,
                    "--originals",
                    self.root,
                    "--publisher",
                    publisher,
                )
                reviewed = json.loads(plan.read_text())
                self.assertIsNone(reviewed["library"])
                self.assertEqual(
                    reviewed["errors"],
                    [{"kind": "library", "error": "invalid public list name"}],
                )
