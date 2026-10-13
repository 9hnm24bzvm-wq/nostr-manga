# Round-three independent UI source-completeness audit

Reviewed 2026-10-13, beginning 04:55 UTC. This reviewer had no implementation
ownership in the preceding rounds. The review inspected the actual composition
of shared, reader and local-library overlays, their resolver/mount points,
external components and retained callers. Historical source-closure claims were
checked against those callers rather than used as evidence.

## Finite verdict

**Source signoff withheld: eight bounded, substantive UI findings were established.**
UI3-01 and UI3-02 were repaired by the frontend owner during this review and their
new source was independently reread below. Six findings remain open at this
report's initial snapshot. Later owner patches require final composed reread and
the user's requested fresh-squad completeness pass.

This was source-only work. No application/browser journey, test suite, Cargo
command, Vite build or runtime was executed. Exact overlay application into a
temporary inspection directory was a source transformation, not an application
test. Runtime acceptance for every finding below remains **unrun**. No source
files were edited by this reviewer; this report is the only repository write.

Initial composed evidence had 103 shared + 20 reader + 59 local repairs across
57 original paths, integration inventory
`03acef125886610a4326981b5617796cec5f41288112d434279f4abbd5831d7e`.
The owner changes independently reread for UI3-01/02 had 63 local repairs and
inventory `4b2f1eb8293bbd0f392e032c40dcd2bb1e81b35c4c490d92e7ebda5501c3c387`.
These source inventories do not certify the stale built artifact or runtime.

## Findings

### UI3-01 — P2: Canonical family selection rebinds old rows and removal

- Initial source: `platform-integrations/local-library/LocalLibraryPanel.svelte:15,184,188`.
  `loadCanonical` awaits a family read and then attaches the result without a
  captured-family/generation fence. The Family selector is editable while
  `pending`; `run` silently declines the new read, but the binding still changes.
  Removal uses current `canonicalFamily`, not the inspected row's family/key.
- Scenario: begin a slow Series read, switch to Chapters before completion.
  Series rows attach beneath Chapters. Remove then addresses the wrong family;
  a coincident key can delete an unintended record, or the advertised removal
  simply leaves the inspected record in place.
- Owner repair independently reread: current panel lines 15-36,71,206-212 clear
  rows/cursor synchronously on selection, fence delayed/disposed reads and
  capture/validate the loaded family and row key before removal. The stale-row
  rebinding defect is source-closed; runtime is unrun.
- Acceptance: delayed reads, family changes while another operation is pending,
  and rapid repeated selection cannot display or remove a row under a different
  family. A confirmed removal addresses exactly the row/family inspected.

### UI3-02 — P1: A multi-batch import can change its destination mid-file

- Initial composed `src/routes/(app)/settings/library/+page.svelte:48-53,109-114`;
  external `platform-integrations/local-library/MangaFollow.ts:55-56`.
  Every CSV/MAL batch independently rereads global library mode. The settings
  importer mutex and global local-library panel pending state are independent.
- Scenario: import more than 400 entries in device mode, then choose account
  mode through the global panel while the first batch is in flight. The next
  batch goes to the account server. One visible file import is split between
  independent destinations. Account identity changes can similarly retarget
  later batches.
- Owner repair independently reread: `library.ts:22-37,184-195,215-220`,
  `MangaFollow.ts:55-58,67,75,84`, and overlay whole-file capture/batch anchors
  freeze one mode/account/scope/generation before file reading. Each batch uses
  that context; device transaction admission checks persisted scope and account
  transport pins the expected owner. The established destination race is
  source-closed; runtime is unrun.
- Acceptance: multi-batch CSV and MAL imports remain in their originally
  selected destination; mode, scope or account changes stop later writes with a
  visible partial-result explanation. Earlier confirmed batches remain there.

### UI3-03 — P1: Private history-state caches survive account/source changes

- Untransformed retained `frontend/src/lib/components/ReportSearch.svelte:22-30`
  restores `_$ReportSearch_feed` without a server read. Its writer is
  `frontend/src/routes/(app)/my/reports/+page.svelte:24-28`.
  `ui/Protected.svelte:17-25` only checks that some current Session exists.
- The same private-draft cache pattern is in
  `components/draft/manga/DraftSearch.svelte:21-29` and
  `routes/(app)/drafts/title/+page.svelte:24-29`; the outer verified guard does
  not bind cached DTOs to their original owner/current permission.
- Actual composed `ChapterFeed.svelte:49-52` also restores private
  `_$ChapterFeed_history` on every route outside `/library` and `/history`,
  including public profile/author/group/home feed views. Current device records
  do not authorize disclosure of the previous account's read-history map.
- Scenario: A views private reports/drafts, performs the retained SPA logout,
  B logs in, and Back returns to A's saved history entry. Cached report details,
  moderator notes or draft metadata are accepted under B without current
  server authorization. Public feed history can retain A's private read flags
  after account or library-source changes.
- This is counterevidence from remaining callers, not a repetition of the
  already-bypassed local-library feed caches in AUD-04.
- Closure: private cache reads must recheck fresh authority; bypassing them is
  sufficient. Pin owner/view before awaits, reject superseded results, and clear
  private maps when that identity changes.
- Acceptance: A→logout→B→Back, permission revocation and device/account source
  switches cannot display A's reports, drafts or read flags under the new view.

### UI3-04 — P2: Device read-toggle reverses the shared history-map update

- Actual composed `ChapterListEntry.svelte:45-58` and
  `ChapterFeedChapterEntry.svelte:43-56` send a payload selected by `isRead`, then
  consult `isRead` again after awaiting `UpdateBatch`.
- Device `UpdateBatch` commits and refreshes `localLibraryState` before resolving
  (`local-library/ChapterHistory.ts:32-34`; `library.ts` localAction/updateChapters).
  In device mode `isRead` is derived from those refreshed records. It has already
  flipped by the time the shared `readHistory` branch runs, reversing the
  intended add/delete operation.
- Scenario: mark an unread chapter read. The durable record and eye flag become
  read, but the shared map is deleted/left absent. Marking read→unread instead
  inserts it. `MangaActions.svelte:199-208` uses that shared map for Start Read,
  selecting or skipping the wrong chapter despite the visible eye being right.
- Closure: capture the requested prior state before the await and apply the
  confirmed requested result, with current library/target ownership checks.
- Acceptance: device and account mark-read/unread, followed immediately by Start
  Read, agree with durable state; switching source while the mutation completes
  cannot attach its result to the other library map.

### UI3-05 — P2: Title history is loaded once and never follows library identity

- Actual composed `routes/(app)/title/Manga.svelte:112-137` loads history only on
  mount. Its shared map is never cleared/reloaded for mode/account/scope changes.
- Scenario: remain on one title and change device→account library. Chapter rows
  in account mode derive their flags from the old device map, and Start Read
  continues using it. A fresh title mount during pending scope verification can
  also lose the initial history read when its view key changes on verification;
  the map has no retry/effect after the scope becomes usable.
- Closure: clear and reload title history under the current library-view key and
  captured title identity, rejecting delayed older results. Device records may
  provide the current device map directly where appropriate.
- Acceptance: pending→verified and device/account/account-owner switches update
  flags and Start Read without reloading the title; old results never reattach.

### UI3-06 — P2: Width changes can make the repaired long-strip threshold unreachable

- Actual composed reader single lines 53-55,61-75 and double lines 56-58,64-73;
  retained `reader-settings.svelte:315-316` exposes the width slider.
- The threshold repair computes a reachable fraction of wrapper height only
  when `attachObserver` runs. The effects track scale/layout/direction, not width.
- Scenario: in LimitWidth/StretchWidth scroll mode, width=10% makes a tall strip
  fit in one viewport, so the threshold is .65. Change width to 100%, making it
  taller than 2.5 viewports. No window resize occurs and the old .65 threshold
  is now impossible, so current-page/progress/preload advancement stalls again.
- This is concrete control-change counterevidence for UI2-04. Its initial tall
  strip and layout-listener repairs are present; the width transition is open.
- Closure: recompute on width/actual element-size changes while preserving
  lifetime/disposal ownership.
- Acceptance: width 10→100→10, sidebar width changes, single/double and paged→scroll
  transitions keep long-strip progress advancing with preload=1.

### UI3-07 — P2: Closing and rapidly reopening a report clears the new subject

- Retained `frontend/src/lib/components/ReportDialog.svelte:52-60` creates an
  uncancelled 200ms reset on close. Lines 64-75 fence reasons reads only by href.
- Scenario: cancel report A and immediately open report B on the same title
  route. A's timer clears B's entity/type/reason while the dialog is open,
  preventing a valid submit. A delayed Chapter reasons read can populate a
  reopened Manga report at the same URL with wrong-category options.
- Closure: own/cancel the close-reset timer on reopen/disposal and capture the
  subject/type/open generation across reasons reads; reject stale results.
- Acceptance: rapid close/reopen, alternate subjects on one route, delayed old
  reasons and navigation leave the current subject/reasons intact and usable.

### UI3-08 — P1: Reused title editor retains another target's draft and late receipts

- Actual composed `routes/(app)/title/[id]/edit/+page.svelte` loads covers only on
  mount and mounts the external editor without a target key.
- `frontend-behavior/MangaEditBoundary.svelte:14` changes current from new props,
  but line 56 renders unkeyed `RetainedMangaEdit`. The actual retained editor
  snapshots `data.manga`/covers once at composed lines 44-55 and 89.
- Scenario: client-side A/edit→B/edit navigation reuses the same route component.
  B's route/header can retain A's draft/covers, and the initialized editor still
  addresses A in mutation calls. The boundary's refresh/retry at lines 23-50 has
  no target generation/disposal fence, so delayed A receipt/cover results can
  also overwrite B's current state after navigation.
- Closure: bind retained editor lifetime to target identity, load parent covers
  per identity, and capture/fence boundary read/retry completions. Do not replay
  an ambiguous publication or rebind its signed intent to a different target.
- Acceptance: A/edit→B/edit and Back/Forward under delayed cover/receipt/retry
  responses keep every field, cover, receipt and mutation attached to the route's
  current target. Leaving during a pending operation cannot repopulate another
  editor.

## Independent reread of round-two repairs

1. UI2-01: aggregate data is component-owned, requested for the current manga and
   language, with generation/disposal and identity checks. No shared cache reuse.
2. UI2-02: restored last page writes one-based `lastIndex + 1` to URL/state.
3. UI2-03: double spread visibility derives its selected member when present.
4. UI2-04: basic tall-strip threshold and lifetime resize/disposal fixes are
   present, but the width-slider counterexample is tracked as UI3-06.
5. UI2-05: keyboard RAF cancellation is owned by blur and component destruction.
6. UI2-06: replacement admits type/header/pixel/decode/session budgets before an
   owner/version-fenced atomic replacement POST; no preceding durable DELETE.
7. UI2-07: current catalogue interpretation requires verified scope; mutations
   compare the freshly verified tuple with transaction-bound stored metadata.
   Explicit saved-device editing has a separate seam.
8. UI2-08: parsed storage is shape/type/range-normalized against real defaults;
   corrupt bytes are preserved until a later explicit edit.
9. UI2-09: Generate.csv now produces scoped CSV consumed by the retained parser;
   JSON and canonical recovery controls remain distinct.
10. UI2-10: edited target, recipient and signer define a synchronous context key;
    mappings/conversations/reviews are invalidated and delayed results fenced.
    Saved signed bodies retain their original target/recipient before transport.

These are finite source results. None has runtime acceptance from this review.

## Inspected scope and negative results

The review traced source composition/resolvers and actual retained reader
navigation, pages/spreads/layout/quality/loading/retry/history; upload direct,
folder and archive admission, replacement, ordering/removal/restore/autosave,
commit/cancel; device/account library adapters, filters/paging, verified scope,
saved-device/list controls, CSV/JSON/account/canonical imports and exports;
public graph publication/recovery and exact signed action outboxes; public
profile/follows/relay metadata and source conversations; preference persistence
and retained preference controls; installed-worker network/cache priority;
announcements list/detail/manage/version controls; retained report/filter/claim
and community/editor paths; and title pending-publication boundary.

No additional concrete defect was established in reconstructed announcements,
initial reader aggregate/last-page/second-spread repairs, replacement admission,
semantic storage defaults, verified native-scope transaction fences, or signed
conversation target-change handling. This does not claim exhaustive review of
every inventoried anchor, full browser offline boot capability, current-source
revocation certification, or absence of all possible failures. The finite source
gate remains withheld pending the findings above and fresh independent review;
the separate runtime gate remains unrun.
