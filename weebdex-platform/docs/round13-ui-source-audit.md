# Round-thirteen independent UI source-completeness audit

Reviewed 2026-10-13, beginning 08:05 UTC. This fresh reviewer did not implement
the UI12 continuation repair. Shared → reader → local-library transforms were
independently applied to an inspection copy with original input hashes and exact
anchor counts enforced. Actual composed callers, mounted resolver configuration
and relevant imported integration modules were inspected.

## Finite verdict

**Source signoff withheld: two substantive, bounded P2 findings were established.**
The continuously intersecting compact-footer scenario from UI12-01 is source-
closed by the new serialized continuation and explicit bounded resume path.
The findings below concern a legitimate empty terminal page hiding acknowledged
results and a newly introduced initial-load cancellation with no live recovery.
Neither requests a layout/grouping/style change or an additional product feature.

No browser journey, application runtime, database operation, Cargo command, Vite
build or test suite was executed by this reviewer. Source transformation and
hashing were inspection operations. All runtime acceptance below is **unrun**.
No implementation source was edited; this document is the repository deliverable.
Both findings were reported to the parent and repair owner immediately after
establishment. The owner subsequently changed the local overlay during this
review. This report concerns the frozen UI12 source below and does **not** certify
that later repair or its executable bundle. A fresh independent pass is required
on the final changed source.

## Independently frozen source

Inspection snapshot: `2026-10-13T08:07:08.457Z`.

- 456 repair definitions, 465 exact occurrences, 96 original paths
- 541 copied/transformed `.svelte`/`.ts`/`.js` source files
- SHA of JSON-serialized composed path/SHA records sorted with `localeCompare`:
  `ee39b979bd09cf4ab187b370feb8c5bee047f7ab7d3edb5e8f8dc818b52229b7`
- Shared overlay:
  `6fc8cd9fae676606b2b6a5d6df36ba68f905f622ad3d64a4c7e5c27640a01894`
- Reader overlay:
  `70ec02db52c0867872f1545ff20667a9b7e0f7e747261de8b5a57f62285c9505`
- Local overlay:
  `560cf6b9d615a4f54eb02cdf201142bcc6480f1c1b5661316244df0db54240fa`
- Independently calculated 59-file external integration inventory SHA:
  `8af5e851f68ca654e7f2ae82a5174f53f3448d8d0da2a59133c4b536baf24ea5`
- Imported `frontend-behavior/feed-continuation.ts` SHA:
  `3f22a2d2085ad91c8a13b25e1eca6c09c4a8699a19b24133b12268d1a5da04a7`
- Imported `local-library/library.ts` SHA:
  `e30fdf5e0dcec64f8eeefa1e9fd98bd49887381514c0b36f646a4b494dec1b68`
- Imported `local-library/LocalLibraryPanel.svelte` SHA:
  `fa384eb25f3702e4bbcc7fec694620dfc6f4ac37d3f4a95bb9e35da5b713cff3`
- Relevant native `backend/src/domains/catalog/read.rs` SHA:
  `330cd648a8a8001667ae40fc5a254beacca1cfd049f8e4b4cd8751f158e0d07b`

All 676 original attachment paths were independently rehashed against
`test-evidence/frontend/source-parity.json`: none was missing or different.
The independently calculated integration inventory matches the UI12 owner
freeze at `test-evidence/frontend/round12-owner-source-repair/source-manifest.json`.
The imported controller, library core and local panel remained byte-identical
at the 08:14 UTC reread. The local overlay had changed by that reread; the composed
inspection copy remains the earlier candidate.

The UI12 owner receipt at
`test-evidence/frontend/round12-owner-source-repair/summary.json` reports passed
84-component/52-script source compilation at `2026-10-13T08:03:55.506Z`, matching
the frozen overlay/inventory hashes above. That is owner evidence, not a compiler
or runtime run by this reviewer. Build13 was being prepared when assigned and
became stale for subsequent repairs. No final executable artifact is certified.

Logical `src/` paths and lines below refer to the actual composed inspection
copy, not the preserved raw attachment. External paths identify actual imported
modules. In particular, the historical `frontend-behavior/HTTP.ts` and unused
upload fragments are not substituted for their executed overlay bodies. The
native authority checks in composed `src/lib/HTTP.ts` were considered before
judging retained REST callers.

## Findings

### UI13-01 — P2: An empty terminal continuation page hides all accumulated updates

- Actual composed `src/lib/components/ChapterFeed.svelte:89-96` accepts a valid
  HTTP 200 page with `data: []` and replaces `feed` with it. Lines `:97-100` save
  that latest empty feed to public navigation state. No append removes the
  already accumulated `entries`; the loop simply has no rows at `:106-124`.
- The entire accumulated grid is nevertheless gated on `feed.data?.length` at
  `:176-181`. After the empty continuation, the component renders the original
  **No results found** branch at `:200-216`, even with many acknowledged entries.
  Its public history restore at `:65-68` also restores the empty latest feed
  alongside the nonempty entries, reproducing the misleading empty view.
- Actual imported `platform-integrations/frontend-behavior/feed-continuation.ts`
  correctly treats the empty page as a stop (`:11,34`). This finding is the
  composed display/cache outcome after that stop, not repeated automatic loads.
- Native `backend/src/domains/catalog/read.rs:459,464-470` filters current visible
  chapters, `:487` applies the requested offset, and `:504-507` returns the same
  request's current count, requested page and an empty array when that offset
  is now beyond the current results. It does not clamp the requested page.
- Concrete scenario: open `/updates` with 101 matching visible chapters. Page 1
  acknowledges 100. Before page 2 is read, the sole remaining chapter is deleted
  or withdrawn from the public catalogue. The legitimate page-2 response is
  `{page: 2, limit: 100, total: 100, data: [], map: {manga: {}}}`. It passes the
  repaired pagination validation. All 100 previously shown chapter results
  disappear and are replaced by **No results found**, even though those 100
  chapters are still visible. Automatic compact continuation or an ordinary
  later scroll can reach this outcome. Returning to the cached public feed
  reproduces it. No malformed server response is needed.
- Impact: the existing Updates infinite-feed workflow loses its acknowledged
  display at a legitimate dynamic end of pagination. The same composed branch
  is used by selected-library Updates and History. This is not an objection to
  the existing per-page manga grouping or duplicate title presentation.
- Closure: keep acknowledged accumulated results renderable when a later page
  is legitimately empty and terminal, while retaining the original initial-
  empty state and accurate exhaustion metadata. Keep public restoration coherent
  with accumulated results. An empty response that still promises more results
  must stop truthfully without discarding prior acknowledgements or blindly
  issuing more reads. Preserve current entity pagination and retained layout.
- Acceptance: reproduce the native 101→100 result set between page 1 and page 2.
  The existing 100 results remain visible, no false empty state appears, and
  continuation settles without more requests. Navigate away and Back to the
  same public feed and verify coherent restored results/exhaustion. Also return
  an initial empty feed and verify the original empty state, then return an
  inconsistent empty page before its reported end and verify truthful stopping,
  retained results and a live authorized retry/reload path. Repeat private feed
  context invalidation and entity pagination to prevent cross-context restore.
- Runtime acceptance: **unrun**.

### UI13-02 — P2: Normal library readiness can strand the initial public feed forever

- Actual composed `src/routes/(app)/updates/+page.svelte:16-27` mounts an unkeyed
  `ChapterFeed` whose callback uses the public `API.Chapter.LatestFeed`. Its
  existence does not depend on local library availability or revision.
- Common composed `src/routes/(app)/+layout.svelte:18-20` mounts the actual
  `LocalLibraryPanel` alongside that route. Its imported source starts
  `openLibrary()` on mount at
  `platform-integrations/local-library/LocalLibraryPanel.svelte:89-92`.
- Imported `local-library/library.ts:58-69` asynchronously opens IndexedDB,
  discovers saved mode and starts independent scope verification. Verification
  changes `scopeStatus` from `pending` to `verified` (or `offline`) at `:79-95`.
  The derived context key includes scope status and account identity at `:28-30`,
  so normal readiness increments the captured context generation. Initial
  Session readiness can likewise change the account identity in this key.
- UI12 newly makes **every** initial ChapterFeed request capture that library
  context at composed `ChapterFeed.svelte:78-82`. After awaiting the public
  request, `:87` silently discards its result when readiness changed the context.
  The loader returns `false` with no `feed` and no visible error. Its onMount
  callback nevertheless sets `isMounted = true` at `:71-73`.
- There is no initial safe-read reschedule after that cancellation. The new
  continuation only mounts after a real `footerRef` exists (`:159-169`); the
  footer requires a nonempty loaded feed (`:175-188`). With `feed` still undefined,
  the component remains in the spinner branch at `:222-225`, with no continuation
  or Retry/Load more control. `/updates` is not keyed by the changed library state.
- Concrete scenario: directly open `/updates` in a fresh tab without a restored
  public-feed cache. Hold its first `/chapter/feed` response. Let normal local
  IndexedDB initialization and `/protocol/capabilities` verification complete,
  changing pending→verified, then release a valid HTTP 200 chapter feed. The
  response is discarded solely because this unrelated initialization finished.
  The page shows a permanent loading spinner until external navigation/reload.
  No explicit account/device switch, malformed response or denied permission is
  needed. The inverse completion order can work, making ordinary startup timing
  determine whether public Updates is usable.
- Impact: the UI12 repair introduced a source-established initial-load stall in
  an existing public route. Cancellation prevents stale attachment but, without
  a live current initial read, does not complete the workflow.
- Closure: distinguish actual public request ownership from initial local/private
  readiness, or safely admit a current initial read after readiness/cancellation.
  Preserve exact route/navigation/disposal fencing and private selected-library
  authority. Do not resurrect an abandoned response or replay a mutation. Bound
  any reschedule to a live current context; a newer request must use its actual
  current preferences/query instead of silently restoring obsolete filters.
- Acceptance: delay the first public feed past local pending→verified, pending→
  offline and initial Session discovery. The current public route reaches real
  results, its real empty state, or a truthful actionable error, never an orphaned
  spinner. Reverse completion order and repeat with saved account mode. Defer
  the same request across actual navigation, Back/Forward and disposal and verify
  no old result/cache/navigation side effect attaches. Verify selected-library
  feeds still reject old account/mode/scope data and any retry is a current safe
  read rather than a mutation replay or indefinite background loop.
- Runtime acceptance: **unrun**.

## Independent reread of the UI12 continuation closure

The mounted `ChapterFeed.svelte:159-169` imports and creates the actual
`feed-continuation.ts` controller. Its four-read burst is sequential (`:25-28`),
waits for Svelte DOM settlement and verifies context before and after each await
(`:29-33`), and rechecks actual current geometry before further reads (`:12,26`).
Success returns from the composed loader only after response validation/append;
invalid pagination and duplicate-only next pages fail before append
(`ChapterFeed:89-95`). Old results are fenced by component lifetime, request
and callback generation, exact URL, navigation epoch and captured context.
Disposal disconnects the observer and invalidates the loader.

A still-compact footer receives the explicit approved **Load more updates**
control after the finite burst, or **Retry loading updates** after failure
(`ChapterFeed:194-195`, controller `:36-42,48`). Duplicate gestures do not create
parallel controller runs (`:19,22`), and the loader separately rejects overlapping
reads (`ChapterFeed:77`). Entity GenericPagination remains the separate existing
branch at `:182-186`. These source facts close the particular continuous-
intersection stall from UI12-01. The findings above are distinct display/end and
initial-admission seams. They do not deny the presence of that repair.

## Finite coverage and remaining gates

The source pass covered mounted overlay/resolver order and literal preservation;
public search/filter/reset and explicit pagination; latest/selected-library
chapter feeds, read flags and public restoration; common navigation and auth-form
ownership; title follow/notification and start-reading seams; reader aggregate,
chapter/page selection, single/double spreads, quality/preload/retry, geometry,
keyboard ownership and device/account history; new/edit upload staging,
autosave/restore/cancel, immutable commit retry, local unknown/unsent/rejected
placeholders, image/archive admission and page-order/removal/replacement seams;
account preference drafts, entity selector cancellation, retained client controls,
auth/settings requested-flow lifetime and restricted recovery; report subject/
reason/submission and private search; discussion target/authority lifetime;
announcement list/detail/manage transport, pagination and mutations; local saved
records/list navigation, import/backup, offline/current-catalogue availability;
and signed public-identity recipient/target review, metadata/conversation
pagination, canonical recovery and exact outbox controls.

Mounted URLSearchParams prefix behavior and executed native transport authority
were checked before judging raw wrappers. Historical fragments, already repaired
raw-original defects, speculative feature requests, style/grouping changes and
races without an established composed scenario were not filed. No third
substantiated in-scope defect was established in this finite pass.

The final source requires a fresh independent closure review because it changed
while this report was prepared. Final executable bundle and actual browser/API/
database/system journeys, offline boot, durable persistence, revocation and
production checks remain separate runtime gates. Source compilation, preserved
bytes and finite source closure cannot establish production readiness.
