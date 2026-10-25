# Refactor parity contract

## Current status

See [the authoritative current rebuild summary](STATUS.md#current-status) for the current functional gate, mixed-load outcomes, checkpoint verification and remaining qualification. The separately dated sections below are historical evidence for their stated builds/times; they do not override that summary. The [architecture map](ARCHITECTURE.md) describes current source boundaries.

## Verified optimized acceptance, 2026-10-10 22:40 UTC

The final complete gate passed exit0 with unchanged recorded inputs:55 Python,6 packaging,91 browser model checks,74 Rust module cases (2 intentional ignores),all84 integration executables,strict all-target Clippy and PostgreSQL multiprocess crash/restart with durable settings. Exact node SHA256: `478ffc3243281a8e1e0c756eaf91b8923d740a2b2a146833b61443a837e73b54`. Browser build: `e4f4862bd75c26bb`. See `results/rebuild-parity-final/canonical-acceptance.json`.

All six explicitly recovered refactor promises have implementation and acceptance evidence. The feature audit found no additional missing required core family. Native Chrome schema4→5/save/reload/export preserved all500,202 records field-exact; three-sample count latency median fell from2075.8ms to0.3ms on this shared host. The separately dated sections below are historical, not the current completion state.

Matching encrypted checkpoint12 was uploaded, downloaded, all1,487 members authenticated/decrypted/verified, restored, and its binaries CLI-smoke-tested. See CHECKPOINTS.json and results/rebuild-parity-final/checkpoint12-recovery.json. Mixed-load/target traffic,100TB production operation,native signer extension,physical mobile/eviction/power-loss qualification remain open. Missing later source prevents exact byte parity; this gate does not promise perfection or every possible failure covered.


## Source of commitments

User message `private-chat-reference-021-redacted` (2026-10-10 20:58:55 UTC) requires full parity and every discussed refactor goal. The preceding request to fix all six proposals and follow Matt Pocock codebase-design refers to assistant message `private-chat-reference-023-redacted` (2026-10-10 01:14:29 UTC), recovered through conversation search on 2026-10-10. The Oct8 baseline is available; the lost later source is not available for byte comparison. Historical test counts and unmapped letter-only answers do not prove current parity.

## Six specific promised fixes

1. Separate configurable catalogue, library-head and live-subscription read budgets under an overall database limit, with bounded waiting for short bursts.
2. Shared live-event dispatch: fetch each notified event once, route to matching subscriptions, bounded client queues, preserve authorization/revocation and avoid closing healthy subscriptions merely because query admission is busy.
3. Storage fairness: reserved foreground capacity and a smaller background cleanup allowance under one overall limit, with cleanup share responsive to storage headroom.
4. Reduce save database round trips through compatible combined checks and set-based accounting/refunds. Preserve short transactions, lock ordering, quotas and commit-time authorization.
5. Reuse immutable root/object validation and previous accepted references so unchanged objects require less work. Preserve authoritative generation, quarantine, ownership and visibility checks.
6. Eliminate the head-ID/visibility split-read race using one consistent database snapshot or a bounded retry, protected by a focused reproduction.

## Cross-cutting requirements

- Preserve implemented protocol features and public API/CLI behavior except an explicitly justified correction.
- PostgreSQL, deadpool-postgres/tokio-postgres and Moka; no SQLx, SQLite runtime or Kratos.
- Local-first browser library with explicit opt-in public publication; four library families, durable publication/recovery, backup/import/export and migration behavior.
- Keep module interfaces small; move complexity/ownership behind cohesive modules instead of adding shallow wrappers or needless abstractions.
- Remove unused production code and redundant/implementation-coupled tests only when meaningful coverage is preserved.
- Tests at actual public/application-used seams; real dependencies and external fault arrangement where practical. No private SQL/counter outcome oracles or test-only public hooks masquerading as interfaces.
- Use red→green slices for new behavior/bugs. Do not retrospectively call inherited or merely migrated tests TDD.
- Preserve security, cancellation, revocation, quota conservation, compatibility and crash/recovery behavior.
- Verify final exact source and optimized executable with relevant full gates after changes.
- Keep encrypted external checkpoints, verified downloaded bytes/decryption/member manifest and recovery information.

## Ownership supplement (2026-10-10 22:13 UTC)

The final review reproduced ordinary HTTP disconnects returning read credits while tokio-postgres still drained executing/queued work. The implemented owned-handler correction retains admission until physical work completes, including nested response deadlines. The real disconnect and nested-deadline causal reds are preserved; the focused release fixture passed all sequential cases in26.39s. See `results/rebuild-admission/http-work-ownership.md`. Full-project acceptance for this changed source is still pending.

## Current acceptance status (2026-10-10 21:31 UTC)

All six named changes are now implemented and have focused acceptance evidence:

1. Classed read admission and bounded waiting: `results/rebuild-admission/parity-admission-dispatch.json`.
2. Shared live dispatch, authorization, bounded fanout and ownership through timed-out SQL completion: same report; real slow-reader and externally blocked-database fixtures are included.
3. Foreground storage reserve and headroom-responsive cleanup: `docs/rebuild/STORAGE-FAIRNESS.md`.
4. Compatible post-lock checks and grouped idempotent refunds: `results/rebuild-backend/parity-lifecycle-final.log` and backend audit.
5. Bounded reuse of accepted immutable validation with current generation/health/authority fences: validation-reuse red/green/profile evidence under `results/rebuild-backend`.
6. One coherent current-head snapshot: head-snapshot red and head/history green evidence in the same directory.

The history worker now uses a typed continuation result instead of mismatched magic batch counts. Client and operator audit fixes include atomic recovery consent fences, local capacity and migration behavior, Unicode-name parity, bounded public downloads and fail-closed managed-serving activation. The browser model gate has81 passing cases; actual Chrome confirmed preservation, Unicode boundary save/reload and offline progress persistence for build72e656def51db26f.

The previous combined optimized gate failed at a shared-database fixture collision. The corrected storage fixture passed all four cases in three fresh release runs without changing production behavior or relaxing deadlines. The next full gate must include that correction and the HTTP ownership supplement. Actual native500k import/save/reload/export preserved all500,202 records exactly, but exposed repeated family counting at about2.08s per call. Transactionally maintained family totals are under focused acceptance before the final aggregate restart. Focused passes do not substitute for that result. Earlier canonical/load evidence remains historical and cannot certify this changed revision. The test audit records removed implementation-coupled assertions, actual-seam replacements and remaining qualification limits.

Target traffic and100TB production operation are separate unproven qualifications. A complete MangaDex-style reader website was deferred earlier; the functional local-first browser library client remains required. No test suite can guarantee every possible failure or perfection. The missing later source and unmapped letter-only answers cannot be reconstructed as evidence of additional exact decisions.
