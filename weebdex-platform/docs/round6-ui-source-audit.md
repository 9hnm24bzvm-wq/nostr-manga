# Round-six independent UI source-completeness audit

Reviewed 2026-10-13, beginning 05:45 UTC. This reviewer did not implement the
preceding patches. The shared → reader → local-library source transforms were
independently applied to an inspection copy. Each mounted original SHA and exact
replacement count was enforced. Actual external modules, resolver order,
retained callers and relevant installed SvelteKit source were read directly.

## Finite verdict

**Source signoff withheld: four substantive, bounded UI findings were established
against the initial 294-definition round-six composition.** The four round-five
counterexamples are source-closed in this snapshot. The restricted-recovery
boundary and normal-Session admission are present; this is not an authentication
runtime pass. The frontend owner began further repairs while this review was in
progress. Those later candidates require their own frozen composition and the
requested fresh independent squad; this report does not certify them.

This is source-only work. No browser journey, application runtime, Cargo command,
Vite build or test suite was executed. Applying transforms for inspection is not
an application test. Every runtime acceptance below is **unrun**. No implementation
source was edited by this reviewer; this report is the repository deliverable.

Initial inspected composition:

- 294 repair definitions, 303 exact occurrences, 64 original paths
- 541 copied/transformed source `.svelte`/`.ts`/`.js` files
- SHA of JSON-serialized, path-sorted composed path/SHA records:
  `64def7a8742e5a6e10be146fdbf977ca0653814b98a216668ace221e255288b6`
- Shared overlay:
  `35233ac867ad3dc82d042cbf3ad2bdd7a057b6c72ecbb59610c0307b0b3c6076`
- Reader overlay:
  `0fa703b3208a1d0abe210062cf4ec02bb813132a83d687cf331d212aae983dce`
- Local overlay:
  `acc47af42d7638f6cc77d2529c71b553f7d1ce73c5b8ebe11b3461fb275a043f`
- Library adapter:
  `7dba07f8120b85b7d63454bae10e0c255cb16eb775f4d0ab311f92bddf432520`
- Upload batch helper:
  `b44de9fb648bf7999c59e034501eb1c0236c8c74cee1eeabb2d32901f0d006cc`
- Retained anonymous recovery:
  `20ba760776b71854e74c17aef228abd234dd5bded650c4fe863a1a94e1115fcf`
- HTTP helper:
  `4edf40151a8773574ae95c8630405fdd47ac864f4eac1d7d96c1f72ccf60816a`

These identify the initial inspected source, not a later regenerated manifest,
the stale built bundle or production behavior. Original logical frontend paths
and line numbers below refer to that composed snapshot. External paths refer to
the corresponding integration source before its later repair.

## Findings

### UI6-01 — P2: Entering page-edit staging silently discards dirty metadata

- Composed `src/lib/components/upload/UploadChapterEdit.svelte:245-260` admits
  `createUploadSession` whenever resume is ready and no session is present; it
  does not check `isChanged`. The Edit Pages and Replace Pages submit buttons at
  `:529-544` disable only for the mutex. The explicit dirty-metadata navigation
  guard at `:127` does not gate this internal mode transition.
- After Begin succeeds, the existing draft remains displayed through
  `bind:groups={draft.groups}` and `bind:draft`, but is disabled by the presence
  of `uploadSession` (`:445-460`). Staging persistence uses
  `snapshot(chapter, files)` (`:80-94`), and page commit also uses
  `snapshot(chapter, uploadSession.relationships.files)` (`:304-305`), rather than
  the displayed unsaved draft. Successful commit replaces `chapter` and calls
  `onReset` (`:316-319`), which overwrites the local draft (`:118-123`).
- Scenario: edit title, chapter/volume, language or groups without saving, then
  choose Edit Pages or Replace Pages. Add/reorder/replace pages and press Save.
  The page changes are submitted using the original metadata; the displayed
  unsaved metadata is silently reset. An invalid local metadata draft can enter
  the same page-only path because staging validity checks only page receipts
  (`:55-63`). This is a direct source data-loss path, not a request for combined
  metadata/page features.
- Closure: before page-session admission, require the user to Save or Revert
  dirty metadata, with truthful guidance and no draft reset, or implement an
  explicitly reviewed atomic draft-staging contract. The bounded save/revert
  gate is sufficient. Preserve the current target/account lifetime fences.
- Related handler seam: `createUploadSession` does not synchronously call
  `ev.preventDefault()`. The installed SvelteKit client intercepts uncancelled
  GET forms at `@sveltejs/kit/src/runtime/client/client.js:2698-2742`, so an
  unconditional hard reload is **not** established. The handler nevertheless
  permits a concurrent form navigation and should own its submission before
  asynchronous staging admission. This is associated evidence, not a separate
  hard-reload finding.
- Acceptance: change valid and invalid metadata/groups, then click both page
  admission buttons. No Begin POST or automatic navigation occurs until the
  dirty draft is deliberately saved/reverted; the draft remains intact and
  Revert works. After the gate clears, real page-edit admission/commit works.
  Include delayed admission, repeated clicks, Back/Forward and target/account
  changes. No unrelated chapter is mutated and no POST is automatically replayed.
- Runtime acceptance: **unrun**.

### UI6-02 — P2: A device-list link opens unfiltered account follows

- External `platform-integrations/local-library/LocalLibraryPanel.svelte:132`
  renders every Custom device list as a plain `/library?list=<device-list-id>`
  anchor. This panel remains available in either selected library mode. The link
  neither selects device mode nor requires an explicit mode switch.
- Actual resolved `platform-integrations/local-library/MangaFollow.ts:8-9`
  forwards the entire query to `/user/follows/manga` in account mode. Only its
  device branch at `:11-13` applies the saved-device list/member filter. The
  account endpoint at `backend/src/domains/library/follows.rs:70-120` has no
  device-list membership predicate. The `/library` route uses this resolved
  adapter (`src/routes/(app)/library/+page.svelte:18-24`).
- Scenario: create device list L containing A, select account mode whose follows
  contain B, then click L in the Custom device lists disclosure. The advertised
  list link shows the account follow feed, including B, rather than L. With an
  expired/signed-out account it reaches LoginRequired even though the device
  list remains locally available. The URL still carries L, making the silently
  ignored filter misleading.
- Closure: explicitly open the clicked list under device-library authority, or
  present a clear device-mode switch boundary. An account-mode adapter must not
  silently treat a device-list query as an unfiltered account feed. This does
  not request account custom-list UI or cross-mode list migration.
- Acceptance: use distinct A-in-device-list and B-in-account-follow fixtures;
  click L while in account mode and after account logout. The visible result is
  L under clearly selected device authority, or an explicit switch-required
  state. Check list query/filter paging, mode changes and Back/Forward. Neither
  account nor device records are imported, rebound or mutated by opening L.
- Runtime acceptance: **unrun**.

### UI6-03 — P1: A queued report decision can execute under a newer account

- Composed `src/lib/API/Report.ts:31-54` creates credentialed Create, Delete,
  Accept and Reject requests without a captured account or
  `X-WeebDex-Expected-Account` header. Composed
  `src/lib/components/ReportSearchEntry.svelte:54-115` captures no actor,
  row/dialog authority generation or disposal fence for its mutations and
  subsequent bound-row updates. The create dialog has target/open generation
  fencing, but its actual transport has the same missing actor binding
  (`src/lib/components/ReportDialog.svelte:80-95`).
- The actual external `platform-integrations/frontend-behavior/HTTP.ts:96-110`
  can keep a request unsent while the shared token budget is empty, then opens
  and sends the XHR using current browser cookies. Component destruction does
  not cancel these Report requests. This establishes an actual asynchronous
  request-admission seam, rather than assuming a sent request changes actor.
- Scenario: moderator A approves/refuses report R with notes while the request
  is unsent/queued. Switch browser cookie and Session to moderator B before
  transmission. Removing the old feed/dialog does not cancel that request; it
  can transmit with B's cookie. `backend/src/domains/moderation/mod.rs:177-198`
  resolves and authorizes that current principal, and its mutations/audits use
  that principal. A's decision can therefore be attributed to B. An accepted
  eligible group-claim report also grants group leadership (`:207-225`), so this
  is consequential moderation correctness. A regular report Create similarly
  risks attribution to a newer eligible account.
- The backend already rejects a differing explicit expected account
  (`backend/src/domains/auth/principal.rs:232-249`). These Report transports do
  not use that guard. The earlier private-feed read fences are present and do
  not close this separate mutation seam.
- Closure: freeze the approving/submitting actor at report action admission,
  send the expected-account header, and fence target/authority/disposal before
  applying completion to a bound row/dialog. Clear obsolete review state on an
  actor/permission change. Preserve already sent original operations and their
  uncertainty; never automatically replay a report decision.
- Acceptance: exhaust/defer transport before send, approve R as A, then perform
  A→logout, A→B and A→logout→A. The request cannot execute as B, resurrect a
  disposed review or update a newer row/dialog. Include Create, Accept, Refuse
  and Delete, plus a consequential eligible group claim. Delay an already sent
  response separately: original audit/destination remains truthful and no replay
  occurs. Normal current-owner report workflows still work.
- Runtime acceptance: **unrun**; no executed exploit is claimed.

### UI6-04 — P2: Reader shortcuts hijack the added panels' native selectors

- Composed `src/lib/components/reader/reader-keyboard-listener.svelte:18-26`
  exempts only input/textarea targets, Ctrl, scroll locking and the report dialog.
  It does not exempt native select targets or already-owned/default-prevented
  events. Its window listener (`:135`) prevents the ArrowUp/ArrowDown keys
  (`:29-56`) and starts the reader scroll RAF (`:59-73`).
- Both chapter routes mount LocalLibraryPanel and ProtocolIdentityPanel beside
  the active Reader (`src/routes/(app)/chapter/[id]/+page@.svelte:11-15` and the
  `[page]` counterpart). The actual native selectors include saved status,
  device list/series and canonical family
  (`platform-integrations/local-library/LocalLibraryPanel.svelte:113,126-127,206`)
  and verified recipient/target type
  (`platform-integrations/local-library/ProtocolIdentityPanel.svelte:106,132`).
  These panels do not set `reader.scroll.locked` or otherwise stop those
  keyboard events from reaching the window handler.
- Scenario: focus one of these native selectors on a chapter page and press
  ArrowDown/ArrowUp to change it. The reader handler prevents the native
  selection behavior and starts page scrolling. The added controls therefore
  cannot reliably be operated by keyboard while the reader is present.
- Closure: reader shortcuts must ignore native editing/select/control-owned
  keyboard interactions, including default-prevented events as appropriate.
  Keep ordinary reader shortcuts available outside those controls; this is a
  functional interaction repair, not styling or a request for new shortcuts.
- Acceptance: keyboard-select saved status, list/series, canonical family and
  protocol recipient/target type on both chapter routes, with reader layout and
  direction variants. The selected control changes without reader scrolling or
  chapter/page navigation. Reader canvas shortcuts still work and holding a
  scroll shortcut still terminates on keyup, blur and destruction.
- Runtime acceptance: **unrun**.

## Independent reread of preceding repairs

1. UI5-01: `library.ts:191-218` captures selected-library context and every-Session-
   write generation, rechecks after response/body/reconciliation, and has no
   await between its final assertion and `Session.set`. Old reconciliation can
   no longer overwrite a newer local Session through the established seam.
2. UI5-02: the upload batch helper produces error/recovery states for failed
   attempted pages and explicit unsent states for later chunks. Retained per-
   item Replace/Remove controls now appear when a placeholder has an error.
   Matching actual newly staged ID/hash/original filename is read-only; an
   absent/ambiguous receipt remains unknown and cannot be replayed. Mutable
   whole-stage cancellation supplies an explicit escape when exact receipt
   recovery is impossible. The permanent idle-spinner counterexample is closed.
3. UI5-03: new-title staging includes its exact additive ConfirmDialog. It captures
   the actor/session, rereads current `can_cancel`, calls the exact-session DELETE
   and clears local state/blob URLs only after acknowledged success. Immutable
   and accepted publications do not receive this cancellation control.
4. UI5-04: the ordinary metadata Revert gate at composed
   `UploadChapterEdit.svelte:507-513` no longer depends on draft validity.
   Its reset remains local. UI6-01 identifies a distinct page-admission path.
5. Recovery: the exact recovery route delegates the restricted boundary and keeps
   the anonymous email-request component separate. The boundary rejects broader
   account data, mounts only password/CSRF/method nodes, sends one password
   update without automatic replay and requires a confirmed normal Session
   before continuation. Actual composed `src/lib/stores/Session.ts:18-27`
   admits only a normal body with identity ID and username; reset-only 200 bodies
   clear the normal store. Its post-read generation fence also rejects an
   initializer response predating a newer Session write. Raw retained Session
   source alone would give the wrong conclusion because its mounted overlay
   owns these admission changes.

All of these are source observations. Interrupted/repeated runtime journeys,
cross-tab cookies, Back/Forward, network ambiguity and storage failures remain
unrun.

## Inspected scope and finite negative results

The review traced actual source/resolver composition; reader aggregate,
single/double spread geometry, width/layout/direction, loading/retry and progress;
new/edit upload admission/restoration, metadata/pages, bounded image/archive/
folder handling, autosave, commit and cancellation; device/account follows and
history, mode/view identity, native namespace transaction boundaries; saved-device
editing and list membership; scoped/native and portable/canonical backups,
preview/CAS/import/export; signer/recipient/source review lifetimes, public
publication/recovery and exact outboxes; normal login/logout/settings versus
restricted recovery; private report/draft feed reads and report mutations;
reconstructed public announcement list/detail and permission/versioned staff
management; preference initialization and the service-worker cache contract.

No additional concrete defect was established in this finite pass for the repaired
double-spread selected member, reachable long-strip resize threshold, fresh title
Start Read context, target-keyed editors, namespace-bounded saved-device editing,
canonical family/key deletion, multi-batch import destination, signed public
target/review invalidation, backup revision/scope gates, or reset-only normal
Session admission. The retained URLSearchParams prefix override was explicitly
checked before evaluating authentication URL handling; concatenation with the
query does not establish a missing-question-mark defect in this composed app.

This is not exhaustive proof of every feature, offline boot, revocation safety,
visual parity or absence of possible defects. The reported source compiler pass
is an owner receipt, not a browser test performed here. The current executable
bundle has not been freshly built/tested by this reviewer. Source signoff remains
withheld for UI6-01 through UI6-04; browser/E2E/system/production gates remain
separate and unrun.
