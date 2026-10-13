"""Subprocess-only harness for public operator interfaces."""

import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
ENV = dict(os.environ, PYTHONPATH=str(ROOT / "vendor/pydeps") + os.pathsep + str(ROOT))


class OperatorCase(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="manga-operator-")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)

    def write(self, name, value):
        path = self.root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(json.dumps(value), encoding="utf-8")
        return path

    def cli(self, module, *args, check=True, **options):
        env = dict(ENV, **options.pop("env", {}))
        result = subprocess.run(
            [sys.executable, "-m", module, *map(str, args)],
            cwd=ROOT,
            env=env,
            text=True,
            capture_output=True,
            timeout=30,
            **options
        )
        if check:
            self.assertEqual(result.returncode, 0, result.stderr)
        return result
