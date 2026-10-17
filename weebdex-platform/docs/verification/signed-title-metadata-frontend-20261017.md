# Scoped signed title metadata frontend

This is a dedicated public-gate interface, not activation of the native editor's
chained cover/relation workflow. `/create/title` renders the signed metadata form.
`/title/{uuid}/edit` reads an actual signed account-authorized canonical title,
including a protected draft when public detail returns404. It never fabricates a
placeholder title or borrows the native cookie Session/Permission result.

The shared create wrapper bypasses its native cookie gate only for the exact title
route; group creation retains its native protection. Five original route sources
have hash-pinned script-only changes. Their retained markup stays unchanged,
while the three new external components are explicitly recorded as intentional
rendered extensions. No global rendered-HTML parity is claimed. Gate0 behavior
remains the original native interface.

## Authority and receipt contract

The controller uses the existing selected signer, service/catalogue capabilities,
service proof and exact-byte NIP-98 transport. It checks the actual native account
context, and rechecks signer-to-account binding at the final pre-dispatch boundary.
Cookies never become public command authority. The backend's current grants,
ownership, source and optimistic-version rules remain authoritative.

Create200 must return draft/version1. Update200 and submit201/publish200/reject200
must return the exact canonical UUID, expected state and one version increment.
A different read UUID needs exact typed server alias proof before becoming a write
target. The form consumes these actual wire statuses directly. A single-language
edit preserves all other actual title/description translations; maps are not
silently truncated. Original-language form fields never substitute an English
fallback as if it were an original-language translation.

Covers, relations, officialLinks, primaryCover, delete and lock/unlock inputs are
rejected before metadata mutation or are absent from the form. Submission is
disabled without an included same-language cover and explains the native cover
prerequisite. The server still checks original cover bytes and visibility. Draft
submission is review-only. An accepted original publication waiting for projection
is displayed as pending, without claiming public detail or reader availability.
Published metadata edits are described as instance-local annotations.

A per-signer/service/catalogue/target sessionStorage marker is persisted before
mutation signing/dispatch. Unknown outcomes, server failures, malformed success
receipts and late context changes keep writes fenced. No automatic mutation retry
exists. A transport-proven unsent request or a known4xx refusal can release its own
marker. Read-only reload does not pretend to reconcile an unknown write. The fence
survives route owners and reloads in that tab; browser-restart persistence or full
unknown-create recovery is not claimed. Private draft listing remains separate.

## Verification

`test-evidence/frontend/signed-title-metadata-20261017/result.json` preserves exact
source hashes, raw finite logs, failed initial checks and compressed byte-preserved
compiler/markup manifests. Final qualification:
-425 focused finite checks pass, including46 new title controller/composed-loader
 cases. These are stand-ins/source seams, not feature or live HTTP counts.
-Actual composed strict gate1:0 errors/12 warnings; gate0:0 errors/15 warnings.
 Both preserve679 canonical frontend files. No diagnostics were suppressed.
-Exact production integration preparation passes and declares all three new
 external rendered components. The anchor generator reproduces the complete
 current manifest without writing source.

The first strict attempt reported four type errors, subsequently repaired. The
first inventory regression also caught five new paths requiring exact association
with the historical25-path manifest; prior assertions were retained. Dependencies
were reused from verified local inputs without installation or download. No
browser or live HTTP runtime was executed by this frontend qualification. A fresh
production Vite build of this source is still separate from these compiler checks.

## Later owned build and independent HTTP check

The actual gate-on production Vite build of source `42424932f62c9c4a3becaa9d68d79fee31c39426`
passed in50.188seconds with434 files/7,114,978bytes. All source,679 canonical
frontend files and39,226 dependency pins remained unchanged. The owned runner
closed normally with0 survivors; no dependency download or browser was used.
Full manifests and byte-preserved compressed build output are in
`test-evidence/frontend/signed-title-production-build-20261017/`. This does not
claim a production build of later application-source successors.

An independent real HTTP rerun of backend source `17dba0827c8e87a815c8bdc88b47008f3bb0eea3`
also passed its one selected signed title create/edit/submit/publish/local-overlay
case. All436 pinned backend/original inputs were unchanged; its fresh owned
PostgreSQL guard reports normal stopped cleanup, a closed port and no survivors,
unknown births or errors. See
`test-evidence/public-api/title-independent-review-20261017/review-result.json`.
This backend journey and the frontend finite/compiler/build checks are separate
evidence. They are not a rendered end-to-end browser journey.

## Pending-publication control guard

A small application-source successor additionally keeps a valid
`accepted_pending_projection` receipt read-only in both the controller and form.
Only a fresh successful server read observing actual projection re-enables edits;
it never replays the original publish. This is separate from an unknown-request
fence. Its one added finite case brings the focused suite to426 passes, including
47 title cases; actual gate1 composed strict remains0 errors/12 warnings with679
canonical files unchanged. See
`test-evidence/frontend/signed-title-pending-projection-20261017/result.json`.
No production Vite build or rendered browser run of this successor is claimed.
