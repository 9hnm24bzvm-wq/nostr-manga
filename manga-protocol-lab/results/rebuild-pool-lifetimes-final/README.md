# Connection-lifetime fixes: final functional acceptance

Completed 2026-10-12 at 02:00:32.753093 UTC, exit 0.

- Optimized node SHA256: `4c499f724af4db1843f54c51e47746114f777de15b446979a3e8e8a0e7a3e23a`.
- All 392 source/test inputs matched before and after execution. Relative to accepted `1c3f7769…`, only three production files and the new public HTTP fixture differ.
- Passed: 58 Python tests, 6 packaging CLI tests, 91 browser model tests, 74 Rust module tests, and 89 integration executables containing 108 cases.
- Two Rust cases were intentionally ignored: the outbox child-process harness is invoked by its parent crash test; the large half-million-record Rust codec case was not executed by this invocation. The separate browser model coverage is not a substitute claim that this Rust case ran.
- Strict all-target release Clippy passed with `-D warnings` in 43.79 seconds. Formatting passed.
- Real PostgreSQL multiprocess checks passed: 12 writer processes produced one unique insert; four workers completed 32 unique jobs; immediate database stop/restart failed closed during the outage, recovered the pool, and allowed a fresh process to read. `fsync` and `synchronous_commit` were both `on`.

## What changed and why the tests matter

`UploadStore::audit_page` now releases its cursor/select connection before operations that check out another connection. It retains the audit mutex, selected rows, bounds and generation/visibility checks. The compatibility resource-list handler releases its committed selection connection before profile/cover/alias lookups. Library commit releases its successfully committed connection before non-authoritative Moka cache work. No pool sizes, queue settings, retry policy or public-export transaction/filesystem visibility fence changed.

The focused evidence in `../rebuild-pool-lifetimes/acceptance.json` preserves two independent public HTTP regressions that failed on the exact prior executable and passed with this candidate using a valid one-connection pool. The same fixture passed again in this aggregate run. Baseline audit failure proves the first empty-page case; later populated-page and cursor-wrap controls passed on the candidate. Both cover and author failures were reached on the compatibility baseline. The Moka tail is supported by source inspection and public lifecycle regressions, without a fabricated causal timing failure. Tests were prepared before execution; the record does not claim strict red-before-implementation chronology.

## Exact gate and process limits

This was the reviewed canonical-equivalent serialized runner adaptation: source-root anchors, four Cargo `-j2` to `-j1` flags, and delegation to the copied PostgreSQL runner. Python/package/browser steps, the full integration inventory, isolated database settings, strict Clippy and durable multiprocess checks were unchanged. The existing optional `audit_gaps` characterization remained excluded. Installed dependencies were used offline, release profile, `CARGO_INCREMENTAL=0`; read diagnostics were off except explicit fixture controls.

The owned-command guard observed no input drift, resource abort, monitoring error, cleanup error or remaining owned process. Owned PostgreSQL pidfiles were absent after completion. Minimum free disk was 4,413,169,664 bytes, and minimum global available memory was 2,906,476,544 bytes. Floors were 2 GiB disk and 1 GiB memory. These sampled global safeguards are not application memory attribution or hard protection against between-sample overshoot/SIGKILL. Final free disk was 5,174,222,848 bytes.

## Scope

This establishes functional regression acceptance for these exact inputs. It does not establish sustained mixed-load capacity, explain the earlier pool-checkout rejection wave, or prove a latency improvement. Audit was disabled and compatibility collection routes were absent from that mixed workload. No new rendered-browser UI acceptance, complete parity against unavailable later source bytes, or current recovery upload is implied.

Primary receipt: `canonical-acceptance.json` (SHA256 `656aa030c83f239bde726813cefb6f6c49ffe9d3cd55e18ca1be38f9779cd6d5`). Logs, exact runner diffs, input hashes, per-executable counts, resource observations and process termination receipts accompany it. The independently authored terminal audit is a separate artifact.
