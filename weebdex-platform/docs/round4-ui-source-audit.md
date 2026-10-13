# Round-four independent UI source-completeness audit

Reviewed 2026-10-13, beginning 05:08 UTC. This reviewer did not own the preceding
implementation patches. The actual shared → reader → local-library composition
was independently applied for inspection, with original SHA/count anchors
enforced. External modules, resolver configuration and retained callers were
read directly. Prior source-closure statements were not accepted as proof.

## Finite verdict

**Source signoff withheld: three substantive remaining UI findings were
established in the reviewed snapshot.** The eight round-three repairs have
source closure at the inspected callers, but that closure does not establish
browser/runtime acceptance. The three new findings below require owner fixes
and the requested fresh independent squad after that patchset.

This was source-only work. No browser/app journey, test suite, Cargo command,
Vite build or application runtime was executed. Source transformation into an
inspection directory was not an application test. No implementation files were
edited by this reviewer; this report is the only repository deliverable write.

The reviewed composition contains **125 shared + 26 reader + 76 local-library
repairs across 62 original paths**. The current integration manifest inventory
is `93b8599bb3a47a7289584a6c96c65e507452e0b27fa001ee1358bf2598f72f2f`.
The digest of sorted composed path/SHA records is
`a88d166414f5d803c2fc8fc9b75c9f8c369b20bf6f68092c6cfc18f377a13690`.

Reviewed source pins:

- Shared overlay: `5f57445f1d82efb026e00b46b73b2f2bff75ce06308925a39f38018edcf95f26`
- Reader overlay: `0fa703b3208a1d0abe210062cf4ec02bb813132a83d687cf331d212aae983dce`
- Local overlay: `8a0f02a82d3748b6763f65df4ef0952d12a44a753b7c74a8e3ac85b95f8fb897`
- Title boundary: `dae40dba2d4d755cb6880197fc52b2886fa909ccad478c558bf760235ea3bb1e`
- MangaFollow adapter: `528e4a4e66753d702e83888b65dda37bbbda965004e16731b57242324a33ccdb`

These pins identify the reviewed snapshot. They do not certify a later owner
patch, an older built artifact, runtime behavior, or production readiness.
Composed line references below use the executed logical frontend path. Where a
path has no overlay, its original frontend lines are explicitly identified.

## Findings

### UI4-01 — P1: A completed device follow change can overwrite the account view

- Actual composed `src/routes/(app)/title/MangaActions.svelte:63-77,103-116,141-150`
  and `src/lib/components/MangaSearchEntryCard.svelte:48-57,69-74` await a follow,
  notification or unfollow mutation, then apply its result to the **current**
  selected source and `$Session.follows`. They do not capture/validate the
  initiating library context. A pathname check in the title caller does not
  detect a library-mode/account change on the same page.
- External `platform-integrations/local-library/MangaFollow.ts:47-53` lacks the
  context capture/assertion already used by the repaired ChapterHistory adapter.
  `library.ts:121-131` validates catalogue scope inside the device transaction,
  but does not check the selected source. Device saving includes the asynchronous
  refresh before returning (`library.ts:111-113`). The global source panel has an
  independent pending state, so the source can change while that work is pending.
- Scenario: start a device follow/status/notification/unfollow change, delay its
  durable save or refresh, and choose account mode in the global panel before
  completion. The callback accepts the device response, changes account-visible
  state and inserts/deletes the corresponding `$Session.follows` entry, although
  no account mutation occurred. A later account notification/status action can
  then use this false local account state. The inverse transition can similarly
  attach an account completion to the device-visible status.
- Closure: freeze the mutation's mode/account/scope generation at admission,
  validate it before transport and inside the device transaction, and reject
  attachment to superseded callers. Do not patch a different source's Session
  map after completion. Already acknowledged work must remain in its original
  destination and be reconciled truthfully.
- Acceptance: delay device/account follow, status, notification and unfollow;
  switch source/account on the same title/search page; finish the old operation.
  Current visible status and Session maps must match the current durable source,
  with no cross-source map insertion/deletion or unintended later mutation.
- Runtime acceptance: **unrun**.

### UI4-02 — P2: Start Read keeps the first language/group aggregate after filters change

- Actual composed `src/routes/(app)/title/MangaActions.svelte:172-196` fetches an
  aggregate only when `aggregated` is absent. The aggregate is never invalidated
  by the title's selected `tlang`/`group` query or changed chapter preferences.
  The callback fences only pathname, despite the query being captured from
  `page.url.searchParams` and preferences at lines 175-184.
- Original, untransformed `frontend/src/lib/components/ChapterListFilter.svelte:33-49`
  applies language/group selection through same-path query navigation. The title
  keeps its MangaActions instance while its chapter-list search subtree is keyed
  by the changed query. The variant-selection dialog is at composed
  `MangaActions.svelte:339-369`.
- Scenario: on a title with multiple variants, use Start Read and close its
  Select Chapter dialog. Change language/group and apply the chapter filter.
  Start Read again reuses the original aggregate and offers or opens a chapter
  excluded by the newly selected language/group. A delayed first aggregate can
  also attach after the same-path query changed because pathname still matches.
- Closure: bind aggregate and target/dialog state to title plus the effective
  chapter-language/group preference/query. Fence late/disposed reads and clear
  superseded selection state. The existing reader-provider aggregate repair
  does not cover this separate title caller.
- Acceptance: Start Read → Cancel → change language/group → Apply → Start Read,
  including Back/Forward, preference changes and delayed first responses,
  offers/opens only the current filtered variants. A superseded dialog/result
  cannot reopen or navigate to an older selection.
- Runtime acceptance: **unrun**.

### UI4-03 — P1: Reused chapter-edit/upload routes retain another target's draft or staging

- Original, untransformed
  `frontend/src/routes/(app)/chapter/[id]/edit/+page.svelte:13-16,50-51` passes
  reactive `chapter` into an unkeyed UploadChapterEdit. Its layout is only a
  ProtectedVerified wrapper, with no target key.
- Actual composed `src/lib/components/upload/UploadChapterEdit.svelte:37-49,98-118`
  derives `initial` from the current chapter, but initializes `draft`/oneshot and
  restores staging only on mount. On a reused route, `draft` remains A while
  `initial`, version and mutation target become B. Submission at lines 135-146
  computes A-versus-B differences and sends them with B's ID/version. The
  beforeNavigate guard at lines 94-97 only covers mutable staging; it does not
  cover a normal metadata draft without an upload session.
- Scenario: open A/edit, then client-side navigate to B/edit or use Back/Forward
  between two chapter editors. A's initial or edited metadata remains in B's
  form. Save can copy A's title/chapter/volume/language/groups onto B, using B's
  genuine version. No authorization bypass is required when the user may edit
  both chapters.
- The corresponding upload route is also unkeyed: actual composed
  `src/routes/(app)/title/[id]/upload/+page.svelte:55-56` mounts UploadChapter;
  composed `UploadChapter.svelte:222-238` restores/reconciles its target only on
  mount. A fully acknowledged A staging session permits navigation because
  `hasUnsavedUpload` is false. A/upload → B/upload can then show B's title with
  A's retained session/files/draft; commit addresses the retained session at
  lines 114-116. Its normal original-target redirect is not rerun.
- Closure: give chapter-edit and title-upload state a lifetime tied to actual
  target identity, with captured target/generation/disposal guards on restore,
  reconcile, claim, begin, autosave and commit completions. Preserve authoritative
  same-target updates and acknowledged staging; navigating must never silently
  delete the original session or rebind its publication intent.
- Acceptance: A/edit → B/edit and A/upload → B/upload plus Back/Forward, both
  with clean metadata and acknowledged/pending staging. Every displayed field,
  file/session, version, autosave and explicit mutation must belong to the route's
  actual target. Delayed A completions must not populate B or navigate B's editor.
- Runtime acceptance: **unrun**.

## Independent reread of the eight round-three repairs

1. UI3-01: canonical selection clears rows/cursor synchronously and fences
   delayed/disposed family reads. Removal captures the actually loaded family/key
   and verifies the inspected row still belongs to that selection.
2. UI3-02: settings CSV/MAL file import captures one frozen import context before
   reading the file; every batch asserts it before/after, and the adapter pins
   account/scope/context through admission and the device transaction.
3. UI3-03: report and draft components no longer restore private navigation
   caches. Parent reads use fresh expected-account checks with identity/generation/
   href fences; identity changes clear feeds and trigger current reads. ChapterFeed
   never restores its cached private map and owns context/lifetime-fenced reads
   for all current entries. These are source findings, not revocation runtime proof.
4. UI3-04: both chapter read-toggle callers capture `wasRead`, target IDs and
   library context before awaiting. Confirmed shared-map updates use that captured
   requested state and reject source/target changes.
5. UI3-05: title history now has a reactive owner keyed by current library view,
   title and availability, clearing/reloading on pending→verified and source/
   account changes. Generation/disposal/identity checks reject superseded results.
6. UI3-06: single/double page wrappers have owned ResizeObservers, recomputing the
   reachable threshold on real geometry changes with RAF/disposal cleanup. This
   covers the identified width-slider/sidebar transition at source level.
7. UI3-07: ReportDialog owns/cancels its close timer, keys reasons by open/type/
   subject/href generation, and prevents an older submit closing a new subject.
8. UI3-08: title edit cover reads are target/generation/disposal-fenced; its
   retained editor is keyed by title ID. Boundary receipt/cover/retry reads have
   target lifetime ownership. The unkeyed chapter/upload paths in UI4-03 are
   distinct remaining callers, not a claim that the title fix failed.

All eight are **source-closed at this reviewed snapshot; runtime unrun**.

## Inspected scope and negative results

The review traced shared/reader/local source composition and resolver order;
device/account follows, history, transaction-bound scope, custom lists, scoped
CSV/JSON/account/canonical import and export; canonical family pagination/removal;
explicit publication/recovery/outboxes; signed identity/profile/follow/relay and
conversation review/read context; installed-worker sensitive-route priority and
storage preservation; retained reader aggregate/page/spread/width/progress;
upload restore, metadata/page controls, mutation/immutable/cancel gates; report
filter/dialog/feed and private draft feed; title editor publication boundary;
managed group/author/user settings mount lifetimes; reconstructed announcement
list/detail/manage controls and their versioned client.

No further concrete new defect was established in this finite pass for backup
scope matching/revision CAS, canonical restore validation, signed target/recipient
review invalidation, the repaired report/dialog/cache flows, reader geometry
ownership, immutable upload child-control disabling, keyed group/author/user
settings layouts, or reconstructed announcements. This does not assert exhaustive
review of every feature anchor, offline app boot, full source revocation safety,
absence of all possible defects, or a runtime/production pass.

The implementation/source gate remains withheld for UI4-01 through UI4-03.
The separate E2E/system/browser gate remains **unrun**.
