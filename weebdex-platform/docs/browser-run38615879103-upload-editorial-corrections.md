# Run38615879103 upload/editorial corrections

Baseline: local `2d9c80135328796856c789050fcd81b0093f4661`, tree
`83141fce1dba67dbd529cc1da53b065bba672133` (public `e940298`). Verified
`browser-e1ea103-corrective-result-38615879103/phases.json` records fully passing
local-library checks, the first upload failures at `failed-page-controls` and
`stage-metadata`, dependent upload prerequisites failing, and editorial steps1–3
failing without sub-checks. Runners finalized; no fatal/page errors were exported.
That evidence does not contain an editorial native error or prove a new hosted pass.

## Proven causes and narrow corrections

1. Actual `createPage` returned a plain object. `processFiles` retains those objects
   in its queue, then inserts them into the parent's deep `$state` array. Rendering
   reads the proxied rows before the awaited batch resolves. Later raw-object error
   mutations do not invalidate the proxy's cached properties; `pages = pages` does
   not repair that. The retained entry shows Replace/Remove only for `!page.file ||
   page.error`, so the failure toast can be visible while item controls remain absent.
   The script-only overlay now creates the candidate with `$state` before either
   queue or array receives it. The existing `UploadSessionFile` cast and placeholder
   fields are retained exactly. No fake remote hashes/IDs, mutation replay, owner,
   staging/revision, session, lease, decoder or mutex boundary changes were made.
2. Lost-begin recovery restores the empty draft and derives `isOneshot` true from
   absent volume/chapter. Both numbered native inputs are disabled in that mode.
   The journey intending Volume1/Chapter7 now explicitly clears the actual Oneshot
   control before filling them. Restore semantics and product controls are unchanged.
3. Actual composed title creation and native `manga_create` require/return201, but
   the ordinary journey required200 before assigning the new title. It now requires
   exactly201. All other transition acknowledgements remain their original statuses.
4. `/cover?manga=...` is not a native GET route. The actual editor reads draft covers
   at `/manga/{id}/covers?draft=true`, with browser owner/staff authorization. The
   journey uses that route and retains its200 assertion, adding an exact nonempty
   original PNG identity/dimensions/version check against the private draft's cover. Both IDs must independently satisfy the native
   ten-ASCII-alphanumeric contract; version must be a positive safe integer, so
   two absent IDs or a coerced/fractional version cannot fake confirmation.

All98 original assertion predicates/messages remain in the same order, with only
that explicit title200→201 contract correction. The historical fingerprint test
normalizes that one predicate only after requiring its precise new value. Genuine
file inputs, real mutations, publication receipt checks, duplicate/version guards,
original polling deadlines, cancellation checks and expected step counts remain.
No broad status alternatives, sleeps or failed-check suppression were introduced.

## Bounded diagnosis and source audit

Upload retains all29 original IDs and adds six fixed boundaries: removal feedback,
private-stage retention, volume, chapter, language trigger and language option.
Editorial now has23 fixed IDs covering registration, each native create control,
acknowledgement, private title/cover reads and each transition/version. Node exports,
Python inventory and fail-closed projection agree. Only fixed IDs/statuses export;
no raw exception, title, account, endpoint, file, cookie or response data is added.

The ordinary upload/editorial source audit checked placeholders, tabs, original
cover file input, contributor dialogs, actual Select popup topology, per-item entry
condition, Save/Revert/Edit Pages/Replace Pages, Submit/Reject/Publish/Lock/Unlock
controls, and native acknowledgement statuses. The separate read-only audit found
no further definite downstream editorial contract mismatch. Runtime-only failures
remain possible; the new finite checkpoints identify their reached boundary.

## Local validation and limits

Using existing verified frontend yarn-lock dependencies (Svelte5.55.1,
TypeScript5.9.3) and existing JSDOM27.0.1, with no installation:

- `test_upload_reactive_candidates.mjs`: ten actual Svelte client/compiler + real
  composed factory/chunk/session guards + real `upload-batch.ts` cases. The former
  raw factory reproduces zero controls after an awaited500; the reactive factory
  shares exact queue/render row identity and exposes truthful recovery after500,
  transport failure, and item rejection. Additional cases cover acknowledged remote
  identity, actual local rejected-candidate replacement success/failure, and held-POST
  owner expiry/staging switch/freeze refusing late receipts. Decoder, HTTP/owner
  services are finite stand-ins;
  the DOM mirrors the retained entry condition, not the full Bits/ImageLoader UI.
- `test_upload_file_admission.mjs`:37 cases pass, including actual retained Mutex,
  add/drop/replace admission, invalid files, concurrent/frozen gates, real batch
  deferred authority/hash reads, owner expiry and same-owner staging switches.
- `test_browser_upload_numbered_mode.mjs`: four cases execute actual composed
  restore and actual checked/stage callbacks against native DOM controls, including
  old disabled-field reproduction, already-numbered mode, and disabled-checkbox
  fail-closed behavior. Bindings are explicit stand-ins, not a full Bits mount.
- `test_browser_editorial_diagnostics.mjs`:46 cases execute the actual outer branch,
  recorder and owned event helper, including every checkpoint interruption, wrong
  create statuses and false cover200 receipts, including missing IDs on both sides
  and non-native IDs/string/fractional/infinite/unsafe versions. Controls/API/database are finite
  stand-ins; the actual retained PNG generator supplies the original image input.
- Upload diagnostics and original selector/assertion suites add six more cases:
  all103 strict Node cases pass. All13 Python diagnostic/gate cases pass.
- `verify-frontend-parity.py`:676 attachment hashes unchanged.
- Fresh actual composed strict projection:0 errors,15 pre-existing warnings,
  53 reachable integration files,679 canonical/checker inputs unchanged. The first
  local spelling exposed the existing partial-placeholder type distinction; the
  final rune wraps the existing cast rather than weakening types or fabricating
  remote metadata. No `any`, non-null or diagnostic suppression was added.

Focused command (set the two dependency roots to the already verified local trees):

```sh
WEEBDEX_FRONTEND_ROOT="$VERIFIED_FRONTEND" \
WEEBDEX_BROWSER_DOM_NODE_MODULES="$VERIFIED_DOM_NODE_MODULES" \
node --unhandled-rejections=strict --test \
 weebdex-platform/tests/ci/test_upload_file_admission.mjs \
 weebdex-platform/tests/ci/test_upload_reactive_candidates.mjs \
 weebdex-platform/tests/ci/test_browser_upload_numbered_mode.mjs \
 weebdex-platform/tests/ci/test_browser_editorial_diagnostics.mjs \
 weebdex-platform/tests/ci/test_browser_ordinary_selector_contracts.mjs \
 weebdex-platform/tests/ci/test_browser_upload_diagnostics.mjs
python -m unittest discover -s weebdex-platform/tests/ci -p test_journey_diagnostics.py
python weebdex-platform/scripts/verify-frontend-parity.py
```

These establish the exact source/engine seams, not hosted browser/backend/database,
image decode/layout or successful full upload/editorial acceptance. No public
publication, hosted run or external mutation was performed in this corrective work.
