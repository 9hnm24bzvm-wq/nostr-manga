# Manga protocol: completion and qualification report

**2026-10-12 · accepted build 1c3f7769**

## Result

**Functional parity with the six recovered refactor commitments is accepted for the exact source and executable below.** Core publication, policy, federation, library/social and operator workflows are implemented. The local-first reference browser has model/interoperability coverage and a scoped native publication/outbox/recovery pass.

**Consistent target-load performance remains unresolved.** Earlier builds have both scoped passes and failed repeats. The latest waiting32 and waiting64 trials on 82c52 failed. The new read-holder diagnostics are functionally accepted but have not yet been used in a mixed-load trial. No throughput remedy, indefinite-capacity claim or completed full reader website follows from this result.

This report uses the [recovered requirements](PARITY-REQUIREMENTS.md), [architecture](ARCHITECTURE.md), [feature audit](FEATURE-COVERAGE.md) and exact acceptance evidence. Unavailable later source prevents byte-for-byte reconstruction claims. Earlier reports and failures remain frozen.

## Exact accepted build

- Node SHA256: `1c3f7769e25ea3bd9f025c0f3d6cad009d7d23de3babe36b1f90a661afc7d713`
- Browser: `e4f4862bd75c26bb`, IndexedDB schema 5
- Gate completed **2026-10-12 00:14:18.609957 UTC**, exit 0; **391 source/build inputs unchanged**
- **58 Python/operator, 6 packaging, 91 browser model, 74 Rust module cases, and 88 integration executables with 106 passing cases**
- Two intentional module ignores: a parent-invoked crash child and a separate large codec fixture not explicitly run by this invocation
- Formatting and strict all-target release Clippy passed; real PostgreSQL crash/restart passed with fsync and synchronous commit on

The run used offline installed dependencies and release artifacts. Reviewed evidence-only runner copies reduce four Cargo job settings from two to one and adjust relocated script paths; the original source scripts, full inventory, Python/package/browser steps and PostgreSQL settings remain unchanged. An independent reviewer verified the terminal log, all 391 inputs, node binary and runner identities. This is an independent audit of one execution, not a second execution.

[Canonical acceptance](../../results/rebuild-read-phases-final/canonical-acceptance.json), SHA256 `838f5cf89d252b09a91b2fcf702e4b6f5213da9f464b8396675b384c94dc03ec`; [independent audit](../../results/rebuild-read-phases-final/INDEPENDENT-AUDIT.md); [technical report](../../results/rebuild-read-phases-final/README.md).

## Recovered feature scope

- **Publication and reading:** signed immutable chapter manifests, verified retained originals, discovery/catalogue queries, explicit pagination and capabilities, and optional managed NGINX serving. Corrections require deliberate deletion/new publication rather than retargeting a signed identity.
- **Identity and policy:** independent account, publisher and peer authority; guarded HTTP/Nostr authentication; disabled-default enrollment; service continuity; local moderation and protected deletion.
- **Federation:** approved peers, verified content, bounded inventories, durable synchronization, cancellation and lease fences, temporary-failure handling and approved-source repair.
- **Libraries and social:** signed library heads, exact reading progress, series, lists and membership; comments/reactions/votes and group roles/publication/recovery.
- **Local browser and operation:** private edits, explicit publication consent, durable outbox, verified public recovery, local backups/schema migration; PostgreSQL migration/import, backup/restore, audit/repair and deliberate restored-instance reactivation.

Rust/Axum/Tokio and PostgreSQL remain authoritative, with deadpool-postgres/tokio-postgres, Moka, internal strfry projection, JavaScript browser code and Python operator workflows. Immutable caches and projections never replace current policy, ownership or generation checks. Full catalogue/chapter-reader/social/group/moderation screens remain outside the completed reference-client scope.

## The six refactor commitments

1. **Bounded read classes:** catalogue, library-head and live reads have separate caps under an overall database limit and bounded waits. Disconnect/deadline does not release unfinished physical work.
2. **Shared live dispatch:** one authoritative lookup per processed notification feeds bounded subscribers, preserving handoff, revocation, overflow and discontinuity behavior.
3. **Foreground/maintenance fairness:** one volume-work owner maintains foreground reserve, bounded background share and headroom-responsive cleanup; receipts and retained bodies keep independent bounds.
4. **Less database work:** compatible checks/refunds are combined, capacity reserve/settle is set-based and fixed blob queries reuse prepared statements, preserving transactions, lock order, rollback, quota and authority fences.
5. **Immutable validation reuse:** accepted descriptors/references avoid repeated decoding while current generation, health, quarantine and authorization are still checked; stale/cold paths fall back safely.
6. **Coherent head reads:** signed identity/body, negative health and projection visibility share one repository operation, with a public concurrent-replacement reproduction.

The [backend mapping](PARITY-REFACTOR-BACKEND.md) and [test-seam audit](PARITY-REFACTOR-TESTS.md) connect these commitments to caller-visible behavior. Changes consolidate lifecycle ownership; they do not claim every large module has been split or all SQL hidden behind a generic layer.

## New diagnostic evidence and its limits

The new build adds bounded admitted-holder observations under the existing exact opt-in `MANGA_READ_ADMISSION_DIAGNOSTICS=1`. Default-off avoids holder allocation, clocks and diagnostic atomic updates, while instrumented call sites retain a task-local absence check. Existing SQL, limits, response contracts and ownership remain intact.

Three classes/eight phases and fixed histograms provide current gauges and saturating completed-duration totals. A single scoped owner restores nested phases; spawned storage work is measured through its inclusive parent await. The existing nonblocking capacity-one sink retains one selected record per controller per second and a 2,048-byte JSON limit.

The passing public fixtures cover real database locks, pool exhaustion, unchanged HTTP429/retry behavior, default-off/unauthorized absence, stalled/broken output, disconnected DB work, actual storage deadlines, draining and recovery. Baseline 82c52 fails at the first missing public holder observation. Later classifiers did not each reach a baseline red; source/tests were prepared before execution clearance, so strict red-first chronology is not claimed. An initial candidate fixture setup failure was preserved and corrected solely by keeping its database-bound backend/storage identity across stage restarts. Five separate process-guard checks are harness evidence, not extra application-test totals.

Interpretation remains limited:

- Handler is residual wall time, not isolated CPU. Pool checkout includes creation/recycling; DB roundtrip includes client/network/server waits; storage includes metadata and admission work.
- Phase durations accrue only on exit/finish. Active unfinished spans are excluded from completed lifetime summaries.
- Draining means an explicit early-response deadline; it does not detect client disconnects.
- Independent atomic fields are not a transactional conservation snapshot. Operator status competes with and observes its own Catalogue admission.
- These observations do not independently identify PostgreSQL wait events or prove why a workload stalled.

[Diagnostic contract](../../results/rebuild-read-phases/PLAN-AND-LIMITS.md), [focused evidence](../../results/rebuild-read-phases/focused-acceptance.json), [fixture correction review](../../results/rebuild-read-phases/FIXTURE-IDENTITY-INDEPENDENT-REVIEW.md).

## Public seams, cleanup and measured improvements

Tests use actual HTTP/Nostr, PostgreSQL/strfry/NGINX, CLI/process behavior, browser public operations and independent codecs. SQL locks, leases, corruption and crashes arrange faults; private helper state is not the correctness oracle. The narrow migration correction also rejects unsupported top-level library fields across three real CLIs, preserving valid sparse read IDs, independent statuses and list positions without adding follow state.

Three obsolete ticket/packaging helpers were removed from active use only after byte-preserved retirement. Current test/packaging entry points remain. [Workflow inventory](WORKFLOW-HYGIENE.md), [migration evidence](../../results/rebuild-migration-input-review/README.md).

Measured optimizations have specific scope: native half-million-library family-count median **2,075.8 → 0.3 ms** in three shared-host samples; capacity batching removed **60 targeted writes across 15 reserve/settle pairs**; prepared blob lookups reduced **218 → 26 Parse operations**, retaining **218 Bind/Execute operations**. These are measured work reductions and a small browser timing diagnostic, not isolated sustained-throughput proof. [Native measurements](NATIVE-BROWSER-QUALIFICATION.md), [batching](../../results/rebuild-capacity-batching/REVIEW.md), [prepared queries](../../results/rebuild-blob-prepares/REVIEW.md).

## Browser qualification

The earlier native schema-5 upgrade/edit/reload/export verified **500,202 records** field-exact except the intended edit. This establishes local-library behavior, not large public publication.

A later **four-record/four-family flow in actual cloud Chrome** passed **17 checked conditions** against real PostgreSQL/strfry using unchanged browser assets and node 82c52: wrong-pin/absent-consent refusal, outage-persistent same-event outbox, explicit retry, signed projected head, unpublished local edit, declined recovery preserving an actual exported backup, and approved recovery plus reload/export matching every baseline field. Existing origin 18767 remained untouched; owned fixture services stopped.

The signer was a synthetic NIP-07 adapter. Native extension prompts, real accounts, mobile, eviction/disk-full and physical power loss are unqualified. The broader **30k/500k publication/recovery plan is paused on a pending action approval**; there is no large-publication pass. [Native browser report](../../results/rebuild-browser-publication/README.md), [acceptance](../../results/rebuild-browser-publication/acceptance.json).

## Performance qualification remains open

All performance results below are build-specific. No mixed-load run has occurred on 1c3f7769.

Earlier 1a45 instrumented storage8/storage12 runs passed the declared 420-second bounded full-offer contract. Strict-window completion remained false, and subsequent stage-profile-off/scheduler trials failed. Those passes do not establish repeatability or qualify this build.

On accepted 82c52, the waiting32 run started **all 1,522,563 offers** but returned **seven Catalogue HTTP429s**. All **50,400 saves**, all images and **128 exact heads** succeeded. Complete diagnostics identified **seven queue_full, zero wait_expired**. Save p95/p99 were **313.622/419.855 ms**. [Waiting32 report](../../results/library-node-mixed/parity-maintenance420-read-admission-001/READ-ADMISSION-REPORT.md).

The sole-factor waiting32→64 comparison also **failed**: **124 unsent saves**, **256 API HTTP429s, one original HTTP429 and two library-read HTTP429s**. All **50,276 dispatched saves**, all **1,296,540 nginx images** and **128 exact heads** succeeded. Diagnostics recorded Catalogue **90 queue_full/167 wait_expired** and Head **two wait_expired**. Save p95/p99/max were **406.021/599.444/2,222.767 ms**. No queue fix was adopted. This shared-host comparison does not prove the larger queue caused the slowdown. [Waiting64 report](../../results/library-node-mixed/parity-maintenance420-read-waiting64-001/WAITING64-REPORT.md).

Candidate-processing counts are not reclaimable-byte debt or proof of a leak. Closing drain is not indefinite steady-state evidence. Bounded completion tails do not excuse missing/rejected offers. CPU/RSS are observations; PostgreSQL RSS sums duplicate shared pages. Shared-host, loopback, hot-corpus 420-second runs do not certify WAN/TLS/cold storage, monthly/peak traffic, 100 TB capacity or a latency SLO.

## Recovery and next boundary

**A matching full 1c3f7769 checkpoint is being prepared; it has not yet been created, saved or delivered at this report freeze.** The prior full 82c52 archive and later completed-evidence addendum remain immutable. The full archive is 58,497,839 bytes / 818 members, SHA256 `0088210c96b9eda00044056df1d520fb65069bb6a5827747c0ef4356c3760ed4`. The evidence-only addendum is 2,129,855 bytes / 70 members, SHA256 `6e322b9cd12c0ba22247e661ecd610f5373a66b5cc76e4a1a2464549e3f9c85a`; neither contains the newly accepted read-phase source.

Both were saved/attached and locally authenticated/member-verified. Independent remote readback remains blocked; recipient download was not observed. Catbox checkpoint 12 remains the latest independently downloaded/decrypted/member-verified and restored/CLI-smoke-tested full checkpoint, on an older build. [Recovery index](CHECKPOINTS.json).

The new checkpoint must use the accepted source plus explicit safe evidence selections, excluding credentials, private keys, key-bearing fixture configurations and runtime databases. The separately prepared run-specific status adapter is outside this acceptance and recovery snapshot. Any later mixed trial needs its own reviewed adapter, unchanged workload contract and exact-build evidence. Functional completion is accepted; performance consistency and the stated product/browser boundaries remain open.
