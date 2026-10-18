# Browser user-flow readiness, 2026-10-18

This is an executed-source/harness audit and a prepared E2E successor, not a
browser pass. No Chromium, application service or database was launched here.
The existing current-source hosted run remains pending.

## Confirmed weak assertion repaired

The native `local-library` journey moved from chapter page 1 to URL page 2 but
accepted any saved page `>=1`. A stale first-page record could satisfy that test.
It now requires the exact chapter/title, page 2 and `completed === false`, and
reports the observed values. Its actual callback is executed by six finite
positive/negative cases; the unchanged old callback is retained as a red control.

## Prepared real-control additions

- `account`, in `ordinary-a`: after actual registration reaches its welcome,
  create a synthetic session-key draft, check concealment/backup gating, copy
  both keys and compare the clipboard with actual displayed fields in memory.
  Wait for the actual copy acknowledgement, hide/discard the draft, clear the
  synthetic clipboard and remove attempted context permissions. Keys never
  enter returned observations. Final error redactions retain synthetic secrets.
- `local-library`, in `ordinary-b`: two real pages in one fresh context observe
  bidirectional status changes without reloading. A status change while both
  pages are already mounted offline must persist locally and reach the second
  page. Online state, sibling ownership and original status are restored. No
  account/protocol mutation or public outbox job may result.
- The same native reader journey now verifies actual last-page completion and
  the title's `Continue Reading` control. Two real English fixture chapters on
  one title must lead to the exact next unread chapter's page 1, leaving that
  chapter incomplete and preserving the previous completion time.

The helper fixtures execute these drivers with explicit DOM/clipboard/HTTP/
storage stand-ins and negative cases. This proves driver contracts and cleanup
sequencing only. It does not prove actual selectors, clipboard implementation,
IndexedDB/BroadcastChannel behavior, reader geometry or browser lifetimes.
Existing original assertions, fixed diagnostic inventories, phase count and
phase deadlines are retained. Product components and canonical frontend bytes
are unchanged.

Independent review found the preceding library step leaves `/library`, whereas
the continuity helper requires the title page. The next step now awaits an
explicit title navigation. A composed callback regression retains that actual
predecessor route and reproduced the refusal before the correction. Empty
unexpected private input/clipboard values also refuse without registering an
empty error-redaction token. Earlier finite receipts remain historical; the
separate review-correction receipt pins the successor and its added cases.

## Existing coverage and remaining boundaries

- Signup/verification/Finish later already has genuine-control specifications.
  The new Generate/Copy branch belongs to `account`; the isolated public-upload
  batch does not execute it. Admission/enrollment and activation of a generated
  key remain separate from copying/discarding a draft.
- `public-upload` is the separate gate-on UUID profile: actual imported-key UI,
  upload/cancel/reorder/remove/replace/reload/reconnect/private previews and
  commit/original-byte checks. It still requires its first genuine current run.
  Its receipt may be accepted-pending-projection; no public readability is
  invented from an upload acknowledgement.
- The native `filters` specification tests search, rating/order and reload.
  Native-profile success cannot qualify UUID catalogue filtering or every new
  profile/settings/title control. Those require a separate gate-on browser
  journey and current artifact association.
- Existing `pwa` verifies real worker registration and refusal of poisoned
  private/authority cache fallback. The new offline case is an **already-mounted
  local device write**. Neither establishes offline app bootstrap, offline
  catalogue/reader-image availability or a bulk offline download feature.
- Two-tab status reflection does not establish simultaneous conflicting writes,
  background-tab timer throttling, mobile browser behavior or storage eviction.
- Completed-chapter continuation is distinct from resuming a partially read
  chapter at a saved page. The latter has no explicit current browser assertion
  in this successor and must not be inferred from `Continue Reading` text.

## Hosted scheduling boundary

The published baseline's push to either admitted CI branch selects `all`.
Manual dispatch requires the workflow on the repository default branch, where
the read-only check found it absent. The separately reviewed integrator proposal
changes push defaults to one `public-upload` batch and explicitly enables the
Chromium sandbox. It does not include these ordinary account/library additions
unless they are separately composed and qualified. No publication or CI trigger
was performed by this audit.
