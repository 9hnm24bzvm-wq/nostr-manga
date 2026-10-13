# Round-seventeen independent UI source-completeness audit

Reviewed 2026-10-13, beginning 09:14 UTC. This fresh reviewer did not implement
the preceding UI16 repairs. Shared → reader → local-library transforms were
independently applied to an inspection copy, enforcing original input hashes
and every exact replacement-anchor count. The review followed mounted
callers, actual resolver order, imported integration modules and the native
contracts that determine the visible result.

## Finite verdict

**Source signoff withheld: two new, bounded P2 findings were established.**
Both affect the existing mounted title-cover editor: preserving an acknowledged
cover result when a later cover operation fails, and actually reading private
draft covers before opening that editor. All three assigned UI16 repairs are
source-closed on the frozen candidate. No third substantive in-scope finding
was established in the finite source review described below.

No browser journey, application runtime, database operation, Cargo command,
Vite build, compiler or test suite was executed by this reviewer. Composition,
copying and hashing were inspection operations. All runtime acceptance below
is **unrun**. No implementation source was edited; this document is the
deliverable. Each finding was reported to the parent as it was established.
The implementation owner may repair source during the review; this document
evaluates the independently frozen candidate, not subsequent changes. A fresh
independent review of changed final source remains necessary.

## Independently frozen source

The inspection copy was materialized at `2026-10-13T09:14:26Z` and matches the
owner source516 freeze at `2026-10-13T09:12:31.003474+00:00`, recorded in
`test-evidence/frontend/round16-owner-source-repair/summary.json` and its
source manifest.

- 516 repair definitions, 525 exact occurrences, 104 original paths
- All 104 transformed-source hashes independently matched the owner manifest
- 541 copied/transformed `.svelte`/`.ts`/`.js` source files
- Independently calculated composed-source inventory SHA:
  `08807db37f25dd398631fb1a48bcabfa2481b1a6e5855947a51e77d8634adb1b`
- 62 external integration inventory files, independently calculated SHA:
  `4cc4d72c2e998cb1a89762645bf114466447d3b5491ae796e1ef9d352f8c2e0b`
- Shared overlay SHA:
  `33603abdad89027de86946577cac3956213dff8ce5ca93478c6944f232650bfa`
- Reader overlay SHA:
  `70ec02db52c0867872f1545ff20667a9b7e0f7e747261de8b5a57f62285c9505`
- Local overlay SHA:
  `d394e81fdd7f6e02745ea905ec69f52d278bdac942ef9b2794110cc4ca6fc473`

All 676 original attachment paths were independently rehashed against
`test-evidence/frontend/source-parity.json`: none was missing or different.
The matching owner receipt reports passed source compilation of 92 components
and 55 scripts. Those are owner evidence, not compiler, bundler or runtime
runs by this reviewer. A prepared or completed Build17 artifact does not
independently certify changes made after this freeze.

Logical `src/` paths and line numbers below refer to actual composed source,
not preserved raw attachments. External paths identify executed imports.
`scripts/vite.platform.config.mjs`, its shared → reader → local plugin order,
the `$platform` / `$localLibrary` aliases and local API/store substitutions
were inspected. Historical fragments and unmounted review copies were not
substituted for executed bodies.

## Findings

### UI17-01 — P2: A later cover failure discards earlier acknowledged identities and versions

**Executed path.** The actual title edit route imports
`$platform/MangaEditBoundary.svelte` at
`src/routes/(app)/title/[id]/edit/+page.svelte:4` and mounts it under the
current title key at `:70-72`. The external boundary's full mutable-DTO guard
at `platform-integrations/frontend-behavior/MangaEditBoundary.svelte:25`
mounts the retained `MangaEdit.svelte` at `:63-66`. The defect is in that
executed retained editor, not an unused original copy.

- `src/lib/components/draft/manga/MangaEdit.svelte:269-275` starts one local
  `tmp` list for the entire cover save sequence.
- After a genuine successful upload, `:294-299` clears the bound row's file
  at `:295`, but merges the server-returned real ID/version only into `tmp`.
  The still-bound `coverDraft` retains the synthetic ID assigned by
  `CoverGroupButton.svelte:30-40` and has no acknowledged upload identity.
- A rejection of any later row throws at `MangaEdit.svelte:300-306`. The
  actual baseline assignment `covers = tmp` at `:362` is never reached.
  The submit catch/finally at `:151-187` leaves the draft available for a
  subsequent explicit Save without merging those earlier acknowledgements.
- Removing the now-fileless acknowledged row uses the existing persisted-row
  branch of `CoverGroupInput.svelte:76-95`, setting `deleted=true`. Its next
  Save sends `API.Cover.Delete(coverDraft.id)` at `MangaEdit.svelte:307-320`
  with the synthetic ID, rather than the actual stored cover's ID.
- Alternatively, remove only the later rejected row and Save again. The first
  acknowledged row no longer has a file, and its synthetic ID has no match
  in `initial.covers`. The `:322-327` branch skips it. The sequence then
  reaches `:362`, and the normal `onReset()` at `:149` / `:191-195` resets
  to a baseline that excludes the cover the backend actually accepted.
- Existing-cover updates have the same acknowledged-version durability gap:
  the returned DTO is only placed in `tmp` at `:350-354`. If a later row
  fails, the draft/baseline keeps the old version. A subsequent explicit
  Save can submit a stale version despite this editor having received the
  newer one.

**Concrete native scenario, source-derived and unrun.** Open a currently
mutable title's Covers editor with a valid ordinary title draft. Select
`01.png`, a valid supported static image, and `02.png`, an image-MIME file
with corrupt PNG data. The file picker admits both by MIME at
`CoverGroupButton.svelte:16-20,45-53,71`; natural filename order is explicit at
`:23-28`. Cover row validation at `CoverGroupInput.svelte:24-25` validates
volume syntax, so the second row is not blocked by file decoding before Save.
Click Save. The first POST is committed and acknowledged with its real DTO;
the second native request returns a legitimate bad request. Remove the first
row and Save to observe the wrong delete target, or remove only the rejected
row and Save to observe the accepted first cover disappear from the editor
while remaining on the backend. This does not require a fabricated status,
transport race, new cover format or unsupported feature.

The backend makes both outcomes real:

- `backend/src/domains/uploads/sessions.rs:1194-1210` checks native edit
  authority, `:1232-1233` validates actual image data, `:1295-1308` assigns a
  real ID and publishes the cover, and `:1327-1332` commits before returning
  the acknowledged DTO.
- `backend/src/domains/uploads/storage.rs:459-461,514-516` returns bad request
  for invalid headers or corrupt decoded data. This is ordinary supported
  failure handling, not a request to add GIF support.
- `backend/src/domains/catalog/repository.rs:379-418` persists and returns the
  real cover DTO. For updates, `:430,442-444` checks the submitted version,
  `:460` increments it, and `:471-474` returns that new version.

**Impact.** The editor loses a truthful successful acknowledgement whenever
a later operation fails. Subsequent ordinary retry/removal/editing can target
an invalid local identity, replay a stale version, or silently erase the
accepted cover from the displayed baseline. The backend has already committed
the first operation; the error toast alone does not repair the user's next
available action.

**Required bounded closure.** Reconcile each genuine cover acknowledgement
into its bound row and authoritative local baseline immediately, retaining
the real identity/version and truthful saved state across later failures.
Keep unsent/failed edits recoverable, and reconcile acknowledged removals and
main-cover state consistently. A later error must not roll back a known
acknowledgement. Do not automatically replay writes, treat ambiguous writes
as success, or lose the existing target/account fences. No new format, feature,
redesign or raw-attachment edit is required.

**Acceptance, unrun.** With real multi-cover saves, make the first upload or
update return a truthful 200 and a later operation fail with native
400/403/409/429 or a transport-unknown outcome. Verify the earlier accepted
row immediately has the exact returned ID/version and remains visible; the
failed/unsent rows remain editable. Remove the failed row and explicitly Save,
then edit/delete the accepted row and verify exact actual IDs/versions, no
duplicate upload and no spurious stale-version conflict. Include an
acknowledged deletion followed by a later failure, Reset and authoritative
reload, main-cover changes, route navigation and account/session change.
Ambiguous operations must remain unresolved until read-only reconciliation
establishes their real outcome.

### UI17-02 — P2: Draft-title editing reads public covers and opens an empty baseline on actual 404

**Executed path and contract.** The actual edit loader at
`src/routes/(app)/title/[id]/edit/+page.ts:6-25` supports a private draft:
`:11-13` selects `API.Manga.GetDraft` when `draft=true` and checks current
session/update permission. However, the mounted page's owned cover read at
`+page.svelte:23-31` calls `API.Cover.List(mangaId)` at `:27` without that
private-draft scope. Actual composed `src/lib/API/Cover.ts:4-6` always sends
`GET /manga/{id}/covers`, does not add `draft=true`, and does not select the
credentialed browser-session path.

- Native `backend/src/domains/catalog/read.rs:579-608` explicitly supports
  private draft covers. With `draft=true`, `:587-600` requires a normal
  browser principal and the actual draft owner or authorized moderator;
  recovery sessions/API clients cannot obtain that access.
- Without `draft=true`, `:602-605` uses the public
  `CatalogRepository::get_manga`. Its implementation at
  `backend/src/domains/catalog/repository.rs:55-58` requires
  `state='published'`. An ordinary genuine draft therefore returns 404 even
  though its owner just successfully read the private draft and its covers
  exist.
- The page only copies the cover body for 200 at `+page.svelte:29`, but always
  sets `isMounted=true` at `:30` for an acknowledged non-200 result. Its
  cleared `covers=[]` from `:25` is then passed into the full editor at
  `:70-72`. This is not an acknowledged-empty list. Actual `HTTP.ts:91-108`
  resolves HTTP non-200 responses normally, so the catch at `:31` does not
  turn this real 404 into a visible error.
- The same scope/empty-baseline defect exists in the executed external
  publication boundary refresh. At
  `platform-integrations/frontend-behavior/MangaEditBoundary.svelte:43-50`,
  a successful authorized `GetDraft` fallback with full relationships still
  triggers unqualified `API.Cover.List(target.id)`. Non-200 leaves
  `nextCovers=[]`, and `:52` installs it as the refreshed baseline.

**Concrete native scenario, source-derived and unrun.** Use the existing
Create Title flow with a valid supported cover. The title creation is
acknowledged at `src/lib/components/draft/manga/MangaCreate.svelte:169-176`
and the cover POST at `:184-208`. On normal successful completion, click its
existing `Edit draft` link at `:315-325`; the link adds `draft=true` for an
unpublished title. The loader reads that owned draft successfully, but the
cover request hits the published-only branch and returns actual 404. The
page mounts the full editor with no covers. The already-saved cover cannot
be reviewed, edited or removed through the existing Covers controls. The
same failure applies when an existing legitimate private draft is opened
directly from My Drafts; no publication or imported protocol record is needed.

**Impact.** A common happy-path Create → Edit workflow hides saved private
covers and supplies an inaccurate empty editing baseline. The UI reports
neither the private-read failure nor the distinction between a genuine empty
cover list and a failed request. This finding is separate from UI16's public
Covers tab acknowledged-empty spinner, which is closed in this candidate.

**Required bounded closure.** Read covers with the actual current private-draft
scope and normal browser authority when opening or refreshing a private full
draft editor. Bind that read to the target and current actor, preserve
owner/moderator/recovery restrictions, and accept only a real acknowledged
200 array as an editable cover baseline. A failed/unknown read must settle
truthfully with existing error/retry/reload behavior rather than instantiate
a full editor with an invented empty list. Preserve genuine 200 `[]` and
the existing public published-title Covers behavior. No backend feature or
blanket change to public cover-list access is needed.

**Acceptance, unrun.** Create a draft with one or several supported covers,
click the actual Edit draft link, and verify exact current cover IDs/versions
are visible and editable. Repeat for direct existing-draft edit and the
publication-boundary private refresh. Exercise owner and authorized moderator
access, genuine no-cover 200 `[]`, expired-session 401, forbidden 403, missing
private target 404, rate limit, malformed response and network failure.
Failures must not produce an empty editable baseline. Verify published public
cover lists remain public, recovery capabilities do not gain draft access,
and target/navigation/account changes cannot install the previous read.

## Assigned UI16 repair closure

1. **Public search terminal pagination — source-closed.** The actual
   `MangaSearchPagination.svelte` uses the same retained capped total for
   `pages`, the pagination root and ordinary path/jump behavior. The library
   branch remains uncapped. Existing composed URLSearchParams serialization
   supplies the retained leading question mark; a raw-original missing-`?`
   report would be false.
2. **Acknowledged-empty public Covers tab — source-closed.** Actual
   `MangaCoversSection.svelte` owns/coalesces reads, clears before target
   changes, accepts the actual acknowledged array, prunes stale selections,
   and has terminal failure/Retry and genuine-empty branches. That public
   tab no longer spins forever for a truthful empty result.
3. **Home carousel preference/module-cache behavior — source-closed.** Both
   actually mounted `MostReadCarousel.svelte` and
   `RecentlyAddedCarousel.svelte` construct queries from current display
   preferences before their existing ranking/sort options. Their pre-render
   effect synchronizes route/preferences with the shared
   `public-manga-widget-read.ts` controller. The controller signature includes
   actual API origin, current route and effective query, clears before new
   rendering, generations/disposes/coalesces reads, and settles genuine ACK
   and failure/Retry. No module DTO cache or unrelated library-session coupling
   remains; the all-content-ratings-disabled `__none__` sentinel is retained.

These are executed-source closures. They do not represent browser acceptance
or perfect production behavior.

## Broader finite review coverage

The following currently mounted paths were followed through their composed
source and executed external imports. This is a finite, evidence-based
coverage statement, not a claim that every state can be certified without
runtime acceptance.

- **Public discovery and title/user/group navigation:** home feed imports,
  preferences and all-rating-disabled query construction, search filtering,
  reset/order/pagination/history ownership, complete tag-catalog loading,
  entity tabs, chapter-filter applied baselines and bounded chapter-feed
  continuation/resume/exhaustion. The prior title/filter A → B → A baseline
  repair remains present.
- **Reader:** aggregate/provider ownership, spread offsets and page selectors,
  current wrapper-geometry tap regions, preload admission for both spread
  pages, bounded XHR/decode failure and explicit Retry, keyboard/modal control
  ownership and disposal, panel/comment/quality controls, actual acknowledged
  account history, device page/progress/final flush and target transitions.
- **Upload and chapter edit:** image/archive/folder admission, staging session
  resume/autosave, cancel/commit, page ordering/metadata/reset, per-page unknown
  recovery, current target/session/expected actor, no blind mutation replay,
  and the actual native immutable-publication edit contract. A fictional new
  minimal receipt returned from a native immutable edit was not used as a
  finding.
- **Local-first and offline library:** local/device/current-account scope,
  custom lists and saved-record editing, backup preview/import/export,
  explicit account import, canonical inspection ownership, publication/
  recovery/adoption with exact signer/recipient/source context and the public
  action outbox. Device reads were not treated as native account history.
- **Authentication and account:** normal login/register/logout/session refresh,
  requested verification flow/current navigation, recovery-only boundary,
  preference dirty-draft preservation and acknowledged baseline/version,
  disabled-email truthfulness, API-client name/rotation/deletion/copy and
  user/group current-account follow/edit paths.
- **Comments, moderation and announcements:** current target/actor fences for
  comment read/vote/create/edit/delete and counters, report subject/reasons/
  close/reopen lifetime, truthful owner report create/status/delete ACK and
  noncached private feeds, reconstructed mounted announcement list/detail/
  manager transport, versions, page switching and deletion.
- **Private title editing and publication:** mounted full-data boundary,
  current target publication receipts, truthful pending/accepted/prepared
  states, explicit original-command retry and read-only reconciliation. The
  actual cover reading and partial-save issues above were the two established
  gaps within those existing controls.

No separate finding was filed for speculative feature additions, visual taste,
unmounted popularity widgets, historical fragments, backend-intentionally
unsupported cover formats, imagined response shapes inconsistent with the
native contract, or generic lifecycle concerns without a concrete mounted
flow and actual adverse outcome. This round's stopping condition is the end
of this bounded source pass with the two reproducible in-scope gaps reported;
it is not a global production completeness signoff.
