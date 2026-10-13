#!/usr/bin/env python3
"""Verify every attachment file against its immutable source fingerprint."""
import hashlib
import json
from pathlib import Path
import sys

root = Path(__file__).resolve().parents[1]
manifest = root / "test-evidence/frontend/source-parity.json"
report = json.loads(manifest.read_text())
failures = []
for item in report["files"]:
    path = root / "frontend" / item["path"]
    if not path.is_file():
        failures.append({"path": item["path"], "error": "missing"})
        continue
    actual = hashlib.sha256(path.read_bytes()).hexdigest()
    if actual != item["source_sha256"]:
        failures.append({"path": item["path"], "error": "hash mismatch", "actual_sha256": actual})
summary = {
    "archive_sha256": report["archive_sha256"],
    "files_checked": len(report["files"]),
    "all_attachment_files_identical": not failures,
    "failures": failures,
    "visual_parity": "not established by source hash comparison",
}
print(json.dumps(summary, indent=2))
sys.exit(1 if failures else 0)
