# Round-five independent UI source-completeness audit

Reviewed 2026-10-13, beginning 05:25 UTC. This reviewer did not own the preceding
implementation patches. The actual shared → reader → local-library source
composition was independently applied into a temporary inspection directory,
enforcing original file SHA and every exact replacement count. External modules,
resolver configuration and retained callers were read directly.

## Finite verdict

**Source signoff withheld: four substantive, bounded UI findings were established
in the initial round-five snapshot.** The round-four title/search completion,
fresh filtered Start Read, and keyed upload/edit target-lifetime repairs are
present. A remaining account-store reconciliation seam is independently tracked
as UI5-01, rather than treating those caller repairs as end-to-end proof.

UI5-01 was repaired by the frontend owner during this review and its new source
was independently reread below. UI5-02 through UI5-04 remain findings against the
initial composed snapshot. Subsequent fixes require composed reread and the
user-requested fresh independent squad after the completed patchset.

This was source-only work. No application/browser journey, test suite, Cargo
command, Vite build or application runtime was executed. Applying source
transforms for inspection was not an application test. Every browser/runtime
acceptance below is **unrun**. No implementation source was edited by this
reviewer; this report is the only repository deliverable write.

The initial composition has **262 repair definitions, 271 exact anchor
occurrences and 63 original paths**. Its integration inventory is
`8caf92dca314014798d98844406c69a15de19bac2eda42e2a5bad1148ee5ffbd`.
The inspection copied/transformed 541 source `.svelte`/`.ts`/`.js` files; the SHA
of JSON-serialized, path-sorted composed path/SHA records is
`7c822ef008c6d6ee50d6c49b8bee13ab1fc61e15a3544ec89e346a9f118c7c6c`.

Reviewed initial source pins:

- Shared overlay: `93ae7a1b35f84923e1c2f2bd5f731a01563b079b1cb844b3806e8dafb271fe91`
- Reader overlay: `0fa703b3208a1d0abe210062cf4ec02bb813132a83d687cf331d212aae983dce`
- Local overlay: `acc47af42d7638f6cc77d2529c71b553f7d1ce73c5b8ebe11b3461fb275a043f`
- Library adapter: `453e7dc49c80c7093062d989aa182d343595fb1290fda4448b0d08d8765fea77`
- MangaFollow adapter: `a45c7898a9b4bd605d319fe9a81c07b971934b4158f650c2f50146fd907eca3b`
- Target owner: `972844d80eb2a3eaca0a66d047bd05c572f3b599929b0a28fe268c9caeff8c09`
- Upload resume: `64d7910713f122deca1fcd222243d63b4299bc159b5b74adf747a8a4624376ab`

These pins identify inspected source only. They do not certify a later patch,
the stale 227-definition built artifact, or production behavior. Composed line
references below refer to the executed logical frontend path.

## Findings

### UI5-01 — P1: Delayed account reconciliation overwrites a newer Session

- Initial external `platform-integrations/local-library/library.ts:186-206`,
  especially line 196, awaits `authoritativeAccount(expectedId)` after a successful
  mutation and then unconditionally calls `Session.set(confirmed)`. The returned
  DTO's identity is checked against the original expected ID, but the currently
  selected Session/library authority is not checked after the awaited read or
  immediately before this store write.
- Composed title/search callers now call `assertLibraryContext` after the adapter
  resolves. That is too late to prevent the adapter's preceding Session side
  effect. This is concrete counterevidence at an additional side-effect seam,
  not a claim that the round-four caller fences are absent.
- Scenario: A's account follow/status/notification/unfollow/history/import is
  accepted. Its `/auth/check` reconciliation starts under A. Delay its valid A
  response, perform SPA logout (composed `auth/logout/+page.svelte:25-26`) and/or
  log B in (`auth/login/+page.svelte:147-148`), then release the original A check.
  The helper reinstalls A's identity, follows, roles and permissions into the
  newer view before the caller rejects its old completion. A→logout→A can also
  replace a newer same-ID Session with an older authority snapshot.
- Closure: the helper that owns `Session.set` must capture admission account,
  selected-library generation and Session-write authority generation. Recheck
  them after awaited response bodies/reconciliation and synchronously immediately
  before the store side effect. Preserve acknowledged original mutations and
  expose uncertainty; do not automatically replay them.
- Owner repair independently reread: current `library.ts:25-26,191-218` tracks
  every Session emission with a monotonic generation, captures generation and
  library context before awaited validity, checks the current account after
  response/body reads and after reconciliation, and has no await between its
  final assertion and `Session.set`. SHA:
  `7dba07f8120b85b7d63454bae10e0c255cb16eb775f4d0ab311f92bddf432520`.
  Owner receipt: `test-evidence/frontend/round5-owner-source-repairs/account-reconcile.json`.
  **This finding is source-closed at that reread; runtime remains unrun.**
- Acceptance: delay reconciliation through logout, A→B, A→logout→A and a newer
  same-account permission/Session refresh. Old results cannot restore the old
  identity/maps/authority or overwrite newer Session data. The original operation
  remains at its original destination without replay.

### UI5-02 — P2: A failed page-upload batch strands unrecoverable local placeholders

- Actual composed `src/lib/components/upload/UploadChapterPages.svelte:209-216,
  247-260,380-396` creates local entries carrying `file`, appends the entire
  selection before sending, then throws on an unsuccessful/network-ended batch.
  The failure does not mark its placeholders as failed or reconcile/remove them.
- Composed `UploadChapterPagesEntry.svelte:95-97,123-141` shows a spinner for an
  entry without `file_name`, but only renders per-page Replace/Delete when
  `!page.file || page.error`. Failed client placeholders still have `file` and no
  `error`, so neither control is available. Bulk Remove all is itself hidden for
  a single page (`UploadChapterPages.svelte:628-657`).
- Composed `UploadChapter.svelte:43-48` cannot enable Upload while such a local
  entry remains; its autosave skips local files at lines 193-197. The unsaved
  signature remains different, so lines 217-221 reject ordinary SPA navigation.
  The chapter-page editor uses the same upload child and has analogous invalid
  file gating.
- Scenario: choose exactly one ordinary image, then fail its POST with a server
  rejection or connection interruption. After the mutex releases it is still an
  indefinitely spinning local page, with Upload, Replace, Delete and single-page
  Remove all unavailable. There is no visible retry/reconciliation path. A later
  batch failure with acknowledged earlier pages can similarly require destructive
  bulk clearing or a hard reload that drops selected client bytes.
- Closure: distinguish definitive rejection, unsent later entries and ambiguous
  acceptance. Preserve acknowledged staging, reconcile current owner/session
  state before replaying anything, and give remaining local entries a truthful
  recoverable failure state with safe per-item discard/replacement/retry where
  justified. A network-ended POST must not be blindly duplicated.
- Acceptance: single first-image failure and later-chunk failure after successful
  earlier pages; include rejected and ambiguously accepted POSTs. The user can
  recover or safely discard the affected local item without deleting unrelated
  acknowledged staging, and no idle entry remains permanently labeled loading.
- Runtime acceptance: **unrun**.

### UI5-03 — P2: A new-title staged upload cannot be explicitly cancelled

- Actual composed `src/lib/components/upload/UploadChapter.svelte:301-397`
  exposes group admission, metadata/page controls, Upload and Upload-and-add-
  another. It imports `cancelResume` but never calls it, has no cancellation
  handler, and renders no staged-session Cancel control.
- Its restoration at lines 230-234 redirects a different title's upload route to
  the retained session's original editor. That preservation is valuable, but
  requires the promised original editor to offer explicit cancellation when the
  authoritative session allows it. The chapter-edit counterpart does implement
  this (`UploadChapterEdit.svelte:338-341,493-496`); new-title staging does not.
- Scenario: begin A's new upload, decide to abandon it (even before adding any
  page), then open B/upload. B restores/redirects to A's preserved staging. At A
  there is no explicit way to discard that mutable `can_cancel` session and start
  B. Clearing page items does not cancel the session itself; committing an
  unwanted chapter is not an acceptable cancellation flow.
- Closure: provide an explicit reviewed mutable-session cancellation control in
  the original new-upload editor, bound to its owner and exact session ID. Clear
  local state only after an acknowledged cancellation. Keep immutable/accepted
  receipts uncancellable and preserve their truthful recovery route.
- Acceptance: begin A with zero pages and with staged pages → cancel → start B;
  fail/delay cancellation; restore mutable versus immutable sessions. Successful
  cancellation releases only A, failed/forbidden cancellation preserves it, and
  no navigation silently cancels another target's work.
- Runtime acceptance: **unrun**.

### UI5-04 — P2: Invalid chapter metadata disables its own Revert escape

- Actual composed `src/lib/components/upload/UploadChapterEdit.svelte:505-510`
  disables the Revert/reset button with `$mutex || !isValid || !isChanged`.
  Initial draft validity is at lines 48-61; the reset handler at lines 116-121
  can safely restore the original metadata without requiring a valid edited draft.
- The new navigation guard at line 125 rejects SPA navigation while any metadata
  draft is changed. The validity-dependent Revert gate leaves that guard with no
  direct reset escape for invalid fields.
- Scenario: clear required language or enter an invalid chapter/volume value.
  The draft is dirty and invalid: Save and Revert are both disabled, and the Back
  link/other SPA navigation is cancelled. The user must reconstruct valid values
  manually or hard reload before the advertised Revert action works.
- Closure: enable Revert for every changed, nonpending ordinary metadata draft,
  independent of draft validity. Retain normal pending/immutable staging gates.
- Acceptance: clear language and enter malformed chapter/volume values, then
  Revert. Original fields/oneshot/group state return, the dirty guard clears and
  navigation works; reset must send no mutation.
- Runtime acceptance: **unrun**.

## Independent reread of round-four repairs

1. UI4-01: title and search follow/unfollow/notification completions now capture
   library context and target identity, reject mismatches, and update Session
   follows only through the matched-account helper. The adapters capture the
   mutation destination and assert context through device transaction admission
   and completion. The distinct helper-side reconciliation write was UI5-01
   above and is source-closed only at the later reviewed owner patch.
2. UI4-02: Start Read fetches a fresh aggregate for every attempt; title, query,
   preferences and library identity define its read context. Generation,
   disposal and same-path query changes invalidate aggregate/selected chapter/
   open dialog state and reject old responses. The established stale-filter
   counterexample is source-closed.
3. UI4-03: chapter-edit and title-upload children are keyed by actual target ID;
   parent actor reads are generation/disposal/target/account-fenced. Owned
   metadata/claim/begin/commit requests and staging restore/save/cancel paths
   assert their original target/browser authority. Ordinary chapter metadata
   navigation is guarded. The established cross-target retained-draft/late-
   parent-completion defect is source-closed; the invalid-Revert interaction is
   separately UI5-04.

These are source observations. None substitutes for interrupted/repeated browser
journeys, Back/Forward, account changes or delayed completion acceptance tests.

## Inspected scope and finite negative results

The review traced composed source/resolver order; retained reader aggregate,
page/spread/quality/layout/width/navigation/loading/retry/progress; new and edit
upload staging admission/restoration/metadata/page controls/autosave/commit/
cancellation and image/archive/folder boundaries; device/account follow/history,
library filtering/paging, source availability and transaction-bound namespace
admission; native/canonical backup, import, export, custom lists, saved-device
editing, public publication/recovery/outboxes; signed public profile/follow/relay
metadata and conversation target/review contexts; account login/logout and
Session reconciliation; private report/draft feed fences and report controls;
title-publication boundary; reconstructed announcement list/detail/manage/version
client; preference hydration and installed-worker cache route priority.

No additional concrete new defect was established in this finite pass for the
repaired reader geometry/aggregate selection, title Start Read filters, keyed
editor target identity, canonical family removal, multi-batch import destination,
signed target/review invalidation, native/canonical backup scope/CAS, or report
dialog/private-history-cache closures. This does not assert exhaustive inspection
of every feature anchor, offline app boot, revocation safety, absence of possible
defects, or a runtime/production pass.

Source signoff remains withheld for the remaining findings in this snapshot.
The separate browser/E2E/system gate remains **unrun**. The previously reported
58-Svelte/28-script source compiler pass is an owner receipt, not a browser test
performed by this reviewer; the older built artifact is stale for this candidate.
