# Backend and library reconstruction

This is a reconstruction from the recovered October 8 tree, tested on October 10.
Earlier results from the lost tree are historical and do not validate these files.
PostgreSQL, deadpool-postgres/tokio-postgres, Moka and schema version 52 remain.

## Application boundaries and invariants

- `library_references` prepares immutable descriptor/summary validation before the author transaction. The prepared value grants no authority. Commit rechecks the account/peer/head fences, then locks sorted exact blob generations and account grants and checks current health. Reclaiming and restaging identical bytes requires a new commit attempt.
- `Backend::run` keeps real strfry import, deletion and verification in that order. Verified imports receive one atomic version/state-fenced acknowledgement on the same detached session that owns the projection advisory lock. Retirement transactions contain at most four actual items, with sorted author locks. Pending export withdrawal and every generation fence remain.
- `projection_complete(event)` describes that event only. It is not a queue-drained signal or proof that an earlier deletion completed. The test cycle barrier uses two sequential imports, and retention tests separately observe the relevant visibility, restoration and grace outcomes.
- Root retention uses set-based hold updates and a single aggregate one-time staging-charge refund while retaining sorted credit/reservation/grant locks. Retirement joins exact held generations to their blob rows and locks in hash order before starting grace.
- Expired-staging selection considers at most 64 sparse unreleased-credit candidates per call, before owner/lease/queue filtering. Protected and foreign candidates advance the cursor. History pruning retires at most 16 eligible events per transaction. Current heads, winner watermarks, active holds and grace remain protected.
- Private and publicly retained library reads use the snapshot layer's independent read permits, rather than mutation permits. Owned snapshot execution preserves generation/health verification and I/O lifetime after cancellation; HTTP library response count/byte admission remains separate.
- `UploadStore::oldest_cleanup_due_seconds` reports the age of the oldest currently due local GC or retry item, or no value when none is due. HTTP status exposes this through the existing operator interface.

Unused generic Repository queue wrappers and body-load counters were removed only after actual sync-operation, restore and multiprocess replacement fixtures passed. Historical migrations and recoverable source copies remain. The separate capacity fixture review also removed its three unused public wrappers; see `results/rebuild-capacity-seams`.

## Genuine red → green slices

| Caller-visible behavior | Baseline failure | Replacement result |
| --- | --- | --- |
| Prepared root cannot adopt restaged generations | `prepared-references-red.log`, 0.40s | `prepared-references-green.log`, 0.44s |
| Unrelated import and four retirements progress while a fifth author is blocked | `projection-independence-red.log`, 0.61s | `projection-independence-green.log`, 0.91s |
| Foreign/protected rows cannot make one cleanup call exceed its 64-candidate scan | `cleanup-scan-red.log`, 0.54s | `cleanup-scan-green.log`, 0.63s |
| One history call cannot prune 17 eligible old heads | `history-budget-red.log`, 0.78s | `core-regression.log`, 0.70s |
| Private and public reads progress while four actual staged writes wait for another author | `library-read-isolation-red-confirmed.log`, 2.80s | `library-read-isolation-green.log`, 0.83s |

Logs are under `results/rebuild-backend`. The initial bootstrap lock failure was a compilation/dependency coordination failure, not a runtime red. The first read-isolation arrangement hit PostgreSQL's default lock timeout; the confirmed red extends only fixture sessions so the deliberate external fault outlives the read deadline.

## Focused regression evidence

- `core-regression.log`: all five core fixtures passed, including the 32-author real-backend batch and prepared-generation race.
- `seam-retention-regression.log`: all eight converted retention/export fixtures passed: library staging, roots, reclamation, account heads, publication reclamation, public export holds, positive export and export health.
- `library-read-isolation-green.log`: all five passed after the final two-permit removal: isolation, prepared generations, staging, roots and reclamation.
- `account-heads-fixed-clock.log`: same-clock contention passed with an explicit shared timestamp, removing the inherited wall-clock-second setup race.
- `relay-lifecycle.log`: real backend projection, history and feed reconnect fixture passed in 1.57s.
- `library-module-seams.log`: 26 passed, two intentional ignores. One is an optional very large codec vector; the other is a subprocess-only crash harness invoked by its parent test.
- `clippy-recheck.log`: all targets passed with `-D warnings` after correcting one test-helper style warning. `clippy-final.log` passed again after the final read-isolation change (26.34s).

A later caller audit corrected an over-broad qualification in the initial conversion: some public convenience methods were still test-only. Their replacement and removal are documented below. The focused tests now target actual lifecycle operations and caller-visible values. Exact remaining physical capacity is checked by filling it with real uploads, rejecting an extra byte, then deleting probe uploads through receipt-backed cleanup. PostgreSQL locks/clocks and filesystem damage arrange external faults; private accounting tables are not outcome oracles.

Independent frozen CBOR bytes now cover a library chapter vector. Private discovery and admission checks were replaced with signed discovery and real receive/process outcomes. Source tests for app-used configuration and public protocol/codec methods remain legitimate seams.

## Caller audit and removed convenience APIs

The audit distinguished a documented SDK from methods merely marked public. Native `plan_write_relays` and `run_job` remain part of the explicit public Rust SDK contract. Runtime caller scans showed the following convenience APIs were unnecessary:

- `reclaim_library_page` and `reclaim_retired_page`: two forwarding layers now replaced in fixtures by the node-used `reclaim_objects_page`, preserving each candidate-policy argument.
- `inventory_rows`: the test now calls the same `inventory_rows_scoped` operation used by peers and snapshots.
- `get_for_publisher`: no callers remain after bounded download delivery replaced the old returned-Vec transport.
- `export_public_original`: the test-only direct exporter is removed. Positive export, re-export and moderator-lock races now enter through actual bounded reconciliation. Real new publications schedule positive work. Wrong MIME, manifest and stale-generation inputs are checked at the application-used reservation interface.
- `quarantine_blob`: the test-only no-receipt entry point is removed. The actual internal quarantine operation always requires a typed physical withdrawal receipt; generation and physical-root checks remain. Corrupt-file reads now exercise the real generation-lock/health-commit path, and real read/export health tests preserve visibility, repair and refund protection.

The source worker separately moved the duplicated group key-path fixture helper out of production. Original source remains recoverable. These are regression migrations and API simplifications, not additional red→green runtime fixes.

`runtime-seam-migration.log` contains the two successful library migrations and a test-arrangement failure: manual withdrawal does not enqueue positive work. `runtime-seam-migration-recheck.log` then passed positive export, reader holds, library reclamation and publication reclamation after genuine publications replaced that assumption. `runtime-seam-final.log` passed all nine post-removal fixtures: library staging1.65s, roots1.77s, positive export2.84s, reader holds4.96s, export health4.30s, read isolation0.82s, prepared generations0.38s, library reclamation10.51s and publication reclamation1.82s. `clippy-runtime-seam-final.log` passed all targets with `-D warnings` in21.88s.

No broad module split is proposed. A possible later maintainability target is mutation admission ownership: node upload, group and Blossom handlers still repeat permit acquisition plus owned spawning. Any replacement should first demonstrate a caller-visible failure and preserve early body admission, shared capacity and cancellation lifetime. The current tested invariant does not justify a speculative rewrite.

## Scope and limits

This evidence is focused and was gathered during parallel reconstruction, not a final frozen aggregate gate. No reconstructed performance claim or full traffic-target claim is made. The whole server's seam-only inventory and canonical regression belong to the combined final gate.

SQL plan/index/buffer experiments are preserved separately in `server/diagnostics/rebuild-oct8`; their unique implementation-performance checks are explicitly diagnostic. Prior source and replaced tests are recoverable under `server/retired-tests/rebuild-oct8`. A filename move does not establish equivalent behavior coverage.

## Final crash-recovery and moderation seam migrations

`http_library_gc_crash` no longer uses ledger sums, outbox/claim counts or SQL query-text matching as acceptance oracles. It verifies exact signed heads and actual strfry import/deletion, fills physical capacity through HTTP, then pauses finalization using an external advisory-lock trigger and PostgreSQL's blocker graph. Known generation files being unlinked locate the fault window only. After the node is killed and blocked transactions roll back, a real stopped-node `UploadStore` admission probe proves charges remain. Restart with new candidate discovery disabled must readmit exactly the retired bytes. Another byte remains refused, current content remains readable, and a second restart followed by an explicit public maintenance quantum cannot refund again.

The first migration run timed out because it incorrectly asked `projection_complete` about a retired predecessor; that operation intentionally certifies only a currently hosted healthy event. The corrected fixture uses a later public batch barrier and actual backend disappearance, without increasing the timeout. This was a test-oracle correction, not a runtime defect or new TDD claim. The initial failure is preserved in `http-library-gc-crash-seam.log`.

`http_moderation` now proves actual backend presence before normal deletion, then checks actual strfry disappearance instead of reading private event state. Duplicate approval is observed through `new_withdrawals=0` and the response's `projection_completion_pending=false`; these fields describe that repeated approval, not a global queue-drain guarantee. Restore readiness is observed through the restore operation itself. The crash case waits for the delayed orphan importer to finish successfully and confirms its actual import output before checking final backend absence, preventing an empty-backend false pass before the importer runs.

`final-crash-moderation-seams.log`: GC crash passed3.61s; moderation passed3.50s. `final-crash-moderation-clippy-recheck.log`: targeted Clippy passed with `-D warnings` after one test-only style correction. Original fixtures remain recoverable under retired-tests. Production source is unchanged by these two migrations. Disposable runtime databases and credentials are outside evidence in `.runtime-final`; the non-secret move record is `fixture-retention.json`.
