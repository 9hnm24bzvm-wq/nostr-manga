# Round-fifteen independent UI source-completeness audit

Reviewed 2026-10-13, beginning 08:42 UTC. This fresh reviewer did not implement
the preceding UI14 repairs. Shared → reader → local-library transforms were
independently applied to an inspection copy, enforcing each original input SHA
and exact replacement-anchor count. Mounted callers, resolver order, imported
integration modules and relevant native response contracts were inspected.

## Finite verdict

**Source signoff withheld: five substantive, bounded P2 findings were established.**
The three assigned UI14 findings are source-closed on the frozen candidate.
New findings concern the existing title chapter filter's applied baseline,
automatic account reader-history acknowledgement, double-spread selection,
tap navigation after closing a left reader panel, and a completed empty
Recommendations tab remaining in its loading state. None requires a new
product feature, a changed visual design or different content grouping.

No browser journey, application runtime, database operation, Cargo command,
Vite build or test suite was executed by this reviewer. Transformation and
hashing were inspection operations. Runtime acceptance below is **unrun**.
No implementation source was edited; this document is the repository deliverable.
Each finding was reported to the parent as it was established. The repair owner
began changing implementation source during this review; this report reviews
the frozen candidate below, not those subsequent repairs. A fresh independent
review of the final changed source is required.

## Independently frozen source

Inspection snapshot: `2026-10-13T08:44:00.073Z`.

- 496 repair definitions, 505 exact occurrences, 97 original paths
- 541 copied/transformed `.svelte`/`.ts`/`.js` source files
- SHA of JSON-serialized composed path/SHA records sorted with `localeCompare`:
  `7810c83a0300373237a46b4461deb593f21a71de6ccb1e5615835ee10e75df0e`
- Shared overlay:
  `6fc8cd9fae676606b2b6a5d6df36ba68f905f622ad3d64a4c7e5c27640a01894`
- Reader overlay:
  `70ec02db52c0867872f1545ff20667a9b7e0f7e747261de8b5a57f62285c9505`
- Local overlay:
  `9960c36dd1ba3d7523f9be0905a47557efab48a6b7ab045aaf94cc83ab87f1f9`
- Independently calculated 61-file external integration inventory SHA:
  `81b210edf3924854ccdde6df9e1d6ae9b1a5cb0be9eafb33fa3b846fd208ba18`

All 676 original attachment paths were independently rehashed against
`test-evidence/frontend/source-parity.json`: none was missing or different.
The independent hashes match the supplied UI14 owner freeze at
`test-evidence/frontend/round14-owner-source-repairs/source-manifest.json`.
Its matching `summary.json` reports passed source compilation of 85 components
and 54 scripts at `2026-10-13T08:42:15.694Z`. That is owner evidence, not a
compiler or runtime run by this reviewer. Build15 completion was subsequently
reported by the owner; that artifact is not independently runtime-certified
here and must not stand in for a bundle incorporating later repairs.

Logical `src/` paths and line numbers below refer to the actual composed
inspection copy, not preserved raw attachments. External paths identify
executed imports. The mounted native HTTP authority fences, local API resolver,
anonymous-recovery derivative, announcement reconstruction and retained
`URLSearchParams.toString()` question-mark-prefix initialization were checked.
Historical HTTP and upload fragments were not substituted for executed bodies.

## Findings

### UI15-01 — P2: Chapter filters cannot return to their initially applied selection

- Actual composed `src/lib/components/ChapterListFilter.svelte:26-31` captures
  `initialSearch` and the editable search once from the initial URL. Its Apply
  navigation at `:43-49` does not update that baseline. The visible Apply
  button at `:131` remains disabled when `search` equals that first snapshot,
  rather than the current applied query.
- Actual composed `src/routes/(app)/title/MangaChapterListSection.svelte:53`
  leaves that filter mounted outside the query-keyed chapter-list lifetime at
  `:59-61`. The title route parents key on `data.manga` at their respective
  `src/routes/(app)/title/[id]/+page.svelte:9` and
  `[id]/[slug]/+page.svelte:9`. Their load modules access only the route
  parameters and `url.searchParams.get("draft")`; ordinary language/group/order
  query changes do not refresh that data key. The installed SvelteKit client
  source tracks named search-parameter access at `client.js:895-900` and
  checks only those tracked parameters at `:1032-1033`. This is an actual
  retained filter, not a proposed component-reuse scenario.
- Concrete scenario: deep-link to a published title with English and Spanish
  chapters using `?tlang=en`. Select Spanish and Apply. The URL and chapter
  results now use `tlang=es`. Select English again. The editable state now
  equals the original English snapshot, so Apply is disabled while the
  actual query/results remain Spanish. The same issue applies to initially
  selected groups. Browser Back/Forward also changes the applied query/feed
  without synchronizing the still-mounted language/group/order fields.
- Impact: an ordinary repeated use of the existing filter cannot apply its
  advertised selection; the controls can describe a different filter from
  the results. Reloading or using another control as a workaround does not
  complete the existing Apply workflow.
- Closure: keep the existing editable fields and applied baseline synchronized
  with their current title/query context, or give the filter that context's
  lifetime. Preserve defaults, preference fallbacks, Reset, ordering, pagination
  and the retained layout. Do not overwrite a still-owned unsent edit because
  an unrelated update arrived.
- Acceptance: initial language A → apply B → choose/apply A; initial group A
  → B → A; combined groups/languages; Reset after multiple applies; Oldest
  toggle; pagination; Back/Forward; and navigation to a different title.
  The displayed controls and disabled Apply state must match the current
  applied query, and every genuinely changed valid selection must be usable.
- Runtime acceptance: **unrun**.

### UI15-02 — P2: Automatic account reader history silently ignores rejected writes

- Actual composed `src/lib/components/reader/reader-history.svelte:28-41`
  admits the existing account mark-read workflow when a readable chapter is
  mounted. Its promise returns `API.ChapterHistory.UpdateBatch(...)` at
  `:38-40` and handles only thrown errors at `:41`; no returned status/body
  acknowledgement is checked.
- Mounted `platform-integrations/local-library/ChapterHistory.ts:32-39`
  forwards account-mode updates to `accountRequest`. Actual
  `local-library/library.ts:207-219` checks the cached Session then returns
  ordinary non-OK HTTP responses normally. Composed
  `src/lib/stores/Session.ts:34-38` checks cached identity validity; it does
  not revalidate the browser authentication cookie for this gesture.
- Native `backend/src/domains/library/history.rs:57-59` requires a live
  principal and `self.preferences.update` for that POST. Therefore a normal
  401 after cookie expiry, or 403 after permission revocation, is a real
  native outcome. It is not a fabricated response or an HTTP-layer throw.
- Concrete scenario: select account-library mode with a valid cached account,
  let its browser cookie expire, then open a public readable chapter. The
  existing reader mark-read request returns 401. The helper resolves normally,
  so the reader's failure toast never runs. The chapter is not saved in account
  history and there is no explanation. Its `accountReadToken` was already set
  at `:37`, so ordinary same-authority rerenders also do not reveal the failure.
- Impact: the existing automatic read/history workflow drops a rejected save
  silently. UI14 repaired the five explicit read/follow click handlers; those
  checks do not cover this actual reader caller. Device progress persistence
  and the absence of false local read acknowledgement are not disputed.
- Closure: require the real account history acknowledgement inside the admitted
  reader intent and surface returned rejection or uncertainty only while it
  still belongs to that reader/account/library. Retain pre-await admission,
  current destination/authority fencing, no automatic mutation replay and
  legitimate once-per-authority behavior. Do not infer successful read history
  from a rejected or stale response.
- Acceptance: actual account 200, expired-cookie 401, permission 403,
  expected-account conflict 409, rate limit and rejected/ambiguous network
  writes on reader entry. Verify truthful feedback, no false successful mark,
  no automatic retry, no stale feedback after mode/account/chapter/disposal
  change, and no repeated successful POST from harmless Session data refresh.
  Regress the separate device page/progress/completion and exit-flush flows.
- Runtime acceptance: **unrun**.

### UI15-03 — P2: Double-mode page selection sends a spread index as a page index

- Actual composed `src/lib/components/reader/reader-page-selector.svelte:38-41`
  uses the current spread's index as the selector value in double mode but
  passes that value directly to `reader.changePage(Number(value))`.
  Its double-mode options at `:67-68` likewise encode each spread index;
  their labels at `:70-78` describe the spread's actual pages.
- Actual composed `src/lib/components/reader/context.svelte.ts:123-137`
  interprets that argument as a raw page index and assigns `this.pages[idx]`.
  `reader-provider.svelte:3-24` forms spreads from page pairs, landscape singles
  and the selected offset, so spread and page indexes are not interchangeable.
  The real reader panel mounts this selector at `reader-panel.svelte:99`.
- Concrete scenario: a four-portrait-page chapter, double enabled, offset 0,
  current spread 1–2. Choose the visible 3–4 option. Its value is `1`, which
  selects raw page index 1, page 2, still in the 1–2 spread. The selected label
  and visible page return to 1–2 instead of the requested 3–4. More spreads,
  a single cover offset and mixed landscape pages produce further mismatches.
- Impact: the existing double-mode page/spread selection control cannot
  navigate to the spread its option advertises. This is separate from the
  previously repaired visibility of a selected spread's second member.
- Closure: translate the selected spread into its actual admitted page index
  at the selector boundary, while preserving raw-index semantics for single
  mode and the existing first/last/next/previous controls. Invalid or obsolete
  options must not produce a different page accidentally.
- Acceptance: every option in chapters with 4 and 8 portrait pages, offset 0
  and 1, mixed portrait/landscape singles, odd final pages, LTR and RTL, paged
  and scrolling layouts, and single↔double toggles. The visible spread, label,
  current raw page and saved page URL must agree with the chosen option.
  Regress direct odd/even deep links and first/last/next/previous navigation.
- Runtime acceptance: **unrun**.

### UI15-04 — P2: Closing a left reader panel makes taps choose the wrong side

- Actual composed `src/lib/components/reader/context.svelte.ts:222-240`
  subtracts a fixed 450 pixels from every click whenever the saved
  `panel_position` is `left` at `:227-230`, then compares that value with
  half `mainRef.clientWidth` at `:233`. It does not inspect actual main-area
  position or whether that sidebar is open.
- Left/Right are existing settings at `reader-settings.svelte:252-275`.
  The panel's Close control calls `sidebar.toggle` at
  `reader-panel.svelte:85`. Actual composed
  `src/lib/components/ui/sidebar/sidebar.svelte:65-75` makes the desktop
  offcanvas gap width zero when collapsed; its mobile branch at `:33-54`
  is a sheet, not a 450-pixel reserved reading-column offset. The reading
  main is the actual `bind:this={reader.mainRef}` element in `reader.svelte`.
- The single and double page wrappers actually bind the listener at
  `reader-pages-single.svelte:96` and `reader-pages-double.svelte:94`.
  Therefore this affects real reader-image taps, not an unused method.
- Concrete scenario: choose a left panel, close it, and use an 800-pixel-wide
  reading area beginning at x=0 in LTR paged mode. A right-half tap at x=600
  becomes `150 <= 400`, selecting Previous. Even x=799 becomes 349, so no
  point in that reading area selects Next. A closed mobile left panel has
  the same wrong fixed offset. RTL reverses the wrong classification rather
  than repairing its geometry.
- Impact: an existing panel setting and Close gesture disable the expected
  side of tap navigation. This is a functional hit-test error, not a request
  to resize or restyle the panel.
- Closure: determine the hit side from the actual current reading-area
  rectangle, accounting for its real left offset/width. Preserve LTR/RTL,
  tap-navigation preferences, paged/scrolling behavior and native controls.
  Do not hard-code an open-panel offset for closed or mobile panels.
- Acceptance: right/left panels open and closed; desktop widths around 800
  and 1200 pixels; mobile closed sheets; viewport/font-scale changes; taps
  well inside both reading halves; LTR/RTL; single/double spreads; paged and
  long-strip tap-enabled/disabled modes. Each half performs the advertised
  previous/next action without changing unrelated settings.
- Runtime acceptance: **unrun**.

### UI15-05 — P2: An acknowledged empty Recommendations tab never finishes loading

- Actual composed `src/routes/(app)/title/MangaRecommendationsSection.svelte`
  accepts a successful response and stores its feed at `:34-39`, then sets
  `isMounted=true` at `:51`. Cache restore at `:24-25` also accepts an empty
  feed. Its markup at `:55-73` renders results only when both mounted and
  `feed.data.length` are truthy; every nonerror empty state renders Spinner.
- The visible title Recommendations tab is hidden only for unpublished
  titles at `src/routes/(app)/title/Manga.svelte:101-105`, and the selected
  tab component actually mounts at `:514` or `:539`.
- Native `backend/src/domains/catalog/read.rs:282-293` reads actual
  `catalog_recommendations` rows and returns a legitimate 200 feed with
  `total:0,data:[]` for a published title with no recommendation rows, or
  when all recommendations are currently hidden/withdrawn. No synthetic
  malformed response or pending database operation is required.
- Concrete scenario: open that existing Recommendations tab for a published
  title with zero visible recommendations. The read completes successfully,
  but the tab spins forever. Returning through its acknowledged cache repeats
  the same false loading state.
- Impact: a completed existing tab workflow has no truthful terminal outcome.
  This does not ask for generating recommendations, adding a recommender or
  exposing unrelated analytics.
- Closure: distinguish mounted acknowledged empty data from a pending read
  and settle the existing tab truthfully. Preserve its results, real error
  handling, current title ownership and successful cache behavior.
- Acceptance: actual 200-empty and 200-results feeds; all recommended targets
  hidden; cache restoration of each; returned/thrown read failures; switching
  tabs and navigating to another title while loading. A successful empty read
  must not leave a loading indicator, and old results must not attach to a
  newer title.
- Runtime acceptance: **unrun**.

## Independent reread of UI14 closures

1. **UI14-01 is source-closed on the frozen candidate.** Composed
   `ChapterFeed.svelte:246-250` keeps the existing entity paginator mounted
   on its last page when multiple pages exist. The separate `:286-290`
   branch mounts it for an empty requested high page without duplicating the
   normal paginator. Nonentity terminal-feed behavior remains separate.
   `GenericPagination` paths are valid under the retained root layout's
   question-mark-prefix initialization; the superficially missing `?` is
   not a finding.
2. **UI14-02 is source-closed on the frozen candidate.** The actual composed
   chapter-feed/list read handlers, card Follow/Unfollow and title Unfollow
   require returned success acknowledgements and report only current-intent
   failures. Their mounted action owner retains destination/selection/native
   authority admission, and the API wrappers preserve captured library
   context. UI15-02 is a distinct automatic reader-history caller, not a
   claim that those five repairs are absent.
3. **UI14-03 is source-closed on the frozen candidate.** Composed
   `LatestUpdatesGrid.svelte:31-48` requires read acknowledgement, distinguishes
   returned/thrown failures, fences current route/filter ownership and settles
   loading. Error/Retry at `:63-65` and the `:77` empty guard prevent a failed
   request from becoming a false empty catalogue. Already acknowledged public
   rows survive a later read failure. Actual `MyUpdatesCarousel.svelte:29-60`
   serializes/coalesces latest-context reads, clears old private rows before
   new ownership, handles acknowledgement/failure and settles the current
   request; its `:85-87` error/retry surface is reachable. Mounted outer
   selected-library keys and current native authority fences were considered.

The prior accumulated-feed/exhaustion/public-cache repairs and separate
four-page serialized continuation with explicit resume remain present. Nothing
in this pass reopens an earlier finding solely from its unchanged raw input.

## Finite coverage and remaining gates

The finite source pass covered actual shared/reader/local composition and
resolver ordering; original-byte parity; public discovery/filter/reset,
generic and manga pagination, title chapter filters and entity Uploads;
homepage public/private update widgets and feeds, acknowledgement/restoration,
read/follow/bell/status changes and start-reading; reader aggregate and
chapter/page navigation, single/double/offset, quality/preload/retry, geometry,
panel/layout/direction controls, keyboard ownership and device/account history;
new/edit upload admission, metadata reset, staging autosave/restore/cancel,
immutable commit, replacement/page deletion/order/duplicate handling and
unknown/unsent/rejected batch recovery; account preference draft lifetime,
entity searches, retained clients and clipboard outcomes; auth/settings
requested-flow ownership and restricted recovery; private library/history,
CSV/MAL import acknowledgement/context and export, saved-device lists/records,
offline scope availability and backup/import controls; report subjects/reasons,
private search, submission and moderation; discussion creation/edit/vote/read
ownership; reconstructed announcement list/detail/manage transport, pagination
and mutation outcomes; and signed public identity, reviewed recipient/target
ownership, canonical records/recovery and exact public/library outboxes.

No sixth substantiated in-scope defect was established in this finite pass.
Unexecuted historical fragments, unmounted popularity grids, speculative
new controls/features, styling/grouping objections, ancillary-stat omissions
and races lacking a concrete composed/native scenario were excluded.

Final changed source still requires a fresh independent closure review.
The final executable bundle and actual browser/API/database/system journeys,
offline boot, durable persistence, permission revocation and production checks
remain distinct gates. Source compilation, preserved bytes and finite source
closure cannot establish perfect completeness or production readiness.
