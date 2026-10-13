# Round-fourteen independent UI source-completeness audit

Reviewed 2026-10-13, beginning 08:18 UTC. This fresh reviewer did not implement
the preceding repairs. Shared → reader → local-library transforms were applied
independently to an inspection copy with original input SHA and exact anchor
counts enforced. Actual composed callers, mounted resolver order and relevant
external integration sources were inspected.

## Finite verdict

**Source signoff withheld: three substantive, bounded P2 findings were established.**
Both UI13 findings are source-closed on the reviewed candidate. The new findings
concern the existing entity paginator becoming unreachable on its last page,
ordinary account-library failures being silently discarded by existing mutation
controls, and mounted homepage update widgets misrepresenting or never settling
failed reads. They do not request new product features or unrelated styling.

No browser journey, application runtime, database operation, Cargo command,
Vite build or test suite was executed by this reviewer. Transformation and
hashing were inspection operations. All runtime acceptance below is **unrun**.
No implementation source was edited; this document is the repository deliverable.
Findings were reported to the parent and repair owner as they were established.
The owner changed production source during this review to address the findings.
This report concerns the frozen candidate below and does **not** certify its
subsequent repair, compilation, bundle or browser behavior. Another independent
pass is required on the final changed source.

## Independently frozen source

Inspection snapshot: `2026-10-13T08:19:32.613Z`. The initially supplied 08:17:43
owner pin was explicitly superseded by the owner's matching 08:19:33 pin. The
only intervening change reported by the owner was using a private-feed pre-effect
for clearing personal rows before the DOM update. This actual final candidate,
including that change, is the source reviewed here.

- 476 repair definitions, 485 exact occurrences, 96 original paths
- 541 copied/transformed `.svelte`/`.ts`/`.js` source files
- SHA of JSON-serialized composed path/SHA records sorted with `localeCompare`:
  `6cbd833cf9eaef5d6a5d7955154cdb636a657894c99fc025f0c9efd506fcdb7a`
- Shared overlay:
  `6fc8cd9fae676606b2b6a5d6df36ba68f905f622ad3d64a4c7e5c27640a01894`
- Reader overlay:
  `70ec02db52c0867872f1545ff20667a9b7e0f7e747261de8b5a57f62285c9505`
- Local overlay:
  `dd53213df50bfce70dd232bb892cf699012181cd06eaf34e53d12e844697b872`
- Independently calculated 59-file external integration inventory SHA:
  `8e953033771c16cf11741848511ec5269a46cc8b7400835160363099f48cf49b`

All 676 original attachment paths were independently rehashed against
`test-evidence/frontend/source-parity.json`: none was missing or different.
The external inventory matches the owner freeze at
`test-evidence/frontend/round13-owner-source-repairs/source-manifest.json`.
The matching owner receipt at `round13-owner-source-repairs/summary.json`
reports passed 84-component/52-script source compilation at 08:19:33.343 UTC.
That is owner evidence, not a compiler or runtime run by this reviewer.

Logical `src/` paths and lines below refer to the actual composed inspection
copy, not a preserved raw attachment. External paths identify actual mounted
imports. URLSearchParams prefix initialization and native HTTP authority checks
were considered before judging callers. Historical HTTP/upload fragments were
not substituted for executed bodies. The inspected source changed by the 08:31
UTC reread; the review copy remains the frozen candidate above.

## Findings

### UI14-01 — P2: Entity Uploads loses its existing paginator on the last page

- Actual composed `src/lib/components/ChapterFeed.svelte:246` requires
  `feed.limit * feed.page < feed.total` before either footer can render. The
  entity branch's only `GenericPagination` is inside that condition at
  `:247-250`. The separate nonentity continuation controller is intentionally
  disabled for entities at `:222`.
- Actual user Uploads mounts this component at
  `src/routes/(app)/user/[id]/uploads/+page.svelte:60-73`, with uploader filtering,
  requested page and limit 100 at `:66-71`. Group/author Uploads mount it at
  their respective `:59-72`, also with 100 chapters per page and actual entity
  filtering. All three key the feed on the current query. None supplies another
  paginator. `GenericPagination.svelte:14-21,25-64` is the existing page-link
  workflow, capable of links back to earlier pages when mounted.
- Native `backend/src/domains/catalog/read.rs:486-487,503-507` applies the
  requested offset and returns current count, requested page and selected rows.
  It does not force the last page back to page 1.
- Concrete scenario: a user, group or author has 101 matching chapters. Page 1
  shows its existing links to pages 1 and 2. Select page 2, or open the valid
  Uploads deep link with `page=2`. The valid response contains one chapter,
  page 2, limit 100 and total 101. Cards render, but `200 < 101` is false, so
  every page link disappears. The user cannot use the existing paginator to
  return to page 1. An exact 200-result set has the same outcome on page 2.
  Browser Back or manually editing the URL is an external workaround, not a
  functioning current-page pagination control.
- Impact: reaching the ordinary last page disables part of the promised
  entity Uploads pagination workflow. This is a composed display guard issue,
  not a requested additional navigation feature or grouping/style change.
- Closure: keep the existing entity paginator reachable on its last page when
  earlier pages exist; distinguish entity page navigation from nonentity
  exhaustion. Also retain a way back to an existing earlier result page when
  a previously valid high page becomes empty after catalogue deletion or a
  changed count. Do not revive nonentity automatic requests at a true end.
- Acceptance: user/group/author sets of 101, 200 and 201 matching chapters;
  navigate and deep-link to each last page, return to page 1 using existing
  controls, and repeat with Oldest ordering. Shrink a set after opening a high
  page and verify recovery to current earlier results. Verify a true initial
  zero-result state and terminal public/private nonentity feeds remain truthful.
- Runtime acceptance: **unrun**.

### UI14-02 — P2: Existing read/follow controls silently discard account failures

- Actual composed read/unread handlers in
  `src/lib/components/ChapterFeedChapterEntry.svelte:38-63` and
  `src/lib/components/ChapterListEntry.svelte:40-65` handle only status 200.
  Their success-only branch updates the captured read map; all other returned
  statuses are ignored. Both have `finally` but no error catch, so thrown
  account failures escape without their own actionable feedback.
- Actual composed `src/lib/components/MangaSearchEntryCard.svelte:42-60,62-77`
  handles only status 204 for Follow and Unfollow. Its existing library-error
  dispatch occurs only in `catch`, so a normal non-204 return never reaches it.
  Actual `src/routes/(app)/title/MangaActions.svelte:132-162` similarly handles
  only 204 for Unfollow. Its status-change and bell handlers already reject
  non-204 at `:77-89,115-126`; those are not findings.
- Mounted `platform-integrations/local-library/ChapterHistory.ts:32-39` and
  `MangaFollow.ts:52-60` return the actual account helper response to these
  controls. `local-library/library.ts:207-219` captures account/context,
  checks cached Session validity and returns `{status, body}` on an ordinary
  non-OK fetch. It deliberately does not throw or update Session in that branch.
  Composed `src/lib/stores/Session.ts:37-41` checks stored identity validity;
  it is not a new browser-session request at every gesture.
- Concrete scenario: render an account-mode library with a currently valid
  Session A. Let its browser authentication cookie expire, then click a real
  chapter read/unread button, card Follow/Unfollow, or title Unfollow. Cached
  Session validity remains true. The native endpoint returns 401; the helper
  returns it normally. Each affected caller skips its success branch and clears
  its spinner without a failure explanation. The still-rendered control can
  repeat the same silent result. Native history mutation actually requires a
  live principal and `self.preferences.update` at
  `backend/src/domains/library/history.rs:57-59`, so this is a legitimate failure
  path, not a fabricated response. A revoked permission or conflicting expected
  account can likewise produce normal non-success responses.
- Impact: a requested existing personal-library change has no trustworthy
  completion/failure outcome. No false successful map update was found, but
  unchanged state without explanation is not a completed mutation workflow.
  The existing device-storage helper separately sets `localLibraryState.error`,
  shown inside the mounted Device library details panel at
  `LocalLibraryPanel.svelte:253`; that indication is not claimed to be absent.
- Closure: surface ordinary non-success and thrown account outcomes at these
  existing controls. Keep captured pre-action read state, target IDs, current
  destination fencing, legitimate successes and truthful uncertain-outcome
  messages. Never replay a mutation automatically or update local flags from
  a rejected/stale result. Obsolete feedback must not attach to a newer target
  or account.
- Acceptance: real account 401, permission 403, expected-account conflict 409
  and rate-limit failure for the affected gestures; rejected and ambiguous
  network writes; device storage failure regression; successful read/unread,
  follow/unfollow and existing bell/status changes. Show actionable error or
  uncertainty, no false read/follow acknowledgement, no auto replay, and no
  obsolete feedback/state update after account/mode/target change or disposal.
- Runtime acceptance: **unrun**.

### UI14-03 — P2: Mounted homepage update widgets do not settle failed reads truthfully

- Actual composed home `src/routes/(app)/+page.svelte:43` mounts
  `LatestUpdatesSection`. Its default persisted view is `grid` at
  `src/lib/components/LatestUpdatesSection.svelte:13-14`, mounting the actual
  `LatestUpdatesGrid` at `:58-59`. This is an executed visible widget, not an
  unused old component.
- In composed `src/lib/components/LatestUpdatesGrid.svelte:60-88`, only a
  status-200 response updates the feed; there is no returned-failure branch.
  Native HTTP returns ordinary final status responses at composed
  `src/lib/HTTP.ts:91-108`, including a final 429 after bounded GET retries
  at `:95`. On such a normal failure, Grid's `onMount` sets `isMounted=true`
  at `:40-42`; with no entries, it renders **No updates found** at `:146-153`.
  The user sees a supposed empty catalogue after an actual rejected/throttled
  read. The catch/ErrorComponent path exists for thrown transport failures,
  but does not handle this returned-failure scenario.
- The same actual home mounts `MyUpdatesCarousel` under the selected library
  view at `+page.svelte:38-41`. Composed
  `src/lib/components/MyUpdatesCarousel.svelte:25-48` awaits
  `API.MangaFollow.LatestUpdates` without a catch/finally and sets
  `isMounted=true` only afterward. Mounted
  `local-library/MangaFollow.ts:34-37` uses `library.ts:203-205` account `apiRead`,
  which throws on a real 401. With a cached account identity but an expired
  browser cookie, the onMount promise rejects before completion. The initial
  skeleton/section branch at `MyUpdatesCarousel.svelte:51-94` remains forever
  without an actionable failure state.
- Concrete scenarios: fresh home in default Grid view with the chapter-updates
  request returning final 429 or 503; and fresh home in explicit account-library
  mode after the selected browser session expires. The first says there are
  no updates; the second never settles its loading state. Neither is a valid
  empty-feed response. The public `/updates` route's repaired ChapterFeed is
  a separate caller and does not repair these actual homepage widgets.
- Closure: distinguish empty successful reads from returned or thrown failures
  in these existing widgets and settle loading truthfully with a usable current
  safe-read recovery. Keep actual route/disposal and private selected-library
  ownership, existing layout and acknowledged data; do not attach old private
  rows to a new account or turn recovery into mutation replay.
- Acceptance: actual public Grid 200-empty, 200-results, final 429, 503 and
  rejected/timed-out transport; private My Updates 200-empty/results, expired
  session 401, lost permission and failed catalogue hydration. Verify no false
  empty-catalogue state or permanent skeleton, a live current recovery route,
  no stale private rows/feedback after mode/account/navigation change, and
  truthful retention of already acknowledged public rows on later read failure.
- Runtime acceptance: **unrun**.

## Independent reread of UI13 closures

1. **UI13-01 is source-closed on the frozen candidate.** Composed
   `ChapterFeed.svelte:240` gates its accumulated grid on `entries.length`.
   A legitimate terminal empty page can update exhaustion metadata at `:107`
   without hiding previous entries. Invalid empty-before-reported-end data
   fails before replacing acknowledgement at `:106`. The public snapshot is
   committed after append at `:134-147`, including accumulated unique chapter
   data and coherent manga/chapter maps rather than only the last empty page.
   Public restore uses its exact href at `:70-74`; private feeds never cache
   these DTOs. This does not certify runtime restoration or dynamic deletion.
2. **UI13-02 is source-closed on the frozen candidate.** Public load ownership
   at `:86-92` excludes unrelated library context/view, and its continuation
   capture does the same at `:226`. Startup library readiness and initial Session
   discovery therefore do not alone discard the public response. The initial
   interrupted-current-route check at `:78-80` and current safe-read Retry at
   `:211-218,234-237` provide a truthful error escape for remaining cancellation.
   Private feeds instead clear personal DTOs synchronously at `:197-209` in a
   pre-effect and serialize/coalesce latest-context reads at `:177-196`, with
   actual context/view/callback/URL/navigation ownership and explicit error
   recovery. No private navigation-history restoration was reintroduced.

The mounted `frontend-behavior/feed-continuation.ts` still supplies the separate
finite four-read serialized burst and approved explicit resume path. It checks
current ownership around awaits, settles the DOM and stops at valid exhaustion,
failure/no progress or disposal. The entity paginator issue in UI14-01 is a
distinct retained guard, not a denial of the nonentity UI12 repair.

## Finite coverage and remaining gates

The source pass covered mounted transform/resolver order and original bytes;
public search/filter/reset and explicit pagination; homepage public/private
widgets, latest/selected-library chapter feeds, read flags and public restore;
common navigation and auth-flow ownership; title follow/bell/start-reading;
reader aggregate, chapter/page selection, single/double spreads, quality,
preload/retry/queue, geometry, keyboard ownership and device/account history;
new/edit upload admission, staging/autosave/restore/cancel, immutable commit
retry, unknown/unsent/rejected placeholders and page ordering/replacement;
account preference drafts, entity-search cancellation, retained clients,
auth/settings requested-flow lifetime and restricted recovery; report reasons,
subject/submission and private feed search; discussion target/authority and
reply/edit/vote ownership; announcements list/detail/manage transport, pagination
and mutations; saved-device records/list navigation, backups/import and verified/
offline catalogue availability; and signed public-identity recipient/target
review, metadata/conversation pagination, canonical recovery and exact outboxes.

No fourth substantiated in-scope defect was established in this finite pass.
Historical fragments, already repaired raw originals, speculative new controls,
styling/grouping objections, ancillary-stat omission and races lacking a concrete
composed scenario were excluded. The Grid continuous-intersection speculation
was not filed: its real native updates response has distinct manga, normally
tall 70-title pages, unlike the compact grouped chapter-feed UI12 scenario.

Final changed source requires a fresh independent closure review. Final
executable bundle and actual browser/API/database/system journeys, offline boot,
durable persistence, authority revocation and production checks remain distinct
gates. Source compilation, preserved bytes and finite source closure cannot
establish production readiness or a runtime pass for all inventoried controls.
