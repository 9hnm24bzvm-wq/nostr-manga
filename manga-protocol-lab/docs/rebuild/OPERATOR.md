# Reconstructed operator tooling

Rebuilt from the recovered October 8 source. Old October 10 results are historical,
not acceptance of this tree. No release or production-capacity claim.

## Deep modules and CLI contracts

- `library_records.py` owns reviewed family normalization, exact record keys,
  ordering, list-name validation, duplicate rejection and the two-million-record
  bound. Public CBOR plans and local browser backups share this boundary.
- `operator_json.py` owns bounded duplicate-safe JSON input and private atomic
  artifact publication, including fsync, exclusive no-overwrite and failure cleanup.
- `python3 -m src.local_library_migration INPUT OUTPUT --mapping MAP` produces a
  private version1 browser backup directly from normalized records, without
  encoding and decoding public CBOR. No signing, key, account or network required.
  `--include-private-lists` explicitly includes those lists locally. Unmapped
  records produce exit2 and optional `--review FILE`, without a partial backup.
- MangaDex review packets use 0700 directories and 0600 files under any umask.
- Restore structurally validates the bounded manifest before contacting PostgreSQL
  or creating output, rejects historical derived serving state and repeats path,
  regular-file and size validation during copying; copied bytes are hash-checked.
- `python3 scripts/package.py --root ROOT --output SOURCE.zip` packages an explicit
  current-source allowlist. `--evidence results/FILE` adds only reviewed individual
  evidence files. Installed vendors/build caches, private runtime, keys, databases,
  retired source, historical raw results and node_modules are excluded. Generated
  browser public assets are a separate client/build artifact; source users rebuild
  them through the browser build CLI. Symlinks
  are refused; file modes, CRC and SHA checks precede atomic destination replacement.
  Explicit evidence selection still requires reviewing content for private data.

## Fresh evidence

Actual red→green slices:

- `results/rebuild-operator-private-{red,green}.log`: CLI packet privacy under umask000.
- `results/rebuild-backup-preflight-{red,green}.log`: unsafe serving-state archive
  rejected before any database adapter contact or output.
- `results/rebuild-local-migration-{red,green}.log`: new local CLI exact-record backup.
- `results/rebuild-package-{red,green}.log`: packager honors selected root/output and
  verified allowlisted contents instead of the obsolete implicit archive inventory.

Other CLI tests are migrated or strengthened regressions, not retroactively TDD.
They use observable CLI exit/artifacts, external sockets, filesystem faults,
interrupted writes, concurrent exclusive output and actual nginx process handles.
No private helper import, mock collaborator call count, private state file or SQL
acceptance oracle is used. Public codec tests retain independent vectors.

`results/rebuild-serving-cli.log` records actual nginx tests: prepare/activate/
suspend/stop, EOF handover, exclusive/wrong ownership, SIGKILL recovery, dead-master
refusal, slow unlinked-reader drain with HTTP ranges/404 and oversized pipe cleanup.
Retired helper/mock tests remain recoverable under `retired/rebuild-operator-tests`.
The older two supervisor experiments remain for provenance until parent archival;
namespace-state mutation and private reload-fault injection are not falsely claimed
replaced by a new public interface.

## Verification limits

These CLI checks do not certify the positive PostgreSQL restore/apply path. Real
PostgreSQL/strfry migration fixtures must separately validate successful restore,
paused policies, exact identity/event recovery, alias registration and replay.
No full-suite result, actual-browser pass, arbitrary syscall-stall/power-loss
resilience or traffic capacity is implied. Final counts belong to this rebuild's
fresh logs, not the previous missing workspace.

## Retired independent research models

An import scan found no application consumers of the SQLite/state/query/accounting
models or historical index/size/strfry experiments. Six source modules and their
three SQLite model suites moved recoverably to `retired/rebuild-independent-models`.
The independent Python HTTP-auth policy suite moved there too: it tested its own
model, not the running Rust service. This exclusion is about relevance to current
acceptance, not removal of failing production tests. `src/auth_model.py` remains
because actual server test clients use its signer; `src/cbor_profile.py` and its
independent public codec vectors remain active. Real service security/accounting
coverage belongs to the current server public-seam gate.

A fifth actual red→green slice is recorded in
`results/rebuild-package-parent-{red,green}.log`: a symlinked output directory used
to redirect replacement outside the selected path; the CLI now refuses it while
preserving the existing archive. This supplements source/output-file symlink,
read-failure atomicity and CRC/SHA artifact checks.

The local bridge also imports its real CLI output through the actual JavaScript
`LocalLibrary.replace`/`snapshot` operations (`rebuild-local-client-interop.log`).
IndexedDB is modeled in that cross-language check; it is not a rendered browser.

A sixth red→green guard, `rebuild-package-private-evidence-{red,green}.log`, closes
an explicit-evidence bypass of the private-runtime exclusion. Even selected
`results/.runtime` files are refused. Arbitrary evidence contents still require
operator review; path filtering is not a universal secret detector.

## Current focused checkpoint

After functional checks, Black 25.1.0 was pinned in requirements-dev.txt and run
only on maintained Python source/tests and the packager. The post-format active
Python suite passed **50 tests** (`results/rebuild-python-final.log`), including
**8 actual nginx lifecycle tests** and the local JavaScript-operation interoperability
case. Packaging passed **6 CLI tests** (`results/rebuild-package-final.log`).
`results/rebuild-python-format.log` records the formatter/check. These are this
rebuild's focused results, not a complete application or real-browser acceptance.

## Additional real restored-instance acceptance, 19:55 UTC

The extended `server/tests/http_migration.rs` passes through actual Python backup/restore, HTTP, PostgreSQL, strfry and nginx. After paused restore, an unauthorized actor cannot audit/repair. An authorized audit discovers a deliberately damaged copied original; an approved peer repairs exact bytes without erasing a moderation tombstone. A newly authorized group member then publishes new original-backed content signed by the recovered group identity. Explicit configuration restart activates nginx, while the copied recurring schedule remains paused until an operator separately enables it; the real worker then imports a newly signed remote profile. Signup stays disabled.

Evidence: `results/rebuild-feature-seams/restore-reactivation-second.log` (9.19s), `restore-reactivation.json`. First-run refusal came from this test's incorrect image dimensions and is retained as harness chronology, not a runtime bug. No source implementation changes were needed. This still models a co-located offline recovery, not host power loss, online backup or every disaster-recovery combination.

Final focused verification at19:55UTC: `composed-final.log` passes `http_migration` (8.83s) and `http_publication` (3.47s), exit0. Three-target Clippy and rustfmt pass. Exact test-source hashes and retained diagnostic chronology are in `results/rebuild-feature-seams/acceptance.json`. No production changes or canonical full-suite claim follows from this slice.
