# Signed public chapter writes

This additive recovery implementation uses the selected-instance public upload
flag and existing exact-byte NIP-98 transport. Original frontend attachment files
remain unchanged. The chapter edit route now loads a dedicated signed editor;
it never promotes a cookie into public write authority or falls back to native
catalogue endpoints.

## Interface

- PUT `/api/v1/chapter/{publicUuid}` accepts a positive `version`, optional string
  `title`, `volume`, `chapter`, `translatedLanguage`, and up to ten distinct group
  UUIDs. Unknown fields, page mutations, malformed numbers, unsupported languages
  and native IDs are rejected. Successful updates return an actual chapter entity
  envelope with current version and included local relationships.
- POST `/api/v1/chapter/{publicUuid}/claim` has an empty body. Existing local group
  leader rules apply. Claiming local ownership grants no original signing key or
  managed-group permission. It returns the actual updated chapter entity.
- DELETE `/api/v1/chapter/{publicUuid}` has an empty body. An authorized ordinary
  uploader/group leader receives202 with a durable private instance moderation
  report UUID. The chapter stays visible and its version/pages remain unchanged.
  Identical pending requests reuse that report; a stale pending target or hourly
  report limit is refused before any original signing operation.
- An actual moderator/staff/sysop with `chapter.remove` can explicitly approve a
  local soft hide through DELETE and receives204 with
  `x-weebdex-instance-removal: soft-hidden`. The UI explicitly confirms this
  immediate effect. Native moderator report acceptance with `report.update` and
  `chapter.remove` also applies the hide, only if the reviewed chapter version
  still matches. Rejection never removes the chapter.

All writes recheck original admission, explicit account binding, active verified
native account grants and catalogue/group eligibility in their write transaction.
Metadata edits are instance annotations; source events, signed metadata and image
pages remain immutable. Deletion retains original/derivative objects.

## Independent original review

The ordinary local request commits first. A fresh transaction then rechecks the
same admitted capability/binding/grants before attempting the existing durable
original deletion request. Original signing is optional for local review: a local
claimant without the original key still queues a valid local removal request.
The response reports actual `sourceDeletionStatus`:

- `instance-local-only`: no source request applies
- `moderator-review-requested`: a real signed kind5 request was submitted; its
  exact event ID is returned in `sourceDeletionRequestId` and the response header
- `not-authorized`: original signing authority was refused before intent creation
- `unconfirmed`: local review is durable but the source request outcome could not
  be confirmed; no source approval or automatic retry is implied

Original moderator approval and peer-copy removal are independent policies.
The old cookie-native DELETE retains its previously documented immediate local
soft-hide behavior after original submission; this patch changes the new public
interface to review-first. Migration97 reserves a disabled operation-only report
reason: it appears in queued reports for reviewers, but generic reason discovery
and POST/report cannot create versionless removal requests.

## Verification boundaries

See `test-evidence/public-api/chapter-writes/result.json` for exact source hashes,
commands and current results. The signed HTTP fixture uses disposable PostgreSQL,
actual TCP, real NIP-98 proofs, the original publication runtime and pinned strfry.
It proves source-owner requests remain visible pending moderation, source bytes
are unchanged by annotations, local claims do not confer source signing authority,
and stale/rate refusals have no original resubmission side effect.

Fake transport and source-overlay tests are separately identified. Strict/Svelte
compilation is not rendered browser acceptance. Real browser execution of the
new editor remains unverified; existing published image revision is unavailable.
