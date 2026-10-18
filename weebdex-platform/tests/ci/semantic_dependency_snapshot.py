#!/usr/bin/env python3
"""Read-only exact integer byte/mode/mtime/symlink proof for actual strict caches."""
import gzip, hashlib, json, os, pathlib, sys
outer = pathlib.Path(sys.argv[1])

def canonical(value):
    return json.dumps(value, sort_keys=True, separators=(",", ":")).encode()

before_bytes = gzip.decompress((outer / "physical-dependencies-before.json.gz").read_bytes())
after_bytes = gzip.decompress((outer / "physical-dependencies-after.json.gz").read_bytes())
assert before_bytes == after_bytes, "Physical dependency bytes or metadata changed between snapshots"
before = json.loads(before_bytes)
discovery = json.loads((outer / "physical-dependency-root-discovery.json").read_text())
assert before and sorted(before) == sorted(discovery["roots"]), "Physical dependency snapshot root inventory is incomplete"
assert not discovery["uncoveredSymlinks"], "Physical dependency discovery contains uncovered symlinks"
summary = json.loads((outer / "physical-dependencies-before-summary.json").read_text())
assert summary == json.loads((outer / "physical-dependencies-after-summary.json").read_text()), "Physical snapshot summaries changed"
assert sorted(summary) == sorted(before), "Physical summary root identities are incomplete"
roots = [pathlib.Path(root) for root in before]
count = files_count = 0
for value, expected in before.items():
    root = pathlib.Path(value)
    assert root.is_absolute() and root.resolve() == root and root.is_dir(), "Physical cache root must be an existing real directory"
    assert expected, "Physical dependency snapshot entries must not be empty"
    actual = {}
    for parent, directories, files in os.walk(root, followlinks=False):
        for name in sorted(directories + files):
            entry = pathlib.Path(parent) / name
            stat = entry.lstat()
            record = {"mode": stat.st_mode & 0o7777, "mtimeNs": stat.st_mtime_ns}
            if entry.is_symlink():
                record.update(kind="symlink", target=os.readlink(entry))
                target = entry.resolve()
                assert any(target == candidate or candidate in target.parents for candidate in roots), "Physical dependency symlink escapes inventoried roots"
            elif entry.is_file():
                record.update(kind="file", bytes=stat.st_size, sha256=hashlib.sha256(entry.read_bytes()).hexdigest())
            elif entry.is_dir():
                record.update(kind="directory")
            else:
                raise AssertionError("Unexpected physical cache entry kind")
            actual[str(entry.relative_to(root))] = record
    assert actual == expected, "Current physical dependency bytes or metadata differ: " + value
    observed = {"entries": len(actual), "regularFiles": sum(row["kind"] == "file" for row in actual.values()), "canonicalManifestSha256": hashlib.sha256(canonical(actual)).hexdigest()}
    assert observed == summary[value], "Physical dependency summary differs from exact snapshot"
    count += len(actual)
    files_count += observed["regularFiles"]
print(json.dumps({"status": "passed", "roots": len(roots), "entries": count, "regularFiles": files_count, "exactNanoseconds": True, "byteModeMtimeSymlinkEquality": True}))
