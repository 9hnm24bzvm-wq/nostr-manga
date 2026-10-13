# Client and operator parity/refactor audit

Audit date: 2026-10-10. This report compares the current reconstructed tree with
`recovery/restored-oct08/manga-protocol-prototype`, the current issue graph/spec,
and actual implementations and executable tests. The lost frozen October 10
source is unavailable: **byte-for-byte or behavioral parity with that missing
tree cannot be certified**. October 8 parity, subsequent requirements, and new
acceptance are separate questions.

The initial audit was read-only, apart from small disposable CLI/IndexedDB-model
diagnostics. Confirmed issues were then addressed through focused client and
operator fixes with separate acceptance evidence below. No deployment, external
publication or application load test was performed for this audit.

## Findings first

| Priority/status                 | Finding                                                                                                                                                                                              | Outcome and observable evidence                                                                                                                                                                                                                                                                                                                            |
| ------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| P1, fixed in this slice         | Python accepted C1 controls in list names and emitted a local backup the browser refused.                                                                                                            | Shared normalization now rejects U+007F–U+009F as well as C0, without rewriting valid Unicode. Real local/public-plan CLIs feed the browser backup/graph interfaces. Red, green and 22 related checks are under `results/rebuild-name-parity/`.                                                                                                            |
| P1, fixed                       | Public recovery previously fetched every chunk with a fixed 1,000,000-byte allowance, ignoring the smaller declared descriptor size until the complete graph has been retained.                      | `recoverPublic` now bounds each response by its descriptor and rejects size mismatch before fetching the next object. Sixteen public recovery checks cover malformed signed graphs and external HTTP failures. Included in the combined 70-check gate; no hostile oversized workload was run.                                                              |
| P1, fixed                       | The serving supervisor's repeated `activate` previously acknowledged `active:true` solely from saved `phase == ready`, even if the recorded master has died. `status` and `drain` do check liveness. | Repeated activation now refuses a dead master with recovery-required guidance. Actual dead-master red and all eight real-nginx CLI checks passed in `results/rebuild-serving-parity/acceptance.json`; pending the next integrated gate. The defect was inherited from October 8.                                                                           |
| P1, fixed in storage supplement | Ordinary local writes were uncapped despite a 2,000,000-record backup/graph limit, allowing locally accepted history to become unexportable.                                                         | Schema 4 maintains the shared limit atomically at the local mutation seam without scanning on each save. Updates/deletes and older over-cap data are retained; explicit capacity diagnosis and bounded, distinct rescue export are provided. See accepted semantics below.                                                                                 |
| P1/P2, fixed in storage model   | Import `finally` clears staging on handled errors; a killed tab does not execute it. Normal reopen previously left abandoned staging until another import began.                                     | Reopen now reclaims only under the import Web Lock with `ifAvailable`; no-lock and active-import paths skip cleanup. External quota/File/lock fixtures prove recovered save capacity and preserved active-import records. Native killed-tab recovery remains separate.                                                                                     |
| P2, fixed UI contract           | The old `maxlength=128` measured UTF-16 units while the record contract admits 128 Unicode scalars.                                                                                                  | The control now admits 256 UTF-16 units and displays the 128-scalar/512-byte contract; actual save normalization still rejects invalid names. DOM red/green covers 128 emoji and 129 ASCII scalars. Pre-fix browser fill/paste did not reproduce truncation, so no causal native-browser red is claimed.                                                   |
| P2, confirmed and fixed         | Deferred recovery replaced local records after reconnect, forget, deselection or another tab's preference change, using an ambiguous count-only preview.                                             | Actual DOM actions plus a signed external HTTP adapter reproduced all four failures. Recovery now captures identity/generation/preferences, refuses stale previews, displays exact owner/origin/service key/event, and checks preferences atomically with replacement. Decline, local edits, reconnect-at-confirmation and commit-abort/retry are covered. |
| P2, retired with evidence       | The old graph writer assumed missing fields, and two one-off packaging helpers targeted obsolete archive names/layouts. None had a current executable caller.                                        | All three are preserved byte-for-byte as non-runnable `.py.txt` history and removed from active scripts. The exact baseline spec/graph/issues are separately preserved. Verified moves plus 55 Python/6 packaging passes are in `results/rebuild-workflow-retirement/`; see `WORKFLOW-HYGIENE.md`.                                                         |

The recovery and supervisor findings were reported immediately and have focused
acceptance. The storage supplement passed 70 model checks after build
`17e9c89ef2b0da1d`; the subsequent UI supplement passed 81 after build
`72e656def51db26f`. The earlier canonical gate at 20:38 UTC does not cover these
later fixes automatically. The next integrated gate remains required.

## Exact evidence boundary

### Family-statistics performance supplement

On the preserved schema-4 build `72e656def51db26f`, three native public `counts()` calls against the existing 500,000-chapter library took 2073.6, 2105.2 and 2075.8 ms; the corresponding 50-row `list()` calls took 4.9, 2.7 and 2.2 ms, all at revision 19. See `results/rebuild-browser/native-counts-baseline.json`. Concurrent compilation means this is a measured hot-path comparison on that host, not isolated throughput or a claim about Chrome's counting algorithm. Both 30k and 500k native exported record sets were verified before changing this build.

Schema 5 derives all family counts once during upgrade, retains their total in the same durable value, and reads statistics/revision together without repeated range counts. Record writes, deletions/cascades and both import formats update that value in their existing atomic transactions. The public interface, 2M admission bound, revision fences, existing over-budget data and pending publication bytes are preserved. No separate tab cache or new public API is added. `results/rebuild-family-statistics/acceptance.json` records the causal external-IndexedDB red, public migration/mutation/rollback/concurrency checks and the successful 91-check/build/format gate for `e4f4862bd75c26bb`. The initial combined attempt and its obsolete range-count pause are retained; the repaired DOM fixture pauses the paged read and keeps its original race outcome. Native schema-4-to-5 upgrade, repeated probe and export verification are separate acceptance and are pending until their own evidence is recorded.

### Earlier acceptance

- `results/rebuild-final/canonical-acceptance.json` records the optimized
  functional gate ending **20:38:27 UTC**, exit 0, with no changed inputs during
  that run. Its real integrations include migration/restore, serving, catalogue
  reindex, Node/browser-module interoperability and the real-image fixture.
- The later picker supplement is build **9b35e90a4531e30a**. Its **42 model
  checks**, formatting and build passed in
  `results/rebuild-client/backup-picker/acceptance.json`. It widened only the
  actual input's `accept` contract and added an actual Export-DOM regression.
- The storage/recovery supplement is build **17e9c89ef2b0da1d**. Its combined
  **70 model checks**, build and formatting passed in
  `results/rebuild-local-storage-parity/acceptance.json`. It includes the new
  storage/upgrade cases and recovery/codec cleanup.
- The UI supplement is build **72e656def51db26f**. Its **81 model checks** and
  exact changes are recorded in `results/rebuild-client-ui-parity/acceptance.json`.
  Prior supplement logs and hashes remain historical rather than being rewritten.
- Broad rendered-browser evidence remains tied to **17e72bdd1b58d37c**, in
  `results/rebuild-browser/acceptance.json`: local chapter/series/list/member
  operations, reload, actual download/confirmed restore, cross-tab progress,
  missing-signer refusal, and cached local operations after origin shutdown.
  This is **not** rendered acceptance of the later build, a manual native-picker
  filter check, a real signer-extension publication/recovery test, mobile QA,
  eviction/quota testing, or full network-offline emulation. Additional native
  browser checks preserved earlier records and saved/reloaded page 35 on build
  `17e9c89ef2b0da1d` (`results/rebuild-browser/parity-upgrade.json`). On final build
  `72e656def51db26f`, actual cloud Chrome preserved existing records, saved and
  reloaded a 128-emoji list name, and rejected 129 ASCII scalars without adding a
  list (`results/rebuild-browser/final-parity-ui.json`). Recovery-consent races
  remain DOM-model evidence, not real signer-extension acceptance.
- `results/rebuild-client/browser-client-backend.json` exercises the actual JS
  modules against Rust/PostgreSQL/strfry under Node with modeled IndexedDB. It is
  valuable transport/codec evidence and is not an installed browser extension.
- `results/rebuild-client/068-backup-large.log` completed a 500,000-list,
  294,500,060-byte import and complete export in 98 seconds, with bounded 64 KiB
  I/O parts but **2,411 MiB highest sampled RSS**. This does not establish low
  overall memory or native-browser responsiveness. It was not repeated here.
- `results/rebuild-real-images/006-final-run.log` and `acceptance.json` establish
  four genuine Met JPEGs (4,646,237 bytes), source-off receiver API/nginx byte
  equality, restart/progress and immutable delete/reupload. This is functional
  retention evidence, not a load or browser-image-rendering measurement.

## Method and requirements used

Inspected the full current browser source, build command, HTML forms, model/DOM
tests and Node backend driver; Python operator modules and their subprocess
tests; current package/test/index/graph scripts; the managed-serving Rust caller;
and the relevant real HTTP migration, catalogue, group and image fixtures.
Compared browser and operator files with the recovered October 8 tree. An AST
comparison established that `serving_supervisor.py` had no semantic changes at
the start of this audit; most of its visible diff was formatting. The backup
changes are preflight extraction and stricter paused-restore policy, not a new
backup protocol. Retired Python research/model modules have no new runtime
replacement obligation merely because they used to reside under `src`.

Requirements consulted: `.scratch/federated-manga-postgres/spec.md` and graph
tickets 13–15, 25, 29, 31, 37–39; `docs/browser-client-plan.md`,
`library-client-delivery-contract.md`, `postgres-backup-restore.md`,
`managed-public-serving.md`, `group-key-backup-recovery.md`; rebuild `PLAN.md`,
`OPERATOR.md`, `ACCEPTANCE-MATRIX.md`, and current acceptance artifacts. Historical
checkmarks and old counts were treated as claims to trace, not proof.

The requested design principles were read from the official Matt Pocock sources:
[codebase design](https://raw.githubusercontent.com/mattpocock/skills/main/skills/engineering/codebase-design/SKILL.md),
[deepening](https://raw.githubusercontent.com/mattpocock/skills/main/skills/engineering/codebase-design/DEEPENING.md),
[TDD](https://raw.githubusercontent.com/mattpocock/skills/main/skills/engineering/tdd/SKILL.md),
[test examples](https://raw.githubusercontent.com/mattpocock/skills/main/skills/engineering/tdd/tests.md),
and [mocking](https://raw.githubusercontent.com/mattpocock/skills/main/skills/engineering/tdd/mocking.md).
This audit uses their small-interface/deep-module criterion and tests operations
through caller-used seams; a public export alone does not make an internal helper
the right acceptance surface. Fault injection belongs at external dependencies.

## Browser parity inventory

| Capability                            | Code and actual outcome tests                                                                           | Parity/result                                                                                                                                                                                                                                                                                                          |
| ------------------------------------- | ------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Four local families and exact IDs     | `store.js:38–204`, `codec.js:136–233`; store/app DOM tests and operator interoperability                | Retained. Chapters preserve nullable page, completion and timestamps; six series states, exact list names/IDs and list-member positions remain. No account/network required.                                                                                                                                           |
| Local CRUD and list-member cascade    | `store.js:38–118`; `store.test.mjs`, `app-dom.test.mjs`                                                 | Retained. List removal removes its memberships and preserves series; transaction completion/abort handling improved. Membership does not require a local parent record, matching the prior permissive data model.                                                                                                      |
| Pagination and late rendering         | `app.js:46–103`; `render-race.test.mjs`                                                                 | Improved. Pages remain at most 50 visible rows; generation checks reject late family/reset/outbox responses. Exact population counts do not imply a multi-page snapshot during concurrent edits.                                                                                                                       |
| Local-only default/explicit consent   | `app.js:286–446`, `publishing.js:34–117`; app DOM, publication UI, backend driver                       | Retained and strengthened. No startup signer/publication; remembering an origin is not automatic publication. Snapshot confirmation names owner and recipients. Later local writes do not mutate the signed job.                                                                                                       |
| External signer and exact identity    | `nostr.js:35–75`, `app.js:372–387`; signer/nostr/publication tests                                      | Improved. Exact event-field/signature binding, owner substitution refusal, 60-second monotonic deadline, late-approval non-transmission, and failed reconnect preserving the prior complete identity. No secret key stored.                                                                                            |
| Service continuity and transport      | `nostr.js:82–232`; backend driver                                                                       | Retained. HTTPS or loopback origin, credentials omitted, redirect refusal, fresh nonce/key proof, exact request/body/status response proof and object hashes. First observation proves possession, not reputation.                                                                                                     |
| Durable outbox/retry/cancel           | `outbox.js`, `publishing.js:118–258`; outbox/publication/Node backend tests                             | Improved. Metadata and immutable objects separated; 8 jobs, 8 recipients, at most 64 objects/4 MiB per read; explicit retries restage expired leases; accepted/superseded recipients skip resend; cancellation is sticky and deleted jobs cannot resurrect.                                                            |
| Preference races and per-owner clocks | `outbox.js:63–110,150–195`, `store.js:222–243`; publication/DOM legacy/race tests                       | Improved. Atomic preference revision/cancellation and before/after-sign fences; per-owner clock survives identity switches, job deletion and legacy migration. Forget cancels entire unfinished snapshots containing that instance, as the UI explicitly states.                                                       |
| Public recovery                       | `publishing.js:259–309`, `app.js:163–193`, `store.js:138–204`; backend driver plus clock/revision tests | Retained. Signed head, expected owner/service, content hashes and graph checks; preview and local revision fence; recovered clock advances later publication. The new negative recovery/descriptor-size checks passed in the combined current-client gate.                                                             |
| JSON backups                          | `backup.js`, input/export controls; backup tests and picker regression                                  | Improved to v2 JSONL with v1 input compatibility. 64 KiB lines/parts, 4 GiB file, 2M records; duplicate/malformed/truncated input and stale replacement fail without replacing current data. v1 inputs remain capped at 256 MiB. Backups contain records, not private keys, relay preferences or publication metadata. |
| IndexedDB migration and aborts        | `store.js:6–24`, `indexeddb.js`, `outbox.js:8–21`; frozen v1 fixture, reopen/abort/quota tests          | v1 records and queued bytes remain readable after upgrade. Existing pending values are not rebuilt from live library data. Unknown lost schemas and browser power loss are not certified.                                                                                                                              |
| Cross-tab and offline                 | BroadcastChannel in store/app; transaction/race tests; prior rendered acceptance                        | Local record changes refresh other tabs. Native Web Locks fairness/exclusion is not modeled by a callback adapter. Preference/job views do not broadcast all changes, but stale writes/signing are fenced; stale display is a UX issue rather than an authority bypass.                                                |
| Published build                       | `build.mjs`, actual CLI fixture                                                                         | Improved. Fresh staging, old-output preservation on missing input, obsolete asset removal, path/length/content cache identity, rollback and next-invocation recovery. Two renames have a brief absence gap; concurrent builds and power-loss-atomic exchange are not promised.                                         |

### Deliberate interface changes, not hidden parity claims

The package remains version 0.1.0, but consumers of the earlier browser modules
must adapt these deliberate safety changes:

- `job(id)`, `jobs()` and `queueSnapshot(...)` return job metadata, including
  `objectCount`, without an embedded `objects` byte array. Do not rebuild a queued
  publication from the current local library: its approved signed content is immutable.
- Read its original bytes with `jobBatch(id, cursor)`, starting at `0`. Process
  each returned `objects` array before requesting `next`; an empty array ends
  iteration. Each batch contains at most 64 objects and 4 MiB. A missing or
  cancelled job rejects rather than returning a resurrectable snapshot.
- `saveJob(job)` updates an existing job's recipient progress fields (`cursor`,
  `state`, `message`, `projected`) and sticky cancellation only. It rejects
  missing/deleted jobs or changed recipient origins/service keys. Attempted
  replacement event, revision or object bytes are not persisted.
- Use explicit-consent `queueSnapshot` to create a new signed publication;
  `saveJob` is no longer an upsert. Use `setRelays`/`relayPreferences` for instance
  preferences rather than the removed arbitrary `setMeta` writer.

The same migration note is in `browser-client/README.md`; the historical return
shapes are not restored because they undermined immutability and cancellation.

The browser remains a manually selected, pinned-origin reference client with
sequential recipient attempts and recovery from one chosen origin. The Rust SDK's
NIP65 discovery, first-acceptance concurrency and newest-complete multi-origin
recovery are not browser features. Ticket 25 explicitly scopes those requirements
to the Rust SDK; they must not be reported as either a newly removed browser
feature or full browser/SDK parity. A finished chapter-reader/catalogue/social UI
is also not supplied by the local-library reference forms.

## Operator parity inventory

| Workflow                                | Implementation and executable seam                                                                | Result/qualification                                                                                                                                                                                                                                                                                                            |
| --------------------------------------- | ------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Exact-ID local migration                | `local_library_migration.py`, `library_records.py`; `test_local_library_migration.py`             | Added direct local-only backup path, explicit mapping/private-list opt-in, no signing/network, no partial backup on unmapped data. Name mismatch found and fixed.                                                                                                                                                               |
| Reviewed public migration               | `mangadex_migration.py`, `postgres_migration.py`; CLI tests and `http_migration`                  | Preserved original bytes/order, labels, source attribution review, exact mappings, explicit apply/public-history consent, pinned service proof, bounded reads, journal identity and idempotent replays. Private lists excluded from publication.                                                                                |
| Private operator artifacts              | `operator_json.py`, packet writers; subprocess umask/exclusive/concurrent/interrupted-write tests | Shared duplicate-safe 64 MiB JSON input and private atomic output. Temporary files and final outputs have controlled permissions. No blanket claim for hostile concurrent parent-directory replacement or every fsync failure.                                                                                                  |
| Stopped-node backup                     | `postgres_backup.py:129–429`; `test_backup_cli.py`, real `http_migration`                         | Preserved explicit owner locks, PostgreSQL snapshot/dump, service/group keys, independent originals/backend copies, healthy receipt/ledger checks, manifest/completion artifact. Backup is offline, co-located and plaintext private data, not checkpoint-source packaging.                                                     |
| Restore preflight and activation policy | `postgres_backup.py:432–588`; backup CLI negatives and composed real migration fixture            | Improved preflight before DB contact/output, path/symlink/size checks repeated during copy, hash verification before pg_restore, empty DB/new directory, matched backend binary, new job/auth generation, cleared derived state and paused policies. Source remains untouched and target does not start a server.               |
| Restored application behavior           | `server/tests/http_migration.rs:588–1119`                                                         | Actual reads preserve identity/library/originals/tombstones/revocations; stale worker/auth rejected; explicit audit/repair, new group-signed publication, nginx activation and separately enabled recurring sync work. Inspect each fixture outcome, not helper counts.                                                         |
| Catalogue reindex                       | `node/catalogue_http.rs`, `http_catalogue.rs:430–520`                                             | Unauthorized retry denied; operator-visible diagnostic followed by authorized reindex restores exact metadata without changing signed event/original bytes. This interface is HTTP, not a missing Python CLI command.                                                                                                           |
| Managed serving lifecycle               | `serving_supervisor.py`, Rust `ManagedServing`/role; real nginx Python/HTTP tests                 | Prepare/activate/suspend/drain/stop, EOF ownership, pidfd/birth/boot/namespace checks, slow unlinked readers, wrong/duplicate owner, dead master and SIGKILL recovery are implemented. Repeated activation now rejects a dead master; eight real-nginx CLI checks passed in the serving lifecycle supplement.                   |
| Offline group-key backup                | `manga-key-tool`, `offline_keys`, real group fixture                                              | Existing key only; identity/mode/format checks, exclusive copy/readback, no network/private stdout; key-only import does not recreate membership and requires explicit bootstrap. Encryption/HSM custody and permanent group erasure remain outside this tool.                                                                  |
| Source packaging                        | `scripts/package.py`, six actual CLI tests                                                        | Replaced old hardcoded packaging with a current allowlist, explicit reviewed evidence, private/runtime exclusions, symlink checks, CRC/SHA verification and same-directory replacement. Generated public client is a separate artifact. Historical load-split helpers should not be mistaken for this interface.                |
| Canonical test/operator entry points    | `scripts/test.sh`, `test-postgres.sh`, CLI tests                                                  | Real integrations are invoked with ignored fixture tests included, isolated databases and native binaries; ordinary `cargo test` alone is insufficient. The runner builds before service-worker tests; actual-checkout API-index freshness/link tests are recorded in `results/rebuild-parity-tests/operator-parity-final.log`. |

No missing required operator backup/restore/reindex/serving family was found.
Missing failure combinations and the specific bugs above still matter. A backup
interrupted after pg_restore but before the final fence/config artifact is not a
completed restore and currently requires a fresh target/operator cleanup; there
is no advertised resumable cross-resource restore transaction. Do not start a
partially restored instance manually or promise cross-host reader control.

## Refactor and test-seam assessment

### Deep modules worth retaining

- The outbox owns immutable bytes, cancellation, preference fences and bounded
  continuations behind operations that callers actually use. The split removes
  expensive graph copies from progress updates; it is not merely more files.
- The transaction adapter centralizes completion/abort lifetime and secondary
  rejection handling. Tests correctly exercise local operations with external
  IndexedDB faults rather than testing that helper's private steps.
- Backup import owns parsing, validation, staging and commit. Its public result
  and preserved old records are the useful oracle. The ownership-safe reopen
  slice now lives with the backup lifecycle rather than leaking lock names into
  the opener. Native process-crash acceptance remains separate.
- Operator JSON and shared record normalization have two genuine production
  consumers and remove divergent policy. The C1 defect demonstrates why
  cross-language acceptance still belongs at the actual consuming interface.
- Managed serving legitimately contains process/pipe/fsync/kernel complexity:
  this complexity would otherwise leak into the node. Preserve that depth while
  fixing truthful lifecycle responses, rather than making each syscall a public
  module or mockable interface.

### Worthwhile locality improvements, without another framework

1. `LocalLibrary` still exposes raw `db`, `channel`, arbitrary metadata and
   outbox internals; backup and app callers know the database name, stores and
   broadcast mechanics. Hide this knowledge incrementally behind actual
   snapshot/backup/preference operations when touching those workflows. Do not
   add a generic repository/event-bus layer merely to hide five forwarding lines.
2. The storage supplement now shares the local admission/backup constant through
   `limits.js` and maintains the invariant transactionally. Keep public graph
   limits a distinct wire contract; do not generalize one numeric coincidence
   into a cross-layer configuration framework.
3. Browser `snapshot()`, graph construction and the UI's `backupBlob()` retain
   complete materialized state. Small I/O parts are not a constant-memory
   workflow. Document this and test realistic devices before designing a new
   streaming download/graph pipeline; do not infer that the 4 GiB file cap is a
   practical mobile capacity.
4. App module globals make repeated boot/reconnect/recovery lifecycle ownership
   implicit. A small mounted-app lifecycle interface could own connection close,
   event subscriptions and operation generations if required by real embedding
   callers. Avoid inventing it solely for tests; current DOM tests already
   exercise useful public controls.
5. Workflow cleanup is now explicit: the unused graph writer and historical
   packaging helpers are retired with byte-preserved originals. No replacement
   graph writer or publication/storage policy change was needed. Current-status
   navigation remains separate from preserved historical counts.

### Test quality closure and remaining qualifications

- Most active browser tests use LocalLibrary, backup/publication operations, DOM,
  public codecs and the actual build CLI. IndexedDB/time/Fetch/Worker/File faults
  are external adapters. Outbox `objectCount`/bounded batches are intentional
  caller-visible contracts, not private SQL/storage-layout assertions.
- The direct tuple-helper tests have been replaced by actual `readGraph`
  decoding of malformed wire tuples, and the unused helper export was removed.
  The combined 70-check gate covers this closure.
- Obsolete `setMeta` was removed after caller migration to the actual
  `setRelays`/`relayPreferences` interface. Frozen legacy database fixtures remain
  justified for schema migration; no current private metadata outcome is needed.
- The backup tests' external IndexedDB aborts and the operator tests' external
  command witnesses are legitimate. A dummy manifest SHA in preflight cases
  does **not** automatically mask the intended failure: the no-database-contact
  oracle is evaluated before later hash verification. Improve positive controls
  where useful without claiming a defect that the call order rules out.
- MangaDex path rejection targets now use valid
  external PNG controls. See `docs/rebuild/PARITY-REFACTOR-TESTS.md` and
  `results/rebuild-parity-tests/operator-parity-final.log`; these operator checks
  are separate from the client model gate.
- `scripts/test.sh` now builds before consuming generated `public/sw.js`. The
  public negative-recovery matrix and exact 30k/500k field fidelity passed in the
  combined current-client gate. Causal recovery logs are
  `results/rebuild-parity-tests/recovery-red.log` and `recovery-green.log`.
- Missing actual-browser acceptance remains meaningful: native Web Locks,
  upgrade/reopen across old tabs, quota/eviction, signer-extension consent and
  cancellation, recovery races, mobile layout and large-library responsiveness.
  Model callback scheduling cannot certify these properties.

## Accepted bounded-storage and orphan-recovery semantics

Implemented after the initial findings with separate causal red/green slices.
`results/rebuild-local-storage-parity/acceptance.json` records the exact logs,
source identities and combined 70-check gate; the earlier gate is not reused.

1. Keep the current **2,000,000 total local records** as the documented backup
   admission limit. Initialize a durable total once during a schema migration,
   then maintain it in the same transaction as records/revision on write,
   removal, local replacement and staged import. Do not scan the entire library
   on every save.
2. At cap, a new record rejects without changing records or revision; replacing
   an existing key and deleting still work. Competing tabs serialize through the
   same transaction, so two inserts cannot both consume the final slot.
3. Preserve older over-cap libraries exactly. Permit read/update/delete and
   reject new keys until back under cap. `counts()` exposes total/limit/over-budget
   diagnosis. Normal backup refuses over-cap data; `exportRescue` streams bounded
   pages/lines with a distinct non-importable header and revision fence. Its sink
   and explicit splitting procedure are documented in the browser README. There
   is no one-click streaming rescue/partition UI, automatic merge or truncation.
4. On normal reopen, request the same named per-database Web Lock as v2 import
   with `ifAvailable`. If unavailable, skip staging cleanup and let local open
   proceed. If granted, clear only staging transactionally and release. An
   active importer can never lose staging to another opener. If Web Locks are
   unavailable, do not guess ownership or clear staging.
5. Seam tests observe preserved records, admission/retry, explicit capacity
   diagnosis and complete importer output. Boundary population is modeled through
   an external IndexedDB count response; actual small legacy rows independently
   validate derivation. Quota and interrupted-File adapters arrange leftover
   storage. No private counter or staging-table value is an outcome oracle. A
   real native process-stop/reopen case remains a separate qualification.
6. Blocked schema upgrades now reject with close/reload guidance through public
   open and the existing app error notice. Already-started old-tab transactions
   finish; admitted data and v1/v3 pending object bytes survive. An issued upgrade
   cannot be canceled; a later successful connection after rejection is closed.

## Narrow Unicode fix: actual red → green

The related contract is identical in browser `normalize('lists')` and Rust
`Record::validate`: nonempty, at most 128 scalar characters / 512 UTF-8 bytes,
no Unicode Cc controls. Unicode format characters such as ZWJ are not blanket
forbidden and names are not normalized, stripped or renamed.

Before the fix, an actual `python3 -m src.local_library_migration` probe exited
0 for `Before\u0085After`; actual `LocalLibrary.replace` rejected its output as
`Invalid list name`. The new CLI-to-browser seam test first preserved seven
valid names exactly in both backup import and public graph decoding, then failed
on U+0080, U+0085 and U+009F being accepted. The production change is only the
missing C1 range in `src/library_records.py`.

- Red: `results/rebuild-name-parity/001-red.log`, exit 1.
- Green: `002-green.log`, exit 0.
- Related local/public-plan/MangaDex regression: `004-related-cli-regression.log`,
  **22 passed**, 4.01 seconds. Subsequent test edits are not certified
  by that run.
- Source/test formatting: `005-format-check.log`, exit 0.
- Changed production/test paths: `src/library_records.py` and
  `tests/test_local_library_migration.py`. No backend or browser source changed
  for this fix. Later changes require the final integrated gate.

## Historical documentation and workflow retirement

Recovered requirements and all 39 original tracker files are preserved under
`requirements-baseline/` with exact source hashes. Old graph statuses are not
current acceptance. Historical reports now have dated scope/current-navigation
banners; the original matrix is explicitly the 20:07 snapshot. Its old counts
and snapshot5 statements remain historical evidence rather than being rewritten.
The three unused workflow helpers are archived under
`docs/history/retired-workflow-scripts-20261010/`; their active copies were removed
only after byte/hash verification. `WORKFLOW-HYGIENE.md` and
`results/rebuild-workflow-retirement/` document this completed cleanup. The next
combined gate must include the new inventory and subsequent runtime corrections.

For the native input length point, the relevant external rule is the HTML
[form-control length contract](https://html.spec.whatwg.org/multipage/form-control-infrastructure.html#limiting-user-input-length:-the-maxlength-attribute).
This standards/source mismatch was identified without claiming native truncation
was reproduced. Pre-fix supported browser fill/paste accepted 128
emoji despite the old attribute (`results/rebuild-browser/scalar-input-before.json`);
no list was submitted during that before-state observation.

## Recovery consent and Unicode UI supplement

`results/rebuild-client-ui-parity/001-name-red.log` is a DOM/HTML-contract failure,
not a native typing-truncation reproduction. `002-name-green.log` accepts the
exact valid 128-emoji name through the actual form and rejects 129 ASCII scalars
through the existing normalizer. Final native cloud Chrome acceptance also saved
and reloaded the exact 128-emoji name and rejected 129 ASCII scalars without an
added list, preserving prior records (`results/rebuild-browser/final-parity-ui.json`).
No stored or wire record schema changed.

`003-recovery-red.log` reproduces four actual local-data replacements after a
changed identity or preference. `004-preview-red.log` additionally proves that
the old preview omitted source identity and that a preference change at the
confirmation boundary could still apply. `005-recovery-green.log` and the later
UI/full gates close these failures through actual DOM controls, real signatures
and external Fetch/signer/IndexedDB adapters.

The UI retains the selected signer object, owner and connection generation,
and checks them plus the saved preference revision before showing the preview.
The preview names owner, origin, service key, event ID and signed timestamp.
After explicit confirmation it rechecks identity; `LocalLibrary.replace` also
checks the reviewed preference revision inside its existing records/meta
transaction alongside the local revision. Reconnect attempts invalidate pending
reviews; during the short commit a reconnect cannot change identity, including
an earlier pending signer's completion. A `finally` releases the busy state.
Cancel, abort, failure and successful retry outcomes are exercised. The
preference transaction defines ordering against concurrent tabs; no background
recovery or automatic replacement was added.

## Acceptance status at 2026-10-10 21:54 UTC

The combined optimized parity attempt ended with exit 101 at 21:51:19 UTC,
with unchanged inputs (`results/rebuild-parity-final/canonical-result.json`).
Two `storage_fairness` cases failed; a shared database fixture collision was
identified. Storage-fixture isolation and HTTP read-ownership corrections require
a new frozen combined run. The focused passes in this report do not establish
full refactor parity or production capacity.

Native Chrome additionally imported, edited, reloaded and exported a four-family
library with 30,000 chapters (30,202 total records), comparing every exported key
and field against its deterministic input. Evidence:
`results/rebuild-browser/native-30000-export.json` and
`NATIVE-BROWSER-QUALIFICATION.md`. Native 500,000-chapter qualification is still
pending at this update; neither model tests nor prepared input substitute for it.
