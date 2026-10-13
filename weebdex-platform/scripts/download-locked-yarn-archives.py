#!/usr/bin/env python3
"""Fetch exact locked package bytes from official npm into an offline mirror.

Only complete archives matching unchanged yarn.lock integrity are admitted.
Partial files are resumable and are never consumed as offline package archives.
"""
import base64
from concurrent.futures import ThreadPoolExecutor
import hashlib
import json
from pathlib import Path
import re
import subprocess
import threading
from urllib.parse import urlparse

root = Path(__file__).resolve().parents[1]
mirror = root / ".cache/restore3/mirror"
partials = root / ".cache/restore3/partials"
partials.mkdir(parents=True, exist_ok=True)
evidence = root / "test-evidence/frontend/official-archive-downloads.jsonl"
lock_bytes = (root / "frontend/yarn.lock").read_bytes()
entries = {}
current = {}
for line in lock_bytes.decode().splitlines() + ["end:"]:
    if line and not line[0].isspace() and not line.startswith("#"):
        if current.get("resolved") and current.get("integrity"):
            url = current["resolved"].split("#", 1)[0]
            entries[url] = current["integrity"]
        current = {}
    match = re.match(r'^\s+(resolved|integrity)\s+(?:"([^"]+)"|(\S+))$', line)
    if match:
        current[match[1]] = match[2] or match[3]

def mirror_name(url):
    parsed = urlparse(url)
    if parsed.scheme != "https" or parsed.hostname != "registry.yarnpkg.com":
        raise RuntimeError("Unexpected preserved lockfile package destination")
    filename = Path(parsed.path).name
    scope = parsed.path.split("/")[1]
    return f"{scope}-{filename}" if scope.startswith("@") else filename

def matches(path, integrity):
    if not path.is_file() or path.is_symlink():
        return False
    data = path.read_bytes()
    return all(base64.b64encode(hashlib.new(token.split("-", 1)[0], data).digest()).decode()
               == token.split("-", 1)[1] for token in integrity.split())

pending = []
for original_url, integrity in sorted(entries.items()):
    name = mirror_name(original_url)
    target = mirror / name
    if matches(target, integrity):
        continue
    partial = partials / f"{name}.partial"
    if target.exists():
        # This is newly owned restore3 output. Invalid interrupted mirror files
        # are demoted to partials, never silently admitted by Yarn.
        target.replace(partial)
    pending.append((original_url, integrity, name))
print(json.dumps({"locked_archives": len(entries), "verified_mirror_archives": len(entries) - len(pending),
                  "pending": len(pending), "lockfile_sha256": hashlib.sha256(lock_bytes).hexdigest()}), flush=True)
log_lock = threading.Lock()

def record(value):
    with log_lock:
        with evidence.open("a") as log:
            log.write(json.dumps(value) + "\n")
            log.flush()

def download(item):
    original_url, integrity, name = item
    parsed = urlparse(original_url)
    official_url = "https://registry.npmjs.org" + parsed.path
    partial = partials / f"{name}.partial"
    result = subprocess.run([
        "curl", "--fail", "--silent", "--show-error", "--location",
        "--max-time", "6", "--continue-at", "-", "--output", str(partial), official_url,
    ], stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
    if matches(partial, integrity):
        destination = mirror / name
        partial.replace(destination)
        if not matches(destination, integrity):
            raise RuntimeError("Post-admission lockfile integrity changed")
        record({"source_lock_url": original_url, "download_url": official_url,
                "mirror_filename": name, "lock_integrity": integrity,
                "bytes": destination.stat().st_size, "status": "verified"})
        return True
    record({"source_lock_url": original_url, "download_url": official_url,
            "mirror_filename": name, "curl_exit": result.returncode,
            "partial_bytes": partial.stat().st_size if partial.exists() else 0,
            "status": "incomplete_not_admitted"})
    # Complete-looking invalid archives or unusable resumed ranges must be
    # retrieved afresh on the next bounded call. Timeouts retain resumable bytes.
    if result.returncode in (0, 22, 33) and partial.exists():
        partial.unlink()
    return False

# The external owned guard bounds the whole stage. Each curl also has its own
# six-second ceiling. Its exact descendant cleanup receipt governs retries.
with ThreadPoolExecutor(max_workers=32) as executor:
    results = list(executor.map(download, pending[:64]))
print(json.dumps({"batch_complete": True, "new_verified_archives": sum(results)}), flush=True)
