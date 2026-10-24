# Approved staged scheduling design

Replaces all earlier draft options. Staged implementation was approved on 2026-10-22 and the isolated backend candidate now has 13 passing actual HTTP fixtures plus 163 passing unit tests. See [the qualification record](scheduled-publication-qualification-20261022.md) for exact source, owner and binary associations and remaining composition gates. Original base: 03e5f1797f648ea3cdd9b2eb2ff89eac3c20a46b.

## Contract and date

Pinned upstream MangaDex 5.13.1, commit 60a3bb3c9f25675435a479c925d857015201739e, SHA256 004031ccdfc4b66727dd1f85d09b5300cf60235bbab2de6f6ea4816094bd6d33: ChapterDraft.publishAt is optional nonnullable exact YYYY-MM-DDTHH:MM:SS; ChapterEdit has no field. POST /upload/{session}/commit returns Chapter directly. Pattern permits some impossible dates, which must fail semantic calendar validation. Reject null, offsets, Z, fractions, duplicate keys, impossible dates, leap seconds. Omission preserves immediate behavior and native serialized payload/hash. Past or equal DB-now value executes immediate publication, not backdated source signing.

Pinned spec lines 1934/1940/1946 and 2216/2222/2228 explicitly define the identical timestamp form for chapter date filters as UTC+0. ChapterDraft itself has no timezone prose. Therefore UTC for publishAt is a consistency inference, to be explicitly documented as this instance's contract, never parsed with process/session/local timezone. SQL conversion uses timestamp AT TIME ZONE 'UTC'; rendering explicitly UTC. Web lookup did not establish more specific authoritative upload prose.


## Approved narrow lock graph

Use a separate bounded scheduled pool over cross-native aggregate enforcement. Actual existing policy is per-session originals only, not per-owner aggregate. Retain that unchanged. Cap total nonterminal scheduled originals+derivatives at500MiB and count32, with30-day horizon; this is explicitly additional bounded scheduling retention. It does not claim to preserve a nonexistent aggregate owner ceiling. It avoids adding advisory locks or changing native attach/replace/begin policies. Same-owner schedule admission and cancellation serialize through the already-required users NO KEY UPDATE transaction fence. Worker uses identical user locking. Different-user schedules do not share budget rows.

Graph (-> means lock acquisition before):

PUBLIC admission/cancel: verified request outside transaction -> node_identity SHARE -> account_permissions SHARE -> protocol_identity_bindings SHARE + users NO KEY UPDATE -> upload_sessions UPDATE -> scheduled intent UPDATE if existing -> catalogue parent/group validation. Native ordinary attach/replace retain existing actor authentication-before-transaction and session/file locks; they neither acquire scheduled budget locks nor scheduled-intent authority. Native cancel cannot cancel scheduled state. Public schedule cancel handles scheduled state with the above graph only.

CLAIM: scheduled intent UPDATE SKIP LOCKED -> token/lease update -> COMMIT. It must acquire no user/session/catalogue lock, and must not await actual execution before committing. Identity values read for execution are only selectors, not an actor.

DUE gate/start: node_identity SHARE -> account_permissions SHARE -> binding SHARE + users NO KEY UPDATE -> scheduled session UPDATE -> intent UPDATE -> direct parent UPDATE for new upload, or existing chapter UPDATE for edit -> catalogue attribution groups SHARE in deterministic order -> pinned publisher_permissions SHARE or managed_groups SHARE. Validate due via DB clock, token, input revision/hash, current account policies, exact source/key/group identity and version. Write one-way publication-start marker and COMMIT before source operations. No source command is called inside this marker commit. This separation makes the start boundary durable without an independent intent update deadlocking against an outer lock.

DUE execute: reacquire the same full graph and repeat all current authority/due/token checks; only then construct sealed worker actor and call shared core with reserved native chapter ID and scheduled-only admission capability. Same-owner cancellation always sees started and refuses; stale workers cannot change the token. An execution transaction holding session/intent keeps lease-expiry takeover blocked. The long-lived section is existing bounded local publication, not a network request: runtime.publish_operator/group calls local descriptors, filesystem storage, source DB publication, DB receipt. Peer synchronization is a separate method, not called; network replication proceeds through outbox later. This preserves original immediate locking instead of copying stale grants across a network boundary.

COMMAND prepare (separate connection while execute tx lives): commands INSERT (users/group FK KEY SHARE) -> command UPDATE -> media_objects KEY SHARE -> alias machinery. It never references/locks upload session or scheduled intent (0070 intentionally has no session FK). Outer users NO KEY UPDATE and catalogue groups SHARE are compatible with FK KEY SHARE. Outer retained file refs pin media; no outer media UPDATE locks. Register the private reserved alias only after due, in a separate short committed identity-registration transaction before command preparation, without holding that alias/advisory lock in outer execution. No source identity was calculated or signed early.

SOURCE local transactions: publisher_permissions SHARE (operator) OR managed_groups SHARE (group), then storage/publication rows. Reacquired SHARE is compatible with outer SHARE. Group member control-plane mutation holds managed_groups UPDATE, preventing revocation during admitted original publication. Source account permissions and publisher permissions are distinct tables; no source publication upgrades the outer user's account admission row. Source publication does not take platform user/session/intent locks.

MATERIALIZE within outer execute tx: existing core catalogue/pages/session commit, CAS scheduled token to complete, COMMIT. Cleanup happens afterward through existing reference-aware queue only. On errors, rollback execute before writing sanitized failure state in a fresh session->intent transaction; do not independently update a row still held by execute.

Counterexamples/tests:
1. Same-owner signed intake versus scheduled worker: both serialize users NO KEY UPDATE before session; no advisory-first reverse path.
2. Native attach/replace on the new open session versus scheduled worker: disjoint session rows, no new owner-fence lock, source shared references KEY SHARE only.
3. Native begin_edit locks target chapter before expire_owner's OPEN sessions. Worker locks SCHEDULED session then same chapter. expire_owner cannot select scheduled rows, so it cannot wait on worker's session; begin_edit finishes or rejects existing open session, releasing chapter. Worker must never temporarily set scheduled session open. This is critical to implementation.
4. Two due new chapters same parent: direct parent UPDATE before source/core avoids SHARE->UPDATE deadlock. Waiting worker holds only its own user/session/intent; parent holder never needs that other owner's locks.
5. Cancel versus claimed worker: claim commits intent before execute; cancel and execute both session->intent. Cancel before durable start wins, worker sees cancelled/token mismatch; cancel after start refuses.
6. Lease-expired takeover while execute live: session/intent locks and token generation fence prevent second execute. SKIP LOCKED claim skips locked intent.
7. Source command FK: users NO KEY UPDATE permits independent FK KEY SHARE; test deliberately stalls command to demonstrate no transaction timeout. Using users UPDATE would deadlock and is forbidden.
8. Alias identity: register only at due in separate transaction; hold no alias lock outer. Source protocol alias trigger must preserve reserved preferred UUID. Test collisions reject without rebinding.
9. Publication-start marker crash: restart first inspects durable source commands for both upload:<session> and, when parent source was absent, manga:<parent>. No command means no original publication path has run (command commit precedes original signing/storage), so exact safe continuation is possible. Prepared command without verified source acceptance is blocked-uncertain, not automatically replayed/purged. Verified exact source event/manifest or source_accepted with verified original receipt allows existing identical materialization continuation. If evidence cannot establish outcome, retain blocked-uncertain and owner receipt.
10. Pure local edit starting-crash: all public mutation+session completion is one transaction; committed state returns receipt, otherwise exact old revision retry may proceed; changed revision blocks. Old media remains publicly readable until successful atomic due commit.

## Immutable ownership and receipt

Add private intent plus scheduled session state. Never turn scheduled session OPEN, even inside execution. Preserve native Commit/Draft serialization and immediate behavior. Immutable input includes exact native canonical commit hash/payload, ordered file IDs and immutable byte descriptors, original user/account-admission revision, service/group/signer identity, parent source identity and version/content fingerprint, due time, private reserved native chapter ID/UUID. No credentials/signature/event ID are persisted at schedule admission. Native protocol source replacement stays rejected; stale edit revision blocks at due with old public version unchanged.

New schedule does not occupy one-open-session index. GET /upload continues current open behavior. Exact owner snapshot and bounded owner list expose scheduled/blocked/source-accepted/readable distinctly. Future receipt is only schedule acceptance: Chapter-shaped unavailable extension with privately reserved UUID, zero readable pages, no event ID/readableAt/navigation. No public_resource_ids, catalogue, source originals, source events/manifests, discussion, counters or outbox before due. Immediate adapters retain their ordinary behavior.

If parent has no original source at admission, capture version and exact metadata fingerprint; due rejects unrelated parent changes. Own interrupted parent publication may advance null to an original event only when the same durable manga command's stored immutable manifest/input/owner and verified source receipt prove that transition. Never reserialize changed parent metadata after command preparation. Include explicit tests for unrelated edit, own accepted parent recovery, and mismatched immutable parent manifest.

Dates are UTC instance contract. Protective scheduling-only limits: configurable30-day horizon,32 outstanding,500MiB retained originals plus derivatives and ambiguous pins. No changes to ordinary per-session original-byte policy or new unscheduled owner locks. Retention due+24h only cuts off new pre-publication attempts; it never auto-purges scheduled/blocked/uncertain work. Owner cancellation before durable start releases references only through existing normal cleanup. Prepared/accepted or unknown started work is not cancelable merely through lease expiry/time. Exact metadata changes require new intent.

## Required verification before integration

Real disposable PostgreSQL+HTTP coverage, not only state mocks: timestamp null/omission/calendar/offset/UTC boundaries, >24h retention, future admission with no original/catalogue effects; scheduled quota including derivative and ambiguous pin bytes; another open upload permitted; owner list/cancel denial; restart before due and after claim/start; competing leases and stale-token CAS; cancellation both sides of durable start; exact identity binding/group/service/admission revocation; same-owner and FK lock graph; unrelated and own parent source transitions; current edit version CAS; source replacement refusal.

Privacy controls use fresh synthetic bytes and hashes absent from existing originals plus an old-source positive control. Before due, exact reserved UUID, search/feed/count/at-home/thumbnail/native original GET/HEAD/Range/export/federation must not reveal new bytes or state. For scheduled edits, old source stays readable. Already-public identical bytes cannot truthfully be promised private and must be distinguished from fresh-content controls. After due, source-accepted is not automatically readable. Client adapters must not navigate on scheduled/blocked/unavailable receipts.

Document tests actually run and resource blockers. No strict pinned extension parity, subsecond schedule, or crash guarantee claims before demonstrated. Independent review and real tests required before integration.

## Initial implementation checkpoint (not qualified)

The first source/schema checkpoint implements this graph in `public_api/scheduled_uploads.rs`, with a separate live-policy reader in `public_api/auth.rs`. `ScheduledUploadActor` is sealed, privately constructed, not deserializable, and scoped to a specific session, payload hash, claim token, reserved identity and original signer/group. Core `Commit`/`Draft` serialization is unchanged. Native cancellation explicitly refuses scheduled state before touching files.

HTTP future commit returns **202 Accepted**, with a Chapter-shaped schedule receipt whose `nostr.scheduleAccepted` is true, `sourceAccepted` false and `readerAvailable` false. It has no event ID and no readable time. This is an explicit instance extension, not a claim of strict pinned response/status parity. Omitted or due/past timestamps retain the existing immediate response. Signed owner extensions are `GET /nostr/uploads/schedules`, `GET /nostr/uploads/{session}/schedule`, and `POST /nostr/uploads/{session}/schedule/retry`. Retry only asks the same immutable gate to recheck; it cannot authorize blind replay of an uncertain command. Signed `DELETE /upload/{session}` cancels only before durable start. Normal `/upload` current-session semantics remain unchanged.

Protective configurable fields are `scheduled_upload_maximum` (default32), `scheduled_upload_bytes` (default524288000) and `scheduled_upload_horizon_seconds` (default2592000). These are instance limits, not MangaDex limits. Retained-byte reservation includes every original/derivative reference conservatively plus any source-less parent cover that a later parent command may pin. This is a quota reservation, not a new pre-due source or media pin. The immutable parent cover ID and canonical cover hash are captured. Ordinary parent-first cover writers and due parent UPDATE fencing prevent choosing a replacement during execution; deletion/replacement between admission and execution blocks the schedule. Parent metadata is compared again against the stored canonical manifest inside original command preparation, including mutable referenced credit names.

Both parent and upload durable commands gain nullable immutable `scheduled_session_id` provenance without a foreign key or changes to serialized native payloads. It is assigned only on insertion. Every command conflict checks exact provenance, including None for ordinary callers: a native immediate path cannot adopt a pending scheduled command. A pre-existing source-less parent command is refused at scheduling admission. Two schedules cannot establish ownership merely from matching owner, group or manifest. Recovery requires the same schedule provenance, exact stored manifest, payload/hash/catalogue identity and a healthy original receipt with verified signature, expected event ID, publisher, kind, timestamp and manifest digest. Unknown prepared outcomes remain blocked and retained.

Maintenance invokes one bounded due claim per existing30-second storage maintenance turn. Claims use DB time and five-minute restart leases, with unique token and monotonic generation; no subsecond punctuality is promised. A process that loses the token cannot complete the same intent. The durable start record commits before any source command. Alias registration occurs due-only in a separate committed transaction, never while its advisory locks could be reacquired by the independent source command. All execution checks run again afterward. Retention cutoff ends new attempts only; it never deletes blocked or ambiguous work. Metadata changes require safe cancellation and a new intent.

Verification at this checkpoint: one dependency-free timestamp unit test passed with the already-installed official Rust1.90.0 compiler. A parse-only standalone module invocation reached expected unresolved crate/import errors, so this is **not** a successful crate build. No PostgreSQL/service fixture, full crate test, race test, restart test or privacy test has run yet. Source review and real disposable tests listed above remain mandatory before integration. Original679 frontend files are untouched.

### First review corrections

All three new schedule list/get/recheck operations require a signed `expectedAccountId` query and compare it against the live bound native account inside the transaction before reading schedules or changing retry state. A binding change produces409, including an otherwise-empty list. The outer full-router private-response classifier recognizes only the exact typed UUID paths and intended GET/POST methods, preserving `private, no-store` and `Vary: Authorization`; inner route middleware alone is insufficient.

A503 may receive at most five automatic DB-timed exponential-backoff retries only when locked failure bookkeeping proves that neither `upload:<session>` nor `manga:<parent>` nor any command attributed to the schedule exists. The start boundary remains immutable, quota is retained, and the entire fresh authority/input gate runs on every retry. A failed command lookup does not prove absence. Any prepared/accepted/uncertain command stays blocked rather than being replayed on a transient classification. Manual owner recheck can ask the same exact proof gate again; it does not waive receipt verification. Completed owner schedule inspection uses the existing current native receipt path and never infers source acceptance or reader success solely from schedule state.

The protective `scheduled_upload_retry_seconds` instance setting defaults to86400 and is bounded to1..86400. Admission captures `retry_until = due_at + configured interval` immutably; later configuration changes do not rewrite existing deadlines. Owner receipts report the captured `retryWindowSeconds`, and list limits report the current admission setting. This only controls when new attempts may start: it never purges private data, bypasses ambiguity verification, or extends source authority. A real-clock2-second fixture qualifies this configured short window, not a claimed24-hour elapsed default run.

### Local edit / native begin-edit lock correction

A real FK edge matters: native `begin_edit_in` holds the chapter before inserting `upload_sessions`, and that insert acquires parent KEY SHARE. A scheduled local edit must therefore hold parent NO KEY UPDATE, not UPDATE, before waiting on the same chapter. NO KEY UPDATE excludes parent metadata/cover writers while allowing the FK reader to finish. This branch does not upgrade the parent later: `commit_impl` chooses `replace_chapter_pages` → `authorize_chapter_edit` / `update_chapter` → chapter media/derivative updates only, never `ensure_upload_target`, `insert_chapter`, source publication, or parent association. New chapter scheduling still uses parent UPDATE.

`begin_edit_in` now calls `expire_owner` before chapter authorization, after its permission gate, within the same transaction. Expiry can lock an older OPEN session and referenced media; taking those locks after the chapter would invert scheduling admission's session→chapter order. Wrong-version or unauthorized requests roll back expiry, file-reference deletion and queued cleanup together. The outer handler still runs filesystem cleanup only after a successful commit. Tests must exercise both contention orders with actual retained media and separately verify accepted source pins survive expiry.

## Candidate integration acceptance batch (not yet executed)

The `public_uploads` binary has ten independent ignored cases. Each requires its
own guarded disposable PostgreSQL lifecycle, never a database shared by tests.

- `scheduled_private_original_export_expiry_cancel_and_rebind`: actual original
  node GET/HEAD/Range/export, positive old source, private new bytes, account/cache
  fences, native cancel refusal and ordinary expiry.
- `scheduled_transient_retry_parent_acceptance_restart_and_receipts`: real SQL
  failures before commands and after source acceptance, exact parent association
  recovery, actual readable/unavailable receipts. Requires existing real STRFRY_BIN.
- `scheduled_captured_deadline_restart_and_blocked_quota`: real configured
  two-second cutoff and immutable captured deadline across configuration changes;
  retained blocked count. Not a claim of 24 elapsed hours.
- `scheduled_claim_cancel_serialization_and_stale_lease_cas`: actual claim/cancel
  locks and competing claims. Only the mutable lease is accelerated, proving CAS
  ownership rather than five minutes of elapsed time.
- `scheduled_derivatives_count_toward_private_budget`: generated derivatives push
  actual retention above a pool that fits originals alone, with rollback.
- `scheduled_local_edit_preserves_old_reader_and_native_lock_orders`: real old
  public bytes, native begin-edit and worker in both chapter-lock queue orders,
  copied-media retention, successful replacement and stale-version refusal.
- `native_begin_edit_expiry_lock_order_and_authorization_rollback`: expired
  media-bearing session, unauthorized/wrong-version rollback and actual NOWAIT
  evidence that expiry waiting precedes the chapter lock.
- `scheduled_command_provenance_accepted_pins_and_legacy_refusal`: two schedules
  sharing a source-less parent, accepted parent/chapter recovery without new events,
  manifest mismatch, retained pins/count budget and legacy NULL provenance.
  Actual native OPEN source pins survive expiry and begin-edit attempts.
- `scheduled_unknown_prepared_source_is_never_replayed_or_purged`: real original
  transaction failure after platform preparation; no source event, negative public
  original reads, conservative owner retry, retained pins and queue budget.
- `scheduled_parent_cover_removal_replacement_and_metadata_edit_block`: actual
  signed cover mutations/normal cleanup and native parent edit. Cover reservation
  is not falsely described as a storage pin; changed inputs block publication.

Application recreation resets state, pool, router and publisher facade in the
same operating-system process. It is not fresh-process, crash or power-loss
qualification. Retry readiness polls the actual DB clock with a bounded wait.
These exact fixtures passed against the frozen isolated backend source recorded
in the qualification report. Their results do not certify later composed source;
source inspection alone remains insufficient execution evidence.

Schedule UI cancellation uses the existing `DELETE /upload/{session}` with the
sole optional signed `expectedAccountId` UUID query. Legacy no-query callers keep
their existing semantics. The bound write transaction compares that account before
upload UUID resolution/private session access, then follows session → intent
cancellation locks and post-commit normal cleanup. A preceding GET is not a
substitute for this transactional fence. Unknown, duplicate or malformed queries
are rejected, and authorization covers the exact full URL including the query.
