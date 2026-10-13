# Manga protocol parity and qualification report

Updated 2026-10-11 08:16 UTC.

## Conclusion

**Functional parity with the six recovered refactor commitments is accepted for the current build.** The feature audit found no additional missing required core backend family. The local-first browser library is implemented and has model, interoperability and actual desktop-browser evidence. This conclusion is against the recovered requirements; the unavailable later source prevents byte-for-byte reconstruction claims.

**Target performance consistency remains unresolved on this shared environment.** The stage-profile-off repeat and subsequent passive-scheduler trial both failed the full-mix contract; no causal fix was established. Instrumented runs of the earlier 1a45 build passed the declared 420-second mixed-load bounded-completion contract at both storage8 and storage12. No mixed-load result exists for the 82c52 diagnostic build or current migration-reviewed tree. Storage12 also kept sampled maintenance candidates low during the observed post-grace interval. Strict-window completion and indefinite production capacity are not established. The limits below are part of the result.

Sources: [recovered contract](PARITY-REQUIREMENTS.md), [architecture](ARCHITECTURE.md), [feature coverage](FEATURE-COVERAGE.md), and [current status](STATUS.md#current-status). Dated pending statements in earlier audits describe their original snapshots; the exact current gate below is authoritative for this build.

## Accepted build

- Node SHA256: `82c52b35720e623a303ec49fad73e65e08e033baf83b4fcc4349890af174c491`
- Browser build: `e4f4862bd75c26bb`, IndexedDB schema 5
- Release-profile gate: **exit 0, all 390 inputs unchanged**, completed 2026-10-11 08:13:29.764250 UTC
- Passed: **58 Python checks, 6 packaging checks, 91 browser model checks, 74 Rust module cases, and 88 integration executables containing 106 passing cases**; two Rust cases were intentionally ignored
- Strict all-target Clippy passed with warnings denied in 0.18 seconds using unchanged cached Rust artifacts; PostgreSQL multiprocess crash/restart passed with fsync and synchronous commit enabled

The [canonical acceptance receipt](../../results/rebuild-migration-review-final/canonical-acceptance.json) identifies the exact inventory and frozen evidence. Receipt SHA256: `bef0b1856542a996cd85167dd9de300c98c7837512cb20a7d0b64fde767f25d2`. The [independent reviewer](../../results/rebuild-migration-review-final/INDEPENDENT-REVIEW.md) reviewed the patch separately from implementation, then operated and audited this single gate; a second independent execution is not claimed. Focused checks overlap this evidence and are not extra totals. This gate supplies no new rendered-browser, mixed-load or recovery-package acceptance.

## Recovered feature scope

The implementation preserves these core workflows:

- **Publication and reading:** signed immutable chapter manifests, verified original bytes, catalogue/discovery queries, explicit pagination and capabilities, retained originals, and optional managed NGINX serving. Corrections use deletion and new publication rather than retargeting an existing chapter identity.
- **Identity and policy:** independent account, publisher and peer permissions; guarded HTTP/Nostr authentication; disabled-default enrollment; service-identity continuity; instance-local moderation and protected deletion without automatic cascade or resurrection.
- **Federation:** explicitly approved peers, verified originals, bounded inventories, durable synchronization jobs/schedules, cancellation and lease fencing, operational retry versus content rejection, and repair from approved sources.
- **Libraries and social:** signed public library heads, reading progress, series, lists and memberships; comments/reactions/votes; group roles, publication and recovery. A backend feature does not imply a finished screen for that workflow.
- **Local browser and recovery:** private local editing, explicit publication consent, durable publication outbox, verified public recovery, local backup/import/export, schema migration and cross-tab/offline local use.
- **Operation:** PostgreSQL migrations, operator import/mapping, offline backup/restore, audit/repair, deliberate restored-instance reactivation, and process/crash recovery.

The retained architecture is Rust/Axum/Tokio with PostgreSQL, deadpool-postgres/tokio-postgres, Moka and an internal strfry projection, plus JavaScript browser and Python operator workflows. PostgreSQL remains authoritative; cached immutable facts, derived projections and file bytes do not replace current policy or ownership checks. No SQLx, SQLite application runtime or Kratos service was introduced. See the [architecture and ownership map](ARCHITECTURE.md).

## The six refactor commitments

1. **Separate bounded read classes:** catalogue, library-head and live reads have configurable class budgets under an overall database bound, with bounded burst waiting. Class caps isolate contention but are not strict reserved shares. Disconnects and deadlines do not release ownership while physical SQL work is still draining.
2. **Shared live dispatch:** one authoritative lookup per processed notification feeds matching bounded subscriber queues. History/live handoff, slow consumers, revocation and discontinuity preserve explicit recovery and fail-closed behavior.
3. **Foreground and maintenance fairness:** one volume-work owner provides a foreground reserve, a smaller normal background share and headroom-responsive cleanup. Body receipts and retained response bodies keep separate bounds; cancellation does not silently free unfinished work.
4. **Less database work without weaker accounting:** compatible checks and refunds are combined; reserve/settle mutations are set-based; fixed blob lookups reuse prepared statements. Short transactions, sorted locks, exact quota conservation, rollback, generation fences and current authorization remain.
5. **Reuse immutable validation:** accepted root/object descriptors and previous references avoid repeated decode/summary work. Current generation, health, quarantine, authority and head/winner checks still execute; stale/cold entries fall back safely.
6. **Coherent head reads:** signed head identity/body, negative health and projection visibility come from one consistent repository operation. A focused concurrent-replacement reproduction verifies the old split-read failure and the coherent response/conflict behavior.

The [backend parity mapping](PARITY-REFACTOR-BACKEND.md), [read and dispatch evidence](../../results/rebuild-admission/parity-admission-dispatch.json), [HTTP ownership evidence](../../results/rebuild-admission/http-work-ownership.md), and [storage fairness contract](STORAGE-FAIRNESS.md) connect these commitments to actual interfaces and focused reproductions. The current aggregate gate covers their final combined implementation.

## Bounded rejection diagnostics

The new 82c52 change adds **opt-in, default-off read-admission diagnostics**, without changing limits, permissions, HTTP bodies, Retry-After or Nostr policy. Enabled authenticated status counters distinguish queue-full, observed wait-expiry and ticket-exhaustion. At most one record per shared controller per second is selected, with a 2,048-byte JSON bound and no URLs, account/request identifiers, SQL or content. Disabled acquisitions avoid diagnostic clocks, counters, worker creation and output.

A capacity-one nonblocking handoff and one optional writer isolate this diagnostic's stalled stderr from request threads. One writer may remain blocked until output progresses or the process exits; shutdown does not join it. Other stderr writers are not covered. Expiry timing includes ticket cleanup and snapshot-lock delay, not isolated timer, handler, database or reactor time.

Real HTTP queue-full/expiry controls reproduced missing opt-in diagnostic output before the change. A separate undrained-pipe red reproduced a synchronous-output HTTP timeout; bounded handoff passed that same fault, including drop/failure visibility and forced kill/reap. This does not prove graceful shutdown or a throughput cure. **Eleven focused behavior tests**, formatting and strict focused Clippy passed before the full gate. [Contract and red/green evidence](../../results/rebuild-read-diagnostics/README.md), [focused receipt](../../results/rebuild-read-diagnostics/acceptance.json).

## Supplied-library migration review

The Python adapters now reject unsupported top-level project-library fields before selecting the documented `read_markers`, `statuses` and `lists` fields. The preserved follows-only input formerly exited successfully with an empty backup. After correction it fails explicitly without an output artifact. Three actual CLI tests reproduced **12 failing subcases**, then passed unchanged; the valid sparse/wrapped control passed both before and after.

Local backup and preview refusals create no output. PostgreSQL review plans carry explicit errors with no partial library; applying them is refused before a network connection or journal, even with review consent. Supported exact read IDs, independent statuses, lists and positions survive actual browser import/export/reimport and public graph decoding. Known nested response wrappers and consent/identity checks remain intact. This does not add separate follow state, infer reads or certify every nested extension/third-party export; retain unsupported exports for review. The node/browser are unchanged, and the fresh complete-tree gate above accepts the Python correction. [Causal evidence](../../results/rebuild-migration-input-review/README.md), [focused receipt](../../results/rebuild-migration-input-review/acceptance.json).

## Public-seam acceptance and cleanup

Tests use HTTP/Nostr, real PostgreSQL/strfry/NGINX, operator CLI/process behavior, public browser library operations, independent codec vectors, and application-used lifecycle interfaces. External SQL locks, corruption, time/lease faults and crashes arrange failures; private ledger balances, helper internals and diagnostic counters are not the passing correctness oracle.

Representative coverage includes exact signed events/originals; approval and revocation; concurrent replacement; paired-budget rollback and exact-fill admission; no early/double refunds; stale-generation rejection; interrupted writes; unlink-before-refund crash recovery; durable synchronization recovery; slow-client ownership; and composed restore/audit/repair/group publication/reactivation. The [test-seam audit](PARITY-REFACTOR-TESTS.md) records replaced implementation-coupled assertions and removed test-only exports/wrappers. Existing regressions are not retrospectively labelled TDD.

Actual desktop Chrome imported, edited, reloaded and exported large synthetic libraries. The final schema-5 upgrade preserved **500,202 records** field-exact except the intended progress/timestamp edit. Model or Node interoperability results are kept separate from native browser evidence. [Native qualification](NATIVE-BROWSER-QUALIFICATION.md).

Three obsolete workflow helpers were removed from active use only after byte-preserved retirement: the stale ticket writer and two historical packaging/splitting helpers. Current packaging and verification entry points remain. The recovered requirements and historical results were preserved rather than rewritten as current acceptance. [Workflow retirement inventory](WORKFLOW-HYGIENE.md).

Module changes consolidate actual lifecycle ownership: shared admission, coherent head reads, typed history continuation, dispatcher source lifetime and immutable validation reuse. This is not a claim that every large module was split or all raw SQL was hidden behind a generic repository layer.

## Measured optimization effects

- **Browser family counts:** three native half-million-library samples changed from a 2,075.8 ms median to 0.3 ms after transactional family statistics. This is a small shared-host diagnostic, not an end-to-end publication benchmark. [Native evidence](../../results/rebuild-browser/native-counts-improved.json).
- **Capacity reserve/settle:** the same passing public fixture recorded 60 fewer targeted write-statement attempts across 15 reserve and 15 settle attempts, or four fewer per paired cycle. Rollback passed before and after; fixture duration did not establish a speedup. [Batching report](../../results/rebuild-capacity-batching/REVIEW.md).
- **Fixed blob lookups:** Parse operations fell **218 → 26**, while **218 Bind and 218 Execute operations remained**. Only prepared SQL is reused, not authority/data results. The unchanged public fixture passed before and after; all ten focused lifecycle fixtures and strict Clippy passed. [Prepared-query report](../../results/rebuild-blob-prepares/REVIEW.md).
- **Correctness corrections:** reclamation no longer unnecessarily locks already-refunded staging grants; recognized peer transport/status/body failures remain retryable without misclassifying permanent or complete-invalid responses. Both have causal red/green evidence and independent review. They do not by themselves prove a throughput cause or remedy. [Combined review](../../results/rebuild-peer-transient/CAPACITY-PEER-INDEPENDENT-REVIEW.md).

## Performance qualification remains incomplete

The preceding accepted 1a45 binary produced two scoped instrumented passes and two failed follow-ups. These results remain build-specific; none is a load qualification of 82c52. Earlier failures remain preserved. This establishes an investigation reference, not a repeatably performance-qualified deployment configuration.

- **Storage8 with stage profiling:** all 1,522,563 operations and 50,400 saves acknowledged, zero errors/skips, 128 exact heads. Due candidates grew 84 → 2,285 during traffic before substantial closing drain. [Report](../../results/library-node-mixed/parity-maintenance420-prepares-profile-001/PREPARED-LOOKUPS-REPORT.md).
- **Storage12 with stage profiling:** the full workload passed; sampled due candidates stayed at most 131 and oldest overdue at most 0.400799 s during approximately 120 seconds of maintenance. Save p99 increased 245.707 → 341.527 ms, main peak RSS 274.33 → 310.91 MiB, and PostgreSQL own CPU 2.306 → 2.373 cores. [Audited report](../../results/library-node-mixed/parity-maintenance420-storage12-profile-001/STORAGE12-REPORT.md).
- **Storage12 with stage profiling off:** failed with 262 API HTTP429 responses, one Rust-original HTTP429, 14 API pending-cap drops and 29 save pending-cap drops. All 50,371 started saves, all nginx images and 128 exact heads succeeded. Save p95/p99 were 766.472 / 987.532 ms. Onset preceded cleanup, but the cause was not isolated. [Audited repeat](../../results/library-node-mixed/parity-maintenance420-stage-profile-off-001/STAGE-PROFILE-OFF-REPORT.md).
- **Same configuration plus passive scheduler sampling:** failed with **110 unsent image offers dropped for generator scheduling lateness and eight API HTTP429 responses**. All **1,296,430 dispatched nginx images** succeeded; there were no nginx HTTP errors. All **50,400 saves** acknowledged and **128 exact, unambiguous heads** verified. Save p95/p99 were **354.110 / 509.569 ms**. Of 1,522,563 planned operations, 1,522,453 started and 1,522,445 acknowledged. [Final diagnostic audit](../../results/library-node-mixed/parity-maintenance420-scheduler-probe-001/SCHEDULER-DIAGNOSTIC-REPORT.md), SHA256 `8c8dc5dbfaee681703097db7bb6cd8b0d6be41789137e03744f95ed6535b6a5a`.

The passive sampler passed **nine actual CLI/process tests** covering live workloads, PID/start identity, exited targets, observer-only signal cleanup, output limits, unavailable/restricted counters, descendant discovery, thread caps and captured cgroup fields. Its separate 33-process/43-thread controlled check measured about **0.0514 core** with complete coverage. In the mixed trial it produced **4,338 samples with no missed 100 ms slots**, **52,444,571 bytes**, and used **0.04707 mean core / 12.465 MiB peak RSS**. These tests are separate from the application gate. The controller received static review and actual lifecycle verification, not an invented extra test count. [Harness acceptance](../../results/rebuild-scheduler-probe/acceptance.json), [review](../../results/rebuild-scheduler-probe/REVIEW.md).

**Diagnostic coverage was incomplete.** Both nodes' threads were covered at all 4,200 traffic points, but the fixed 43-thread cap truncated most generator and some selected PostgreSQL telemetry. Cgroup/PSI information was unavailable. There was no unique runnable-wait spike identifying the API rejection cause; the affected image worker's scheduler counters were missing. Neither absence of host throttling nor a causal effect from profiling removal/sampler addition was established. In the final trial, due candidates peaked at 119 and maximum overdue was 0.405226 s; low candidate counts did not make the full workload pass.

Storage slots 8 → 12 was one effective factor with coupled foreground, upload/read-task and normal maintenance budgets, not cleanup-only isolation; no persistent default changed. Its frozen original specification retains the inherited descriptive `unchanged.storage_upload_slots=8` discrepancy; the factor, launch setting and both actual configurations establish 12. [Current status and evidence](STATUS.md#current-status).

Every trial retains the predeclared finite-start/finite-completion contract. Strict in-window remained false; bounded late starts/completions do not excuse substantive skipped or rejected offers. Candidate counts are not reclaimable-byte debt, deleted bytes or a storage leak. CPU/RSS figures are observational; PostgreSQL RSS sums duplicate shared pages. Shared-host, loopback, hot-corpus 420-second trials do not certify indefinite operation, WAN/TLS/cold storage, monthly/peak traffic, 100 TB capacity or a latency SLO.

**No 82c52 mixed-load run has started.** The next trial is waiting for the user's still-pending approval to delete compiler dependencies and old cache archives for disk headroom; no deletion is claimed. The new diagnostics supply rejection reason/wait observations, not admitted-handler or pool timing. Complete generator telemetry or an isolated generator host remains useful further evidence. Functional acceptance is established; target performance consistency remains unqualified.

## Recovery and remaining boundaries

**A full checkpoint matching the migration-reviewed tree is being prepared; it has not yet been created or saved at this report freeze.** The earlier 82c52 archive was saved in Library and delivered as seven ciphertext parts plus a guide at **07:49:37 UTC**: **58,217,627 bytes / 781 members**, SHA256 `fa004c278aec90da54068ab0d5ec995684335cc66ca95159d7c48825e7b491c3`. Local reassembly was byte-identical; all members passed AES authentication and hash/CRC/length verification. That immutable archive predates the Python correction, 08:13 gate and this updated report, despite the unchanged node/browser. Its frozen source, documentation and evidence remain preserved.

**Independent remote readback remains blocked.** Library saving/delivery and local verification are not independent remote recovery verification. Catbox checkpoint 12 remains the latest independently downloaded, decrypted, member-verified and restored/CLI-smoke-tested full checkpoint. The earlier 1a45 full archive, 4e58 source-only backup and failed 108 MB transfer remain distinct historical deliveries. [Recovery index](CHECKPOINTS.json).

Remaining scope boundaries are explicit: full catalogue/chapter-reader/social/group/moderation frontends; installed native signer-extension end-to-end and large public-graph publication; physical mobile/eviction/power-loss behavior; and production security/capacity certification. A complete MangaDex replacement, automatic global library merge, universal third-party compatibility and every possible failure interleaving are not established by this acceptance.
