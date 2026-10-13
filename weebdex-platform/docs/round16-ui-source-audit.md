# Round-sixteen independent UI source-completeness audit

Reviewed 2026-10-13, beginning 08:58 UTC. This fresh reviewer did not implement
the preceding UI15 repairs. Shared → reader → local-library transforms were
independently applied to an inspection copy, enforcing original input hashes
and exact replacement-anchor counts. The review followed mounted callers,
resolver order, imported integration modules and relevant native contracts.

## Finite verdict

**Source signoff withheld: three new, bounded P2 findings were established.**
They affect the existing public search paginator, an acknowledged-empty Covers
tab, and home feeds after saving changed display preferences. All five assigned
UI15 repairs are source-closed on the frozen candidate. No fourth substantive
in-scope issue was established in the finite source review described below.

No browser journey, application runtime, database operation, Cargo command,
Vite build, compiler or test suite was executed by this reviewer. Composition
and hashing were inspection operations. Runtime acceptance below is **unrun**.
No implementation source was edited; this document is the deliverable. Each
finding was reported to the parent as it was established. The implementation
owner began repairs during this review; this document evaluates the frozen
candidate, not those subsequent changes. A fresh independent review of the
changed final source is required.

## Independently frozen source

The independently composed inspection copy matches the owner source503 freeze
at `2026-10-13T08:57:53.374824+00:00`, recorded in
`test-evidence/frontend/round15-owner-source-repairs/summary.json` and its
`source-manifest.json`.

- 503 repair definitions, 512 exact occurrences, 100 original paths
- 541 copied/transformed `.svelte`/`.ts`/`.js` source files
- Independently calculated integration inventory SHA:
  `7da41e12d128f19ba20c0baeeae0332239669c9f11518b54a315ba873fd740f9`
- Shared overlay SHA:
  `33603abdad89027de86946577cac3956213dff8ce5ca93478c6944f232650bfa`
- Reader overlay SHA:
  `70ec02db52c0867872f1545ff20667a9b7e0f7e747261de8b5a57f62285c9505`
- Local overlay SHA:
  `eb9924d05454a24ce86d03b54da4c8615b307abf49a0fc6389fce69b89e2ee89`

All 676 original attachment paths were independently rehashed against
`test-evidence/frontend/source-parity.json`: none was missing or different.
The matching owner receipt reports passed source compilation of 88 components
and 54 scripts, with no runtime tests, at `2026-10-13T08:57:53.318Z`. It records
Build16 completion with artifact-manifest SHA
`0d211721e9bbb9b758e7994ee0ec3236e35d34e39f81c040f8b2f650d375c840`.
These are owner evidence, not compiler, bundler or runtime runs by this
reviewer. That artifact cannot certify changes made after the freeze.

Logical `src/` paths and line numbers below refer to the actual composed
inspection copy, not preserved raw attachments. External paths identify
executed imports. The local API resolver, mounted native HTTP authority fences,
anonymous-recovery derivative, reconstructed announcements and retained
`URLSearchParams.toString()` question-mark-prefix initialization were inspected.
Historical fragments and unmounted review copies were not substituted for
executed bodies.

## Findings

### UI16-01 — P2: Public search pagination omits its final permitted page

- Actual composed `src/lib/components/MangaSearchPagination.svelte:9` computes
  `pages = ceil(min(feed.total, 10000) / feed.limit)` for public discovery.
  `makePath` at `:11-16` and the existing jump prompt at `:34-43` admit that
  terminal page. However, the mounted pagination root at `:20-23` receives
  `count = min(feed.total, 10000 - feed.limit)`, subtracting an entire page
  before computing the visible links. The public Search route mounts this
  paginator at `src/routes/(app)/search/+page.svelte:59-62` after an actual
  acknowledged feed at `:24-34`.
- This is the behavior of the installed dependency, not an inferred widget
  contract: `.build/frontend-restore3/node_modules/bits-ui/dist/bits/pagination/`
  `pagination.svelte.js:19-22` computes `ceil(count / perPage)` and `:223-225`
  seeds its displayed page set with `1` and that computed terminal page.
- The actual native contract allows the omitted page.
  `backend/src/domains/catalog/query.rs:129-130` permits `limit` through 100
  and `page` through 1,000,000. `backend/src/domains/catalog/read.rs:141-147`
  uses the requested offset `(page - 1) * limit` and reports the real matched
  total. No native 10,000 result-offset fence is present in that feed path.
  The retained frontend 10,000-row cap need not be removed to repair this.
- Concrete scenario: make 10,000 published, currently visible native titles
  match a public search. Select the existing 98-items-per-page option
  (`src/routes/(app)/settings/preferences/+page.svelte:251-261`). The path
  clamp permits page 103, but the widget sees count 9,902 and renders terminal
  page 102. Page 102 ends at item 9,996; the final four results on native page
  103 have no ordinary terminal-page link. Manually entering 103 in the jump
  prompt or deep-linking it can fetch that real page, demonstrating the
  inconsistent visible navigation rather than an unavailable result.
- Impact: ordinary advertised pagination cannot reach all results within its
  own retained cap. The current page, terminal link and jump clamp disagree.
- Required closure: give the widget and path/jump clamp the same retained
  capped total. Preserve the uncapped library-follow branch and current visual
  design; unbounded catalog pagination is not requested.
- Acceptance, unrun: use real totals below, at and above 10,000 with each
  existing page size 28/42/56/70/84/98. Verify the final partial page is linked,
  current/last/jump navigation agrees, and Back/Forward restores it. Verify
  account/device library pagination remains uncapped and correct.

### UI16-02 — P2: An acknowledged-empty Covers tab remains a spinner

- Actual composed `src/routes/(app)/title/MangaCoversSection.svelte:39-42`
  stores the acknowledged 200 cover list, including an empty array. `:54`
  marks the read finished. The render at `:109-141` requires nonempty covers;
  every remaining nonerror result falls through to the Spinner at `:142-145`.
  Pending and completed-empty states are therefore indistinguishable.
- Initial visibility is accounted for. Actual composed
  `src/routes/(app)/title/Manga.svelte:89-94` hides Covers when the loaded
  detail DTO has no cover. The component is dynamically mounted for the
  selected visible tab at `:514` and `:539`. An initially coverless title is
  **not** the claimed reproduction.
- Concrete native scenario: open a published mutable title that has one
  visible cover, and remain on its Chapters tab. A staff account with the
  existing `cover.delete` permission removes that last cover in another
  authorized editor/tab. Select Covers in the original, still-loaded title.
  The old detail DTO still exposes the tab; its fresh Cover.List request
  genuinely succeeds with `200 []`, and the completed tab spins indefinitely.
  The title loader reads `draft` and route parameters, not the selected tab,
  so selecting that tab does not refresh the loaded cover relationship.
- Native reachability: `backend/src/domains/catalog/repository.rs:476-505`
  admits authorized cover deletion, locks/requires the mutable parent, and
  marks that cover deleted without requiring another cover to remain.
  `backend/src/domains/catalog/read.rs:602-608` verifies the still-visible
  public parent and returns an acknowledged JSON empty array when no current,
  nondeleted visible covers remain. A missing parent/error is not used as
  evidence of empty success.
- Impact: a legitimate completed read of an existing visible tab never settles
  into an honest terminal state. Reloading merely hides the now-coverless tab;
  it does not make the mounted read complete correctly.
- Required closure: distinguish pending from acknowledged-empty and show a
  truthful empty/unavailable result after the real empty list. Preserve the
  initial tab-visibility guard, nonempty locale selector and lightbox behavior.
- Acceptance, unrun: open the retained tab after removing its last cover and
  after a cover source becomes unavailable while the parent remains visible.
  Actual 200 empty must settle; failures must remain failures, not inferred
  emptiness. Nonempty locale/lightbox controls must still work, and navigating
  away or changing title during a read must not attach an old result.

### UI16-03 — P2: Home caches ignore newly saved display preferences

- Actual composed `src/lib/components/MostReadCarousel.svelte:1-3` and
  `RecentlyAddedCarousel.svelte:1-3` keep feed state at module lifetime.
  Most Read only fetches when that feed is absent (`:18-19`), and Recently
  Added only fetches when `feed.data` is absent (`:18-19`). Both display those
  cached cards without applying the current preferences
  (`MostReadCarousel.svelte:52-55`, `RecentlyAddedCarousel.svelte:51-55`).
  Their actual home mounts at `src/routes/(app)/+page.svelte:28,44` have no
  effective-query/preference key. Route unmount/remount does not erase module
  state.
- These preferences are existing, consequential display controls. The real
  signed-in Preferences page exposes Safe, Suggestive, Erotica and Pornography
  checkboxes at `src/routes/(app)/settings/preferences/+page.svelte:265-288`.
  Its successful native update at `:75-76` calls the executed
  `platform-integrations/frontend-behavior/account-preferences.ts:13-22`, which
  synchronously installs the acknowledged preferences into `prefs` and Session.
- Concrete ordinary scenario: with Erotica or Pornography enabled, load Home
  and receive a genuine matching explicit title in either home feed. Navigate
  to Preferences, uncheck that rating, save a real successful native response,
  then navigate Home in the same SPA. Both mounted components reuse the old
  feed and can still show the newly excluded card. No malformed response,
  polling feature or unusual component reuse is necessary. Conversely, an
  acknowledged-empty module cache can keep its section hidden after widening
  preferences. Recently Added also caches across changes to its actual
  language/tag query preferences (`:21-25`).
- Most Read additionally always sends Safe at `MostReadCarousel.svelte:25`,
  even when the existing Safe checkbox is off. With all four ratings disabled,
  it still requests Safe results. The actual shared convention at
  `src/lib/stores/prefs.ts:31-37` sends the sole `__none__` rating to represent
  that existing empty selection; the mounted Most Read query bypasses it.
  This is included in the same display-preference finding rather than counted
  as a separate defect.
- Native results honor the supplied filters:
  `backend/src/domains/catalog/read.rs:131-144` applies public defaults,
  `manga_filters` and actual ranking/selection, while
  `backend/src/domains/catalog/query.rs:238-245` includes `contentRating` in
  the real SQL filter. The native feed cannot repair a client that never asks
  again or continues rendering a previously admitted response.
- Impact: saving an advertised explicit-material preference does not reliably
  govern existing mounted home content, including excluded adult cards.
- Required closure: own cached/read results by the actual effective preference
  query, or issue a correctly owned current read when that context changes.
  An older in-flight result must not restore newly excluded cards. Respect
  all four existing rating choices, including Safe-off and all-off, while
  retaining the native ranking and existing visual layout.
- Acceptance, unrun: enable then exclude Erotica/Pornography and revisit Home
  within the same SPA; exclude Safe and then all four ratings; re-enable after
  an acknowledged empty result; change Recently Added language/tag preferences;
  exercise Back/Forward, startup account-preference hydration and a pending
  older request while preferences change. Display only current admitted
  results and distinguish current failure from acknowledged empty success.

## UI15 assigned closures independently verified

1. **Applied title chapter filter baseline:** actual composed
   `src/lib/components/ChapterListFilter.svelte:26-44` derives the baseline
   from the actual title/path/query and synchronizes it on real navigation.
   Editing an unchanged URL does not overwrite the unsaved draft. Apply now
   compares against the current applied selection rather than initial mount.
2. **Automatic account reader history:** actual composed
   `src/lib/components/reader/reader-history.svelte:29-44` captures a current
   account/chapter action, asserts it around the real write, requires the
   acknowledged 200 body and reports a still-current failure. The retained
   account token prevents an automatic replay. Device progress remains a
   separate admitted durable write with final admitted-page flush at `:15-24`
   and debounce at `:46-55`.
3. **Double-spread page selector:** actual composed
   `src/lib/components/reader/reader-page-selector.svelte:11-18,49` validates
   the existing value and maps a displayed spread ordinal to that spread's
   first actual page index. Single-page selection retains its real page index.
4. **Tap navigation after panel changes:** actual composed
   `src/lib/components/reader/context.svelte.ts:222-237` uses the actual current
   reading surface rectangle, including its left offset and width. It ignores
   prevented/interactive/nonprimary clicks and retains direction and scroll
   tap-navigation preferences.
5. **Acknowledged-empty Recommendations:** actual composed
   `src/routes/(app)/title/MangaRecommendationsSection.svelte:18-54` owns and
   serializes/coalesces current-context reads, validates the real response,
   avoids restoring historical DTOs as current availability and reaches a
   terminal state. `:57-81` separates failure with existing Retry, results,
   acknowledged empty and pending. The empty branch is no longer a Spinner.

These are finite source closures, with runtime acceptance still unrun. They do
not erase the three new findings on the same frozen candidate.

## Finite coverage and remaining gates

The source review followed existing mounted workflows across:

- Discovery search and filter/reset/keying, Group/User search, title chapter
  lists, feed continuation and paginator limits, related/recommendation/cover
  tabs, home feed callers and their real query/filter contracts
- Reader language/group/chapter/page selectors, single/double/long-strip
  mapping, keyboard and interactive focus, surface/scroll geometry, image
  queue/retry/cancellation/blob/decode lifetime, settings and automatic history
- New/edit upload target ownership, staged source/pages admission, bounded
  archive/image processing, restore/autosave, navigation/cancel/commit and
  actual acknowledged mutation paths
- Local library mode/scope/readiness, durable action ownership, progress,
  bookmark/import/export/reconciliation, CSV/backup handling and mounted
  account/device controls through their executed API resolver and store
- Account preferences and API-client controls, normal-account protection,
  login/register/verification form ownership, restricted recovery derivative,
  Session initialization and the executed native HTTP/mutation fences
- Mounted public-identity/signer/recipient/source controls and imported
  publication/projection/outbox/recovery closures; verified mappings and
  account/device separation rather than identity appearance alone
- Report subject/reason/submission ownership, moderation list/action
  acknowledgement, comments context and current results, actual announcement
  feed/detail/manage/pin refresh and owner-scoped response handling
- Service-worker authority-route NetworkOnly ordering and narrowly scoped
  cache cleanup versus retained immutable asset caching

The inspection does not convert inherited warnings into findings without a
mounted failing scenario. Initially hidden tabs, unmounted statistics/widgets,
explicitly unavailable routes, raw preserved pre-overlay behavior and
speculative product additions were excluded. In particular, the Covers finding
depends on a retained *already visible* tab, and the paginator finding retains
the existing UI result-count cap.

No further substantive issue was established in those inspected source
families and imported closures. This is neither exhaustive production proof
nor runtime certification. The three source findings require repair and a new
independent composed-source review. Then the final rebuilt artifact and actual
end-to-end browser/backend journeys must verify the acceptance scenarios,
startup ordering, navigation lifetimes, real permission outcomes and offline
behavior against the exact final source. Cold-offline application boot, visual
layout, browser persistence and real network/storage behavior were not executed
or certified by this audit.
