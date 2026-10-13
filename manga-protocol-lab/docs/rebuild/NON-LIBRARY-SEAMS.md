# Reconstructed non-library test seams

Working checkpoint, 10 October 2026. This tree was recovered from October8;
all results below were executed on the reconstructed sources.

## Passed public interfaces

- `series_metadata::parse`: independent canonical wire input preserves the complete descriptive model, rejects unsupported features/schema and enforces explicit cover declarations.
- `manifest::{series,chapter}`: literal canonical bytes replace the production encoder fixture; accepted language tags return exact identifiers, labels and original descriptors, malformed tags fail.
- Nostr `Event::verify`, actual signing and authorization decoding: independent Python/coincurve UTF-8 vector, exact event identity/signature and tamper rejection.
- `mangadex_compat` UUID/time codecs: independent fixed outputs; leap-day, epoch and upper-bound parsing/formatting, invalid calendars and out-of-range values.
- Caller-used bounded response: normal and independent gzip bytes, headers and externally supplied lease recovery after consumption.
- `capabilities::Profile::document`: configured origins, limits and admission semantics, including public query-option rejection beyond the advertised limit. Capabilities are not a health or permission grant.

The existing enrollment policy validation and service proof/announcement tests use
real application-called configuration/codec interfaces and were retained. New vector
coverage has changed tests only; it does not change production protocol behavior.

## Fresh evidence

- `results/rebuild-seams/series-metadata-public-first.log`: 1 passed.
- `results/rebuild-seams/independent-codec-vectors-first.log`: 4 passed.
- `results/rebuild-seams/public-contracts-first.log`: 4 passed.

These focused runs overlap; their counts must not be added as a suite total.
Retired checks and replacement qualifications are recorded in
`server/retired-tests/rebuild-20261010/README.md`. They were removed only after
replacement passes. This is migrated regression coverage, not invented historical
TDD. No full-server, browser or production-capacity acceptance follows.

## Domain and HTTP replacements

HTTP discovery passed on real PostgreSQL, with synthetic signed metadata as explicit
input setup and HTTP status/event IDs as outcomes. It covers all advertised fields,
NULL/language truth, injection-shaped literals and syntax/type/depth/length failures.
Evidence: `http-discovery-public-first.log` and `domain-public-first.log`.

Group authority fences and first-actor attribution passed through public publication
visibility and actual authenticated operator audit HTTP. Enrollment migration,
legacy denials, insertion/revocation races, budget rollback, lifetime capacity and
account-versus-publisher permission passed through real migration/enrollment/permit
interfaces. SQL only arranges faults. Their earlier internal-table outcome assertions
are preserved as retired diagnostics. First passes: `domain-public-first.log`.

The admission reconstruction's actual HTTP slow-reader fixtures passed before the
remaining private bounded-response timeout/counter checks were retired. Count/deadline
fixture: `results/rebuild-admission/download-count-green.log`, 125.06s. Byte-budget
fixture: `download-bytes-green.log`, 10.16s. Those checks exercise actual disconnect,
120-second release, admission and concurrent upload behavior.

## Export lifecycle and final checks

Public export reconciliation passed through actual PostgreSQL/strfry/nginx and
caller-used lifecycle methods: repeated reconfiguration preserves progress,
unpublished staged bytes gain no public hold, shared originals remain readable,
last-reference deletion becomes HTTP404, version/generation changes survive stale
completion, and verified repair restores exact bytes. SQL and filesystem mutation
only arrange faults; they are not outcome oracles.

The first expanded export run failed when a newly added maintenance call consumed
a row before an old diagnostic SQL read. Both the original invariant and added
public behavior passed when sequenced correctly, followed by the final public-only
fixture. This test-harness correction is not claimed as product TDD.

Final verification on 10 October 2026:

- `results/rebuild-seams/module-regression-final.log`: **75 passed, 2 ignored**.
  The ignores are the child-process harness invoked by its parent crash test and
  the optional half-million-record library stress test. This is the module suite
  at this reconstruction point, not the entire HTTP suite.
- `domain-regression-final.log`: **4 passed**: HTTP discovery (2.96s), group fences
  (0.44s), enrollment fences (0.35s), public export queue (3.11s).
- `format-final.log`: owned Rust source/test formatting passed.
- `vector-reproduction.json`: independent fixture generation reproduced every
  literal JSON byte in a separate temporary directory.
- `summary-final.json`: qualification, file hashes and exact evidence inventory.

No new production interfaces or behavior changes were introduced by this test
cleanup. Those results identify the main four-fixture checkpoint. Broader concurrent
reconstruction and final all-target release validation remain the project owner's
separate gate; these results do not certify future changes or production capacity.


## HTTP group extension

The additional `http_groups` fixture now observes failed audit-write rollback through
public chapter discovery, unchanged authenticated audit output and continued event
reads. A revoked in-flight upload uses a valid manifest; an authorized administrator
cannot publish it afterward. Key-only recovery rejects former members through HTTP,
then returns the exact newly bootstrapped membership list.

- `results/rebuild-seams/http-groups-public-first.log`: replacement assertions passed
  alongside the old SQL assertions (1.91s).
- `http-groups-regression-final.log`: covered SQL oracles removed, passed (2.02s).
- `http-groups-seams-final.log`: final seam-only fixture passed (1.92s).

The exact plaintext-key scan of a `managed_groups` database row has **no equivalent
public proof**. Its code and narrow limits are preserved separately in
`server/diagnostics/managed-group-key-storage-check.rs.txt`. The canonical suite
still checks offline backup output, key-file permissions and public authority
boundaries; those checks do not certify the database's internal secret storage.


The final group revocation fixture replaces its scheduler sleep with an actual
HTTP `100 Continue` barrier before revocation. The accepted request then fails
with HTTP400 and grants no later publication. `group-final-protocol-barrier.log`
passed both group fences (0.39s) and HTTP group/recovery behavior (1.90s).

## Public endpoint and DNS policy

The source inventory is recorded in `SOURCE-TEST-INVENTORY.md`. Endpoint origin,
private/special literal destination rejection, exact private overrides, identity
binding, redirect refusal and proof-size rejection now use the actual public module
operation and live HTTP peers. An independent Linux child-process resolver fixture
covers mixed public/private answers, empty/17-answer bounds, classification of
public IPv4/IPv6, and successful vetted-address pinning despite later rebound DNS.
It does not modify global resolver configuration or add production test interfaces.

- `endpoint-module-final.log`: 4 public-interface tests passed.
- `endpoint-dns-seams-final.log`: 1 integration test passed, no ignores. It invokes
  its own isolated child process; an earlier two-entry harness was simplified so
  there is no no-op child test in the canonical list.
- `seam-extension-clippy.log`: targeted library/group/endpoint Clippy passed with
  warnings denied.
- `seam-extension-format.log`: changed source/test formatting passed.
- `extension-final.json`: exact current extension files, log hashes and limits.

The test-only `groups::recovered_key_path` export was moved into fixture support,
and the internally used address classifier is now private. Runtime behavior is
unchanged. The third key-path caller, staging reclamation, compiled but an earlier
run failed at its existing exact reclamation-batch count before the changed call;
that failed log and stopped PostgreSQL fixture are retained and the owning workstream
is addressing the internal outcome oracle. It is not counted as a passing regression.

`summary-final.json` remains the historical 19:37 four-fixture checkpoint. Its file
hashes are not a promise that later extension edits have not changed those files;
use `extension-final.json` for the files changed in this subsequent slice.


## Three-node peer HTTP migration

The final `http_peers` fixture preserves bilateral authority, foreign-author denial,
filtered transfer, source-read metrics through the actual operator status API,
exact original/library bytes, independent fallback, source shutdown, rotating audit,
approved repair and the crash-after-durable-repair/before-health-commit window.

Physical accounting is observed through the running owner's HTTP interface: a
one-byte control succeeds before fill, known remaining capacity fills exactly,
another byte is rejected, and the test's temporary uploads are explicitly deleted.
This is checked before/after repair and after restart. The fixture raises only its
staging allowance to the unchanged 2MiB physical allowance so staging admission
cannot mask the physical boundary. No second storage owner is opened.

Remaining database queries only locate external corruption targets or coordinate
injected locks by blocking-PID relationships. The one raw generation-file read is
an explicit pre-kill fault-window condition. Actual outcome assertions use HTTP
bytes/visibility/admission, not that file or database rows.

- `http-peers-public-recheck.log`: old and new assertions passed together (7.59s).
- `http-peers-seams-final.log`: canonical replacement passed (7.42s).
- `http-peers-clippy.log` and `http-peers-format.log`: targeted checks.
- `peers-final.json`: exact source/evidence hashes and qualifications.

The first attempt failed compilation after changing fixture retention from TempDir
to Path while two multiline `.path()` calls remained. The error was corrected and
the failed log preserved; it is a fixture-edit error, not product TDD. Failed
runtime data is retained outside the source tree, while successful fixtures clean
up after their processes stop.

The staging reclamation owner's later public-capacity migration passed (4.19s) and
Clippy, resolving the earlier key-path caller's unrelated exact-batch test failure.
Broader fixture migrations and the canonical all-target gate remain separate.
