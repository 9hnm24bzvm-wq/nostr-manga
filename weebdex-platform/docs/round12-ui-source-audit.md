# Round-twelve independent UI source-completeness audit

Reviewed 2026-10-13, beginning 07:52 UTC. This fresh reviewer did not implement
the preceding repairs. The shared → reader → local-library transforms were
independently applied to an inspection copy. Original input hashes and exact
anchor counts were enforced. Actual composed callers, mounted resolver
configuration and external integration modules were read.

## Finite verdict

**Source signoff withheld: one substantive, bounded P2 finding was established.**
Both UI11 repairs are present and source-close their reported scenarios. The
new finding concerns continuation of the existing infinite chapter-feed
pagination when its rendered cards remain compact. It is not a request for a
new feature or a change to the retained card layout/grouping/styles.

No browser journey, application runtime, database operation, Cargo command,
Vite build or test suite was executed by this reviewer. Source transformation
and hashing are inspection operations. All runtime acceptance below is
**unrun**. No implementation source was edited by this reviewer; this report
is the sole repository deliverable. The finding was sent to the parent and
repair owner when established. A subsequently changed source requires a fresh
independent pass; this report does not certify a later repair.

## Independently frozen source

Frontend inspection snapshot: `2026-10-13T07:54:11.907Z`.

- 444 repair definitions, 453 exact occurrences, 96 original paths
- 541 copied/transformed `.svelte`/`.ts`/`.js` source files
- SHA of JSON-serialized composed path/SHA records sorted with `localeCompare`:
  `5712ee67ef916157568249d7da43456eb2084f207874caa2f8947d6042160f0c`
- Shared overlay:
  `6fc8cd9fae676606b2b6a5d6df36ba68f905f622ad3d64a4c7e5c27640a01894`
- Reader overlay:
  `70ec02db52c0867872f1545ff20667a9b7e0f7e747261de8b5a57f62285c9505`
- Local overlay:
  `0f08079f49c0af3672792855f7aee9f7dde931f13ad83af9d2ab1ac5f69639c5`
- Independently calculated 58-file external integration inventory SHA:
  `e49b500490f92f13434d73af1c63a7d715dde1fd417696705d5c72421f851344`

All 676 original attachment paths were independently rehashed against
`test-evidence/frontend/source-parity.json`: none was missing or different.
All 58 external integration paths were independently read and rehashed against
the UI11 owner freeze: none was different at the 08:01 UTC unchanged-source
check, and the verified frozen records independently hash to the inventory
above. During final report preparation, the repair owner changed the local
overlay for UI12; that later source is not certified by this frozen review.
Logical `src/` paths and lines below refer to the composed inspection copy,
not the preserved raw attachment. External paths identify the actual mounted
modules. Unused historical fragments are not substituted for executed source.

The existing owner compiler receipt at `2026-10-13T07:52:00.308Z` reports
passed 84-component/51-script source compilation with this exact external
inventory. Its owner freeze is
`test-evidence/frontend/round11-owner-source-repairs/summary.json`. That is
owner evidence, not a compiler/runtime run by this reviewer. Build12 was being
prepared during the review; no executable bundle or browser acceptance is
certified here.

## Finding

### UI12-01 — P2: A continuously visible chapter-feed footer stops pagination

- Actual composed `src/lib/components/ChapterFeed.svelte:145-170` creates one
  IntersectionObserver for the current footer. Its callback loads exactly one
  following page at line 156. The effect's synchronous reads do not depend on
  `feed` or `entries`, and after that load no continuation checks whether the
  footer still intersects. The observer is not unobserved/reobserved after a
  successful load.
- Its existing non-entity pagination at `:183-195` keeps that same footer while
  `feed.limit * feed.page < feed.total`. There is no next-page/load-more control
  in this branch. GenericPagination at `:184-187` is supplied only for the
  user/author/group entity branch selected at `:33-36`.
- The actual rendered `ChapterFeedMangaEntry.svelte:32-33,63-79` starts trimmed
  and shows only the first three chapters per card. `ChapterFeed.svelte:91-112`
  groups each 100-chapter response into its manga cards. It is therefore an
  ordinary valid response shape for hundreds of chapters to produce just one
  or a few compact cards per page.
- The existing `/updates` caller at `src/routes/(app)/updates/+page.svelte:16-27`,
  `/library/updates` at `:18-29`, and `/history` at `:18-29` all use this actual
  component with 100 chapters per page. The latter two mount it under the
  current selected-library view key. All three use the non-entity branch.
- Concrete scenario: open one of those feeds containing 300 or more matching
  chapters concentrated in one/few manga, with sufficient viewport space that
  the trimmed first and second page cards keep the footer inside the expanded
  observer root (`rootMargin: window.innerHeight + "px"`, line 164). The first
  observer entry loads page 2. The footer remains intersecting before, during
  and after that successful load; no new threshold crossing occurs. The same
  footer remains mounted, so no fresh observer is attached. Page 3 is never
  requested although `total` still promises it. Merely remaining at the end,
  scrolling within a short page, or waiting does not advance the feed. Users
  must discover a layout workaround such as expanding enough already-loaded
  chapter cards to force a new crossing; the default feed stalls at 200.
- Impact: the existing Updates/My Updates/History browsing workflow does not
  reliably reach its remaining results. It also affects any other valid compact
  non-entity result distribution. This is a continuation defect, not an objection
  to duplicate title presentation or an unrequested grouping redesign.
- Closure: retain the current layout and entity pagination, but provide a live,
  serialized continuation while more results remain and the footer is still
  within the intended loading range. Recheck current geometry after a successful
  load and stop at actual exhaustion, load failure, invalidated route/library
  context, disposal or a response that makes no forward progress. Avoid parallel
  loads, repeatedly requesting the same page, blind background exhaustion and
  continuation inherited by a newer feed. Yielding after a finite responsive
  burst must still have an independent live resume path if the footer remains
  in range; an arbitrary burst cutoff alone recreates the defect.
- Acceptance: use at least 300 matching chapters from one/few manga, keep cards
  trimmed, and keep the footer continuously inside its expanded root. Pages
  beyond 2 become reachable without toggling Show More/Show Less or navigating
  manually. Repeat with several compact pages, normal taller feeds, viewport
  resize and restored public-feed history. Verify exact end-of-feed settlement,
  zero duplicate/parallel page requests, truthful error/no-progress handling,
  entity pagination unchanged, and no old continuation or history write after
  route change, disposal, device/account selection or authority invalidation.
- Runtime acceptance: **unrun**.

## Independent reread of round-eleven closures

1. **UI11-01 is source-closed for the delayed saved-device-list redirect.**
   Actual `platform-integrations/local-library/LocalLibraryPanel.svelte:61-78`
   captures its route owner synchronously at the link gesture and checks it
   before work and after each snapshot/mode await. Actual
   `frontend-behavior/route-action-owner.ts:4-10` binds a captured gesture to
   component lifetime, action generation, exact browser URL and navigation
   epoch. `reader-navigation.ts:6-15` advances that epoch at navigation start,
   Popstate/Hashchange and disposal. The common app layout installs the guard,
   and reader-panel destruction separately invalidates the route owner. An
   acknowledged device-mode preference may remain committed; suppressing the
   obsolete redirect does not roll it back. Saved-list/scope checks remain.
2. **UI11-02 is source-closed for same-page device progress after mode reentry.**
   Composed `src/lib/components/reader/reader-history.svelte:43-52` captures
   the current history intent before constructing its progress token and
   includes `intent.destinationKey`. Actual
   `local-library/action-owner.ts:9-20` derives that key from target,
   selection epoch, admitted context and native authority. Actual
   `library.ts:36-42,119-128` advances explicit-selection admission independently
   of harmless record/store refreshes. A still-mounted reader returning to
   device mode can enqueue the unchanged current page under its new destination;
   old queued actions remain fenced. Durable progress still checks the
   destination inside its scope-bound transaction (`library.ts:169-179`),
   retains previously completed status and preserves the already-admitted exit
   flush. No account-history mutation replay was proposed or certified.

## Coverage and remaining gates

The finite source pass inspected overlay ordering, import resolution,
preservation and the preceding closures; common navigation/auth-form ownership;
public search/filter/reset/pagination; selected-library search/updates/history
and read flags; title follow/bell/start-reading; reader aggregate/context/page
selection, single/double images, geometry, queue/quality/retry and history;
upload admission, staging/autosave/restore/cancel, immutable retry, page order
and replacement seams; account preferences/client controls and auth/recovery;
report reasons/submission/search/review and private response fencing;
announcement feed/detail/manage transport and action ownership; saved-device
records, lists, backups/import and verified/offline catalogue scope; and signed
public-identity target/recipient/review ownership, metadata/social pagination,
canonical recovery/import/publication and exact outbox controls.

Mounted URLSearchParams prefix behavior was considered before judging query
concatenation. Raw-original defects repaired by the mounted transforms,
unused wrappers, speculative features, style/grouping changes and merely
possible races without an established concrete scenario were not filed. No
second substantiated in-scope defect was established in this finite pass.

Final executable bundle and actual browser/API/database/system journeys,
offline boot, persistence, revocation and production checks remain separate
runtime gates. Compilation, preserved source bytes and finite source closure
cannot substitute for those gates or establish production readiness.
