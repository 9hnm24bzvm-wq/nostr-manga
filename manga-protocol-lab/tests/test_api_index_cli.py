import shutil
import subprocess
import sys
from operator_support import OperatorCase, ROOT


class ApiIndexCLI(OperatorCase):
    def setUp(self):
        super().setUp()
        (self.root / "scripts").mkdir()
        (self.root / "docs").mkdir()
        (self.root / "server/src").mkdir(parents=True)
        self.script = self.root / "scripts/api_index.py"
        shutil.copyfile(ROOT / "scripts/api_index.py", self.script)
        self.doc = self.root / "docs/API-INDEX.md"
        self.doc.write_text(
            "Intro\n<!-- generated-routes:start -->\nold\n<!-- generated-routes:end -->\nTail\n"
        )

    def run_cli(self, source=None, *args):
        if source is not None:
            (self.root / "server/src/node.rs").write_text(source)
        return subprocess.run(
            [sys.executable, str(self.script), *args],
            capture_output=True,
            text=True,
            timeout=5,
        )

    def test_cli_generates_methods_and_preserves_surrounding_document(self):
        result = self.run_cli(
            'r.route("/one", get(a::read).post(write))\n.route(\n"/two", delete(remove),\n)'
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        text = self.doc.read_text()
        self.assertIn("2 declared paths; 3 explicit path/method pairs.", text)
        for row in [
            "| GET | `/one` | `a::read` |",
            "| POST | `/one` | `write` |",
            "| DELETE | `/two` | `remove` |",
        ]:
            self.assertIn(row, text)
        self.assertTrue(text.startswith("Intro\n"))
        self.assertTrue(text.endswith("\nTail\n"))
        result = self.run_cli(None, "--check")
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_stale_check_does_not_modify_document(self):
        before = self.doc.read_bytes()
        result = self.run_cli('r.route("/one",get(read))', "--check")
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(self.doc.read_bytes(), before)

    def test_missing_local_contract_link_fails_check_without_rewriting(self):
        self.assertEqual(self.run_cli('r.route("/one",get(read))').returncode, 0)
        self.doc.write_text(self.doc.read_text() + "\n[Contract](missing.md#scope)\n")
        before = self.doc.read_bytes()
        result = self.run_cli(None, "--check")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("missing.md", result.stderr)
        self.assertEqual(self.doc.read_bytes(), before)
        (self.root / "docs/missing.md").write_text("# Scope\n")
        self.assertEqual(self.run_cli(None, "--check").returncode, 0)

    def test_actual_checkout_inventory_and_contract_links_are_current(self):
        result = subprocess.run(
            [sys.executable, str(ROOT / "scripts/api_index.py"), "--check"],
            capture_output=True,
            text=True,
            timeout=5,
        )
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_unhandled_and_duplicate_route_declarations_fail_without_overwriting(self):
        before = self.doc.read_bytes()
        for source in [
            "r.route(path,get(read))",
            'r.route("/one",custom(read))',
            'r.route("/one",get(read)).route("/one",post(write))',
            'r.route("/one",get(read)',
        ]:
            result = self.run_cli(source)
            self.assertNotEqual(result.returncode, 0)
            self.assertEqual(self.doc.read_bytes(), before)
