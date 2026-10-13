# Frontend authority, staging and error behavior

All 676 attachment files remain literal byte-identical. Behavior fixes live in exact SHA-256/count-anchored build transforms and external modules. The combined transform manifest is `test-evidence/frontend/integration-source-manifest.json`. No globally identical rendered HTML claim is made: the required anonymous device-library availability guards, mode keys, explicit local/protocol panels, reconstructed announcements and minimal pending receipt wrapper are documented additions. Unrelated supplied markup/classes/styles must pass strict comparison after only exact approved exceptions are reversed.

## Ambiguous transport and cancellation

The HTTP adapter automatically retries only GET, at most the configured bounded attempts. POST/PUT/PATCH/DELETE are never blindly replayed, including after 429 or response loss. Ambiguous mutation errors appear in the existing toast/error UI as **Request outcome unknown**, directing the user to reload/reconcile before resubmission. A real versioned, immutable publication retry still requires an explicit user action and its backend receipt/idempotency fence.

An owned cancellable timer polls the refilling request-budget limiter. Cancellation clears pending limiter/retry/deadline callbacks, removes internal XHR listeners, aborts only its own request and settles its promise once. No queued callback can reopen it. Tokens are a time-refilling budget, not held permits. The overall default deadline is 60 seconds; explicit deadlines are bounded to five minutes. The existing FIFO/finally Mutex patch releases its state on success or exception.

## Private resumable upload

GET `/api/upload` returns the current owner-authorized complete session or 204. Private DTO fields include draft, exact page_order, version, state, can_mutate/can_cancel/can_retry_commit, publication status, expiry and actual receipt. The private upload loader may use current-authorized minimal target metadata when public detail is unavailable; it never replays a historical hidden full public DTO.

Opening/reloading a title upload or page editor never automatically deletes staging. Ordered persisted files, private previews and metadata are restored. Existing inputs become disabled when authority says the immutable publication cannot mutate. Editor cancellation discards local state only after a real server 204; 409/error retains it visibly.

Changes debounce for 500 ms and are saved with owner-only versioned PATCH `/api/upload/{id}`. File operations share the parent Mutex. Before saving, the adapter refreshes current files/version while detecting another tab's acknowledged form/order changes. Commit flushes the latest confirmed private draft/order; prepared/source-accepted retries send only the exact durable saved payload. Unsaved navigation/unload is warned rather than silently discarding changes. A browser closed before acknowledgement may retain only its last server-confirmed draft. No signed secrets/command material are returned to JavaScript.

Actual pending publication receipts do not navigate to an unreadable reader. Accepted-pending title receipts can be refreshed through a scoped existing-styled boundary; full editor rendering requires a genuine full DTO. Revoked/deleted/unauthorized receipts are hidden after a current authoritative rejection.

## Bounded archive and image admission

The browser admits at most 500 pages and 500 MiB total session bytes. Standard single-disk ZIP/CBZ input has a 100 MiB compressed limit. Before JSZip allocation, an EOCD/central-directory check admits at most 2048 entries and 8 MiB directory metadata; unsupported ZIP64/split-disk/trailing metadata is rejected truthfully. Decompression streams enforce per-image 25 MiB and remaining aggregate budgets before buffering output. Folder scans share 500-file/2048-entry/16-level/500 MiB admission budgets across batches and roots.

Images are processed one decoder at a time. Bounded header inspection rejects unknown/corrupt dimensions and more than 50 million decoded pixels before browser image decoding; decoding also has a 30-second deadline. Large supported images retain the original splitting behavior, with actual PNG extensions on generated slices. Private upload failures never count as successful files.

## Installed service-worker authority

The first registered route is same-origin NetworkOnly with fetch cache `no-store` for `/covers/`, `/data/`, `/uploads/`, `/originals/`, `/api/`, `/auth/`, `/proxy` and `/og-image`. It precedes every broad legacy cache predicate, including favicon substring matches in IDs/query text. No stale persistent fallback is allowed after authorization/revocation failure or offline state.

Activation deletes exactly the supplied legacy CacheStorage caches **covers** and **favicons**: both could contain authority-sensitive URLs under the original broad matchers. The immutable **assets** cache, unrelated origin data, browser library IndexedDB, local preferences and signing/outbox records are preserved. Existing skipWaiting/clientsClaim behavior remains. Deployments must revalidate `service-worker.js`; acceptance includes an actually installed worker, poisoned/warm cache, adversarial favicon IDs/query, logout/account change and offline behavior. The manifest alone does not promise full offline chapter access.

## Qualification

Source preparation and compilation are distinct from browser evidence. Browser/runtime tests remain subject to the implementation freeze gate; no journey is passed until its real user controls and persisted results are observed. Fragment `.ts` files currently mirror reviewed generated replacement bodies embedded in the overlay manifest; changing a review copy alone does not change executed code. The exact mounted `after` strings are authoritative until the planned post-correctness locality refactor.

## Final prepared-title publication boundary

The additive `MangaEditBoundary.svelte` delegates the existing editor only for a
current full DTO whose `can_mutate` is not false. A prepared immutable original
publication command is read-only and may expose a retry button only when the
server supplies `can_retry_publication=true` plus its actual saved
`publication_retry_version`. That explicit retry uses exactly that version; no
replacement draft or guessed signed command is constructed. Read-only refresh
tries fresh public detail first, then the publisher-authorized private pending
receipt only after public 403/404. Unavailable current reads remove the receipt;
a historical full DTO is never replayed. This is an intentional rendered UI
extension, not recovered original missing markup.

ZIP admission requires a standard single-disk central directory ending exactly
at the admitted EOCD and rejects later competing EOCD markers. This is a bounded
parser-consistency guard because the installed JSZip chooses its last marker;
no executed ZIP-bomb exploit is asserted.

## Round2 source repairs (runtime pending)

- Reader aggregates are component-owned and scoped to manga plus the actual language query. Every mounted reader rereads current authority; disposal/generation fences reject late responses, and failures are not retained as cached success. Automatic previous-chapter last-page navigation uses one-based page URLs/state.
- `frontend-reader-overlays.mjs` contains20 additional original-SHA/count anchors, independently read before mounting. Double spread visibility accepts the selected second member. Scroll observer thresholds remain reachable for long images, resize/layout work is lifetime-owned, and keyboard RAF stops on blur/destruction. Original markup/classes/styles remain unchanged.
- Replacement image admission applies genuine MIME/filename/signature,25MiB encoded bytes,50-million pixels before decode,30-second decode deadline, one-page dimensions and500-page/500MiB session limits. Existing durable bytes are never DELETEd before upload. The browser sends one owner/version-fenced multipart POST to `/api/upload/{session}/files/{file}/replace`. Its stable file ID/order and returned version are authoritative. Ambiguous transport is reconciled only with a fresh owner GET matching actual SHA256+original filename, never automatic mutation replay.
- Stored preferences are semantically validated against default shapes/types and the actual reader control ranges; JSON `null`, wrong objects/arrays and invalid choices cannot crash app initialization. Corrupt original storage bytes remain untouched until an explicit subsequent edit.
- Current-catalogue follow/history/progress, hydration and interpretation require the verified origin/namespace/service tuple, including a colliding existing native ID. The actual IndexedDB mutation transaction checks persisted scope. Explicit saved-device edit/removal/list and backup controls remain available without network verification unless a persisted mismatch fence exists. The additive saved-device disclosure identifies this scope and does not reinterpret catalogue IDs.
- The retained Generate.csv control exports genuine scoped CSV, with catalogue origin/namespace/service fields; its importer checks these fields before resolving current IDs. Foreign scoped native aliases are rejected. JSON device backups and portable canonical backups have separately labelled controls.
- Public signed identity panel target/recipient/key/signer generation changes invalidate transient mappings, conversations, replies and reviews, reject stale reads and guard transport. Exact signed intents are saved under their original recipient/body before the final post-sign context fence and remain explicitly reconcilable.

Current generated composition evidence is `test-evidence/frontend/integration-source-manifest.json`; compiler evidence is `composed-svelte-source-compile.json`. Compile/source checks are not browser acceptance. The older157-repair artifact is stale until a new bounded build succeeds.

## Round3 source context fixes (runtime pending)

- Canonical inspection captures the exact loaded family/key; family selection synchronously removes old rows and invalidates pending reads. Removing an old row never reinterprets its key under a new family.
- CSV/MAL imports freeze one mode/catalogue-scope/account context before parsing, carry it through every400-record batch, and check context again inside device mutation/account expected-ID requests. A destination change stops subsequent batches; already acknowledged batches remain in their original destination.
- Report/draft feeds never restore private DTOs from history and write no private feed DTO back into `page.state`. Fresh pre/post browser identity checks plus authenticated expected-account headers and local authority generations fence responses. ChapterFeed caches no private history on public routes.
- Read/unread controls capture intended state before refreshed local state can change their derived flag. Stable mode/account/scope context is checked inside writes and before updating UI maps. Title and feed maps clear and reread on current library ownership/revision changes; stale results cannot attach.
- A bounded ResizeObserver observes actual page geometry, so the width slider can change a long strip's reachable intersection threshold without a window resize. Only changed geometry schedules one RAF, with observer/frame disposal.
- Report dialog close timer, reasons requests and accepted submit dismissal are owned by exact subject/type/open generation. Reopening cannot be reset by an old200ms callback.
- Title-edit cover queries are keyed/lifetime-fenced to current target. Exactly one approved original key boundary preserves layout while disposing the previous retained editor; the additive pending wrapper also keys its retained child, captures retry target/version and rejects late refresh/retry/cover results after target change/disposal. It does not mutate a newly selected title through an old request.

The227-definition source compiler passed; aggregate build and independent fresh review have distinct receipts. Real E2E remains held until the independent gate.

## Round4 source fixes (runtime pending)

- Follow/unfollow/notification adapter mutations capture stable mode/account/scope authority. Device writes check it inside the transaction, account writes use the frozen authenticated expected-account header. Caller caches use one `applyAccountFollow` seam that only applies acknowledged changes to the same captured account; a device result never becomes a different account's follows.
- Start Read fetches current authority on every attempt, with complete title/query/preferences/library context and disposal/generation fencing. Changed filters/identity invalidate the previous aggregate and variant dialog. Cancellation retains no stale aggregate for the next attempt.
- Chapter editor and title upload have exactly enumerated target-key boundaries, preserving original layout/classes/styles. Parent user lookup is target/account generation-fenced. A shared `target-owner.ts` captures immutable target/browser actor for the keyed editor lifetime, sends mutations with the expected-account header, and rejects late completions after target/authority/disposal changes. Requests already sent stay bound to their original target, without automatic replay.
- Staging restore/autosave/cancel reads and writes retain that same owner. Metadata editing now prevents silent SPA navigation with unsaved fields; normal acknowledged staging remains durable across deliberate navigation. Existing prepared-command immutability and private versioned resume behavior remain.

The previous227-definition bundle succeeded with its own clean terminal receipt, but is stale after these new source changes. Current source manifest is262 repair definitions (271 exact anchored occurrences)/63 originals. Current compiler is source-only; real E2E remains unrun.

## Round5 finite source repairs (runtime pending)

- Account mutation reconciliation captures the expected account ID, selected-library generation and every-Session-write generation before awaited validity. After the response and again immediately before the synchronous Session store write, all must still match. Logout, A→B, A→logout→A and a newer same-ID Session write discard an older confirmation. Accepted-write reconciliation uncertainty stays visible; no mutation is replayed.
- `upload-batch.ts` owns the immutable browser actor and one non-replayed batch. Every accepted file/error receipt is validated. Failure produces per-item error states, so a single page exposes the retained Replace/Remove controls. Unattempted later chunks are marked unsent and can be discarded safely without deleting previously acknowledged pages.
- A lost/invalid batch response uses only fresh owner GET reconciliation, matching an actual newly staged file ID, SHA256 and original filename against the before-ID set. Absence/ambiguity does not establish rejection. Unknown entries cannot be resent, replaced, treated as duplicates or silently removed without exact reconciliation; explicitly cancelling the whole mutable stage is available when no receipt can be established.
- New-title upload has one explicitly approved additive `Cancel staged upload` confirmation control. It captures the current actor/session, rereads `can_cancel`, and clears bound state/blob URLs only after successful DELETE204. DELETE409 or transport uncertainty retains staging. Prepared immutable publications have no cancellation control. This is intentional additional rendered HTML, with original style/classes reused.
- The original metadata `Revert` disabled expression has one explicitly approved behavior exception: changed ordinary local fields can revert even while invalid. Busy/nonpending safeguards remain, reset is local-only, and clean navigation can resume.

Current exact anchor and external-source hashes are in the generated integration manifest. All676 attachment files remain byte-identical. The compiler/source proof is distinct from browser execution; runtime acceptance remains unrun.

### Restricted email recovery completion

The exact `/auth/recovery` route now delegates to an explicitly approved external
boundary. Its anonymous email-request component is copied byte-identically from
the attachment and checked during preparation. A200 reset-only capability contains
no identity, email, follows, preferences, roles or permissions; normal Session
initialization rejects it and does not hydrate account preferences. A restricted
capability mounts only a CSRF-bound password settings form, without relaxing any
Protected guard or settings-parent redirect. Backend recovery settings nodes must
exclude profile/email/username for both errors and normal restricted responses.
After confirmed successful password reset, a fresh authoritative normal Session
is required before showing the account continuation control. Password updates are
never replayed automatically, and uncertainty gives sign-in/recovery guidance.
The initializer's own final store/preference write also rejects responses predating
a newer Session write. The mounted route/limited password UI is a deliberate
rendered extension, with exact path/hash exception and original styles retained.

`account-recovery` browser source performs the actual email request, one-time local
mail link redemption, password-only form, private-library403, normal-session
rotation, old/new password checks, reused-link400 and unchanged device-library
checks. These journeys have not executed.
