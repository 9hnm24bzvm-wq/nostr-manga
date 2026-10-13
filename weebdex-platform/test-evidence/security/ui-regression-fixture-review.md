# Independent native UI regression fixture review

Reviewed 2026-10-13 09:37–09:43 UTC. **Finite implementation verdict: the four named native UI controls are source-clean at the exact boundary below. No additional concrete implementation blocker was established.** The stale source index must be reconciled before this boundary is used for a runtime launch. This is a source review, not runtime acceptance or launch authorization.

No fixture, PostgreSQL, platform, original backend, provisioner, Node, Chromium, Cargo or test suite was launched by this reviewer. Work consisted of source/document/receipt reads, hashes, a non-executing Python AST comparison and writing this report. No implementation source was changed.

## Exact reviewed boundary and compiler provenance

- `tests/runtime/browser_fixture.py`: `69e5827b634925773ee53f03925f6c0f0dca0ad32834ab281b03d7175c2d17a1`
- `backend/examples/browser_fixture_provisioner.rs`: `59a3fcbbf65a096c82f61ca0b1d51c790b49543c9130026effba2eb4b2ae7845`
- `backend/examples/browser_fixture/ui_regressions.rs`: `bd196125d87c927702d821b277291b208247208cd0ed9f1097777efa0920d2f9`
- `docs/browser-fixture-contract.md`: `c058981918eb6782378300150e94721ac4e654459c6b16be5e562a2e24e8d579`
- Candidate017 `candidate-source.json`: `bb3dea91be2d4dbb52666b6007346a64699ee75976b0c47714b6449288e069fe`
- Actual compiled example: `46003dcdc97b48276a2b30ff603467adf902a8e64c447a43816d339fe5d5e2bb`

Compiler evidence lives in `../weebdex-platform-build-environment/synchronous-restore/results/bridge-authorized-017/`. Its candidate manifest independently matches both final Rust source hashes. The actual example under `targets/platform/debug/examples/browser_fixture_provisioner` independently matches the binary hash above. `cargo-check-fixture-003-result.json` hashes to `b58471e6b6b6ec6db8c674879c62ab1a60b250aa84792fc898609cc859da86ef`; `cargo-build-fixture-003-result.json` hashes to `87f1154fb1f370c176124349f0acf9e7a501a7b671d8544a2b40235d1cdd8130`. Both record passed/exit0, unchanged inputs, a reaped leader, no remaining owned members and no abort or cleanup error. The codegen log records the normal example build finishing successfully at 09:42:13. These are existing compiler receipts, not compiler work reproduced by this reviewer.

Earlier helper hashes `86893a0b…`, `9a713c6f…` and earlier example hashes are superseded for this review. The final revision includes persisted uploader identity checks, group discussion roots, genuine seed-time preference recency, bounded object/header checks and a persisted search-title/JSON byte budget.

## Narrow baseline comparison

The retained checkpoint0758 supervisor independently hashes to the previously reviewed `70401b969911da6b44bec5f41531476b5d425fcb1c9b2a4354e46839d2930c59`; its provisioner hashes to `1ee707435069e7c8925efb8247c6b99244d043935cc2b5470c4fb58c556e842a`. Full textual diffs against those exact copies were inspected. Only `Fixture.__init__`, `protocol_setup` and `fixture_controls` change; every other Fixture method has byte-identical source. Top-level additions are the four-name timeout map, optional CLI flag and its requirement for the reviewed provisioner. The Rust entrypoint diff consists only of the fixture module, optional scenario field and four explicit dispatch arms.

The production `backend/src`, migrations, Cargo manifest and lockfile are byte-identical to that checkpoint. The extension adds no production endpoint or dependency. BF-01–BF-06 retain their previously reviewed source mechanisms: exact birth-identity/pidfd and adopted-session ownership; phase budgets; sanitized environment without NODE_PATH; immutable complete-manifest snapshot verification; Popen-owned waits serialized against orphan reaping; monotonic forced history and nonzero unclean-terminal exit. No lifecycle regression is present in this narrow diff.

## Private authority and mutation scope

The optional flag enables exactly seed/readers/status/withdraw. Startup does not seed bulk rows. The queue keeps its private 0700 containing root, bounded non-link 4096-byte command, safe unique request ID, explicit active-browser phase acknowledgment and actual allowed-action check. At supervisor lines702–716, only request_id/action/browser_phase/scenario are accepted for these controls; seed requires pagination, withdraw requires terminal or entity, and readers/status reject scenario fields. Event IDs, database details, SQL, object paths, user IDs, arbitrary targets and listener-restart fields cannot pass this UI-control schema. The supervisor supplies its own UI origin.

The provisioner retains deny_unknown_fields, bounded regular request files, canonical request containment, a direct private reserved browser-final root and fixture marker. Its connection guard requires the exact password-free disposable loopback database and recorded non-default reserved PG port. These actions select the primary disposable database, never a recipient or deployment database. Dynamic SQL identifiers in the helper are selected only from hardcoded group/author tables; all data values are bound parameters. Root descendants and file names come from constants or verified hashes, never the queue.

## Genuine native data and removal

The helper writes real persisted native catalogue data. Each chapter batch obtains its persisted verified actor, calls production ensure_upload_target and publish_chapter, and adds real chapter/media joins. There is no HTTP response adapter. Protocol source/event associations are absent. Retry collisions must match the recorded fixture identity; staff permissions are read from the current disposable users table rather than claimed by the queue.

Hardcoded batches contain505 Spanish compact chapters,101 French terminal chapters and101 isolated Japanese entity uploads. Their timestamps make chapter000101 the terminal edge of the corresponding newest-first101-row input. The10,000 search titles are metadata-only, with exact persisted count and at most4MiB measured title-plus-JSON data. Only actual seed-time preference recency is written; no popularity or activity value is fabricated. Most Read acceptance requires genuine browser analytics posts.

The portrait object is the real SeedChapt2 page0 PNG: verified lowercase storage hash, canonical ancestor before reading, regular non-link file, at most32MiB, exact persisted byte length/digest,600×900 PNG header before full decoding and matching decoded/persisted dimensions. Bulk chapters reuse it. Reader activation adds genuine English/Spanish alternatives, four portraits and eight mixed pages, with generated900×600 landscape PNG data limited to1MiB. Landscape writes require the canonical fixture object root/shard, regular bounded existing objects or create_new private files, matching existing bytes and a real media-object row.

Withdrawal maps terminal/entity only to UiTm000101/UiEn000101. Under a row lock it verifies manga, persisted uploader, fixture marker and absence of a protocol_event_id. The current verified staff actor must have chapter.remove; CatalogRepository::delete_chapter rechecks production authority and records the real soft-hide/version/audit mutation. Replays do not invent a second deletion. Returned current_total values are actual production-visible database counts, not a decremented fixture counter.

Usage nuance: reader activation makes the global Spanish chapter feed505→506 by adding a Spanish chapter under UiBiM00001. UiFeedM001 itself remains505, and its current_total correctly counts only that manga. Static total is the original scenario size; current_total is the observed post-mutation size. Runner assertions must use the appropriate scope.

## Budgets, rebuild and evidence truthfulness

Seed/readers get120-second provisioner deadlines; status/withdraw get30. Each admitted action requires its deadline plus20 seconds in both the active phase and fixture window. Existing owned-child timeout, force history and cleanup remain in force; timeout or nonzero exit cannot publish success. Successful supervisor receipts require fixture_only/native_only and the exact scenario marker, and expose both operation data and ui_regressions. A descriptor is setup/status evidence, never proof a browser assertion passed.

A real catalogue rebuild still joins listeners and migrates only the disposable platform schema. The new branch clears supervisor native metadata and both exact private descriptor files; it rejects unsafe descriptor files and requires a fresh seed. No arbitrary unlink target is accepted. Native sessions/users remain intentionally disposable. Original signing authority, service identities, account admission, original storage and key lifecycle are unchanged. No signer key, authentication request body, recovery link or runtime database was read or included in this report.

## Finite remaining gates

At review time `ui-regression-fixture-source.json` still hashes to `e01b12af259b779c0612983f046dc8c0a734946c984e3436167e5595361260f7` and pins the superseded86893a0b helper. `backend-current-build.json` also retains an older example entry. Reconcile current metadata to candidate017 and the final example before launch; do not use those stale pins as current compiler provenance. This is the sole source-established coordination blocker at this boundary, not another implementation finding.

Fresh final frontend complete-manifest/index identities and parent runtime clearance remain required. Actual acceptance must observe genuine UI pagination/shrink/readers/search/preferences behavior, final control receipts and terminal lifecycle evidence. The four-control extension contains no genuine home-grid503 dependency-fault seam. This report approves no503 evidence or later supervisor revision. A separately reviewed real dependency fault is required for that case; fabricated successful/failed HTTP responses do not establish it. Runtime databases, private control/authentication material, server/signer secrets and licensed images/artwork-bearing screenshots must remain excluded from checkpoints.
