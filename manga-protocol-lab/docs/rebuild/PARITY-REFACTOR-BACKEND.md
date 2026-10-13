# Backend parity and refactor audit

Initial audit: 2026-10-10 21:05 UTC. Implementation follow-up: 21:32 UTC. This is a requirement audit, not a perfection claim. Production and tests are frozen after focused acceptance; the new combined optimized gate is pending.

## Result first

**All six explicitly recovered refactor promises are now implemented and focused-accepted. The new combined optimized gate and target-load qualification remain pending.** The earlier canonical checkpoint did not satisfy the six promises; the audit exposed real gaps rather than merely stale documentation. Each has specific implementation and public-seam evidence below. The additional history-worker pacing regression was also reproduced and repaired. This establishes implementation progress, not full production capacity or byte-for-byte recovery of the lost October 10 tree.

The canonical optimized functional gate did pass at 20:38 UTC, with unchanged recorded inputs and node SHA256 `5b9464e8b635302eb8df55eaeaabf3e40eab647ef2188a925cd2d07730022e97`: 50 Python tests, 6 packaging tests, 42 browser-model tests, 75 Rust module tests with two intentional ignores, 75 integration executables, strict Clippy and independent-process PostgreSQL crash/restart. Source: `results/rebuild-final/canonical-acceptance.json`. This does not establish the newly recovered requirements, the later admission patch, production traffic, or 100 TB operation.

### Evidence boundaries

- Baseline: immutable recovered October 8 tree at `/workspace/scratch/3eafc9756ff3/recovery/restored-oct08/manga-protocol-prototype`. The missing October 10 source cannot be compared byte-for-byte. Earlier reported outcomes are historical evidence only.
- Requirements: `.scratch/federated-manga-postgres/spec.md`, its 39-ticket graph, API and reconstruction documents, and the recovered six-item promise, message `Sentinel_dac77d529b048191856c03a0e6dcf732` (2026-10-10 01:14:29 UTC), followed by the user's request to fix all of it. Historical letter-only answers have no additional inferred meaning.
- All 52 schema migrations match the October 8 baseline byte-for-byte. All 124 route literals found in the baseline still occur in the current tree; the extra `/redirected` literal is a test fixture. This is a useful regression screen, not proof of behavior or complete dynamic-route coverage.
- There are no missing baseline Rust source files. Removed generic queue/reservation/export convenience methods had no runtime callers after replacements. Their real sync, upload, reconciliation and fault/recovery replacements passed before removal; see `results/rebuild-queue` and `results/rebuild-capacity-seams`. Private body-load counters and test-only key-path helpers were removed or moved to fixture support. This is intentional cleanup, not lost protocol functionality.

## The six exact promises

| Promise | Current implementation and responsibility boundary | Focused acceptance and remaining qualification |
|---|---|---|
| **1. Configurable catalogue/library-head/live budgets, overall bound and burst waiting** | **Implemented.** `read_admission.rs:40,112,159` owns class/overall admission atomically, bounded FIFO waiting, timeout and owned permit release. `node/read_admission.rs` and relay callers acquire before authoritative reads. Class caps provide partial isolation, not reserved class shares. | `http_read_classes`, HTTP/Nostr contention and public admission lifecycle tests pass. Queued work holds neither active class nor overall capacity; already-issued DB work retains its credit until completion. Evidence: `results/rebuild-admission/parity-admission-dispatch.json`. New combined optimized gate pending. |
| **2. Shared live dispatcher, one source fetch, bounded queues, preserved policy and no immediate healthy close on busy reads** | **Implemented.** `relay_dispatch.rs:24,74,79` owns one lookup per relevant notification, filtered bounded per-session fanout, source ownership and explicit discontinuity. `RepositorySource` retains DB/admission ownership through actual server completion; relay policy checks remain authoritative. Transient read pressure defers delivery/policy work instead of immediately closing a healthy session. | Six real fixtures and seven public lifecycle tests pass, including two slow clients, history/fanout, revocation, feed loss/recovery and source/policy deadlines. The source deadline still fails closed, and overflow requires explicit resubscription. Evidence: `parity-admission-dispatch.json` and its linked logs; no target fanout-capacity claim. |
| **3. Fair storage, foreground reserve, smaller maintenance share and headroom adaptation** | **Implemented.** `storage_admission.rs` and FilesystemVolume own one active-work scheduler; upload-body and download-response limits remain distinct. For total≥2, background allowance reserves at least one foreground place; pressure can increase cleanup only below the total. Existing one-slot configurations retain FIFO progress rather than an impossible simultaneous reserve. Actual `fstatvfs` and charged/reserved quota headroom drive hysteresis. | Fifteen focused lifecycle cases and Clippy pass, including externally stalled cleanup, foreground staging, quota-pressure adaptation and cancellation. Actual filesystem free space is sampled but was not force-exhausted in these fixtures. See [storage fairness evidence](STORAGE-FAIRNESS.md) and its exact scheduling contract. No sustained/100 TB qualification. |
| **4. Compatible checks combined, grouped refunds, short transactions and lock order** | **Implemented.** Blob-generation locking remains a separate statement before fresh health/reference checks. Staging combines quarantine/reclaim checks; root commit combines grant/health checks; reclaim combines six eligibility decisions and four final receipt-protection checks. `capacity::refund_reservation_tx` owns reservation then sorted-grant locks and one idempotent grouped refund. Current accepted holds limit credit transfer to newly added generations. Independent backend imports, four-item retirement, sorted authors and generation-bound hold/blob locking remain intact. | Thirteen real lifecycle/credit/reclaim/crash/publication/projection fixtures pass in `results/rebuild-backend/parity-lifecycle-final.log`, exit0, plus all-target Clippy with `-D warnings`. Actual unlink-before-finalization crash recovery, no early/double refund and exact-capacity conservation remain covered. New full gate pending; no unmeasured SQL optimization speed claim. |
| **5. Reuse prior accepted immutable validation; unchanged objects cheaper without bypassing live fences** | **Implemented.** `library_references::ValidationCache` retains only bounded immutable facts for the exact previously accepted event and generations. Same verified root hash/size avoids repeated decode; unchanged family/bucket descriptors avoid summary reload. Every object still receives current receipt/grant checks, sorted generation locks, health/authority checks, aggregate/bucket validation and head/CAS/winner recheck. Cold/stale/evicted entries fall back conservatively. | Genuine blocked-summary/credit red→green through `LibraryObjects::commit`; cold/new descriptors wait, revocation and quarantine reject, and a reused-family plus restaged-generation race rejects stale preparation. Numeric diagnostics observe 0 summary requests/0 retention objects for warm unchanged roots, versus 1 summary request cold and 1 reused+1 requested in the mixed case. Evidence: `validation-reuse-profile-green.log` and `validation-work-observations.json`. No target throughput inference. |
| **6. Consistent signed-head identity/body/projection snapshot** | **Implemented.** `Repository::current_library_head` (`db.rs:463`) returns the signed current body, current negative health and projection flag from one primary statement. HTTP and library views share the same operation. Pinned multi-read views still verify their expected head and return a real conflict after replacement. | `library_head_snapshot` deterministically reproduced the old 404 by pausing the reader's DB snapshot while a real commit replaced the head, then passed with the exact coherent signed response and explicit view conflict. `head-history-green.log` also passes complete reader workflows and reclamation. New combined optimized gate pending. |

### Initial findings and the additional history regression

These are historical findings against the 20:38 canonical checkpoint, retained
so the repair is auditable. Line numbers below are from that initial source.

- Item 1 had a hardcoded shared four-slot `query_slots` and immediate try-acquire;
  correct placement before checkout did not provide class budgets or waiting.
- Item 2's `relay_feed.rs:13–15,38,88–94` broadcast only IDs, with per-session
  `relay.rs:83–93` lookups and busy errors closing sockets.
- Item 3 let default four-way cleanup consume every outer mutation slot
  (`upload.rs:22–26,537–624`) while another shared I/O semaphore remained in
  `storage.rs:62–65,96,186,398`. There was no filesystem-headroom policy.
- Item 4 had grouped retention refunds but separate compatible staging/reclaim
  checks; final cleanup re-locked/re-read and refunded each budget independently.
- Item 5 moved preparation outside the author transaction but still reloaded all
  immutable summaries and decoded identical roots (`library.rs:103–129`,
  `library_references.rs:27–100`). Avoiding unchanged hold writes alone was not
  equivalent to validation reuse.
- Item 6 performed the head-ID, visible event and projection queries separately
  in `node.rs:2978–3005`, with the same ID/body split in
  `library_view.rs:69–87`.
- History pruning selected 16 rows while `node.rs:770–775` continued immediately
  only for `Ok(64)`. The inserted 250ms sleeps imposed a ceiling below 64
  retirements/s. **Repaired:** `HistoryPrune` returns an application-used typed
  `more` result derived from the single private batch limit. The real background
  worker now drains full bounded pages without idle pacing. Its synchronized
  regression failed with a full 16-row page still due, then passed, preserving
  grace, current roots and the winner watermark.

## Requirement-by-requirement backend feature parity

**Retained** means source and named actual seam coverage are present, with the canonical checkpoint passing. It is narrower than exhaustive failure coverage. **Partial** identifies a known unimplemented requirement or remaining qualification. References below are `server/src/<module>.rs` and `server/tests/<fixture>.rs` unless otherwise stated.

| Spec story | Verdict | Source / actual seam evidence and qualification |
|---|---|---|
| 1. Same small-to-large architecture | Retained architecture; scale unproven | `db`, `storage`, managed nginx; no SQLx/SQLite runtime replacement. Real 100 TB operation not tested. |
| 2. Multi-process authority | Retained | PostgreSQL transactions/leases; `postgres_foundation`, real sync-worker `multiprocess.py`, crash/restart. |
| 3. Authenticated hash/size-verified originals | Retained | `upload`, `publisher_authority`, `storage`; `http_upload`, `http_publication`, `http_real_images`. |
| 4. Durable reservations/backpressure | Retained; scheduling repair focused-accepted | `capacity`, `storage_accounting`; `capacity`, `credit_lanes`, `reclamation_parallelism`. Foreground reservation/headroom repair now has the focused item 3 evidence above. |
| 5. Stable service key across domain changes | Retained | `service_proof`, `service_announcement`, `endpoint_policy`; service/remembered identity, relocation and TLS fixtures. No automatic trust from possession. |
| 6. Idempotence/interrupted-write recovery | Retained | generation receipts, real UploadStore lifecycle; publication/staging fences and crash fixtures. |
| 7. Complete locally retained originals | Retained | `publication`, `health`, `public_export`; real-image source-stop/API/nginx equality and corruption withdrawal. |
| 8. Signed immutable manifests | Retained | `manifest`, `library_manifest`, publication signatures; independent codec vectors and public publication tests. Experimental wire profile remains explicit. |
| 9. Delete then new upload, no revision links | Retained | `moderation`, tombstones; `http_moderation`, `http_real_images`, no inherited progress identity. |
| 10. Local accepted formats/policy | Retained | `StorageConfig`, decoder validation and publisher/read policy; configuration/HTTP negative tests. |
| 11. Preserve original bytes | Retained | four real JPEGs, 4,646,237 bytes; receiver API/nginx equality, no recompression. |
| 12. HTTP verified-manifest discovery | Retained | `catalogue`, `discovery`; catalogue reindex/recovery and discovery HTTP fixtures. |
| 13. Offset/limit/timestamp pages | Retained | explicit ordering, live pagination; `http_discovery`, `http_catalogue`. Multi-request snapshot is not promised. |
| 14. Exact case/NULL/RFC4647 semantics | Retained | actual HTTP matching and independent protocol vectors; no inferred case folding for other fields. |
| 15. Explicit query-budget errors | Retained; class isolation focused-accepted | `query`, `filter`, admission; discovery/count errors and DB-outage HTTP tests. The stronger class-isolation promise is covered by item 1 above. |
| 16. Local approval for protected deletion | Retained | `moderation`, protected-kind exclusion from raw strfry import; `http_moderation`, actual backend scan disappearance. |
| 17. Shared originals/tombstones | Retained | `reclamation`, `publication_reclamation`; shared-reference, crash-after-unlink and replay refusal fixtures. |
| 18. No series cascade | Retained | moderation/ordinary deletion tests; independent chapter remains readable. |
| 19. Explicit approved peers/selection | Retained | `peers`, inventories and sync; two-node HTTP/NIP42 peer fixtures, bilateral approval and scope rejection. |
| 20. Independent instances/no central membership | Retained architecture | local admission/keys, no central identity service. Default local role policy still applies. |
| 21. Approved repair/fallback verification | Retained | `health`, `peers`; corruption repair, relocated approved source, exact receiver originals. |
| 22. Bounded inventories/resumable jobs | Retained | `inventory`, sync jobs/schedules; durable real queue and HTTP inventory tests. |
| 23. Stale/cancelled lease fencing | Retained | sync lease generation/token methods; `sync_queue`, `sync_schedules`, multiprocess replacement tests. |
| 24. Corruption detection/withdrawal | Retained | generation-bound read observation, audit/quarantine; storage lifecycle and public serving shutdown-unconfirmed tests. |
| 25. Personal role keys/group-signed API | Retained | `groups`, purpose-specific HTTP; group role/profile/retry tests and restored new publication. |
| 26. Persistent revocation | Retained | transaction authority fences, restart bootstrap tests; group/account/enrollment fences. |
| 27. Key backup/migration | Retained | actual key CLI and `http_migration`; key-only restore requires explicit local roles. No key-erasure endpoint promised. |
| 28. One public library/progress/lists | Retained; consistency repair focused-accepted | `library`, `library_view`, all four record families; 30k/500k transfer and reader fixtures. Item 6 now has a focused concurrent-replacement regression. |
| 29. Comments/reactions/votes | Retained | `social`, ordinary deletion; signed successful up/down/custom/empty reactions, exact pagination and dependency-aware federation. Ranking is not promised. |
| 30. Anonymous reading/local access policy | Retained | read policies and independent peer read policy, HTTP/Nostr denial and outage cases. Item 2 live-burst/deadline/revocation behavior now has focused acceptance. |
| 31. MangaDex reviewed migration | Retained supported subset | Python operator import/mapping CLI, compatibility aliases and real restored originals/library. Arbitrary unchanged-client/password migration is not promised. |
| 32. Schema/projection/backup recovery | Retained | 52 explicit migrations; independent backend acknowledgements; real operator restore/reactivation. History pacing regression is repaired and focused-accepted. |
| 33. Caches fail closed/current authority | Retained; live routing focused-accepted | Moka immutable bodies plus primary visibility; exact events through DB outage/restart, health/revocation tests. Shared dispatcher retains these checks in the focused fault tests. |
| 34. Bounded/observable resources | Implemented; combined gate pending | Download count/bytes/lifetime, cleanup oldest-due, pools and task health are retained; class admission, storage fairness/headroom and shared dispatch have focused acceptance above. Target-load and full deployment resource qualification remain separate. |
| 35. Honest cold/warm/write/failure measurement | Partial qualification | functional faults and bounded load diagnostics exist. Monthly target run saturated the single-core Python generator; sharded generator is under separate qualification. No production traffic/100 TB certification. |

### Later ticket requirements not implied by the original 35 stories

- **T17 / Q93D / Q94A:** explicit order and optional exact totals with count-budget error: retained and tested. Only these mapped answers and Q95C are interpreted.
- **T19–21:** independent publisher grants, expiring/group staging leases, durable reclaims, typed sync skips/continuation: retained through `http_publishers`, `staging_reclamation`, `http_sync_skips`, lease/race fixtures.
- **T22–24:** bounded Blossom subset/CORS, capability document, signed endpoints/first-key remembrance/possession-versus-trust: retained through the corresponding real HTTP fixtures. Universal extension compatibility is not claimed.
- **T25:** explicit native multi-relay library publish/retry/cold recovery: retained through `http_library_client` and negative cases. Browser signer extension qualification is a separate gap.
- **T26–28:** catalogue rich metadata/aliases, private moderated submissions, documented MangaDex compatibility subset: retained with public positive/negative tests, private quota and approve/reject/expiry outcomes.
- **T29:** restored serving/signup/schedules remain paused; foreign namespace/volume protections: retained through operator and complete restore/reactivation fixture.
- **T30:** the six missing refactor implementations and specific seam violations are repaired with focused evidence; complete current-source optimized acceptance, measured load qualification and final recovery checkpoint remain pending. A graph status cannot close those gates.
- **T31–36:** reader compatibility snapshot pinning/budgets; group profile CAS/audit; persisted retry identity; optional closed-default enrollment; independent approved-peer WebSocket policy; denial versus operational DB errors: retained functional coverage. Signed-head and live-admission repairs now have the focused regressions above; the whole-source gate is still required.
- **T37–39:** functional local-first browser, explicit publish/recovery, rendered desktop/mobile qualification: local models and real supported cloud-browser local flows pass; the newer picker-filter build has model/DOM acceptance. Actual signer extension, mobile, eviction/storage-pressure and large native-browser responsiveness remain unqualified. Reader-site presentation, manga image-reader/social/moderator UI design was intentionally deferred; do not label it a restored backend regression or silently erase the mobile test requirement.

## Deep-module and seam assessment

Method used: [codebase-design](https://raw.githubusercontent.com/mattpocock/skills/main/skills/engineering/codebase-design/SKILL.md) and [deepening guidance](https://raw.githubusercontent.com/mattpocock/skills/main/skills/engineering/codebase-design/DEEPENING.md). A useful module hides lifecycle, ordering, error and performance rules behind the same interface application callers use. Fewer files are not inherently deeper, and moving pass-through methods into more files does not improve ownership. Dependency boundaries here are in-process domain code; substitutable local PostgreSQL/filesystem/strfry/nginx processes; approved remote peer protocol; and genuinely external browser/signer/deployment environments. Test real local dependencies as agreed, and preserve observable coverage before retiring internal tests.

Good depth already present:
- `PreparedReferences` keeps immutable root validation outside the author transaction while preserving generation/health/grant fences inside it; no hypothetical trait hierarchy was introduced.
- Filesystem owned operations plus bounded response snapshots retain resources until actual I/O/body completion; cancellation does not falsely refund or free an in-flight task's ownership.
- Backend verified-import acknowledgement is separated from bounded author retirement on its advisory session. Per-event `projection_complete` explicitly does not promise whole-queue drain.
- Actual sync lifecycle replaced demonstration queue APIs; fixtures now use the same lease/error/cancel outcomes as workers.

Ownership improvements from this audit:
- One volume-work admission owner replaces competing work-budget ownership;
  upload body receipts and retained download bodies remain independently bounded.
- Signed-head identity/health/projection now belongs to one repository operation.
- History maintenance returns continuation explicitly instead of leaking a SQL
  page-size constant to its background caller.
- Shared dispatch owns filtered fanout, queue loss and source work lifetime;
  session policy continues to own authoritative permission checks.
- Previous-root validation owns its bounded immutable cache, while capacity owns
  whole-reservation refund ordering. Neither exposes test-only state accessors.

Remaining maintainability trade-off: `Repository.pool` still exposes raw SQL to
many application modules. A wholesale generic-repository layer is not justified
by this audit. Continue consolidating actual duplicated lifecycle ownership when
touching those paths; do not mechanically split node.rs or add hypothetical
single-implementation abstractions merely to change file counts.

**Repaired seam-only finding:** the old `relay_feed.rs:122–143` private-field/ResetOnDrop/AbortOnDrop test has been retired after real application-used Feed loss/recovery/cancellation coverage passed (`results/rebuild-admission/live-dispatch-green.log`). The final focused dispatcher gates also passed; combined optimized acceptance remains pending. Most configuration/codec methods are valid seams even when `pub(crate)`; the visibility keyword alone does not determine legitimacy. SQL used to arrange an external crash/lock fault is distinct from SQL table counts used as correctness outcomes.

## Remaining implementation and completion criteria

1. Run the new frozen-source optimized functional gate across all current inputs.
   Focused passes are established for all six promises; they are not a substitute
   for the combined run.
2. Qualify the sharded generator and run accurately identified mixed workloads.
   Keep the earlier scheduling-drop evidence; zero HTTP failures is not success
   when work was dropped before dispatch. Production traffic/storage targets
   remain unproven.
3. Preserve the final actual source in a newly verified encrypted checkpoint.
   Earlier archive verification does not protect subsequent edits.
4. Keep native browser/signer/mobile/storage-pressure and physical-disk
   exhaustion qualifications explicit. Deferred reader-site presentation is a
   separate product boundary, not an absent backend implementation.

The lost October 10 bytes remain an **unknown recovery limit**. The six known
implementation gaps and history mismatch have **specific focused repairs**;
remaining acceptance/capacity limits are recorded separately.

## Focused implementation follow-up

The history above preserves the defects found in the earlier canonical checkpoint.
The following current changes are not retroactively covered by its earlier gate.

- **Item 6 implemented and focused green:** `Repository::current_library_head`
  reads head identity, signed body, current negative health and per-event
  projection status in one primary statement. HTTP and library views share it;
  no private pass-through remains. `library_head_snapshot` uses a disposable
  reader-schema delay and real concurrent commits to reproduce the old 404, then
  observes the exact coherent signed response and preserved pinned-view conflict.
  Red: `head-snapshot-red3.log`; green: `head-history-green.log` (0.93s).
- **History pacing implemented and focused green:** `HistoryPrune` returns
  `removed` and `more`, computed from one module-owned batch limit. The real node
  continues full pages without guessing that limit. Red: a full 16-row page still
  remained after the synchronized two-second progress bound; green: no due page
  remained. `library_history_worker` passes (5.11s including 258 real commits and
  backend retirement). This is a maintenance progress regression, not a traffic
  capacity claim. Bounded transaction, grace, winner watermark, reclamation and
  complete reader workflow tests also passed in `head-history-green.log`.
- **Item 5 implemented with focused green:** bounded previous-accepted validation
  reuses only immutable facts; current generation/authority/health and whole-root
  limits/bucket placement remain checked. `library_validation_reuse` originally
  blocked behind immutable-summary/unchanged-credit storage; it now completes,
  while new descriptors and cold instances still wait. Revocation and observed
  child corruption reject. `library_prepared_references` was strengthened so an
  accepted unchanged family is actually reused while another prepared generation
  is reclaimed/restaged; the stale commit is refused and the retained family stays
  readable. Both passed in `validation-reuse-profile-green.log` (0.41s/0.39s).
  Diagnostic observations show 0 summary loads and 0 new retention objects for
  an unchanged warm root, 1 requested summary on cold start, and 1 reused plus 1
  requested summary for the mixed changed-family case. See
  `results/rebuild-backend/validation-work-observations.json`. No latency/target
  throughput improvement is inferred from these small fixtures.
- **Item 4 implemented and expanded focused gate green:** blob locking remains its own
  statement before fresh checks. Staging quarantine/reclaim checks and root
  grant/health checks are combined. Reclaim's six protection decisions share one
  post-lock snapshot; cleanup finalization's four checks share another. A single
  `capacity::refund_reservation_tx` owns reservation then sorted grant locks and
  grouped, idempotent refunds for every outstanding charge. The old per-budget
  helper is preserved outside compiled source. Current accepted holds limit
  staging-credit transfer to genuinely added generations. The thirteen-fixture
  credit/reclamation/crash/publication/projection gate passed with exit0, as did
  all-target Clippy (`parity-final-clippy.log`, 28.85s). The new combined optimized
  gate remains pending.

Cache limits, cold behavior and opt-in numeric diagnostics are documented in
[the library concurrency contract](../library-concurrency.md). All six promises
still require the final consistent-source functional gate and separately honest
load qualification; lost historical source is not used as a substitute.
