# Public report contract

Recovery status: RECONSTRUCTED-UNVERIFIED after the executor replacement on 2026-10-15. The original implementation was lost before upload. This document describes the reconstructed intended behavior; fresh verification is required before treating it as confirmed.

Source: MangaDex OpenAPI 5.13.1, the pinned source and hash in `mangadex-operation-matrix-v1.json`.

The reconstructed instance implementation provides these report operations under `/api/v1`:

- `GET /report/reasons/{category}` returns UUID report reasons with localized `reason`, `detailsRequired`, `category` and a persisted positive `version`, inside a collection envelope.
- `GET /report` returns a collection of reports. Supported filters are `category`, `reasonId`, `objectId` and `status`; ordering is `order[createdAt]=asc|desc`; `includes[]=user|reason` expands only those relationship attributes. The default limit is 10, the range is 1–100, and offset is bounded at 10,000.
- `POST /report` accepts an application/json object containing `category`, `reason` (UUID), `objectId` (UUID), and optional `details`, with an 8 KiB body limit. Optional instance extensions `expectedAccountId` (captured canonical bound-user UUID) and `expectedReasonVersion` (positive integer) fence the current binding and the reason revision reviewed by the user. An explicit null, wrong scalar type, or duplicate extension field is rejected; omission preserves older client compatibility. Success is HTTP 201 with `{"result":"ok"}`.

The public categories are `manga`, `chapter`, `scanlation_group`, `user` and `author`. The native-only `thread` and `reply` categories remain available through the existing native interface. Statuses are `waiting`, `accepted`, `refused` and `autoresolved`. Native short IDs are never accepted or emitted by the public report transport. Existing aliases are accepted, and output uses their durable preferred UUIDs. Reason identities use the separate `report_reason` namespace, so a manga, user or report UUID cannot substitute for a reason.

## Authority and privacy

These three operations inherit authentication in the upstream spec. Here, each requires a fresh original-module Nostr request capability bound to the exact configured service, URL, method and body, plus an active, verified native account with an explicit operator-managed Nostr binding. Original admission, binding and account state are rechecked and locked in the same transaction as the protected operation. A public UUID, cookie, native bearer token, or admitted but unbound Nostr key cannot create or read a private report.

Creation also requires the native `report.create` permission. The shared native mutation core retains reason/category matching, required details, current source visibility, target existence, group-claim safeguards, per-user serialization, a limit of 20 reports per hour, waiting-duplicate rejection, and an atomic create audit record. Duplicate and rate limits retain the native 409 and 429 errors. A POST cannot choose another submitter, status, version or review outcome.

Ordinary users see only their own reports. The existing instance staff queue remains available only to an account that has both a native moderator role and `report.update`; either alone is insufficient. Soft-deleted reports are excluded. The public response contains report details, canonical object UUID, status and creation timestamp. It does not expose internal IDs, moderator notes, audit records, account contact details or target content. A report's private history can retain a target UUID after withdrawal, but cannot expand or reveal the withdrawn target. The `user` expansion is the submitter's public username, roles and version, and `reason` expands the reason attributes.

Reports, reason administration and review decisions are local instance moderation data. They are not signed relay events, do not claim to submit to MangaDex, and do not grant protocol or global moderation authority. No automatic announcement is sent.

## Migration and verification required

Migration 92 adds durable reason aliases, backfills existing reasons set-wise, creates aliases for future native inserts, and increments a reason's version on label, details requirement or enabled-state changes. Identity/category changes are rejected. Existing migrations and native keys remain unchanged.

`backend/tests/public_reports.rs` reconstructs the actual TCP HTTP fixture using an owned disposable PostgreSQL instance, with synthetic keys and accounts only. It covers genuine signed requests, missing/denied admission and bindings, wrong service/URL, replay, payload tampering, native-cookie non-authority, permissions and ownership, aliases and wrong-kind rejection, strict fields and body/query bounds, pagination/ordering/expansions, versioning, hidden/deleted targets, native audit, concurrent duplicate serialization, rate guards, additive migration and immutable alias behavior. The unchanged native `community_http` suite must also be run to recheck existing report/review/group-claim behavior.

## Signed UI integration, 2026-10-22

The generic report dialog's public catalogue subjects now use the signed public transport, never the native short-ID endpoint. The UI explicitly loads actual reasons and always signs both optimistic extensions. The backend locks the reason row through its version comparison and report insertion. A changed reason produces409 without an insertion; the UI discards the old selection and requires fresh review. No automatic retry or success is inferred from an uncertain response. Pending outcome markers survive closing/remounting and contain no report explanation.

Local controller, mounted owner-component, strict Rust unit, and composed semantic checks are recorded in `test-evidence/report-draft-repair-20261022/qualification.json`. Real HTTP test binaries compile, including the stale-reason counterfactual. Live database/HTTP execution and final integrated production build remain separate qualification, not implied by these local results.

## Personal inspection extension, 2026-10-22

The pinned5.13.1 GET/report parameters have no submitter/owner filter. The optional
instance extension `scope=mine` narrows the existing signed listing to the actual
transaction-validated bound account, including for moderators with report.update.
No client-supplied owner ID is accepted. Omitted scope retains the existing
owner/moderator queue behavior; empty, repeated, unknown and identity-shaped scope
values are rejected. This is read narrowing, not a permission grant or a write.
The existing category/status/UUID/order/include and offset bounds still apply.

The additive `/reports/signed` route is an explicit signed personal-report viewer.
It is outside the native `/my` cookie gate. It loads only after a user action,
requests scope=mine and the captured expectedAccountId with ten rows and descending creation time, and supports the
four current statuses, public report categories, exact subject UUID and pages1
through1001 (offset0 through10000). Selecting a filter starts again on page1.
The row/collection contracts, requested filters, canonical UUIDs, submitter,
ordering, reason revision and bounds are checked before rendering. Ten rows allow
valid native-created catalogue report details up to20,000 Unicode scalar values
per row within the existing signed transport's1MiB response budget. The public
submission budget is still8KiB; it was not increased.

Every read captures the current signer generation, service identity, catalogue,
destination, bound account and route generation. Cancellation/disposal aborts its
transport signal, and both account-context checks and the data read use it.
The optional expectedAccountId query extension is always supplied by this viewer.
Under the existing bound-account read transaction locks, the server compares it
to the actual actor’s preferred public user UUID before counting or selecting
private reports; a mismatch returns409 without rows, even for an otherwise empty
filter/page. It is a stale-account fence, not a client-selected owner or permission
grant. Omitting it preserves existing API behavior, including moderator queues.
Malformed/repeated IDs are rejected.
Account/service validation is repeated after the read even for an empty page.
Late, cancelled, switched-identity, changed-service and rebound-account responses
cannot repopulate the view. Nothing is persisted as a browser-history feed.

The unconfirmed submission warning links directly to the same subject's signed
inspection filter. Inspection cannot determine the original write's outcome from
a similar row or absence from a page. It never deletes a pending-write marker,
changes the report submission controller, or replays a write. No override/retry
control is provided; unresolved outcomes require operator reconciliation. The
existing exact201 success, explicit4xx refusal, unsent/uncertain distinction,
reason-version fence and duplicate/rate criteria are unchanged.

Native-session reports remain at `/my/reports`, with a clearly labelled link from
the signed view and form. Native thread/reply report dialogs remain native and
are neither requested through the public categories nor presented as UUID
reports. Report reason labels/revisions in the listing describe the current
reason, not a fabricated historical submission snapshot.

Finite controller and source-mounted Svelte/JSDOM coverage is recorded in the
signed-report-inspection evidence directory. Primitive visual components and
request transport in those tests are finite stand-ins. The prospective actual
browser report case also follows the new inspection link and status filter,
checks signed private scope and denies native fallback; it has not been run.

## Separate signed native report history

The additive `/reports/signed/native` view now provides signer-bound personal
thread/reply report inspection without the `/my` browser-cookie gate. It uses
separate native-ID endpoints, not widened public UUID report categories. See
[native report inspection](native-report-inspection.md) for its mandatory account
fence, owner-only authority, bounded projection and qualification status. The
existing `/my/reports` interface remains the explicitly separate native-session
mode; neither interface silently switches to the other's actor.
