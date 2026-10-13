#!/usr/bin/env python3
"""Copy only stable archives verified against the preserved Yarn lockfile.

Never copy unverified metadata, unpacked node_modules, or mutable cache state.
Metadata may nominate a URL, but only the immutable lockfile authorizes its bytes.
"""
import base64
import hashlib
import json
from pathlib import Path
import re
import sys
from urllib.parse import urlparse

root = Path(__file__).resolve().parents[1]
old_cache = root / ".cache/restore2/yarn-packages/v6"
mirror = root / ".cache/restore3/mirror"
mirror.mkdir(parents=True, exist_ok=False)
lock = (root / "frontend/yarn.lock").read_text()
entries = {}
current = {}
for line in lock.splitlines() + ["end:"]:
    if line and not line[0].isspace() and not line.startswith("#"):
        if current.get("resolved") and current.get("integrity"):
            url = current["resolved"].split("#", 1)[0]
            if url in entries and entries[url] != current["integrity"]:
                raise RuntimeError(f"Conflicting lockfile integrity for {url}")
            entries[url] = current["integrity"]
        current = {}
    match = re.match(r'^\s+(resolved|integrity)\s+(?:"([^"]+)"|(\S+))$', line)
    if match:
        current[match[1]] = match[2] or match[3]

def stat_identity(path):
    stat = path.stat()
    return (stat.st_dev, stat.st_ino, stat.st_size, stat.st_mtime_ns, stat.st_ctime_ns)

def integrity_matches(data, integrity):
    tokens = integrity.split()
    for token in tokens:
        algorithm, encoded = token.split("-", 1)
        if algorithm not in {"sha512", "sha384", "sha256", "sha1"}:
            return False
        actual = base64.b64encode(hashlib.new(algorithm, data).digest()).decode()
        if actual != encoded:
            return False
    return bool(tokens)

copied, rejected = [], []
for metadata in sorted(old_cache.rglob(".yarn-metadata.json")):
    archive = metadata.parent / ".yarn-tarball.tgz"
    try:
        nominated = json.loads(metadata.read_text()).get("remote", {}).get("reference")
        if nominated not in entries or archive.is_symlink() or not archive.is_file():
            continue
        url = urlparse(nominated)
        if url.scheme != "https" or url.hostname != "registry.yarnpkg.com":
            continue
        before = stat_identity(archive)
        data = archive.read_bytes()
        after_first = stat_identity(archive)
        second = archive.read_bytes()
        after_second = stat_identity(archive)
        if before != after_first or before != after_second or data != second:
            rejected.append({"url": nominated, "reason": "archive changed during independent read"})
            continue
        if not integrity_matches(data, entries[nominated]):
            rejected.append({"url": nominated, "reason": "immutable lockfile integrity mismatch"})
            continue
        filename = Path(url.path).name
        scope = url.path.split("/")[1]
        if scope.startswith("@"):
            filename = f"{scope}-{filename}"
        destination = mirror / filename
        if destination.exists() and destination.read_bytes() != data:
            raise RuntimeError(f"Offline mirror filename collision: {filename}")
        destination.write_bytes(data)
        if not integrity_matches(destination.read_bytes(), entries[nominated]):
            raise RuntimeError(f"Copied archive verification failed: {filename}")
        copied.append({"url": nominated, "mirror_filename": filename,
                       "bytes": len(data), "lock_integrity": entries[nominated],
                       "sha256": hashlib.sha256(data).hexdigest()})
    except (OSError, ValueError, KeyError) as error:
        rejected.append({"archive": str(archive.relative_to(root)), "reason": type(error).__name__})
receipt = {"verified_archive_count": len(copied), "bytes_copied": sum(item["bytes"] for item in copied),
           "old_unpacked_modules_or_metadata_reused": False, "old_processes_touched": False,
           "old_execution_cleanup": "unknown", "archives": copied, "rejected": rejected}
(root / "test-evidence/frontend/restore3-salvage-receipt.json").write_text(json.dumps(receipt, indent=2) + "\n")
print(json.dumps({key: value for key, value in receipt.items() if key not in {"archives", "rejected"}}))
