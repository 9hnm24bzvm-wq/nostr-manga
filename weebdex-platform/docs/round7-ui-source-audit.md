# Round-seven independent UI source-completeness audit

Reviewed 2026-10-13, beginning 06:04 UTC. This reviewer did not implement the
preceding patches. The mounted shared → reader → local-library transforms were
independently applied to an inspection copy, enforcing every original SHA and
exact replacement count. Actual resolver configuration, external integration
modules, retained callers, relevant backend endpoints and installed SvelteKit
component generation were read directly.

## Finite verdict

**Source signoff withheld: five substantive, bounded UI findings were established
against the initial round-seven snapshot.** The four round-six counterexamples
are source-closed in that snapshot. The frontend owner began further repairs
after useful findings were reported; the shared overlay changed during this
review. This report does not certify those later candidates. They require their
own frozen composition and the requested fresh independent review.

This is source-only work. No browser journey, application runtime, database
operation, Cargo command, Vite build or test suite was executed. Applying
transforms for inspection and checking source hashes are not application tests.
Every runtime acceptance below is **unrun**. No implementation source was edited
by this reviewer; this report is the repository deliverable.

Initial independently inspected composition:

- 309 repair definitions, 318 exact occurrences, 66 original paths
- 541 copied/transformed source `.svelte`/`.ts`/`.js` files
- SHA of JSON-serialized composed path/SHA records, sorted with `localeCompare`:
  `8fb2cc91f46e486199d48a742107273559bd6881f2e726c4082f3787d663676f`
- Shared overlay:
  `299ccc43def826c6453cbc922cef4ffecedb503281724be1227dc28ec805259d`
- Reader overlay:
  `70ec02db52c0867872f1545ff20667a9b7e0f7e747261de8b5a57f62285c9505`
- Local overlay:
  `acc47af42d7638f6cc77d2529c71b553f7d1ce73c5b8ebe11b3461fb275a043f`
- Library adapter:
  `7dba07f8120b85b7d63454bae10e0c255cb16eb775f4d0ab311f92bddf432520`
- MangaFollow adapter:
  `c2b4bc21f6839f2a2a588fd7b0d1c2d38fdc26b390d187193993780b3a213e58`
- Device panel:
  `6074d5567a3caa39cade3921aef045f0cbf13fe803488139ef913ec5583ac7d1`
- Signed identity panel:
  `1f9da0538967206a6fef07e4c59fa1b25b05807f9e1bc7f48cd05766c583785e`
- Authenticated-mutation helper:
  `feba7952eb231940a3f0cfd5223baee1873c96a923818291c6c91b2694f95fdd`
- HTTP helper:
  `4edf40151a8773574ae95c8630405fdd47ac864f4eac1d7d96c1f72ccf60816a`

All 676 original attachment paths were independently rehashed against
`test-evidence/frontend/source-parity.json`: no missing or differing original
file was found. Logical frontend paths/lines below refer to the initial composed
inspection copy, not the raw preserved attachment or a later source candidate.
External paths refer to the directly inspected integration module.

## Findings

### UI7-01 — P1: Queued community mutations can execute as a different account

- Composed `src/lib/API/Thread.ts:15-24,44-77` sends credentialed reply
  create/update/remove/hard-delete and thread/reply vote mutations with no
  captured actor, expected-account header or authority cancellation. These
  methods are actually called by `comments-create.svelte:81-111`,
  `comments-edit-dialog.svelte:63-96`, `comments-votes.svelte:43-117`, and
  `comments-entry.svelte:79-190`; they are not unused API capabilities.
- Actual `platform-integrations/frontend-behavior/HTTP.ts:96-110` can defer opening
  and sending an XHR until the shared refilling token budget admits it. The
  community components do not cancel these requests on disposal or an account
  transition. The report-specific helper is not used by Thread transports.
- Scenario: account A submits a comment, but its POST is still unsent in the
  limiter wait. Log out/switch browser authority to eligible account B, including
  through another tab, before transmission. The request can send with B's cookie.
  `backend/src/domains/community/mod.rs:42-47,220-247` resolves that principal and
  inserts `a.user_id` as the reply author and audit actor. A's text is published
  as B. Votes can similarly change B's record; moderator changes can use a newer
  eligible moderator. This is an actual request-admission seam, not a claim that
  an already-sent request changes its cookie or that backend authorization is
  bypassed.
- The same retained transport pattern has visible callers in group/user follows:
  `API/Group.ts:70-85` and `API/User.ts:35-50`, called by
  `group/[id]/Group.svelte:119-143` and `user/[id]/User.svelte:120-144`. Catalogue,
  author/group/profile and client mutations also still use credentialed original
  wrappers; for example `API/Client.ts:10-31` is called by its retained create and
  secret/delete controls. These are related actor-binding instances of the same
  admission flaw, rather than separate findings or requests for new features.
- The server already rejects a differing explicit expected account at
  `backend/src/domains/auth/principal.rs:232-249`. The repaired Report transport
  shows an existing bounded way to use that guard.
- Closure: freeze actor and target at the actual user-action admission, carry the
  expected-account binding through transport waits, cancel obsolete unsent
  intent, and fence completion/navigation/row changes by actor, target and
  disposal. Capturing only after an awaited `Session.isValid()` is insufficient
  for controls that admit their intent before that await. Preserve the original
  destination and uncertainty of already-sent operations; never replay them
  automatically. Apply the same transport guarantee to the actual retained
  credentialed mutation callers, without changing their supported features.
- Acceptance: defer transport before send for post/edit/remove/moderator-delete,
  votes and group/user follows; perform A→logout, A→B and A→logout→A. No operation
  is executed or attributed to B, no obsolete completion updates a new view,
  and ordinary current-account actions still work. Separately delay an already
  sent response and verify truthful original-actor state with no replay. Include
  the retained credential-creation/rotation and catalogue moderation callers
  when verifying a shared transport repair.
- Runtime acceptance: **unrun**; no executed exploit is claimed.

### UI7-02 — P1: Same-route forum navigation keeps the previous thread authority

- Composed `src/routes/(app)/forums/threads/[id]/+page.svelte:9-15` mounts
  `Comments.Provider` without a thread/target key while both old and new
  `data.thread` values remain truthy.
- `src/lib/components/comments/comments-provider.svelte:15-28` captures its
  initial thread and statistics into an object passed to `setComments`.
  `context.svelte.ts:25-30` retains that object and returns `props.thread`; there
  is no incoming-thread reconciliation. `comments.svelte:91-93` keys only the
  query, so changing the thread ID with the same query does not recreate its
  Feed. `Thread.svelte:10-44` renders the captured thread too.
- Installed SvelteKit `src/core/sync/write_root.js:65-88` supplies changed data
  to the same page constructor, with no URL/parameter key. Neither root layout
  adds one. Thus a route parameter change does not implicitly dispose this
  captured context. The keyed group/user layouts elsewhere do not cover the
  standalone forum route.
- Scenario: open thread A, then use normal SPA navigation to
  `/forums/threads/B` with the same query. B's route data is fetched, but the
  existing header, replies, composer and vote authority still refer to A. A new
  comment goes to `comments.thread.id` A while the browser URL identifies B.
  This affects the real discussion destination, not just stale presentation.
- Closure: recreate or fully reconcile the discussion context at a confirmed
  thread identity boundary, including its composer, edit target, statistics,
  feed and vote state. Do not silently carry A's mutable form into B.
- Acceptance: navigate A→B→A using SPA links and Back/Forward, with equal and
  differing queries and the real reply deep-link route. Header, thread ID,
  replies, composer/edit/report/vote targets and statistics always match the
  URL's loaded thread; no action mutates the previous thread. Delayed old
  operations are independently fenced as in UI7-01/UI7-03.
- Runtime acceptance: **unrun**.

### UI7-03 — P2: A disposed reply-feed request overwrites the newer query and history

- Composed `comments-feed.svelte:17-39` loads replies on mount and then assigns
  `comments.feed` and calls `replaceState(window.location.href, {...page.state,
  _$Replies_feed: ...})`. It captures no href/query/target generation and has no
  disposal fence. Its response belongs to the query read before the await, but
  its history write uses the URL and state current after the await.
- `comments.svelte:91-93` recreates Feed on query navigation while retaining the
  same provider/feed setter. The cache reader at `comments-feed.svelte:18-19`
  accepts the unscoped history DTO without checking thread, query or viewer.
- Scenario: load Latest replies slowly, switch to Oldest or page 2, and let the
  newer query finish first. The older disposed Feed later installs its Latest/
  page-1 data into the shared provider and writes that DTO under the newer URL.
  The selector/URL now describes a different order/page from the visible replies;
  a later remount or Back/Forward restores the wrong cached DTO. Navigation to
  another discussion while a read is pending can contaminate that history entry
  by the same current-URL write, even after UI7-02's target key is added.
- Closure: admit a feed result/history write only for its captured current
  thread/query/viewer and live generation; namespace or validate cache entries
  by that same context. Discard late errors and results after destruction. This
  repairs the retained pagination/sort/history behavior, without new controls.
- Acceptance: delay both orders of Latest↔Oldest and page 1↔2 completion, then
  change threads, refresh the comments section and use Back/Forward. Reply order,
  paging, thread membership and viewer vote fields match the current context;
  older responses never write a newer history entry. Current-query caching and
  normal pagination still work.
- Runtime acceptance: **unrun**.

### UI7-04 — P2: Old preference responses overwrite a newer account's active settings

- Composed `settings/preferences/+page.svelte:72-76` awaits `Settings.Update`
  and directly writes `$prefs = $Session.preferences = res.body`, with no
  captured actor, Session-write generation, target/disposal fence or returned
  account identity. `API/Settings.ts:19-22` also sends its credentialed PUT
  without expected-account binding.
- The retained block controls have the same completion seam in
  `group/[id]/Group.svelte:154-169` and `user/[id]/User.svelte:155-170`. They
  mutate the active excluded-group/uploader array before confirmed server
  success, so refusal/uncertainty also leaves an unacknowledged local filter.
- Backend `library/preferences.rs:16-18,70-81,113-117` updates the authenticated
  actor and returns that actor's preference projection. The DTO has no actor
  label for these callers to validate. Optimistic versioning does not bind it to
  the frontend Session that exists when it eventually arrives.
- Scenario: send A's preference/block change and delay its response, then load
  B's Session and preferences before resolving A's response. The old callback
  overwrites B's active filters and `Session.preferences` with A's projection,
  even if the original server mutation correctly remained A's. A newer same-ID
  preference/Session refresh can also be overwritten. The unsent queued variant
  can update B when versions happen to match. UI5's fixed `accountRequest` cannot
  protect these retained callers because they do not use it.
- Closure: freeze and enforce the original actor at request admission, then
  validate current actor and every-Session-write generation before updating
  either preference store. Avoid mutating current account filters before an
  acknowledged response, or restore only the still-current original context on
  reliable rejection. Preserve unknown outcomes and require reload/reconciliation
  rather than automatic resend. Keep anonymous local preference saves working.
- Acceptance: delay an already-sent A save/block response across A→B,
  A→logout→A and a newer same-ID refresh; neither current store is overwritten.
  Defer send and change cookies/account; no B preference mutation occurs. Reject
  and disconnect group/uploader blocking separately: current filters never
  silently claim an unconfirmed server result. Anonymous save/reset, valid
  current-account saves and version-conflict feedback remain usable.
- Runtime acceptance: **unrun**.

### UI7-05 — P2: Failed duplicate removal destroys previews of retained staged pages

- Composed `UploadChapterPages.svelte:495-514` revokes each duplicate thumbnail
  during collection, before its batch DELETE has succeeded. A failure leaves the
  full `pages` array because the actual removal at `:517` is never reached.
  `revokeThumbnail` at `:221-224` irreversibly revokes a blob URL.
- These URLs are actually retained after successful staging:
  `platform-integrations/frontend-behavior/upload-batch.ts:18,64-68` creates each
  remote page with its local thumbnail. Atomic replacement similarly installs a
  blob URL at composed `UploadChapterPages.svelte:279`.
- Scenario: stage two identical images, choose Remove duplicates, and receive a
  forbidden/429/failed DELETE. Both pages remain staged. On a subsequent normal
  drag/remount, the duplicate preview tries to reload its now-revoked URL.
  `UploadChapterPagesEntry.svelte:80-123` recreates ImageLoader around the drag
  conditional; `ImageLoader.svelte:32-52,56-61` can never reach the preview child
  for that revoked URL and eventually leaves its loader. An already decoded
  image need not disappear immediately; the concrete failure is its subsequent
  reload. This does not claim the staged source file was lost.
- Closure: collect candidate removals without revoking their live previews;
  revoke only after the corresponding acknowledged local array removal. Keep
  all retained previews intact on rejection or unknown outcome, and do not
  automatically replay the deletion.
- Acceptance: denied, rate-limited and disconnected duplicate DELETEs retain
  their staged rows and usable previews through drag/remount/reorder. A successful
  removal removes only the intended duplicates and revokes only removed blobs.
  Unknown receipts still require the existing reconciliation/cancellation path.
- Runtime acceptance: **unrun**.

## Independent reread of round-six repairs

1. UI6-01: composed `UploadChapterEdit.svelte:245-253` owns the SubmitEvent
   synchronously and gates page-session admission on dirty metadata before any
   Begin/current-session request. The draft is kept with save/revert guidance.
   The ordinary Revert path remains valid for invalid metadata.
2. UI6-02: actual device panel `LocalLibraryPanel.svelte:60-70,145` verifies the
   clicked saved list, explicitly selects device mode, checks scope/list again
   and then navigates. Actual `MangaFollow.ts:10-17` rejects invalid, missing or
   account-mode device-list queries rather than substituting account results.
3. UI6-03: composed Report mutations all call the authenticated-mutation helper
   (`API/Report.ts:31-34`). That helper captures the actor, sets the expected
   header and cancels on authority/intent changes. Reviewed dialog and row
   callers have target/disposal lifetime ownership and fence completions.
   UI7-01 identifies different retained transports outside that repaired seam.
4. UI6-04: composed keyboard handler `reader-keyboard-listener.svelte:18-29`
   excludes native/editable/control-owned and default-prevented events. Focus,
   blur, keyup and destruction stop the held scroll RAF (`:127-139`). The new
   panels' real selectors are covered by that exclusion.

These are source closures only. Their interrupted/repeated real browser journeys
remain unrun.

## Finite scope and negative results

The pass traced the actual transform/resolver authority; discovery/filter and
library pagination/query seams; device/account follow and history lifetimes;
saved-device editing, custom lists, scoped/native and portable/canonical backups;
explicit service adoption, signer/recipient/target reviews, public identity/social
actions, recovery and exact outboxes; reader aggregate/layout/double-spread/
geometry/loading/retry/progress/keyboard; upload admission, dirty metadata,
staging restoration/autosave, page receipts, replacement/removal/reordering,
bounded image/archive/folder handling, commit and cancellation; retained profile,
group, user, community and client controls; normal authentication versus the
restricted reset boundary; report reads/mutations; reconstructed public and staff
announcements; persistent preference admission and the mounted cache contract.

No further concrete defect was established in this finite pass for the repaired
reader selector ownership, selected double-spread member, reachable long-strip
geometry, target-keyed upload editors, unsent/unknown upload placeholders, exact
immutable commit/cancellation controls, library-store reconciliation, namespace-
bounded saved-device mutations, import revision/context gates, canonical family/
key selection or signed public review/outbox destination ownership. The retained
URLSearchParams prefix override was checked before evaluating concatenated query
paths. The raw reader JSON-script candidate is already `<`-escaped by its mounted
shared overlay and is not duplicated as a finding.

This is not exhaustive proof of UI completeness, visual parity, offline boot,
revocation safety or production behavior. The owner's 62-component/31-script
compiler receipt is a separate source receipt, not a test performed here. Later
composition, actual current executable bundle, mobile/browser/E2E/system and
production gates remain separate. Source signoff is withheld for UI7-01 through
UI7-05 in the initial inspected snapshot.
