# Restored 4c499 checkpoint: minimal smoke and availability audit

2026-10-12, after authenticated archive verification and authorized promotion to the project directory.

## Immediately usable

The three restored Linux x86_64 executables have their exact checkpoint hashes, executable mode 0755, and resolve every required system shared library on this Debian 13 host. The optimized node is `4c499f724af4db1843f54c51e47746114f777de15b446979a3e8e8a0e7a3e23a`.

The node and key tool reach their source-defined CLI validation. Neither implements `--help`; expected usage failures exited 1 rather than being misreported as help success. The demo's offline `--fixture-public-keys` operation exited 0 and returned only public synthetic fixture identities. No application/database service was started and no key was generated.

Published browser build `e4f4862bd75c26bb` includes all 56 service-worker asset entries. Its restored public modules passed a small independent SHA-256 vector, literal CBOR encode/decode vector, and four-family public library graph roundtrip. The Nostr module loaded and rejected a malformed event. This is a Node module smoke, not rendered-browser, IndexedDB, signing, HTTP or full client-suite acceptance.

Local-library migration, MangaDex migration, serving-supervisor, packaging and API-index CLI help commands succeeded. These prove entrypoint availability only. PostgreSQL migration help is blocked by missing `cbor2`; PostgreSQL backup help is blocked by missing `coincurve`.

## Verified restoration limits

All 387 present canonical inputs match the accepted 392-input manifest, with zero mismatches after smoke. The five absent files are historical diagnostics:

- `server/diagnostics/managed-group-key-storage-check.rs.txt`
- `server/diagnostics/rebuild-oct8/README.md`
- `server/diagnostics/rebuild-oct8/library_ref_index.rs`
- `server/diagnostics/rebuild-oct8/reclamation_reference_index.rs`
- `server/diagnostics/rebuild-oct8/retention_index.rs`

This smoke does not treat the restored tree as byte-complete. The separate packaging audit covers their dependency/recovery implications. All 11 evidence files referenced by the recovered final acceptance match their recorded hashes. The historical 02:00 UTC full-gate result is preserved evidence, not a newly rerun gate.

The checkpoint explicitly excluded external dependencies/caches, actual runtime databases and keys, large raw/private runtime archives, and then-unaccepted fast PostgreSQL sampler/supervisor helpers and launch wiring. Their absence does not mean their code or prior runtime data can be reconstructed from this checkpoint. Existing older `pg_diagnostics.py` and its tests are present. Do not silently substitute them for the missing newer sampler.

## Exact environment gaps before broader execution

- No project `vendor/`, browser `node_modules/`, Rust build cache or test executables. No `cargo`, `rustc`, `rustup`, PostgreSQL executables, `strfry` or `nginx` in PATH.
- Source bootstrap pins Rust 1.90.0 with rustfmt/Clippy; strfry revision `4cd3cf64850caf47dda46c2a2abbbf3525a64d10`; nginx 1.30.5; PostgreSQL 18.6. These are recorded project requirements, not newly fetched or installed dependencies.
- Python: Pillow 12.3.0 matches. `aiohttp` is 3.14.4, differing from required 3.13.5. `cbor2` 5.6.5, `coincurve` 21.0.0 and development `black` 25.1.0 are absent.
- Browser rebuild/model tests need locked npm dependencies: noble curves/hashes 2.0.1, fake-indexeddb 6.2.4, jsdom 27.0.1 and prettier 3.6.2. The published assets include their own browser runtime modules but not the complete test/build dependency tree.
- Python 3.12.14, Node 24.19.0, npm 11.9.0, git, make, GCC/G++ and pkg-config are available. cmake, bison, flex and m4 are absent from PATH. Development metadata for OpenSSL 3.5.7, zlib 1.3.1 and PCRE2 10.46 is present.

The concrete next prerequisite is a separately approved, pinned project-local dependency bootstrap, followed by a fresh bounded runtime/full-gate plan. No download, bootstrap, service start, full gate or mixed load was attempted here. Resource observations are recorded only for planning; capacity is not reserved.

Machine-readable commands, exact exit codes, package inventory, current hashes, versions and logs are in `smoke-and-availability.json` and adjacent logs. All project source inputs remain unchanged by this smoke.
