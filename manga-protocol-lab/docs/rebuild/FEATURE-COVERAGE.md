# Reconstructed feature coverage

## Current status

See [the authoritative current rebuild summary](STATUS.md#current-status) for the current functional gate, mixed-load outcomes, checkpoint verification and remaining qualification. The separately dated sections below are historical evidence for their stated builds/times; they do not override that summary. The [architecture map](ARCHITECTURE.md) describes current source boundaries.

## Verified optimized acceptance, 2026-10-10 22:40 UTC

The final complete gate passed exit0 with unchanged recorded inputs:55 Python,6 packaging,91 browser model checks,74 Rust module cases (2 intentional ignores),all84 integration executables,strict all-target Clippy and PostgreSQL multiprocess crash/restart with durable settings. Exact node SHA256: `478ffc3243281a8e1e0c756eaf91b8923d740a2b2a146833b61443a837e73b54`. Browser build: `e4f4862bd75c26bb`. See `results/rebuild-parity-final/canonical-acceptance.json`.

All six explicitly recovered refactor promises have implementation and acceptance evidence. The feature audit found no additional missing required core family. Native Chrome schema4→5/save/reload/export preserved all500,202 records field-exact; three-sample count latency median fell from2075.8ms to0.3ms on this shared host. The separately dated sections below are historical, not the current completion state.

Matching encrypted checkpoint12 was uploaded, downloaded, all1,487 members authenticated/decrypted/verified, restored, and its binaries CLI-smoke-tested. See CHECKPOINTS.json and results/rebuild-parity-final/checkpoint12-recovery.json. Mixed-load/target traffic,100TB production operation,native signer extension,physical mobile/eviction/power-loss qualification remain open. Missing later source prevents exact byte parity; this gate does not promise perfection or every possible failure covered.


## Current reconstruction evidence, 2026-10-10 22:28 UTC

**The current frozen-source optimized aggregate is pending, not accepted.** The
latest attempt started at 22:25:32 UTC after an npm registry-metadata interruption;
the installed-dependency run is now fully offline. Follow
`results/rebuild-parity-final/canonical-release.log`. At this update,
`canonical-result.json` still records the earlier 21:51:19 exit101 attempt and
must not be mistaken for the current run's terminal result. Final acceptance
requires the new terminal exit0 and unchanged recorded inputs.

- Current browser build **`e4f4862bd75c26bb`**, schema5, passed **91 model checks**,
  build and formatting: [family-statistics acceptance](../../results/rebuild-family-statistics/acceptance.json).
- Actual native cloud Chrome upgraded the existing schema4 library, saved and
  reloaded progress, and exported **500,202 records** (500,000 chapters, 100 series,
  2 lists, 100 memberships). Every key/field matched deterministic input except
  the deliberate page99/UI timestamp edit. The downloaded export is 64,861,650
  bytes: [exact native export](../../results/rebuild-browser/native-500000-schema5-export.json).
  The earlier 30,202-record native round trip remains scoped to its earlier build.
- Repeated public `counts()` calls on the same native half-million library had
  a three-sample median of **0.3ms**, versus **2075.8ms** before the family-statistics
  change: [diagnostic](../../results/rebuild-browser/native-counts-improved.json).
  This is shared-host, three-sample evidence, not isolated capacity or end-to-end
  large-library publication performance.
- HTTP disconnect/nested-deadline ownership focused acceptance passed in **26.39s**:
  [ownership evidence](../../results/rebuild-admission/http-work-ownership.md).
  Storage fairness passed all four cases in three fresh release runs (**12/12**)
  after fixture isolation; these focused passes do not replace the aggregate.
- The six recovered refactor commitments have implementations and focused
  evidence: [contract](PARITY-REQUIREMENTS.md), [backend mapping](PARITY-REFACTOR-BACKEND.md).
  No newly missing required core API feature family was identified in this
  reconciliation. The earlier composed restore/audit/repair/group-publication/
  reviewed-serving/scheduled-sync coverage is present in
  `results/rebuild-feature-seams/acceptance.json`; it is not an untouched feature hole.
- The obsolete workflow helpers are retired with preserved originals and narrow
  CLI/package evidence: [inventory](WORKFLOW-HYGIENE.md). The current full run must
  validate the frozen final inventory and all later ownership/client changes.

Remaining acceptance boundaries: terminal full-source parity, a verified final
checkpoint with matching executable and current browser artifacts, native signer-
extension publication/recovery, native large public-graph publication, mobile,
quota eviction/power loss, and target traffic/100TB operation. Native local500k
upgrade/save/reload/export is **no longer pending**. Full catalogue/chapter-reader,
social/group/moderation frontends remain deliberately deferred implementation,
not lost backend regressions; this is not a complete MangaDex successor website.
The missing later source prevents byte-for-byte reconstruction claims.

Earlier sections below are dated historical snapshots. Their old builds, test
counts, blocked-browser probes and pending-feature wording do not override this
current summary or newer build-specific evidence. No tests or source changes
were performed for this documentation reconciliation.

## Latest acceptance, 2026-10-10 20:38 UTC

The final frozen optimized functional gate passed, exit0, unchanged inputs. See `results/rebuild-final/canonical-acceptance.json`. The reconciled feature review is `results/rebuild-final/feature-coverage-review.md`; its then-pending canonical gate is now complete, while its browser/deployment/load qualification limits remain. The sections below preserve the earlier reconstruction chronology.

## Earlier update, 2026-10-10 19:32 UTC

The 19:15 audit below is retained as a **historical reconstruction snapshot**. Its `restoredunverified` labels and then-missing browser/projection/download work must not be read as current results. Use [ACCEPTANCE-MATRIX.md](ACCEPTANCE-MATRIX.md) for executable seams and precisely bounded current evidence.

Since that snapshot:
- Python/operator/codec and safe packaging acceptance passed **50 + 6 tests**, including eight real-nginx lifecycle cases (`results/rebuild-operator-final.json`). Real two-node HTTP/PostgreSQL/strfry import and Python backup/restore also passed (`results/rebuild-queue/http_migration.log`, `backup-restore.json`). Restored activation/repair composition remains additional coverage.
- Browser bounded version-two backup/staged atomic restore, durable outbox/cancellation, signing fences and build promotion are implemented; the completed model gate passed **38 tests** (`results/rebuild-client/054-full-model-gate.log`). These are Node/DOM/IndexedDB-model results. Later large-backup qualification is separate.
- Prepared library-reference generation fencing, independent projection acknowledgements and bounded cleanup have current real lifecycle passes (`results/rebuild-backend/core-regression.log`). The earlier “queued” projection note is superseded by these logs; no performance conclusion follows.
- HTTP/Nostr pre-database admission, retryable account-store outages, and separate download count/byte/deadline behavior have green targeted logs under `results/rebuild-admission/`. Slow-reader route-specific coverage is not comprehensive.
- Actual-browser acceptance is **pending**, not wholly environment-blocked: after a standalone Chromium launch failed with EPERM, the parent found the supported already-running cloud browser and began testing snapshot5. No pass is inferred before those results.
- Public codec/discovery/lifecycle replacements and actual sync/process recovery have focused current passes. They do not establish an uninterrupted final combined gate.
- Earlier reconstructed encrypted checkpoints have been verified and delivered; consult [CHECKPOINTS.json](CHECKPOINTS.json). Checkpoint5 copying is complete, but this audit does not assert its upload/delivery outcome or final release completion.

No required core backend feature family was found wholly absent. Concrete remaining coverage includes operator catalogue reindex recovery, successful downvote/plaintext and exact social pagination, restored-instance reactivation/repair, representative slow-reader route variants, and actual-browser durability/publication. The matrix assigns these separately from deliberately deferred full-reader-site design and unqualified production capacity.

### Follow-up at 19:38 UTC

Two targeted coverage holes are now closed without production changes. The complete `http_catalogue` fixture passed with operator reindex recovery, authorization refusal, exact metadata/event/original assertions and no private-table polling (32.30s). The complete `http_publication` fixture passed successful downvote/empty/custom reactions, signed plaintext preservation and exact social pagination/time bounds (2.50s). Logs: `results/rebuild-feature-seams/{catalogue-first,social-first}.log`, both exit0. These are additional regression coverage, not invented red→green fixes. Targeted Clippy also passes (`clippy-final.log`), after a separately owned shared test-helper lint was corrected; the first lint failure is preserved. Seven public filesystem lifecycle tests and separate health/configuration cases also passed under `results/rebuild-admission/`; representative slow-reader route variants remain open.

### Follow-up at 19:55 UTC

The composed restored-instance workflow now passes: explicit audit/repair of a corrupt restored original, preserved tombstone and signed chapter, fresh group publication using the recovered key, deliberate nginx activation, and separately resumed recurring sync, while enrollment stays closed (`restore-reactivation-second.log`,9.19s). The initial test-image dimension mismatch was a harness error; production was unchanged. Publication ledger/outbox/health-counter assertions have been replaced by real full-quota, exact-byte and idempotent HTTP outcomes (`publication-quota-first.log`,3.37s).

Mixed-route slow-reader count/byte cases now pass across Blossom/public/peer/review/compatibility surfaces; the common response deadline is tested separately once. Narrow actual-browser snapshot5 local-save/export/restore evidence also exists under `results/rebuild-browser/checkpoint5-*.json`. Final-client signer/offline/eviction/performance qualification and a frozen combined gate remain distinct.


Final focused verification at19:55UTC: `composed-final.log` passes `http_migration` (8.83s) and `http_publication` (3.47s), exit0. Three-target Clippy and rustfmt pass. Exact test-source hashes and retained diagnostic chronology are in `results/rebuild-feature-seams/acceptance.json`. No production changes or canonical full-suite claim follows from this slice.

## Historical audit at 19:15 UTC

Snapshot: 2026-10-10 19:15 UTC. This is a read-only implementation/evidence audit of the recovered October 8 tree, with this report as its only authored artifact. Other workers are actively changing backend, browser, operator and test files; this is not a frozen release assessment. No tests were run for this audit.

## Verdict

The recovered source contains the reference backend's major accepted feature families and a functional local-library browser client. It is not yet a freshly verified, completely refactored, optimized release. Historical October 8/10 pass statements are not acceptance of this tree. Rebuilt evidence currently proves individual browser/operator slices, not the whole system.

“Fully functional core” can mean the documented signed publication, retained originals, discovery, approved federation, social, group, public-library and operator workflows plus the local-first browser library client. “Literal all features” must not imply a complete reader website, every MangaDex endpoint/client, automatic social ranking, every Nostr extension, production security certification or achieved target traffic. Those broader outcomes are absent or expressly outside the reference scope. Website visual design was deferred; the functional browser client remains required.

The recovered issue graph exists at `.scratch/federated-manga-postgres/graph.json`, with specification and ticket files alongside it. It is stale: its README still calls implemented service discovery, library delivery and compatibility work pending. Its statuses and historical letter-only selections are not fresh acceptance evidence or new requirements.

## Evidence labels and priorities

- **rebuiltverified**: a specific current reconstruction log demonstrates the described narrow behavior. It does not confer verification on neighboring features or prove a later-edited source revision.
- **restoredunverified**: implementation and relevant tests exist, but no completed current-tree acceptance run was established by this audit.
- **missing**: implementation for the specifically described outcome is absent. This is a scope gap only when that outcome is required.
- **blocked**: meaningful acceptance requires an unavailable dependency, authorized infrastructure or a scope decision.
- **P0**: data loss, disclosure, authority or publication/retention correctness; **P1**: required functional interoperability and operation; **P2**: broader qualification, maintainability or explicitly deferred product work.

Paths below are relative to the project root. Rust test names refer to `server/tests/<name>.rs`; browser tests are under `browser-client/tests/`. Test existence is not a pass. The acceptance seam must also match the rebuild plan: public behavior, application-used lifecycle interfaces, actual HTTP/Nostr, or operator CLI/process outcomes, rather than private SQL counters/helper assertions.

## Requirement → implementation → evidence

| Requirement / actual outcome | Actual module or interface | Existing test / inspected evidence | Status | Implementation gap versus validation gap | Priority |
|---|---|---|---|---|---|
| Nostr signed identities and events; exact request/body binding | `server/src/auth.rs`, `relay.rs`, `accounts.rs`; `/events`, NIP42 root WebSocket, NIP98 HTTP | `http_relay`, `http_accounts`, `http_admission_errors`, browser `nostr.test.mjs` | restoredunverified | Core is present. Re-run signature/identity mismatch, replay, revocation and retryable outage behavior on final tree. | P0 |
| Rust wrapper around real strfry; PostgreSQL authority, deadpool/tokio-postgres and Moka; no SQLx/Kratos runtime | `server/Cargo.toml`, `db.rs`, `backend.rs`, `node.rs` | `postgres_foundation`, `http_backend`, `multiprocess.py`; dependency declarations inspected | restoredunverified | Selected dependencies are present; no forbidden runtime dependency found in the manifest. Real strfry build/runtime and process/database restart acceptance remain pending at this snapshot. Historical prototype is still in tree, not the selected deployment. | P0 |
| Independent instances, public/default anonymous or restricted read policy, separate account/publisher/peer permission | `accounts.rs`, `publisher_authority.rs`, `peers.rs`, `node.rs`, `relay.rs`; `/admin/accounts`, `/admin/publishers`, `/admin/peers` | `http_publishers`, `http_peers`, `http_admission_errors`, `http_relay` | restoredunverified | Backend policies exist. A finished per-instance reader website is a separate missing product surface, not proof policy support is missing. Cache/authority and overload behavior need current wire tests. | P0 |
| Optional disabled-default enrollment | `enrollment.rs`, `node/enrollment_http.rs`; GET/POST `/enrollment` | `http_enrollment`, `enrollment_fences`, restore policy tests | restoredunverified | Present; must preserve disabled default, independent publisher permission and no revocation bypass. | P0 |
| Host complete original bytes for every offered chapter; allowed formats; no silent recompression | `upload.rs`, `storage.rs`, `manifest.rs`, `publication.rs`; `/upload/{hash}`, `/publications`, `/blobs/{hash}` | `http_upload`, `http_publication`, `blob_receipts`, `upload_recovery` | restoredunverified | Present. Current full-original hash/image validation, incomplete-publication rejection and shared-original retention need re-execution. | P0 |
| Immutable exact publication identities; deletion then new upload without revision links/progress inheritance | `publication.rs`, `manifest.rs`, `moderation.rs`; `/publications`, `/deletion-requests` | `http_publication`, `publication_fences`, `http_moderation`, `http_reader_state` | restoredunverified | No in-place correction workflow is required. Compatibility PUT deliberately returns 501. Re-run exact-ID/replay/deletion fences. | P0 |
| Local approval default for protected deletions; no cascade or resurrection; explicit restore | `moderation.rs`, `reclamation.rs`; admin approve/restore routes | `http_moderation`, `publication_reclamation`, `ordinary_deletion` | restoredunverified | Implemented distinction from ordinary account/social deletion must survive refactor. No global erasure guarantee exists. | P0 |
| Canonical external manifests, exact bytes, page properties and extension negotiation | `manifest.rs`, `series_metadata.rs`, `library_manifest.rs` | codec units, `http_publication`, `http_sync_skips`; browser `codec.test.mjs` | restoredunverified | Implemented experimental profile. Fresh independent vectors and browser/Rust interoperability required; existing self-roundtrips alone are insufficient. | P1 |
| Rich catalogue, cover originals, series/title/category discovery | `catalogue.rs`, `discovery.rs`, `node/catalogue_http.rs`; `/series`, `/catalogue/{key}`, `/query` | `http_catalogue`, `http_discovery`, `http_publication` | restoredunverified | Backend is present; client browsing UI absent. Cover retention and metadata index consistency need fresh integration coverage. | P1 |
| HTTP manifest query separate from strfry; explicit order, offset/limit, timestamp range, exact optional totals and language/absence semantics | `discovery.rs`, `filter.rs`, `node/catalogue_http.rs` | `http_discovery`, filter units | restoredunverified | Present. Re-run bounded errors and exact totals; target-scale latency remains unqualified. | P1 |
| Shared HTTP/NIP11 capabilities, supported schemas/formats and public budgets | `capabilities.rs`, `protocol_limits.rs`, root and compatibility capability routes | `http_capabilities` | restoredunverified | Present; support advertisement does not establish health or capacity. Route inventory must be regenerated/checked after edits. | P1 |
| Approved-peer HTTP sync and distinct allowlisted read-only NIP42 peer WebSocket | `peers.rs`, `relay.rs`, `node.rs`; `/federation/*`, `/reconcile`, `/federation/relay` | `http_peers`, `http_peer_websocket`, `http_tls_proxy` | restoredunverified | Both transports present. Required rejection/revocation/WSS evidence must be rerun; normal reader subscriptions are not a substitute for peer-only checks. | P0 |
| Bounded resumable sync, schedules, cancellation/leases, supported-item continuation | `sync_jobs.rs`, `sync_schedules.rs`, `inventory.rs`, `transfer_rejection.rs` | `http_sync_jobs`, `sync_queue`, `sync_schedules`, `http_sync_skips`, `sync_skips` | restoredunverified | Present. Verify stale-worker completion fences, operational retry versus policy skips and inventory bounds. | P0 |
| Service-key continuity across domain changes and remembered first possession proof | `service_identity.rs`, `service_proof.rs`, `service_announcement.rs`, `peer_discovery.rs`; `/identity`, service announcement and discovery routes | `http_remembered_identity`, `http_service_announcements`, `peer_relocation_fences` | restoredunverified | Signed HTTP route exists. Optional WebSocket announcement projection absent by documented boundary. Browser pins identity by configured origin but does not itself provide a full relocation-management UI. | P0 |
| Managed groups, personal-member role authorization, group-signed publication/profile, actor audit | `groups.rs`, `node.rs`; `/groups/{key}/*`, `/admin/groups/{key}/audit` | `http_groups`, `group_fences`, `http_group_profile`, `http_group_retry` | restoredunverified | Present. Keep caller-persisted `created_at` retry identity; legacy omitted timestamp is not retry-safe. Membership and key custody are distinct. | P0 |
| Group key backup, identity-preserving import and founder-loss continuation | `offline_keys.rs`, key CLI, `/admin/groups/import`, explicit bootstrap | `http_groups`; `docs/group-key-backup-recovery.md` | restoredunverified | Operator holds ordinary key; agreed administrator can retain offline backup. No private-key HTTP export or automatic key distribution to every member. Key-only import intentionally does not recreate membership. Verify surviving-instance publication with originals, not just successful key copy. | P0 |
| Group clearing and reversible disable lifecycle | group profile and enabled routes | `http_group_profile`, `http_groups` | restoredunverified | Permanent group erasure/deletion endpoint absent by documented MVP choice; do not infer a new requirement from a historical letter answer alone. | P1 |
| Public profiles, comments/replies, reactions and votes | `accounts.rs`, `social.rs`, `ordinary_deletion.rs`; `/account-metadata`, `/social`, `/account-deletions` | `http_publication`, `http_accounts`, `http_peers`, `http_sync_jobs`, `ordinary_deletion`; `docs/public-social-api.md` | restoredunverified | Kind 1111 comments and kind 7 reactions exist; +/empty and - provide vote representation. Vote totals/ranking, Sybil-resistant one-person-one-vote and social UI are not implemented/promised. Explicit downvote evidence should be checked, not inferred from upvote success. | P1 |
| One public library per key; immutable root/chunks, exact IDs, lists/status/progress and ordinary deletion | `library.rs`, `library_manifest.rs`, `library_storage.rs`, `library_view.rs`, `node/reader_http.rs`; `/libraries`, `/library/*` | `http_reader_state`, `library_roots`, `library_staging`, `http_accounts`, `ordinary_deletion` | restoredunverified | Core exists. Critical rebuilt projection/ref-retention changes are underway and require current public-lifecycle evidence. Last-write-wins is intentional, not multi-device merge. | P0 |
| Multi-instance library delivery, durable retry and verified cold recovery | `library_client*.rs`; browser `publishing.js`, `outbox.js`; library HTTP routes | `http_library_client`, `library_client_negative`, `http_browser_client`, browser `publication.test.mjs` | restoredunverified | Native/browser implementations exist. Re-run denial/retry, immutable event replay, wrong-key/root/chunk rejection and source-offline recovery. Browser-module Node transport is not rendered-browser/signing-extension acceptance. | P0 |
| Browser local-first saves, no automatic publication, no private key capture, explicit public consent | `browser-client/src/app.js`, `store.js`, `indexeddb.js`, `nostr.js`, `publishing.js` | `store.test.mjs`, `app-dom.test.mjs`, `nostr.test.mjs`, `publication.test.mjs`; current `results/rebuild-client/013-forget-sign-green.log` | restoredunverified | Functional library editor exists. Narrow local-storage/outbox slices passed, but no fresh complete browser flow/backend integration established here. Preserve no-network local saves and explicit publication/retry decisions. | P0 |
| Durable bounded outbox, sticky cancellation/removal, immutable queued bytes, forget-during-sign fence | `browser-client/src/store.js`, `outbox.js`, `publishing.js` | `results/rebuild-client/010-bounded-outbox-green.log`, `013-forget-sign-green.log`; `outbox.test.mjs`, `store.test.mjs` | rebuiltverified | Specific current model/IndexedDB tests pass. Real browser crash/quota behavior and final backend transport remain validation gaps. Logs verify this slice, not all client behavior or performance bounds. | P0 |
| Local browser backup/import, atomic replacement and offline app | browser export/import handlers, store replacement, worker, service worker and build script | `store.test.mjs`, `service-worker.test.mjs`, `local-browser.cjs`; historical rendered QA | restoredunverified | Current app still materializes JSON export/import at this snapshot; bounded streaming/staged backup and atomic build promotion are rebuild work, not already proven. Real confirmed restore/offline/reload remain fresh-browser validation gaps. | P0 |
| 30,000 and 500,000 chapter reader fixtures | `server/tests/http_library_client.rs` loops over both sizes; browser `codec.test.mjs` | Tests exist for both exact sizes; older logs historical | restoredunverified | Existing fixture coverage is substantive. Need final-node full delivery/interruption/recovery and final-browser resource behavior; successful codec construction alone does not establish usable UI, save throughput or durable recovery. | P1 |
| MangaDex-shaped exports/original migration and reader/group compatibility | `src/mangadex_migration.py`, `local_library_migration.py`, `postgres_migration.py`, Rust compatibility/reader HTTP | `http_migration`, `http_reader_state`, `http_group_profile`, Python migration/CLI tests | restoredunverified | Supported subset exists; passwords/tokens and unchanged arbitrary clients are not compatible. Preserve reviewed alias mapping and exact-ID progress rather than guessing. | P1 |
| Exact local migration without signing or publication | `src/local_library_migration.py` CLI | `tests/test_local_library_migration.py`; `results/rebuild-local-migration-green.log` | rebuiltverified | Narrow CLI case passes; not acceptance of complete MangaDex export variants or PostgreSQL migration. | P1 |
| Private submission/review workflows | `submission_review.rs`, `node/review_http.rs`; `/submissions`, admin decision routes | `http_submission_review`, `submission_review_fences` | restoredunverified | Present. Re-run private blob visibility, authorization, revocation and review-to-publication completeness. | P0 |
| Crash-safe upload/staging reservations, accounting, lease/GC safety and original repair | `staging_operation.rs`, `upload.rs`, `storage_accounting.rs`, `reclamation.rs`, `staged_deletion.rs` | `upload_recovery`, `staging_reclamation`, `http_staged_delete`, `publication_reclamation`, `http_library_gc_crash` | restoredunverified | Implemented but currently under reconstruction. SQL ledger counts can support diagnosis; acceptance must show retained readable bytes, refusal/success and safe recovery through approved seams. | P0 |
| Guarded/direct original serving, drained readers and independent byte/count/download budgets | `serving.rs`, `serving_role.rs`, `public_exports.rs`, `export_queue.rs`, `download_budget.rs`, `capacity.rs` | `http_public_serving`, `http_tls_proxy`, `serving_process`, `http_download_lifecycle` | restoredunverified | New lifecycle fixture compiled per current build log; compile is not runtime success. Body-drop, timeout/cancel, pre-DB admission and upload independence must be demonstrated. No production CDN/proxy qualification. | P0 |
| Operator backup/restore, safe inactive restore, audit and repair | `src/postgres_backup.py`, `operator_json.py`, `serving_supervisor.py`; `/admin/storage/*`, `/admin/check`, `/admin/status` | `http_migration`, `test_postgres_backup.py`, `test_backup_cli.py`; rebuild preflight/private logs | restoredunverified | Existing broader workflow needs fresh real database/process test. Narrow reject-before-side-effects and private CLI artifact cases have current green logs; they do not prove full restore/startup. | P0 |
| Restore preflight refusal before database/output, private atomic operator artifacts and safe packaging | `src/operator_json.py`, `src/postgres_backup.py`, `scripts/package.py` | `results/rebuild-backup-preflight-green.log`, `rebuild-operator-private-green.log`, `rebuild-package-green.log` | rebuiltverified | Each log records a narrow single CLI test. Complete crash/failure matrix, final source allowlist and archive extraction/rebuild are separate acceptance gates. | P0 |
| Clean complete refactor and optimized speed | extracted HTTP/domain modules, prepared query/cache work, browser worker/outbox modules | Current slice logs; backend current status still awaits red/green | restoredunverified | Significant modularity exists, but `node.rs` remains about 3,676 lines at inspection and current source hygiene work is unfinished. File size is a maintainability signal, not proof of a bug. Optimize measured bottlenecks; no blanket speed or cleanliness claim. | P1 |
| 8B images/month, 350M page views/month, 300M progress saves/month, 100TB retained library | configured budgets, nginx serving, PostgreSQL and cache architecture; `bench/` | Historical capacity studies and explicitly failed runs; no fresh target acceptance | blocked | Architecture and fixtures are not capacity certification. Needs authorized realistic infrastructure, load distribution/peaks, sustained accepted writes, projection backlog, latency/error budgets, disk/retention and failure measurements. 300M/30d averages ~115.74 saves/s; 120/s is only a baseline, not peak or month proof. | P2 |
| Encrypted recoverable current checkpoint including source/build/docs/test evidence, excluding secrets/private data/DBs | Packaging/checkpoint workflow in rebuild plan | No completed final reconstructed checkpoint verification established by this audit | restoredunverified | Final immutable archive, encrypted upload, downloaded-byte hash, decryption/member verification and recovery delivery remain a separate completion gate. A baseline backup is not backup of later edits. | P0 |

## Genuine holes, deliberate boundaries and immediate regression risks

### Implementation holes recorded at 19:15 (superseded where listed above)

1. At this snapshot the browser backup path still uses whole-snapshot JSON materialization. The rebuild plan calls for bounded/staged backup operations. Treat this as unfinished implementation, not merely missing tests, until the browser worker replaces it and proves atomic failure behavior.
2. The backend worker's current `results/rebuild-backend/STATUS.md` explicitly says projection independence is a newly written regression awaiting runtime red/green. Do not count it as reconstructed behavior yet. The required outcome is unrelated imports progressing while one retirement is externally blocked, while that predecessor remains readable and gains fresh grace after retirement.
3. Source hygiene/refactoring and download admission/lifetime work are actively incomplete. A successful test compilation or an old comprehensive log must not close them.
4. The browser has no catalogue search/browse or chapter-image reader, social interactions, managed-group publication UI, submission review UI or moderator console. These are deferred frontend implementations, not lost regressions: website design was excluded from the prior scope. The current restoration scope supplies those workflows by backend/CLI and requires the functional local-library browser client. Disclose that gap when discussing “all features”; do not silently broaden the build or call this a complete MangaDex successor.

### Validation holes recorded at 19:15

- **Historical standalone-browser blocker, superseded by the pending supported-browser test above.** The parent reports a fresh installed `/usr/bin/chromium` normal headless launch, sandbox enabled and without `--no-sandbox`, aborted in `process_singleton_posix` after `socket()` returned EPERM; direct AF_UNIX socket creation also returned EPERM. Evidence: `results/rebuild-browser/startup.log` and `startup.exit`, exit 134. No bypass was attempted. This is an environment blocker; model tests remain useful but limited.

- Current full real PostgreSQL/strfry/nginx regression, migration/restart and final optimized binary runs are not established here. Dependency restoration and active source edits prevent treating old logs as current acceptance.
- Current-browser signing extension, real offline behavior, confirmed import/recovery and 30k/500k resource limits remain to be checked. DOM/fake IndexedDB tests and Node-driven HTTP interoperability answer different questions.
- Existing retained tests include historical private-helper, table-count or synthetic index-plan evidence. Such tests may describe useful historical facts but cannot substitute for the current agreed acceptance seam. The backend worker is already converting some of these.
- Target traffic/storage, independent-machine disaster recovery and independent security review remain unqualified. Do not hide them behind test totals.

### Boundaries that must not be “fixed” into unrequested semantics

- Votes are ordinary signed reactions; no automatic ranking or person-identity proof.
- Groups use an ordinary instance-custodied key and explicit offline recovery; disabling does not erase keys, membership or published content.
- Account libraries are public when explicitly published, one per key, with selected replacement semantics; local edits never silently become public, and there is no automatic global merge.
- Protected chapter/series deletion requires local policy approval by default, differs from ordinary deletion and never promises universal erasure.
- HTTP catalogue/manifests, NIP98 transfer and NIP42 event subscriptions remain separate surfaces; raw strfry must not become an authorization bypass.
- Experimental wire kinds/extensions, supported MangaDex subset and original-image-only representation are disclosed compatibility limits, not unimplemented universal standards.

## Next acceptance order

1. Finish and prove P0 browser/outbox/backup, projection-retirement and download/admission slices through the allowed public seams.
2. Freeze exact source inputs; run all required core workflows including negative authorization, corruption, cancellation, process/database restart, source-offline originals/group/library recovery and both reader sizes. Keep command, dependency versions, source identity and result together.
3. Run real browser local save/reload, backup restore, explicit publication and recovery against the final node. Clearly separate model, Node transport and rendered-browser evidence.
4. Recheck generated route inventory and update present-tense entry documentation; historical `START-HERE.md`, requirement matrix and browser README currently retain old pass statements and deferred-UI wording that can mislead about this reconstruction.
5. Produce and independently verify the encrypted final checkpoint before calling recovery complete. Separately report remaining product/production/capacity boundaries; only then proceed to measured speed improvements without erasing failed baselines.

## Untouched end-to-end restore verification priority

The existing `http_migration` fixture is broader than the new one-case backup preflight CLI test. Preserve and rerun its actual operator-process/HTTP outcomes after final edits, while replacing private SQL outcome assertions where they are still used:

1. Refuse a live-source backup and refuse a corrupt archive before destructive restore work.
2. Restore with serving, signup and recurring synchronization paused; no stale derived public serving tree is implicitly activated. Preserve service identity and policy data without reviving old process/worker authority.
3. Start the restored real node and reject pre-restore NIP98 tokens, while freshly signed operator requests succeed.
4. Read the retained chapter, library head and exact original bytes; removed series stays absent, without cascading removal to a retained chapter.
5. Verify revoked group/account access remains denied through HTTP and that retained authorized group workflows still function. Key presence or SQL membership counts alone are insufficient.
6. Exercise audit/repair and explicitly reviewed serving reactivation on the restored volume, including missing/corrupt original handling. Verify scheduled work stays paused until operator action, through its API.

The source already contains substantial HTTP outcomes for items 1–5; this audit found no wholesale missing backup/restore implementation. The gap is fresh execution and full allowed-seam coverage, especially around restored authority, workers and serving activation. General core API inventory likewise revealed no confirmed absent required feature family; absence of a downvote-specific wire test, broad browser UI or a target-load result must not be mislabeled as absence of the underlying core protocol.
