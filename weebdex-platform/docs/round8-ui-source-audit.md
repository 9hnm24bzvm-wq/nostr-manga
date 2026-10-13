# Round-eight independent UI source-completeness audit

Reviewed 2026-10-13, beginning 06:21 UTC. This reviewer did not implement the
preceding repairs. The shared → reader → local-library transforms were applied
independently to an inspection copy, enforcing their original SHA/count anchors.
Actual resolver configuration, relevant external modules, retained callers and
backend endpoints were read directly.

## Finite verdict

**Source signoff withheld: four substantive, bounded findings were established.**
Three are new workflow counterexamples; UI8-01 is an unclosed action-admission
variant of UI7-01. The previous limiter-wait counterexample is repaired and is
not repeated as if the new transport lacked that repair.

The owner was still finishing its round-seven candidate when this pass started,
and made further source changes during this review. Findings below refer to the
explicit initial composed snapshot, plus directly read external modules whose
relevant bytes matched its independently hashed inventory. They were reported
immediately to the parent and frontend owner. This report does not certify a
later repair candidate; it requires the requested fresh independent review.

No browser journey, application runtime, database operation, Cargo command, Vite
build or test suite was executed. Transforming source and hashing files are not
application tests. Every runtime acceptance below is **unrun**. No implementation
source was edited by this reviewer; this report is the repository deliverable.

Initial independently inspected composition:

- 372 repair definitions, 381 exact occurrences, 79 original paths
- 541 copied/transformed `.svelte`/`.ts`/`.js` source files
- SHA of JSON-serialized composed path/SHA records, sorted with `localeCompare`:
  `a8cad3bfa7f1fc2d8abe2dddfe6d587c3c24039ff09511c10edbc6f9af299270`
- Shared overlay:
  `46e207f127af73b66f109d81c9d96e8483d7eca2eb28c1930fc570223c277580`
- Reader overlay:
  `70ec02db52c0867872f1545ff20667a9b7e0f7e747261de8b5a57f62285c9505`
- Local overlay:
  `acc47af42d7638f6cc77d2529c71b553f7d1ce73c5b8ebe11b3461fb275a043f`
- Independently calculated external integration inventory SHA:
  `71a97440c4517579327d22957b002fc197d3b39f3f5bbacc4cfa93675b5e8cee`

All 676 original attachment paths were independently rehashed against the
`source_sha256` entries in `test-evidence/frontend/source-parity.json`: none was
missing or different. Logical frontend paths/lines below refer to the composed
inspection copy, not the raw preserved attachment. External paths identify the
actual integration modules. The initial owner compiler receipt was a separate
72-component/38-script source pass, not a test run by this reviewer. Later
73-component/40-script compiler evidence appeared during the review and does
not close the findings below or provide browser evidence.

## Findings

### UI8-01 — P1: Group/user follow intent can still be admitted under a newer actor

- Composed `src/routes/(app)/group/[id]/Group.svelte:124-132` and
  `src/routes/(app)/user/[id]/User.svelte:125-133` admit the clicked follow/unfollow
  callback, then await `Session.isValid()` before creating any mutation owner,
  captured actor/target or disposal fence. Their newly added `preferenceOwner`
  is used for blocking, not for these follow controls.
- Composed `src/lib/stores/Session.ts:48-53` implements that check through the
  Session mutex. `src/lib/Mutex.ts:31-38` waits for the previous FIFO operation,
  so a forced Session refresh can hold the callback across a real authority
  transition. Destruction does not cancel this plain asynchronous callback.
- Composed `src/lib/HTTP.ts:39-46` correctly captures native authority at POST/
  DELETE admission and carries the expected-account header through limiter
  waits. It cannot recover the actor of a click that occurred before the caller's
  await. In this counterexample the HTTP object does not exist until afterward.
- Scenario: the page is showing account A, a current-authority refresh is pending,
  and A clicks Follow. Resolve that refresh as account B before the queued
  `Session.isValid()` completes. The provider authority key disposes the old
  Group/User child, but its old callback continues, sees a valid Session B and
  constructs a newly admitted request bound to B. B's follow record is changed
  using A's earlier action. The same pre-send disposal seam allows an old action
  to continue after navigating away, even without changing the actor.
- Backend `backend/src/domains/library/follows.rs:344-375` uses the resolved
  actor's `user_id` for the group/user follow insertion/deletion. The central
  expected-account check cannot reject this request merely because it came from
  an old callback: its newly captured expected actor is B too.
- Closure: capture the clicked actor, target and selected follow/unfollow intent
  synchronously before any await; carry an owned lifetime through the validity
  wait, request and response. Assert after each wait and cancel obsolete unsent
  intent. Preserve uncertainty for already-sent actions and never replay them.
- Acceptance: hold a forced Session refresh before Follow/Unfollow, then exercise
  A→B, A→logout→A and navigation/disposal before releasing it. The old callback
  sends no newly admitted mutation for a replacement context. Separately defer
  the transport limiter and already-sent response; the existing binding still
  holds, and ordinary current-account follow/unfollow remains usable.
- Runtime acceptance: **unrun**.

### UI8-02 — P1: Account reader history creates an authority-remount feedback loop

- Both composed reader routes, `src/routes/(app)/chapter/[id]/+page@.svelte:15-19`
  and `src/routes/(app)/chapter/[id]/[page]/+page@.svelte:15-19`, nest the entire
  `Reader.Provider` and `Reader.Reader` under `Comments.Provider`.
- Composed `comments-provider.svelte:53-55` now keys *all* its rendered children
  on `[thread.id, nativeAuthority.actor, nativeAuthority.generation]`.
  `platform-integrations/frontend-behavior/mutation-authority.ts:9-12` increments
  this generation on every Session write, including acknowledged own writes.
- Composed `src/lib/components/reader/reader-history.svelte:11-13` automatically
  calls `API.ChapterHistory.UpdateBatch` every time an account-mode History
  component mounts. `platform-integrations/local-library/ChapterHistory.ts:32-34`
  routes this POST through `accountRequest`.
- Actual `platform-integrations/local-library/library.ts:206-212` performs
  `Session.set(confirmed)` after every successful non-GET reconciliation. This
  happens even when the account's already-read chapter state is unchanged.
- Scenario: open any readable chapter in account-library mode. Its normal history
  POST succeeds, the helper writes Session, authority generation increases, and
  the provider key destroys/recreates the entire reader. The new History mounts
  and posts again. Each successful reconciliation triggers the next iteration.
  The reader's queue, blobs and aggregate are repeatedly destroyed/restarted;
  history POSTs and authoritative checks continue without another user action.
  An idempotent backend history update does not break the frontend loop because
  the own Session write still occurs.
- Closure: keep the reader lifetime independent of per-write discussion authority
  invalidation. Discussion personal reads/composer/mutations must still reset or
  reject obsolete authority, but an ordinary own account reconciliation must not
  recreate a parent reader and re-admit its automatic history write. Do not fix
  this by weakening the mutation-generation guard or replaying old private DTOs.
- Acceptance: account-mode reader mount results in one admitted visit/history
  operation, then reaches a quiet stable state after successful reconciliation.
  Loaded pages, current page, aggregate and queue remain stable across same-ID
  follow/history/preference reconciliation. Actual account/permission changes
  still invalidate personal discussion results and pending mutations. Device-
  mode progress, chapter navigation and comments remain functional.
- Runtime acceptance: **unrun**; no measured request count is claimed.

### UI8-03 — P2: The retained thread share button has no behavior

- Composed `src/routes/(app)/forums/threads/[id]/Thread.svelte:236-238` renders
  a plain native `<button>` containing the `Share2` icon. It has no click handler,
  href, submit/form owner, attachment/action or delegated listener. A complete
  finite scan of non-generic native buttons found this single unsupported
  native-button anchor; `Share2` has no other use in the composed frontend.
- The button is rendered in the real general/user-profile thread header. Clicking
  or keyboard-activating it has no sharing or copy result. This is an already
  supplied visible button, not a request to add a new API capability or control.
- Closure: wire the existing control to share the current canonical thread URL
  using the native share API where supported, with a clipboard fallback and
  truthful cancellation/success/failure feedback. Do not automatically post to
  any third-party service or copy a previously selected thread URL.
- Acceptance: mouse and keyboard activation share/copy the exact current thread
  URL; native cancellation is harmless; unavailable/denied clipboard access
  reports failure without success. SPA A→B and Back/Forward never share A from
  B's button. Original layout/classes/styles are preserved.
- Runtime acceptance: **unrun**.

### UI8-04 — P2: Remove duplicates discards distinct same-name local pages

- The acknowledged-removal repair correctly preserves thumbnails on failed
  DELETEs, but composed `UploadChapterPages.svelte:490-502` still classifies a
  row by `row.file_hash ?? row.original_file_name`. It then removes all rows
  sharing that key and releases the discarded thumbnails.
- Actual `platform-integrations/frontend-behavior/upload-batch.ts:35-38,64-66`
  retains a local `File` on known-unsent and per-item-rejected placeholders, but
  these rows have no `file_hash`. Rejected receipts also replace the recovery
  object without retaining its previously computed hash. Unknown outcomes are
  correctly blocked by the duplicate control and are not this finding.
- Scenario: drop valid, different images from two folders with the same basename,
  such as `part-a/page1.png` and `part-b/page1.png`. A failed early batch leaves
  both later pages as truthfully unsent local placeholders, or the server
  individually rejects both. Remove duplicates treats the distinct bytes as
  equal solely by their shared original filename. One legitimate local page is
  silently removed from the intended sequence and its preview URL revoked.
  No remote DELETE is needed for these local rows, so the successful local-only
  removal path establishes the defect. The original files on disk are unaffected.
- Closure: establish true content equality for local pages before classification,
  or preserve any row whose duplicate equality cannot be proven. Keep existing
  unknown-outcome fencing and acknowledge remote deletion before removing its
  corresponding row or preview.
- Acceptance: distinct same-basename known-unsent/rejected pages both survive
  Remove duplicates. Byte-identical local pages and acknowledged staged pages
  are removed only when equality is established. Failed/unknown remote deletions
  preserve every retained row and usable preview; order and commit state remain
  truthful. No automatic re-upload or mutation replay occurs.
- Runtime acceptance: **unrun**.

## Independent reread of the preceding repairs

1. UI7-01's original limiter seam is repaired at the native transport: dependency-
   free synchronous Session authority binding, expected-account header, frozen
   JSON/FormData payload, cancellable limiter admission and response checks are
   present in the composed HTTP implementation. Thread mutations additionally
   use common owned intents. UI8-01 identifies the still-earlier retained follow
   callback seam, not a missing global limiter fence.
2. UI7-02's old captured-thread getter is repaired: discussion context reads the
   current thread, clears its mutable form/feed/vote state and keys by target/
   authority. UI8-02 is the new interaction with its broad descendant lifetime.
3. UI7-03 is source-closed in the inspected candidate: reply feed results/errors
   check captured thread, href, authority, live generation and disposal. No
   personal feed DTO is read from or written to navigation history. Vote reads
   have owned result admission. Later personal-read changes are not certified.
4. UI7-04 is source-closed for the preferences page and group/uploader blocking:
   frozen submitted values and actor/authority snapshots, common mutation intent,
   acknowledged-only array changes and guarded synchronous preference/Session
   apply are present. Anonymous local save/reset remains separate.
5. UI7-05's specific failed-DELETE preview counterexample is source-closed:
   candidate thumbnails remain live until acknowledged removal, and
   `blob-lifetime.ts` does not revoke URLs shared by retained rows. UI8-04 concerns
   the different content-equality classification seam.

These are finite source conclusions. Their repeated/interrupted browser journeys
are unrun. Changes to `thread-read.ts`, announcement clients/routes and the shared
overlay were observed during the pass; those later candidate bytes are outside
this snapshot's signoff.

## Scope and remaining gates

The pass independently traced source/resolver/transport admission, central REST
and retained follow callers, discussion target/query/viewer lifetimes, all five
preceding repairs, supplied native buttons, preferences/blocking, reader aggregate/
page/queue/keyboard/history composition, upload admission/restoration/autosave/
unknown receipts/replacement/removal/duplicate controls, scope-bound device and
account library adapters, saved-device lists/backup/import controls, signed public
identity/review/exact outboxes, account/client/authentication and restricted
recovery boundaries, discovery filters, report flows and reconstructed announcement
transport ownership. It established no further concrete in-scope defect within
that finite pass. This is not an exhaustive button/workflow or visual proof.

No new controls are required simply because an unused API wrapper exists. The
retained URLSearchParams prefix override was checked before evaluating query
concatenations. Raw-original behaviors already changed by mounted transforms
were not counted as fresh findings. Service-worker offline boot, revocation,
final executable bundle, persistence, mobile/browser/E2E/system/production gates
remain separate and unrun. The earlier 309-repair bundle is stale for this source
candidate. No final completeness or production-ready claim is supported here.
