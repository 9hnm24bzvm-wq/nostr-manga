# Manga protocol parity and qualification

Snapshot: 2026-10-12 02:09 UTC, after the pool-lifetime acceptance gate and bounded helper qualification.

**The recovered feature scope and six refactor commitments have functional acceptance on the current build. Repeatable target performance remains unresolved.** This report records the accepted changes, their public regression evidence and the remaining browser, performance and recovery limits. It preserves the earlier dated reports and their build-specific results.

The scope comes from the [recovered requirements](PARITY-REQUIREMENTS.md), [architecture](ARCHITECTURE.md) and [feature audit](FEATURE-COVERAGE.md). Missing later source prevents a byte-for-byte reconstruction claim.

## Exact accepted build

The canonical-equivalent release gate completed at **2026-10-12 02:00:32.753093 UTC**, exit 0, with **392 source/build inputs unchanged**:

- 58 Python/operator checks and 6 packaging checks
- 91 browser model checks
- 74 Rust module cases, with two intentional ignores
- 89 integration executables and 108 passing integration cases
- Formatting and strict all-target Clippy with warnings denied, completed in 43.79 seconds
- Real PostgreSQL multiprocess immediate-stop/restart, fail-closed outage handling and fresh-process recovery reads, with fsync and synchronous commit on

Node SHA256: `4c499f724af4db1843f54c51e47746114f777de15b446979a3e8e8a0e7a3e23a`. Browser assets remain `e4f4862bd75c26bb`, IndexedDB schema 5. The two module ignores are a crash-parent-invoked child and a separately invoked large codec fixture; they are not additional passes in this gate.

Reviewed evidence-only runner copies use single-job Cargo builds and adjusted paths. Original scripts, Python/package/browser steps, fixture selection and PostgreSQL settings are unchanged. The independent audit checks this single execution; it does not claim a second execution.

[Acceptance](../../results/rebuild-pool-lifetimes-final/canonical-acceptance.json), SHA256 `656aa030c83f239bde726813cefb6f6c49ffe9d3cd55e18ca1be38f9779cd6d5`; [independent terminal audit](../../results/rebuild-pool-lifetimes-final/INDEPENDENT-AUDIT.md); [technical report](../../results/rebuild-pool-lifetimes-final/README.md).

## Recovered features and refactor commitments

The backend supports signed immutable chapter publication, retained verified originals, discovery/catalogue reads, pagination and capabilities, optional managed NGINX delivery, independent account/publisher/peer authority, authentication, moderation and protected deletion. Federation includes approved peers, bounded inventories, durable synchronization, lease/cancellation fences and approved-source repair. Signed library heads preserve exact reading progress, series, lists and membership; social and group operations preserve their authority and publication boundaries.

The local browser supports private edits, explicit publication consent, durable outbox, verified recovery and local backup/schema migration. Operator workflows cover PostgreSQL migration, backup/restore, audit/repair and deliberate restored-instance reactivation. Full catalogue/chapter-reader, social/group and moderation frontends remain outside the completed reference-client scope.

The six commitments remain implemented and tested:

1. Separate bounded Catalogue, Head and Live read classes under an overall database limit, without releasing unfinished physical work on early response.
2. Shared live dispatch with bounded subscribers and preserved handoff, revocation and overflow behavior.
3. Foreground/maintenance fairness with foreground reserve, bounded background work and independent retained-body/receipt limits.
4. Reduced database work through combined operations, set-based capacity reserve/settle and prepared fixed lookups, preserving rollback, lock order and authority fences.
5. Immutable validation reuse while checking current generation, health, quarantine and authorization.
6. Coherent signed-head reads across identity, body, health and projection visibility.

Rust/Axum/Tokio and PostgreSQL remain authoritative. Moka caches and strfry projections do not replace current authority checks. [Backend mapping](PARITY-REFACTOR-BACKEND.md) and [public-seam audit](PARITY-REFACTOR-TESTS.md) connect these commitments to observable behavior.

## New connection lifetime corrections

Three narrow changes release pooled clients before work that no longer needs them:

- Storage audit releases cursor clients before nested repository checkouts and filesystem work, retaining its audit mutex and cursor/health decisions.
- Compatibility collection reads release their committed selection client before cover, profile and alias lookups. Their selected rows and authorization remain unchanged.
- Library commit releases its client after successful transaction commit, before non-authoritative Moka maintenance.

With a real one-connection pool, the identical public HTTP test executable first observed the preserved 1c3 build return **400 instead of 200 for signed empty audit**, and **503 instead of 200 for both nonempty cover and author collections**. The candidate passed both cases, including authorization, publication, relationships, cursor wrap and exact original-byte controls. Later audit assertions not reached on the baseline are candidate controls, not separate causal failures.

Six existing public regressions also passed before the full gate. The Moka change has source-proven lifetime reduction and public validation/root/head regression coverage; no deterministic cache timing failure is invented. Patch and fixture preparation preceded execution clearance, so strict red-first chronology is not claimed.

No pool size, queue, retry policy, SQL statement or managed-public-export transaction/filesystem fence changed. Audit was disabled and these compatibility collection routes were absent from the failed mixed trial. The fixes do not establish its cause or a throughput improvement. [Focused report](../../results/rebuild-pool-lifetimes/README.md), [independent review](../../results/rebuild-pool-lifetimes/INDEPENDENT-ACCEPTANCE-AUDIT.md).

## Test coverage and measured work reduction

Tests exercise real signed HTTP/Nostr, PostgreSQL/strfry/NGINX, CLI/process behavior, browser public operations and independent codecs. Faults use controlled locks, leases, corruption and crashes; private helper results are not the correctness oracle. The migration correction rejects unsupported top-level library fields across three actual CLIs while preserving supported sparse data. Three obsolete ticket/packaging helpers were retired with preserved bytes; active test and packaging entry points remain. [Workflow inventory](WORKFLOW-HYGIENE.md), [migration evidence](../../results/rebuild-migration-input-review/README.md).

Measured effects remain narrowly stated: capacity batching removed **60 targeted writes across 15 cycles**; prepared lookups reduced **218 Parse operations to 26**, retaining 218 Bind and Execute operations; a native half-million-library diagnostic reduced family-count median **2,075.8 to 0.3 ms** across three shared-host samples. These measurements do not isolate sustained throughput. [Batching](../../results/rebuild-capacity-batching/REVIEW.md), [prepared queries](../../results/rebuild-blob-prepares/REVIEW.md), [native measurements](NATIVE-BROWSER-QUALIFICATION.md).

## Latest performance evidence

**No mixed-load run has occurred on the accepted 4c499 build.** The latest completed 420-second trial used its accepted 1c3 predecessor, storage12, waiting32, admitted-read diagnostics enabled and stage profiling off.

That trial **failed**: **1,522,563 planned operations**, **1,521,591 started**, **1,521,274 acknowledged**, **317 HTTP429 failures** and **972 unsent saves**. All **1,296,540 nginx image offers** succeeded. Saves were **49,428 started, 49,414 acknowledged and 14 rejected**. All 128 projected heads matched their last known acknowledged values, but the harness retained **14 conservative ambiguity flags**, so this is not a clean-head qualification.

The 14 save responses were `library_busy/work_queue_timeout`, before those requests started their head transactions. Earlier inline staging or authentication side effects remain possible. The harness poisons every failed-save reader and suppressed 825 later offers; its ambiguity flags do not demonstrate 14 uncertain head commits or data loss.

At the first rejection wave around 355.378 seconds, the selected diagnostic showed three Catalogue holders and one Head holder in pool checkout. Complete read counters matched 234 Catalogue queue-full, 66 Catalogue wait-expiry and three Head wait-expiry responses. Five-second pool/PG samples missed the brief interval. Fast healthy-connection recycling performs no PG health query; checkout still combines availability/scheduling and possible closed-connection replacement. Aggregate PG states, checkpoint overlap and process CPU changes do not identify an application query, blocker or causal host stall.

The fixture ended within its observation budget with an all-clear reporting guard. Normal cleanup was attempted; PostgreSQL shutdown is supported by its log and absent pidfile. Complete node/NGINX descendant termination was not independently verified. The separate-exec manual stop path could not see the launch PID, so it is not validated fallback control. The separate helper qualification below addresses this future observation/control boundary; it does not retroactively strengthen the old cleanup proof or establish a performance cure.

[Final phase-trial report](../../results/library-node-mixed/parity-maintenance420-read-phases-001/READ-PHASES-REPORT.md), SHA256 `8a0b81d8673eeaefa4594bdadcc68b402cac867646c1c3a74c78deb456c9e29c`; [independent diagnostic audit](../../results/library-node-mixed/parity-maintenance420-read-phases-001/INDEPENDENT-DIAGNOSTIC-AUDIT.md).

Earlier 82c52 waiting32 and waiting64 trials both failed; no queue-size fix was adopted. Earlier 1a45 storage8/storage12 instrumented trials passed their bounded full-offer contract, but later repeats failed. These build-specific passes do not establish consistency. Candidate counts are not reclaimable-byte debt, and closing drain does not prove indefinite sustainability. PostgreSQL RSS sums duplicate shared pages. Shared-host, hot-corpus loopback trials do not certify WAN/TLS/cold storage, monthly/peak demand, 100 TB capacity or a latency SLO.

## Bounded diagnostic helper qualification

A separate fixed-output 100 ms PostgreSQL observer and same-launch process owner passed **42 independently audited helper checks**. One persistent official psql connection uses fresh autocommit snapshots, 75 ms server/200 ms client deadlines, fixed numeric fields, a 480-second lifetime and 24 MiB retention. The owner bounds console output, handles cancellation and anchored-child cleanup, and retains failed ownership proofs rather than inferring control over another namespace.

Three warmed off/on pairs each used fixed 2-second warm-up and 10-second measurement windows. All **6,000 measured SELECT offers** completed, with **100/100 measured samples** from each enabled observer. Controller CPU was **0.4–0.7% of one core**, psql **0–0.2%** at 10 ms tick resolution; zero ticks do not mean zero cost. Workload p95 changes were **−7.50%, −2.06%, +9.20%**. These small controlled measurements do not predict mixed-load overhead. Actual next-load wiring and adequate disk headroom remain prerequisites. [Helper qualification](../../results/rebuild-mixed-prep/FAST-PG-QUALIFICATION-003.md), [independent audit](../../results/rebuild-mixed-prep/FAST-PG-INDEPENDENT-AUDIT-003.md).

## Browser and recovery limits

Native local schema migration/edit/reload/export previously verified **500,202 records**. Separately, actual cloud Chrome passed a **four-record, 17-condition** publication/outage-outbox/retry/signed-head/recovery flow against real PostgreSQL/strfry on 82c52, with unchanged browser assets. The signer was synthetic NIP-07; native extension prompts, real accounts, mobile eviction, disk-full and physical power loss remain unqualified. The broader **30k/500k publication and recovery plan is paused on a pending action approval**. [Native publication report](../../results/rebuild-browser-publication/README.md).

A matching full 4c499 checkpoint is being prepared and has not been created or delivered at this report freeze. The latest delivered full archive covers accepted 1c3: **62,037,185 bytes and 1,096 members**, SHA256 `37ca5ee2e0f5020d9c7b69acc0dc7ca71a270bdd42c461ee45752ab990fb14e3`. Its eight ciphertext parts, guide and report were saved and attached on 2026-10-12 at 00:27:25 UTC. Local authenticated-member and exact reassembly checks passed. It predates these lifetime changes and the phase-trial evidence.

Independent remote readback remains blocked/unverified; recipient download was not observed. Catbox checkpoint 12 remains the latest independently downloaded, decrypted, member-verified and restored/CLI-smoke-tested full checkpoint, on an older build. New packaging must use explicit safe evidence selections and exclude credentials, private keys, key-bearing fixture configurations and runtime databases. [Recovery index](CHECKPOINTS.json). Later delivery notes belong in current status; these frozen report contents remain unchanged.
