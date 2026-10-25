#!/usr/bin/env python3
"""Package the complete verified static UI independently of dependency/build caches."""
import gzip
import hashlib
import io
import json
from pathlib import Path
import tarfile

ROOT = Path(__file__).resolve().parents[1]
UI = ROOT / "artifacts/frontend"
EVIDENCE = ROOT / "test-evidence/frontend"
OUTPUT = ROOT / "artifacts/frontend-static.tar.gz"
digest = lambda data: hashlib.sha256(data).hexdigest()
manifest = json.loads((EVIDENCE / "artifact-manifest.json").read_text())
expected = {item["path"]: item for item in manifest["files"]}
actual = {str(path.relative_to(UI)): path for path in UI.rglob("*") if path.is_file()}
if set(actual) != set(expected) or any(path.is_symlink() for path in UI.rglob("*")):
    raise SystemExit("UI output differs from complete regular-file artifact manifest")
payload = {}
for name, path in actual.items():
    if name.startswith("/") or ".." in Path(name).parts:
        raise SystemExit("Unsafe archive path")
    data = path.read_bytes()
    if len(data) != expected[name]["bytes"] or digest(data) != expected[name]["sha256"]:
        raise SystemExit("Artifact changed: " + name)
    payload["frontend/" + name] = data
for source, name in [
    (EVIDENCE / "integration-source-manifest.json", "frontend-exact-transform-manifest.json"),
    (ROOT / "docs/frontend-authority-and-resume.md", "frontend-authority-and-resume.md"),
    (ROOT / "docs/local-library-integration.md", "local-library-integration.md"),
    (ROOT / ".build/local-library-noble-2.0.1/node_modules/@noble/curves/LICENSE", "third-party/noble-curves-LICENSE"),
    (ROOT / ".build/local-library-noble-2.0.1/node_modules/@noble/hashes/LICENSE", "third-party/noble-hashes-LICENSE"),
    (ROOT / ".build/local-signing-base-2.0.0/node_modules/@scure/base/LICENSE", "third-party/scure-base-LICENSE"),
    (EVIDENCE / "artifact-manifest.json", "frontend-artifact-manifest.json"),
    (EVIDENCE / "bootstrap-csp.json", "frontend-bootstrap-csp.json"),
    (ROOT / "docs/frontend-preservation.md", "frontend-preservation.md"),
    (ROOT / "docs/frontend-overlays.md", "frontend-overlays.md"),
    (ROOT / "docs/announcements-reconstruction.md", "announcements-reconstruction.md"),
    (ROOT / "docs/frontend-query-and-privacy-policy.md", "frontend-query-and-privacy-policy.md"),
]:
    payload[name] = source.read_bytes()
temporary = OUTPUT.with_suffix(OUTPUT.suffix + ".new")
with temporary.open("wb") as destination:
    with gzip.GzipFile(filename="", mode="wb", fileobj=destination, mtime=0) as compressed:
        with tarfile.open(fileobj=compressed, mode="w", format=tarfile.GNU_FORMAT) as archive:
            for name, data in sorted(payload.items()):
                info = tarfile.TarInfo(name)
                info.size = len(data)
                info.mode = 0o644
                info.uid = info.gid = info.mtime = 0
                info.uname = info.gname = ""
                archive.addfile(info, io.BytesIO(data))
with tarfile.open(temporary, "r:gz") as archive:
    members = archive.getmembers()
    if len(members) != len(payload) or {item.name for item in members} != set(payload):
        raise SystemExit("Archive inventory differs")
    for member in members:
        if not member.isfile() or archive.extractfile(member).read() != payload[member.name]:
            raise SystemExit("Archive verification failed: " + member.name)
for name, path in actual.items():
    if digest(path.read_bytes()) != expected[name]["sha256"]:
        raise SystemExit("Artifact changed during packaging: " + name)
temporary.replace(OUTPUT)
receipt = {
    "archive": str(OUTPUT.relative_to(ROOT)), "sha256": digest(OUTPUT.read_bytes()),
    "bytes": OUTPUT.stat().st_size, "verified_regular_members": len(payload),
    "ui_files": len(actual), "independent_of_node_modules_and_build_workspace": True,
    "contains_runtime_databases_credentials_or_user_records": False,
    "verification": "Every archived regular member byte-compared and every UI input rehashed after packaging",
}
(EVIDENCE / "portable-package.json").write_text(json.dumps(receipt, indent=2) + "\n")
print(json.dumps(receipt))
