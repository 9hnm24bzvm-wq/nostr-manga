# Round-eleven independent UI source-completeness audit

Reviewed 2026-10-13, beginning 07:31 UTC. This fresh reviewer did not implement
the preceding repairs. Shared → reader → local-library transforms were applied
independently to an inspection copy, enforcing original source hashes and exact
occurrence anchors. Actual composed callers, mounted resolver configuration,
external integration modules and relevant native backend handlers were read.

## Finite verdict

**Source signoff withheld: two substantive, bounded findings were established.**
The four UI10 repair families are present and source-close their reported
scenarios. The new findings concern supplied custom-list navigation and actual
device-progress persistence across explicit library selection. They do not
claim the repaired auth callbacks or previous-chapter blanket flag remain.

No browser journey, application runtime, database operation, Cargo command,
Vite build or test suite was executed by this reviewer. Source transformation
and hashing are inspection operations, not application tests. All runtime
acceptance below is **unrun**. No implementation source was edited; this report
is the sole repository deliverable. Findings were sent to the parent as they
were established. Later candidates require a fresh independent pass.

## Independently frozen source

Frontend snapshot: `2026-10-13T07:32:53.997Z`.

- 444 repair definitions, 453 exact occurrences, 96 original paths
- 541 copied/transformed `.svelte`/`.ts`/`.js` source files
- SHA of JSON-serialized composed path/SHA records sorted with `localeCompare`:
  `8df912070a82d0e1f8764acdb9730911c855e988e3aea897672b930647830b28`
- Shared overlay:
  `6fc8cd9fae676606b2b6a5d6df36ba68f905f622ad3d64a4c7e5c27640a01894`
- Reader overlay:
  `70ec02db52c0867872f1545ff20667a9b7e0f7e747261de8b5a57f62285c9505`
- Local overlay:
  `85b7aad688a47f459b4bd993d6956bc2d68f881fb52a0c12bbac9426d39aa344`
- Independently calculated 56-file external integration inventory SHA:
  `f7b20453c22c8ccc7122cb5dc064969bf786cdd5d421ff08bb7275f90b918650`

All 676 original attachment paths were independently rehashed against
`test-evidence/frontend/source-parity.json`: none was missing or different.
Logical `src/` paths and lines below refer to the composed inspection copy,
not the preserved raw attachment. External paths name the actual mounted
integration files; historical explanatory fragments are not substituted for
executed source.

The existing owner compiler receipt at `2026-10-13T07:29:15.109Z` reports passed
84-component/49-script source compilation with these exact integration hashes.
Its matching owner freeze is
`test-evidence/frontend/round10-owner-source-repairs/summary.json`. This is
owner evidence, not a compiler/runtime run by this reviewer. Build11 was in
progress when the review was assigned; no final executable bundle is certified
by this source review.

Relevant native source was separately read:

- `backend/src/domains/auth/clients.rs` SHA:
  `3d5f109601d0064fc7b11d4f4ccc3e74de38c2f5f0d45c703fc0713b88c39d94`

## Findings

### UI11-01 — P2: A delayed custom-device-list link overwrites newer navigation

- Actual `platform-integrations/local-library/LocalLibraryPanel.svelte:60-70`
  handles the supplied custom-list anchor. It prevents native navigation, reads
  the device snapshot, selects device mode, reads another snapshot and finally
  calls `goto('/library?list=' + id)` at line 69.
- Its checks at lines 63-67 bind saved list existence, catalogue scope and
  selected library mode. None captures or checks the navigation that admitted
  the link. The component's teardown at lines 80-85 fences only the canonical
  record reader and removes an error listener; it does not fence this callback.
- Composed `src/routes/(app)/+layout.svelte:18` mounts this actual panel in the
  common app layout. Ordinary app-route navigation retains it. The two actual
  reader entry routes also mount the panel at line 13 in their keyed chapter
  lifetime, where disposal likewise does not invalidate this link callback.
- Actual `platform-integrations/local-library/library.ts:119-128` makes mode
  selection asynchronous through initialization, optional Session readiness,
  module loading and the IndexedDB preference transaction. Changing routes
  does not invalidate the library context or those waits.
- Scenario: with a saved custom device list, activate its supplied panel link
  and defer its snapshot or mode transaction. Navigate to `/search`, or use
  Back/Forward to a newer route, then release the old work. With unchanged
  list/scope/account, every existing guard passes and the old callback sends
  the user back to `/library?list=<old-list>`. A reader-mounted panel can do the
  same after its chapter view has been disposed. This is an actual existing
  navigation action, not a request for another list feature.
- Closure: bind this link's eventual navigation to its admitted route/navigation
  generation and live panel lifetime. After each wait, obsolete work must not
  redirect over newer navigation. Preserve exact saved-list/scope checks and
  the intended explicit device selection. A preference transaction already
  committed may remain committed; do not claim discarding the redirect reverses
  that transaction or replay any action.
- Acceptance: defer each relevant storage/mode wait after activating the actual
  saved-list link, navigate elsewhere, use Back/Forward and dispose/reopen the
  reader or panel. Releasing old work cannot alter the newer route. A current
  link still selects device mode and opens its exact saved list; deleted lists,
  changed scope/mode and concurrent newer selections remain rejected truthfully.
- Runtime acceptance: **unrun**.

### UI11-02 — P2: Mode reentry permanently suppresses a fenced same-page progress save

- Composed `src/lib/components/reader/reader-history.svelte:43-52` deduplicates
  device progress with a token containing only chapter, page and completion.
  It assigns that token **before** the debounced durable save is acknowledged.
  Neither leaving device mode nor returning to it resets that token.
- Its `flush` at lines 14-21 removes `queued` before checking the captured
  destination. On an invalidated intent it returns without saving or resetting
  `progressToken`. Destruction at line 23 can flush only the remaining queue.
- Actual `platform-integrations/local-library/action-owner.ts:9-20` correctly
  captures and verifies the selected-library admission across waits. Actual
  `library.ts:36-42,119-128` advances selection epoch at each explicit mode
  choice; context generations at lines 28-33 also change across mode changes.
  Therefore the old device save is correctly fenced after device→account→device,
  even if the reader chapter and current page are unchanged.
- Scenario: mount a verified device-mode reader for a chapter with no previous
  device record. Defer its first 350 ms flush, switch to account mode and allow
  the old flush to run, then switch back to device without changing the page.
  The old intent is rejected and its queue removed. The returning device effect
  sees the same chapter/page/completed token at line 48 and schedules nothing.
  Remaining on that page and then leaving the reader saves no current device
  page or completion record. The same seam applies when an admitted save's
  storage transaction is deferred and fenced by the mode change before commit.
- Closure: deduplicate against a successfully settled/current destination, or
  invalidate the device-progress deduplication state when its admitted context
  is lost. A still-live reader reentering device mode must be able to admit its
  current immutable page action again under the **new** device context. Keep
  old-account/mode/disposal fences, local transaction atomicity, exit-flush
  semantics and completion retention. Do not replay a sent account-history
  mutation or resurrect an obsolete reader's action.
- Acceptance: with no prior device record, defer both the progress timer and
  the device-save transaction separately across device→account→device while
  retaining the same reader/page. The old context cannot write, but the current
  device context saves the current page/completion once and survives reload.
  Repeat at a non-final and final page, A→B authority changes, scope-verification
  changes and reader exit. Same-context local-store refresh must not produce
  an endless save loop; account history is neither retargeted nor replayed.
- Runtime acceptance: **unrun**.

## Independent reread of round-ten closures

1. **UI10-01 is source-closed for its reported auth/settings callbacks.** The
   common `auth-form-owner.ts` fences disposal, generation and exact browser URL,
   and flow writes also check concrete flow ID. Actual login/register/settings
   forms recheck after their Session waits. The auth and settings layout keys
   include pathname and requested flow, so a same-route reopened or different
   requested flow receives a new form lifetime. Restricted recovery and its
   executed anonymous derivative use the guarded source. This conclusion does
   not certify the repeated browser flow matrix.
2. **UI10-02 is source-closed for the blanket previous-chapter flag.** The
   composed Reader consumes only the exact chapter/URL/generation intent.
   `reader-navigation.ts` clears its pending intent after failed/cancelled
   completion, and the common app navigation guard clears different destinations.
   No module-global boolean is inherited by an unrelated Reader. Successful
   previous-chapter last-page navigation still needs actual browser acceptance.
3. **UI10-03 is source-closed.** Tag readiness no longer invokes draft Reset.
   The composed preferences form stores an independent dirty baseline; unrelated
   same-authority refresh retains an edited draft and its original version,
   while Reset, real authority changes and acknowledged saves hydrate the
   intended stored state. No new draft-reset finding was established.
4. **UI10-04 is source-closed.** Native client creation changes only its
   successful response to 201 after commit and retains private no-store headers.
   The actual supplied dialog consumes 201 and installs the one-time key.
   List/rename/rotation response status and credential authority are preserved.
   Real exactly-once creation/key presentation remains an unrun runtime gate.

## Coverage and remaining gates

The finite pass inspected transform/resolver composition and preservation;
the preceding closures; current native transport/admission/no-replay behavior;
public search/filter/reset/pagination and selected-library filters/updates/history;
title follow/bell/start-reading and read flags; reader aggregate/context/pages/
queue/quality/retry/keyboard/history; upload image/archive/folder admission,
splitting/batch/replacement/remove/duplicates, staging/autosave/restore/cancel;
account preferences, client controls, auth and restricted recovery; discussion
target/authority/vote state, report reasons/submission/search/review controls and
private projections; real announcement feed/detail/manage; device records/lists/
backup/import/scope verification and offline saved-record seams; signed public
identity/review/target/recipient binding, canonical import/recovery/publication
and exact outbox cancellation/reconciliation/retry controls.

Mounted URLSearchParams prefix behavior was considered before judging query
concatenation. Raw-original defects already repaired by mounted transforms,
unused wrappers, speculative features, style changes and merely possible races
without an established concrete scenario were not filed. No further
substantiated in-scope defect was established in this finite pass.

Final executable bundle and actual browser/API/database/system journeys,
offline boot, persistence, revocation and production checks remain separate,
unrun gates for this reviewer. Passing source compilation or preserved source
bytes cannot substitute for those gates or establish production readiness.
