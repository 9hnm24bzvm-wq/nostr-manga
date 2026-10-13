#!/usr/bin/env python3
"""Package current source plus explicitly selected nonprivate evidence.

No installed dependencies, runtime state or implicit historical result inventory.
The archive is verified before atomic replacement; an earlier archive survives
any read/write/verification failure. Evidence selection requires operator review.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import re
import stat
import tempfile
import zipfile

DIRECTORIES = (
    "src",
    "tests",
    "scripts",
    "docs",
    "deploy",
    "browser-client",
    "server/src",
    "server/tests",
    "server/migrations",
    "server/examples",
    "bench/library-node",
)
FILES = (
    "README.md",
    "CONTEXT.md",
    "requirements.txt",
    "requirements-dev.txt",
    "pyproject.toml",
    "server/Cargo.toml",
    "server/Cargo.lock",
    "server/README.md",
    "server/REFACTOR.md",
    "server/TEST-SEAMS.md",
    "runtime/run-demo.sh",
    "runtime/node.example.json",
    "runtime/README.md",
    "runtime/BUNDLE-README.md",
)
EXCLUDED = {
    "vendor",
    "target",
    "node_modules",
    "__pycache__",
    ".git",
    ".runtime",
    "private.runtime",
    ".npm-cache",
    ".browser-test-home",
    "retired",
    "retired-tests",
    "history",
    "diagnostics",
    ".env",
    ".env.local",
    ".credentials",
}
EXCLUDED_FILES = {".DS_Store"}
PREFIX = "manga-protocol-lab/"
CANONICAL_MANIFEST_LIMIT = 4 * 1024 * 1024


def canonical_inputs(path):
    """A reviewed identity manifest is a coverage requirement, not a privacy bypass."""

    def unique(pairs):
        result = {}
        for key, value in pairs:
            if key in result:
                raise ValueError("duplicate canonical manifest key")
            result[key] = value
        return result

    with path.open("rb") as source:
        raw = source.read(CANONICAL_MANIFEST_LIMIT + 1)
    if len(raw) > CANONICAL_MANIFEST_LIMIT:
        raise ValueError("canonical manifest exceeds size limit")
    try:
        value = json.loads(raw, object_pairs_hook=unique)
    except (ValueError, UnicodeDecodeError, RecursionError) as error:
        raise ValueError("invalid canonical manifest") from error
    files = value.get("files") if isinstance(value, dict) else None
    if not isinstance(files, dict) or not files or len(files) > 4096:
        raise ValueError("canonical manifest requires a nonempty bounded files map")
    for name, digest in files.items():
        if not isinstance(name, str):
            raise ValueError("unsafe canonical input path")
        relative = PurePosixPath(name)
        historical = relative.parts[:2] == ("server", "diagnostics")
        public_asset = relative.parts[:2] == ("browser-client", "public")
        blocked = any(
            (
                part in EXCLUDED
                and not (historical and index == 1)
                and not (public_asset and index == 2 and part == "vendor")
            )
            or part in {"runtime", ".aws", ".codex", "credentials", "secrets"}
            or part.startswith((".runtime", ".env"))
            for index, part in enumerate(relative.parts)
        )
        if (
            not name
            or not relative.parts
            or relative.is_absolute()
            or ".." in relative.parts
            or relative.as_posix() != name
            or "\\" in name
            or any(ord(char) < 32 for char in name)
            or blocked
            or relative.suffix.lower()
            in {".key", ".pem", ".p12", ".sqlite", ".sqlite3", ".db"}
            or (historical and relative.suffix.lower() not in {".rs", ".md", ".txt"})
            or (
                public_asset
                and relative.suffix.lower() not in {".js", ".json", ".css", ".html"}
                and relative.name != "LICENSE"
            )
        ):
            raise ValueError("unsafe canonical input path: " + name)
        if not isinstance(digest, str) or not re.fullmatch(r"[0-9a-f]{64}", digest):
            raise ValueError("invalid canonical input hash: " + name)
    return files, hashlib.sha256(raw).hexdigest()


def sha_file(path):
    h = hashlib.sha256()
    with path.open("rb") as source:
        for block in iter(lambda: source.read(1024 * 1024), b""):
            h.update(block)
    return h.hexdigest()


def source_paths(root, evidence, required=None):
    found = set()

    def visit(path, reviewed_historical=False, reviewed_public=False):
        relative = path.relative_to(root)
        # Generated browser assets belong to the separate client/build artifact.
        if relative.parts[:2] == ("browser-client", "public") and not reviewed_public:
            return
        if any(
            (
                part in EXCLUDED
                and not (
                    reviewed_historical and index == 1 and part == "diagnostics"
                )
                and not (reviewed_public and index == 2 and part == "vendor")
            )
            or part.startswith(".runtime")
            for index, part in enumerate(relative.parts)
        ):
            return
        if path.is_symlink():
            raise ValueError("source symlink refused: " + str(relative))
        if path.is_dir():
            for child in sorted(path.iterdir()):
                visit(child)
        elif path.is_file():
            if (
                path.suffix in {".pyc", ".key", ".pem", ".sqlite", ".db"}
                or path.name in EXCLUDED_FILES
            ):
                return
            found.add(path)
        elif path.exists():
            raise ValueError("nonregular source entry: " + str(relative))

    for name in DIRECTORIES + FILES:
        visit(root / name)
    # Historical diagnostics remain excluded by default. Only exact reviewed
    # canonical source paths may be added; never scan this directory wholesale.
    for name in required or {}:
        if PurePosixPath(name).parts[:2] == ("server", "diagnostics"):
            visit(root / name, reviewed_historical=True)
        elif PurePosixPath(name).parts[:2] == ("browser-client", "public"):
            visit(root / name, reviewed_public=True)
    for name in evidence:
        path = Path(name)
        if (
            path.is_absolute()
            or ".." in path.parts
            or not path.parts
            or path.parts[0] != "results"
        ):
            raise ValueError("evidence must be an explicit results file")
        if any(part in EXCLUDED or part.startswith(".runtime") for part in path.parts):
            raise ValueError("private/runtime evidence path refused")
        source = root / path
        if any(
            (root / Path(*path.parts[:i])).is_symlink()
            for i in range(1, len(path.parts) + 1)
        ):
            raise ValueError("evidence symlink refused")
        if not source.is_file() or source.suffix not in {
            ".json",
            ".log",
            ".txt",
            ".exit",
            ".sha256",
        }:
            raise ValueError("explicit regular evidence file required")
        found.add(source)
    return sorted(found)


def package(root, destination, evidence, canonical_manifest=None):
    if root.is_symlink():
        raise ValueError("symlink source root refused")
    root = root.resolve(strict=True)
    if not root.is_dir():
        raise ValueError("source root must be a directory")
    destination = destination.absolute()
    if any(parent.is_symlink() for parent in destination.parents):
        raise ValueError("symlink output parent refused")
    if destination.is_symlink() or destination.is_dir():
        raise ValueError("output must be a regular archive path")
    required, canonical_sha = (
        canonical_inputs(canonical_manifest)
        if canonical_manifest is not None
        else ({}, None)
    )
    paths = source_paths(root, evidence, required)
    selected = {path.relative_to(root).as_posix() for path in paths}
    omitted = sorted(set(required) - selected)
    if omitted:
        raise ValueError("canonical input omitted: " + ", ".join(omitted))
    if not paths:
        raise ValueError("no current source files selected")
    if destination in paths:
        raise ValueError("output cannot be a selected input")
    destination.parent.mkdir(parents=True, exist_ok=True)
    fd, name = tempfile.mkstemp(
        prefix="." + destination.name + ".", suffix=".tmp", dir=destination.parent
    )
    temporary = Path(name)
    os.close(fd)
    manifest = {}
    try:
        with zipfile.ZipFile(temporary, "w", zipfile.ZIP_DEFLATED) as archive:
            for path in paths:
                relative = path.relative_to(root).as_posix()
                # Refuse a path swapped to a symlink between enumeration and copy.
                if any(
                    (root / Path(*Path(relative).parts[:i])).is_symlink()
                    for i in range(1, len(Path(relative).parts) + 1)
                ):
                    raise ValueError("source changed to symlink")
                fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW)
                with os.fdopen(fd, "rb") as source:
                    mode = os.fstat(source.fileno()).st_mode
                    if not stat.S_ISREG(mode):
                        raise ValueError("nonregular source file")
                    info = zipfile.ZipInfo(PREFIX + relative)
                    info.create_system = 3
                    info.compress_type = zipfile.ZIP_DEFLATED
                    info.external_attr = (
                        stat.S_IFREG | (0o755 if mode & 0o111 else 0o644)
                    ) << 16
                    h = hashlib.sha256()
                    with archive.open(info, "w") as out:
                        for block in iter(lambda: source.read(1024 * 1024), b""):
                            h.update(block)
                            out.write(block)
                    manifest[relative] = h.hexdigest()
            archive.writestr(
                PREFIX + "SHA256SUMS.json",
                json.dumps(manifest, sort_keys=True, indent=2),
            )
        with zipfile.ZipFile(temporary) as archive:
            if archive.testzip() is not None:
                raise ValueError("archive CRC verification failed")
            for relative, expected in manifest.items():
                h = hashlib.sha256()
                with archive.open(PREFIX + relative) as source:
                    for block in iter(lambda: source.read(1024 * 1024), b""):
                        h.update(block)
                if h.hexdigest() != expected:
                    raise ValueError("archive SHA verification failed")
            # Compare the copied bytes, not merely the source files or a manifest
            # generated from the selected subset. Never promote an incomplete or
            # changed accepted input set over a prior archive.
            mismatched = sorted(
                name
                for name, expected in required.items()
                if manifest.get(name) != expected
            )
            if mismatched:
                raise ValueError(
                    "canonical input hash mismatch: " + ", ".join(mismatched)
                )
        with temporary.open("rb") as verified:
            os.fsync(verified.fileno())
        result = dict(
            archive=str(destination),
            bytes=temporary.stat().st_size,
            files=len(manifest) + 1,
            sha256=sha_file(temporary),
            canonical_coverage=dict(
                checked=canonical_manifest is not None,
                manifest_sha256=canonical_sha,
                required=len(required),
                matched=len(required),
            ),
        )
        os.replace(temporary, destination)
        directory = os.open(destination.parent, os.O_RDONLY)
        try:
            os.fsync(directory)
        finally:
            os.close(directory)
        return result
    finally:
        temporary.unlink(missing_ok=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--root", type=Path, default=Path(__file__).resolve().parents[1]
    )
    parser.add_argument("--output", type=Path)
    parser.add_argument("--evidence", action="append", default=[])
    parser.add_argument(
        "--canonical-input-manifest",
        type=Path,
        help=(
            "Require every reviewed canonical files-map path and SHA256 in the "
            "archive; required for a full accepted-source coverage claim"
        ),
    )
    args = parser.parse_args()
    destination = args.output or args.root / "deliverables/manga-protocol-source.zip"
    print(
        json.dumps(
            package(args.root, destination, args.evidence, args.canonical_input_manifest)
        )
    )


if __name__ == "__main__":
    main()
