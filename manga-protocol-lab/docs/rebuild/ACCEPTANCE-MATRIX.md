# Historical executable acceptance matrix, 2026-10-10 20:07 UTC

## Current status

See [the authoritative current rebuild summary](STATUS.md#current-status) for the current functional gate, mixed-load outcomes, checkpoint verification and remaining qualification. The separately dated sections below are historical evidence for their stated builds/times; they do not override that summary. The [architecture map](ARCHITECTURE.md) describes current source boundaries.

## Verified optimized acceptance, 2026-10-10 22:40 UTC

The final complete gate passed exit0 with unchanged recorded inputs:55 Python,6 packaging,91 browser model checks,74 Rust module cases (2 intentional ignores),all84 integration executables,strict all-target Clippy and PostgreSQL multiprocess crash/restart with durable settings. Exact node SHA256: `478ffc3243281a8e1e0c756eaf91b8923d740a2b2a146833b61443a837e73b54`. Browser build: `e4f4862bd75c26bb`. See `results/rebuild-parity-final/canonical-acceptance.json`.

All six explicitly recovered refactor promises have implementation and acceptance evidence. The feature audit found no additional missing required core family. Native Chrome schema4→5/save/reload/export preserved all500,202 records field-exact; three-sample count latency median fell from2075.8ms to0.3ms on this shared host. The separately dated sections below are historical, not the current completion state.

Matching encrypted checkpoint12 was uploaded, downloaded, all1,487 members authenticated/decrypted/verified, restored, and its binaries CLI-smoke-tested. See CHECKPOINTS.json and results/rebuild-parity-final/checkpoint12-recovery.json. Mixed-load/target traffic,100TB production operation,native signer extension,physical mobile/eviction/power-loss qualification remain open. Missing later source prevents exact byte parity; this gate does not promise perfection or every possible failure covered.


## Latest combined attempt and pending restart, 2026-10-10 21:54 UTC

The optimized parity attempt ended **exit 101** at 21:51:19 UTC, with no changed
inputs: [terminal record](../../results/rebuild-parity-final/canonical-result.json).
Two `storage_fairness` cases failed, and a shared database fixture collision was
identified. A new full run is required after storage-fixture isolation and HTTP
read-ownership corrections are frozen. **Full parity is not yet accepted.**

Separately, the final client build `72e656def51db26f` has 81 passing model checks
and scoped actual-browser evidence. Native Chrome has also completed the
30,000-chapter, four-family import/edit/reload/export comparison with all 30,202
records verified: [native export](../../results/rebuild-browser/native-30000-export.json)
and [qualification scope](NATIVE-BROWSER-QUALIFICATION.md). Native 500,000-chapter
qualification is pending; no target capacity, mobile, signer-extension, eviction
or power-loss claim follows.

The three obsolete workflow helpers are now explicitly retired with verified
originals and a 55-Python/6-packaging narrow pass. See [workflow inventory](WORKFLOW-HYGIENE.md).
The next full run must include this new file inventory. Historical evidence below
remains scoped to its original time/build.

## Current evidence navigation, 2026-10-10 21:40 UTC

The original matrix below is preserved as the 20:07 reconstruction snapshot.
Its open-item wording, counts and snapshot5 qualifications are historical; they
must not override the later evidence linked here. The current accepted refactor
requirements are in [PARITY-REQUIREMENTS.md](PARITY-REQUIREMENTS.md). See
[STATUS.md](STATUS.md) and the [client/operator parity audit](PARITY-REFACTOR-CLIENT-OPERATOR.md)
for scoped implementation and remaining qualification boundaries.

- Final client build `72e656def51db26f` passed **81 model checks**, build and final
  formatting: [client acceptance](../../results/rebuild-client-ui-parity/acceptance.json).
- Actual cloud Chrome on that build preserved previous synthetic records, saved
  and reloaded a 128-emoji list name, and rejected 129 ASCII scalars without an
  added list: [native UI acceptance](../../results/rebuild-browser/final-parity-ui.json).
  This does not establish real signer-extension recovery/publication, mobile,
  native quota/power-loss behavior or large-library responsiveness.
- **The new combined optimized parity gate is pending at this update.** Focused
  client/native passes do not substitute for its terminal result. The earlier
  20:38 combined gate remains evidence only for its prior frozen inputs.

## Preserved 20:07 audit

Initial audit: 2026-10-10 19:32 UTC; acceptance follow-up updated 20:07 UTC, reconstructed October 8 sources. The initial audit inspected implementation, test assertions and completed reconstruction logs without executing another suite. The assigned HTTP follow-ups below were then executed separately. Work remains concurrent. Earlier October 8/10 aggregate results do not validate this tree. [API-INDEX](../API-INDEX.md) is the route inventory; [PLAN](PLAN.md) defines the accepted seams.

**Finding:** all ten core feature families below have implementations and meaningful executable coverage. No absent required backend family was established. Remaining issues include missing combinations/edge-case assertions, acceptance not yet rerun on a frozen tree, and broader product/production boundaries. A test file's existence is not a pass.

## Commands and interpretation

From the project root, with the restored [toolchain](TOOLCHAIN.md):

```sh
export PYTHONPATH="$PWD/vendor/pydeps:$PWD${PYTHONPATH:+:$PYTHONPATH}"
export CARGO_TARGET_DIR="$PWD/prototype/target"
# Substitute an exact fixture basename from the table, one at a time:
MANGA_TEST_FIXTURE=http_catalogue bash scripts/test-postgres.sh
# All current acceptance, after coordinating a source freeze and port ownership:
bash scripts/test.sh
# Operator and client-only gates:
python3 -m unittest discover -s tests
python3 -m unittest discover -s scripts/tests
(cd browser-client && npm test && npm run build)
```

The PostgreSQL runner creates isolated ephemeral databases on **55432** and invokes integration tests with `--include-ignored`; ordinary `cargo test` can skip these important fixtures. Do not run it concurrently with another owner of that port or a performance measurement. A selected fixture still runs Rust module/bin checks, Clippy and the separate-process recovery gate. Do not sum overlapping focused logs as one suite. No uninterrupted final combined pass is established by this audit.

## Feature → executable seam → observable acceptance

All Rust fixture names below mean `server/tests/<name>.rs`. Files still being cleaned up may contain additional historical private SQL or counter assertions: those are not accepted outcome evidence. SQL/filesystem faults may arrange a case; final assertions must use the listed public outcomes.

| Family and implementation | Executable coverage | Required public outcomes and current qualification |
|---|---|---|
| **Catalogue**: `catalogue.rs`, `discovery.rs`, catalogue/compatibility HTTP | `http_catalogue`, `http_discovery`, `http_capabilities` | Exact multilingual metadata and cover bytes, case/NULL/language/filter rules, explicit order/offset/limit/totals, immutable aliases, unsupported mutation refusal, source-offline replicas, policy-protected reads. Fresh discovery and complete catalogue HTTP passes exist, including operator reindex recovery (G3 closed). Capabilities/final combined rerun remains pending. |
| **Originals and deletion**: upload/storage/publication, moderation/reclamation, managed serving | `http_upload`, `http_publication`, `http_blossom`, `http_moderation`, `http_peers`, `http_public_serving`, `http_download_lifecycle`, `http_mixed_downloads`; Python `test_serving_supervisor.py` | Complete validated originals before hosting, exact bytes/ranges, private staging and grant isolation, shared-original retention, local deletion approval/no cascade/no resurrection, corruption withdrawal and approved repair, source-offline serving. Fresh Blossom, download and eight real-nginx operator checks pass. Publication HTTP and public storage/health lifecycle checks also pass. Other whole workflows need final rerun. Representative mixed-route slow-reader cases now pass (G4); deadline remains tested once at the common response seam. |
| **Groups**: `groups.rs`, offline key CLI, group/profile/audit HTTP | `http_groups`, `http_group_profile`, `http_group_retry`, `group_fences` | Personal-member role enforcement, group-signed immutable publication, private first-actor audit, revocation, clear/disable/re-enable, persisted retry identity, key-only import with explicit bootstrap, founder-offline new original-backed publication. Fresh public group fence, profile and retry fixtures pass. Profile/retry outcomes now use HTTP audit, signed reads and separate account/member admission; the remaining complete group workflow needs a final-tree rerun. No permanent erasure endpoint is promised. |
| **Social**: accounts/social/ordinary-deletion HTTP | `http_accounts`, `http_publication`, `http_peers`, `http_sync_jobs`, `ordinary_deletion` | Signed profiles, comment/reply target and author validation, successful reaction, duplicate idempotence, author-only deletion and no reply/chapter cascade, dependency-ordered federation. Existing HTTP checks cover these. Successful downvote/plaintext and exact social pagination now pass in the complete publication HTTP fixture (G2 closed); aggregation/ranking is deliberately absent. |
| **Library**: library objects/roots/projection, native delivery/recovery, browser local operations | `http_reader_state`, `http_accounts`, `http_library_client`, `library_client_negative`, `http_browser_client`, `backend_projection_independence`, `library_prepared_references`, `library_reclamation`; client tests | All four record families, exact IDs, signed replacement and protected shared bytes; immutable retry/cancel, verified cold recovery, no automatic publication, author/generation fences and independent projection progress. Fresh browser model and backend lifecycle slices pass. Existing native 30k/500k interrupted delivery/cold recovery and Node-to-real-backend fixtures need final rerun; model/Node transport is not actual-browser acceptance. |
| **Sync**: peers/inventories, sync jobs/schedules, item outcomes | `http_peers`, `http_inventory`, `http_sync_jobs`, `http_sync_skips`, `http_peer_websocket`, `sync_queue`, `sync_schedules`; `multiprocess.py` via runner | Bilateral approved keys, independent read-only NIP42 peer transport, resumable jobs/lease fences/cancel, supported-item continuation, bounded large inventories, scheduled imports and preserved prior content. Fresh real application-queue/process crash/restart checks pass. Full peer/inventory/schedule wire workflows remain to rerun. |
| **Policies**: read admission, independent account/publisher/peer permission, enrollment, private review | `http_publishers`, `http_enrollment`, `enrollment_fences`, `http_admission_errors`, `http_relay`, `http_submission_review`, `http_blossom` | Denial versus retryable outage, exact request/body/replay checks, closed-default bounded signup without publisher grants, no revocation bypass, private one-off review without permanent publishing authority, pre-database admission. Fresh HTTP/Nostr admission, outage, review/Blossom/relay, HTTP enrollment and enrollment fence checks pass; no complete route-by-role cross-product claim. |
| **Identity**: signing/service proof, remembrance/announcements, endpoint policy | `http_service_proof`, `http_remembered_identity`, `http_service_announcements`, `peer_relocation_fences`, `http_tls_proxy`, `http_peer_websocket`; public independent codec vectors | Exact signature/body/URL binding, first possession without trust grant, changed-key refusal, signed relocation with old founder offline, rollback/revocation fences, real TLS/WSS scope. Fresh independent vectors pass; full identity/relocation/TLS fixtures need rerun. No automatic reputation or universal Nostr-extension support. |
| **Import**: `library_records.py`, local/MangaDex/PostgreSQL migration CLIs, compatibility aliases | Python `test_mangadex_cli.py`, `test_migration_plan_cli.py`, `test_local_library_migration.py`, `test_aliases_cli.py`; `http_migration` | Exact reviewed mappings and original hashes, private-list exclusion unless explicit, no invented reads, no silent signing/publication, private exclusive output, real signed import and idempotent retry, source-offline retained chapter/library. Fresh Python CLI and two-node HTTP/PostgreSQL/strfry acceptance pass. This supports a documented subset, not account-password or arbitrary unchanged-client migration. |
| **Backup and operation**: `operator_json.py`, `postgres_backup.py`, supervisor; browser backup module | Python `test_backup_cli.py`, `test_serving_supervisor.py`, packaging tests; `http_migration`; client `backup.test.mjs` | Live-owner refusal, bounded/path-safe structural preflight, copy/hash validation before database restore, empty target, independent original files, preserved identity/tombstones/revocations, stale-token/worker rejection, paused serving/signup/schedules; atomic local restore and interruption preservation. Fresh operator and real restore fixture pass. Composed activation/repair/new group publication/recurring sync now passes (G1); power loss/online backup/multi-host disaster recovery remain unqualified. |

## Fresh evidence used, without aggregating it

- `results/rebuild-python-final.log` and `rebuild-package-final.log`: **50 Python/operator/codec and 6 packaging tests**, including eight real-nginx lifecycle tests. Input hashes: `rebuild-operator-final.json`.
- `results/rebuild-queue/http_migration.log`, `backup-restore.json`, `publication_fences.log`, `postgres_foundation.log`, `multiprocess.log`: real migration/restore, actual sync lifecycle and independent processes. The earlier process-run failure was fixture restart configuration; corrected run passed.
- `results/rebuild-client/054-full-model-gate.log`: **38 Node/DOM/IndexedDB-model tests**, zero failures/skips. Later large-backup work is separate and not claimed passed here.
- `results/rebuild-backend/core-regression.log`: four lifecycle fixtures pass, including independent imports/four retirements despite a fifth author's lock, prepared-generation rejection and bounded cleanup. These repair prior explicitly reproduced failures; this is not throughput acceptance.
- `results/rebuild-admission/{http,nostr,nostr_auth,download-count,download-bytes,cleanup-status}-green.log` and the four `http_*-regression.log` files: current targeted admission/download and related HTTP regressions. Slow-reader count case runs the actual 120-second deadline.
- `results/rebuild-seams/{domain-public-first,independent-codec-vectors-first,public-contracts-first}.log`: current public discovery/lifecycle and independent vectors, with overlapping runs.
- `results/rebuild-feature-seams/{catalogue-first,social-first}.log` and `.exit`: both complete HTTP fixtures passed on unchanged production, closing G2/G3 without manufacturing failures. The two-target Clippy rerun passed (`clippy-final.log`, exit0) after another owner corrected a shared test-helper lint; the earlier lint failure remains recorded.
- `results/rebuild-admission/storage-lifecycle-final-green.log`: seven actual application-used filesystem lifecycle checks, including Linux file-lease cancellation faults. `storage-health-config-final-green.log` and `storage-health-lifecycle-green.log` each pass one public configuration/repair case. These do not certify every OS/filesystem stall.

**Actual browser: partial snapshot5 evidence.** A fresh standalone Chromium launch failed with socket EPERM (`results/rebuild-browser/startup.log`, exit134). The parent subsequently used the already-running supported cloud browser. `checkpoint5-save.json`, `checkpoint5-export.json` and `checkpoint5-restore.json` record the rendered app's local-save, prepared-backup and completed local-import states, with the expected chapter at page7. This is narrow snapshot5 evidence; it does not prove later client changes, real signer integration, offline/eviction behavior or large-library responsiveness. No security bypass or older rendered pass is counted.

Additional publication cleanup: `publication-quota-first.log` passes the complete HTTP fixture in3.37s after replacing private ledger/outbox/health-counter assertions. Known unique HTTP input bytes fill physical capacity; full staging capacity is reused after retention and deletion; excess bytes fail, duplicates retain exact identity, and corruption never refunds usable quota. The original SQL-oriented test is recoverable under `retired/rebuild-feature-seams/`. This is migrated regression coverage on unchanged runtime.

## Follow-up acceptance gaps and outcomes

1. **G1, P0 recovery composition, closed by fresh regression:** extended `http_migration` now passes paused startup, operator-only audit, real restored-file corruption, approved-peer repair, unchanged chapter/tombstone, a newly authorized member publishing fresh original-backed content under the restored group key, explicit configuration activation of real nginx, and separately enabled recurring sync importing a newly signed remote profile. Enrollment remains closed. Evidence: `restore-reactivation-second.log`, 9.19s. The first attempt correctly rejected a test image whose dimensions disagreed with its helper descriptor; that fixture error was corrected. Runtime unchanged, so no production red→green is claimed.
2. **G2, P1 social contract, closed by fresh regression:** The initial audit found `http_publication` successfully posted `+`, while its only `-` was rejected after deletion-before-arrival. Its social-list assertion checked length rather than exact signed content/order. Add successful `-`, empty/custom/plaintext content preservation and deterministic timestamp/ID tie ordering with limit/offset/since/until. No vote-count feature is implied. The assigned additions passed `http_publication` first run, 2.50s. Exact signed records, successful downvote/empty/custom reactions, duplicate idempotence and all listed pagination/bounds assertions are now present. Runtime was unchanged; no behavioral red→green is claimed.
3. **G3, P1 catalogue operator recovery, closed by fresh regression:** the initial audit found no server test calling `POST /admin/catalogue/reindex`; `http_catalogue` then waited on a private derived-index table after restart. Inject a recoverable index fault, observe `/admin/catalogue/errors`, authorize reindex and await exact metadata via `/catalogue` or `/series`, preserving event/original identity. Include non-operator rejection. The replacement passed the complete `http_catalogue` fixture first run, 32.30s. SQL only arranges interrupted legacy indexing; HTTP diagnostics, denied/authorized retry, recovered metadata and unchanged event/original bytes are the outcomes. Runtime was unchanged; no behavioral red→green is claimed.
4. **G4, P0 bounded-response route coverage, representative gap closed:** `http_mixed_downloads` passes count and byte pressure across standard Blossom, public originals, approved-peer originals, private review and compatibility aliases. It checks domain/peer/reviewer/privacy denial, shared overflow, upload progress, ownership rotation and exact-byte recovery. Evidence: `results/rebuild-admission/mixed-count-green.log` (14.24s), `mixed-bytes-green.log` (66.25s), both exit0. The 120-second deadline remains proven once at the common response seam rather than repeated on every route. No exhaustive route/fault cross-product claim.
5. **G5, P0/P1 real browser durability/publication:** snapshot5 has narrow rendered local-save/export/restore evidence; final-client reload/offline/eviction behavior, multi-tab stale consent, real signer interaction and public recovery remain separately qualified. Model quotas/transaction aborts and 30k/500k codec tests do not establish native browser eviction, storage pressure or large-library responsiveness. Use the supported browser without lowering its security.


Final focused verification at19:55UTC: `composed-final.log` passes `http_migration` (8.83s) and `http_publication` (3.47s), exit0. Three-target Clippy and rustfmt pass. Exact test-source hashes and retained diagnostic chronology are in `results/rebuild-feature-seams/acceptance.json`. No production changes or canonical full-suite claim follows from this slice.

Additional final-seam cleanup at20:07UTC: `groups-public-first.log` passes `http_group_profile` (0.98s) and `http_group_retry` (2.09s). Private audit/account/event SQL outcomes are replaced by operator-authenticated HTTP audit reads, exact signed event/query/original reads, independently denied/explicitly admitted/revoked account posting, and authorized member publication. External audit-write and expired-grant faults remain. `enrollment-public-second.log` passes `http_enrollment` (1.38s): duplicate requests leave the next hourly slot available, two keys compete for one remaining lifetime slot, the lifetime limit survives restart/hourly reset, and closing signup preserves existing HTTP account/read permissions. The first enrollment compile attempt had a test-only borrowed-temporary error; no production failure or fix is claimed. The original fixtures are recoverable outside active tests. Scoped hashes and lint/format results are recorded in `results/rebuild-feature-seams/acceptance.json`.

## Deliberate boundaries, not missing core implementations

A finished catalogue/reader/social website, automatic vote totals/ranking, person-level Sybil resistance, NIP46 signer installation, universal MangaDex/Nostr compatibility, permanent group erasure, online backups, encrypted/HSM key custody, and production target-scale/security certification are not supplied by this reference implementation. The local-first functional browser library is required and exists; “website design deferred” must not erase its acceptance requirements. Immutable corrections remain deletion plus a new publication, with no automatic progress inheritance. Public libraries use selected replacement semantics, not automatic multi-device merge.

Checkpoint recovery is separate from feature acceptance. Verified earlier reconstructed snapshots are recorded in [CHECKPOINTS.json](CHECKPOINTS.json); checkpoint5 was copied during this audit, and its later upload/verification/delivery must be confirmed by the parent. No final release, all-failures guarantee, or achieved traffic target is claimed.
