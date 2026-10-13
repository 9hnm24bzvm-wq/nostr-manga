# Round-two independent UI and browser-data source audit

Reviewed 2026-10-13, starting 04:35 UTC. This is a fresh source review of the
actual shared-plus-local overlay composition, retained reader/upload/preferences
callers, native IndexedDB adapter, original canonical library/signing modules,
reconstructed announcements and pending-title editor module. Historical audit
closures were checked against callers rather than accepted as proof.

## Verdict

**Source signoff withheld: ten concrete in-scope defects were found.** Three
reader repairs were subsequently authored under the parent's explicit assignment
in a separate module; these require independent owner review, mounting and final
source reread. Other findings were sent to the frontend owner/parent immediately.
This report captures the inspected snapshot, not later owner patch dispositions.

No browser journey, application/runtime test, Cargo invocation or bundler was
run. Composing exact source overlays into temporary inspection files and checking
replacement anchors/retained markup is source inspection only. Every runtime
acceptance below remains unrun. This finite report does not claim that all
possible failures were found or that every inventoried anchor was individually
verified in a browser.

## Findings

### UI2-01 — P2: Reader aggregate cache omits translation identity and late-result ownership

- Supplied source: `frontend/src/lib/components/reader/reader-provider.svelte:1-3,73-82`;
  inspected composed source: the same module state, `onMount` lines 74-83.
- `lastMangaId` caches only the title, while the actual request includes
  `tlang=chapter.language`. English chapter → title → French chapter of the same
  title reuses English entries. `chapterIndex` becomes -1 and chapter/group/
  previous/next controls show or navigate the wrong translation set. A delayed
  old-title request can also overwrite the module aggregate used by a newer reader.
- Required source closure: key the returned aggregate by manga and language;
  invalidate unsuitable active data and ignore disposed/superseded requests.
- Runtime acceptance: switch translations within a title; switch titles before
  a delayed aggregate resolves; every selector/next/previous remains attached to
  the current reader. Owner notified.

### UI2-02 — P2: Auto-back last-page route stores a zero-based page

- `frontend/src/lib/components/reader/reader-provider.svelte:64-67,105-108`;
  composed lines 65-68,108-111.
- Auto-advance backward selects `pages[n-1]` but writes URL/state page `n-1`.
  Reader restoration interprets that value as one-based. For ten pages the reader
  initially shows page 10, but refresh/quality recreation opens page 9; a one-page
  chapter produces `/0`.
- Runtime acceptance: auto-back into a ten-page and one-page chapter, then reload
  and switch quality. URL and displayed final page must agree. Owner notified.

### UI2-03 — P2: Selecting the second member of a double spread blanks the reader

- `frontend/src/lib/components/reader/reader-pages-double.svelte:8,75`.
- The retained visibility expression compares currentPage with `spread[0]`.
  Direct `/2`, selecting page 2, or enabling double while on page 2 makes its
  `[page1,page2]` spread hidden even though it is the selected spread.
- Assigned repair: `platform-integrations/frontend-reader-overlays.mjs` derives
  the selected member when currentPage belongs to this spread, otherwise its
  first member. Retained markup/styles are unchanged. **Self-authored; independent
  review and mount required.**
- Runtime acceptance: every odd/even deep link and page-selector value remains
  visible in double mode, including offset, RTL and mode toggles. Scrolling into
  another spread must still select that spread's first member.

### UI2-04 — P2: Long-strip observer threshold is unreachable

- `frontend/src/lib/components/reader/reader-pages-single.svelte:16-21,49` and
  `reader-pages-double.svelte:17-23,51`.
- Any page taller than 2.5 viewports cannot intersect 40% of its own area. Valid
  long strips therefore never become currentPage; progress remains stale and
  the preload queue does not advance beyond its previous window. A reader
  initialized paged also never registers the resize listener when later switched
  to scroll mode.
- Assigned repair: the separate reader overlay fits the threshold within the
  viewport, owns one resize listener regardless of initial layout, and fences
  tick/observer work after disposal. **Self-authored; independent review and mount
  required.**
- Runtime acceptance: scroll through several strips taller than three viewports
  with preload=1; page identity, loading and progress advance. Resize after a
  paged→scroll transition; leave while a mount tick/observer notification is queued.

### UI2-05 — P2: Held-key scrolling survives blur or reader exit

- `frontend/src/lib/components/reader/reader-keyboard-listener.svelte:9-15,123-129`.
- The recursively scheduled animation frame stops only on keyup. Hold a scroll
  key and navigate away or blur before keyup; the old loop keeps scrolling the
  next page or remains active after focus loss.
- Assigned repair: the separate reader overlay cancels on blur and destruction,
  with a lifetime-owned blur listener. **Self-authored; independent review and
  mount required.**
- Runtime acceptance: hold/release normally, blur/refocus while held, and navigate
  while held; no old frame continues scrolling after blur/exit.

### UI2-06 — P1: Replacement upload bypasses admission before deleting valid staging

- Actual composed `UploadChapterPages.svelte:264-287`; corresponding retained
  `replaceFile` function in
  `frontend/src/lib/components/upload/UploadChapterPages.svelte:222-253`.
- The new add/ZIP image admission lives in
  `platform-integrations/frontend-behavior/upload-process.fragment.ts:24-55`.
  Replacement directly creates a local page, deletes the valid staged target and
  sends its replacement without these size/pixel/decode/aggregate checks.
  A malformed or oversized selected replacement discards the old valid page
  before the server rejects the new one. This is caller counterevidence against
  AUD-14's historical source closure, rather than a repeated unsubstantiated claim.
- Required closure: validate the chosen replacement and its resulting session
  budget before any DELETE; keep the target when admission fails.
- Runtime acceptance: replace with invalid header, oversized bytes, pixel bomb
  and over-budget aggregate. Each fails visibly with the old page retained and
  no destructive request. An ordinary admitted replacement still works. Owner notified.

### UI2-07 — P1: Current-catalogue writes can enter an old namespace before verification

- `platform-integrations/local-library/library.ts:12-15,86-123`;
  `device-store.ts:135-147`;
  mounted reader history overlay in `local-library/overlays.mjs:314`.
- Device availability allows pending/offline/origin-only writes; mutation checks
  only an already-persisted mismatch. With saved namespace A and rebuilt current
  namespace B, stall capabilities discovery while loading a fresh B title and
  follow/mark it before verification finishes. A B native ID enters the preserved
  A scope. A reused native ID can also inherit/edit A follows/progress through B's
  current catalogue UI. The durable post-detection fence does not prevent this.
- Required closure: current-catalogue ID interpretation/mutation needs a freshly
  verified tuple equal to transaction-bound stored scope. Explicit old-device
  inspection/export/removal/list management can remain offline; do not silently
  interpret a fresh current route as an offline edit of a colliding old ID.
- Runtime acceptance: delay/fail discovery during A→B switch, including reused
  native IDs, across tabs. Current-route writes fail without mixing identities;
  explicitly scoped offline records/backups remain usable. Owner repair discussion
  in progress; no source closure asserted here.

### UI2-08 — P2: Syntactically valid corrupt preference bytes still crash callers

- `platform-integrations/frontend-behavior/PersistentStore.ts:12-22`;
  `frontend/src/lib/stores/prefs.ts:25-28,40-46` and reader preference callers.
- Parsing `null`, a primitive or an object missing required arrays succeeds and
  is blindly asserted as T. For example localStorage key `xzp0yv4s` containing
  `null` reaches `prefs.languages` and crashes; malformed reader preference shape
  similarly reaches object-field callers. The catch only covers JSON syntax and
  storage exceptions. This is semantic-corruption counterevidence against AUD-11.
- Required closure: validate stored shape against the actual store contract or
  a narrow validator before admitting it; preserve invalid old bytes and activate
  safe defaults without silent overwrite.
- Runtime acceptance: null/primitive/incomplete/wrong-type arrays and old reader
  shapes cannot kill bootstrap; explicit valid edits persist, denial is visible.
  Owner notified.

### UI2-09 — P2: The retained CSV export button downloads an incompatible JSON file

- Actual composed `settings/library/+page.svelte:159-167,224`;
  `platform-integrations/local-library/overlays.mjs` export dispatch repair.
- In default device mode “Generate .csv” exports `weebdex-device-library.json`,
  whereas the retained importer accepts CSV/XML. The visible CSV workflow no
  longer yields its requested format or an inverse compatible import. The
  separate explicit JSON backup control already exists in LocalLibraryPanel.
- Required closure: preserve an actual device CSV export at this promised button,
  or obtain a narrowly reviewed, truthful format change for that control.
- Runtime acceptance: use the visible CSV button, inspect CSV headers/escaping,
  then import through the matching retained workflow; scoped JSON backup remains
  a separate explicit recovery path. Owner notified.

### UI2-10 — P2: Signed-conversation target edits retain the previous target/review

- `platform-integrations/local-library/ProtocolIdentityPanel.svelte:12,18-22,34,69-80`.
- The global mounted panel has editable targetKind/targetId, but invalidation
  tracks only resource props, selected origin and signer generation. Load target
  A then edit the field to B: source, reply parent and review still refer to A,
  so the next reviewed action addresses the previous conversation despite the
  newly selected target. Delayed reads can repopulate an old context after change.
- Required closure: include editable target identity and capture/validate read
  context across awaited mapping/social/metadata calls; clear stale review/source/
  replies rather than reattaching them to a new selection.
- Runtime acceptance: change target/recipient/signer during and after delayed
  reads. Only the current selected conversation can populate controls, and no
  previous review can remain approved under the new target. Parent notified.

## Finite inspected coverage and remaining evidence

The review traced composed source API resolution, library guards/reactivity keys,
device/account read and write separation, scope discovery/adoption/import/export,
canonical original backup/publication/recovery/outbox seams, exact signed public
action persistence/retry, retained reader navigation/display/loading/progress,
upload restore/autosave/order/commit/cancel and direct/ZIP/folder admission,
preference hydration/persistence, service-worker route priority/legacy purge,
logout session clearing, pending-title receipt refresh/retry and reconstructed
announcement forms/version handling. No additional concrete service-worker or
pending-title defect was established in this pass. That is a finite negative
source result, not a runtime/privacy/revocation certification.

The isolated source-transform check for the new reader module verifies all exact
anchors and unchanged retained markup/styles for its three supplied source files.
Svelte compilation, combined final manifest evidence and independent review are
the frontend owner's gates. Installed-worker cache/revocation, multi-tab/quota,
staged/accepted disconnect recovery, real signer/CORS/admission and every visible
control's actual browser outcome remain unrun.

## Necessary design seams

Read the official [Matt Pocock codebase-design skill](https://github.com/mattpocock/skills/blob/main/skills/engineering/codebase-design/SKILL.md).
Only demonstrated seam changes are warranted here:

- Keep native namespace admission in the device mutation module, with explicit
  current-catalogue versus saved-device intent; do not distribute independent
  truthiness checks across controls.
- Use one image-admission module for add, archive and replacement callers before
  destructive staging changes.
- Keep reader selection identity and observer/animation lifetime local to their
  existing modules, with tests crossing the same interface used by controls.
- Give selected signed-conversation context one invalidation/read-generation
  contract rather than maintaining unrelated source/review variables.

No broad rewrite or hypothetical architecture is recommended before these
concrete defects and the separate execution gates are resolved.
