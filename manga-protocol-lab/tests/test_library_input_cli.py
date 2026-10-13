"""Shared migration boundary exercised only through operator/browser entry points."""

import json
import socket
import subprocess
from operator_support import OperatorCase, ROOT
from test_migration_plan_cli import PUBLIC


M = "00000000-0000-4000-8000-000000000001"
M2 = "00000000-0000-4000-8000-000000000002"
C1 = "00000000-0000-4000-8000-000000000011"
C2 = "00000000-0000-4000-8000-000000000012"
C3 = "00000000-0000-4000-8000-000000000013"
L = "00000000-0000-4000-8000-000000000020"
MAPPING = {
    "series": {M: "aa" * 32, M2: "bb" * 32},
    "chapters": {C1: "11" * 32, C2: "22" * 32, C3: "33" * 32},
}


class LibraryInputCLI(OperatorCase):
    def adapter(self, kind, value, name):
        source = self.write(name + "-source.json", value)
        mapping = self.write(name + "-mapping.json", MAPPING)
        output = self.root / name
        if kind == "local":
            result = self.cli(
                "src.local_library_migration",
                source,
                output,
                "--mapping",
                mapping,
                "--timestamp",
                "123",
                check=False,
            )
        elif kind == "preview":
            result = self.cli(
                "src.mangadex_migration",
                "library",
                source,
                output,
                "--mapping",
                mapping,
                check=False,
            )
        else:
            source = self.write(name + "-bundle.json", {"library": value})
            result = self.cli(
                "src.postgres_migration",
                "plan",
                source,
                output,
                "--mapping",
                mapping,
                "--originals",
                self.root,
                "--publisher",
                PUBLIC,
                "--timestamp",
                "123",
                check=False,
            )
        return result, output

    def assert_rejected(self, kind, value, name, message):
        result, output = self.adapter(kind, value, name)
        if kind != "plan":
            self.assertNotEqual(result.returncode, 0, result.stdout)
            self.assertIn(message, result.stderr)
            self.assertFalse(
                output.exists(), "rejected input produced an import artifact"
            )
            return
        # A plan with explicit errors is review output, never an applicable library.
        self.assertEqual(result.returncode, 0, result.stderr)
        plan = json.loads(output.read_text())
        self.assertIsNone(plan["library"])
        self.assertEqual(plan["errors"], [{"kind": "library", "error": message}])
        key = self.root / "synthetic.key"
        key.write_text("01".zfill(64))
        key.chmod(0o600)
        journal = self.root / (name + "-journal.json")
        with socket.socket() as witness:
            witness.bind(("127.0.0.1", 0))
            witness.listen()
            witness.settimeout(0.1)
            applied = self.cli(
                "src.postgres_migration",
                "apply",
                output,
                "--originals",
                self.root,
                "--origin",
                f"http://127.0.0.1:{witness.getsockname()[1]}",
                "--service-key",
                PUBLIC,
                "--key-file",
                key,
                "--journal",
                journal,
                "--accept-review",
                "--publish-public-library",
                check=False,
            )
            self.assertNotEqual(applied.returncode, 0)
            self.assertIn("unresolved plan errors", applied.stderr)
            with self.assertRaises(TimeoutError):
                witness.accept()
        self.assertFalse(journal.exists())

    def test_unsupported_fields_are_review_errors_before_outputs_or_publication(self):
        for kind in ["local", "preview", "plan"]:
            for extra in [
                {"follows": [M]},
                {"read_marker": [C1]},
                {"z_extra": 0, "a_extra": 0},
            ]:
                with self.subTest(kind=kind, fields=list(extra)):
                    self.assert_rejected(
                        kind,
                        {"read_markers": [], "statuses": {}, "lists": [], **extra},
                        kind + "-" + next(iter(extra)),
                        "unsupported library fields: " + ", ".join(sorted(extra)),
                    )

    def test_non_object_export_is_an_explicit_review_error(self):
        for kind in ["local", "preview", "plan"]:
            with self.subTest(kind=kind):
                self.assert_rejected(
                    kind, [], kind + "-array", "library export object required"
                )

    def test_supported_sparse_wrapped_export_round_trips_without_invented_reads(self):
        value = {
            "read_markers": {"result": "ok", "data": {M: [C3, C1, C3]}},
            "statuses": {"result": "ok", "statuses": {M: "completed", M2: "on_hold"}},
            "lists": [
                {
                    "result": "ok",
                    "data": {
                        "id": L,
                        "type": "custom_list",
                        "attributes": {
                            "name": "Sparse imports",
                            "visibility": "public",
                        },
                        "relationships": [
                            {"type": "manga", "id": M2},
                            {"type": "manga", "id": M},
                        ],
                    },
                }
            ],
        }
        results = {}
        for kind in ["local", "preview", "plan"]:
            result, output = self.adapter(kind, value, kind + "-supported")
            self.assertEqual(result.returncode, 0, result.stderr)
            results[kind] = output
        expected = {
            "chapters": [
                {"id": key, "completed": True, "page": None, "updated_at": 123}
                for key in ["11" * 32, "33" * 32]
            ],
            "series": [
                {"id": "aa" * 32, "status": "completed", "updated_at": 123},
                {"id": "bb" * 32, "status": "on_hold", "updated_at": 123},
            ],
            "lists": [
                {"id": L.replace("-", ""), "name": "Sparse imports", "updated_at": 123}
            ],
            "members": [
                {
                    "list": L.replace("-", ""),
                    "series": "aa" * 32,
                    "position": 1,
                    "updated_at": 123,
                },
                {
                    "list": L.replace("-", ""),
                    "series": "bb" * 32,
                    "position": 0,
                    "updated_at": 123,
                },
            ],
        }
        self.assertEqual(json.loads(results["local"].read_text())["records"], expected)
        preview = json.loads((results["preview"] / "review.json").read_text())
        self.assertEqual(preview["unmapped"], [])
        self.assertEqual(
            preview["experimental_library_fields"],
            {
                "completed": ["11" * 32, "33" * 32],
                "series_status": {"aa" * 32: "completed", "bb" * 32: "on_hold"},
                "lists": [{"name": "Sparse imports", "series": ["bb" * 32, "aa" * 32]}],
            },
        )
        self.assertEqual(json.loads(results["plan"].read_text())["errors"], [])
        # Real operator artifacts cross the browser's import/export and public graph APIs.
        code = """import 'fake-indexeddb/auto';
import {readFileSync} from 'node:fs';
import {LocalLibrary} from './src/store.js';
import {importBackup, backupBlob} from './src/backup.js';
import {readGraph} from './src/codec.js';
Object.defineProperty(globalThis,'navigator',{value:{locks:{request:async (_n,fn)=>fn({})}}});
const first=await LocalLibrary.open('sparse-first'), second=await LocalLibrary.open('sparse-second');
try {
  await importBackup(first,new Blob([readFileSync(process.argv[1])]),0);
  await importBackup(second,await backupBlob(first),0);
  const plan=JSON.parse(readFileSync(process.argv[2],'utf8'));
  const items=plan.library.objects;
  const objects=new Map(items.map(item=>[item.hash,new Uint8Array(Buffer.from(item.data,'base64'))]));
  const root=items.find(item=>item.type==='root');
  console.log(JSON.stringify({local:(await second.snapshot()).records,wire:readGraph(objects.get(root.hash),objects)}));
} finally {first.close();second.close();}
"""
        result = subprocess.run(
            [
                "node",
                "--input-type=module",
                "-e",
                code,
                str(results["local"]),
                str(results["plan"]),
            ],
            cwd=ROOT / "browser-client",
            text=True,
            capture_output=True,
            timeout=15,
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        observed = json.loads(result.stdout)
        self.assertEqual(observed["local"], expected)
        # Graph order follows hash buckets, while local backup order follows record IDs.
        for family, records in observed["wire"].items():
            records.sort(
                key=lambda row: row["series"] if family == "members" else row["id"]
            )
        self.assertEqual(observed["wire"], expected)
