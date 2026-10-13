# Catalogue and signed-source semantics

Public discovery defaults to `safe`, `suggestive`, and `erotica` when no
`contentRating` parameter is supplied. Explicit `pornographic` selection is
supported. The additive frontend overlay emits the sole value `__none__` when
all four ratings are unchecked; it means an empty result set. Mixing the
sentinel with other rating values returns 400. Followed-title library queries
do not impose discovery defaults. Direct title detail and chapter-list reads
also do not discard a known title merely because of its rating.

The retained reader consumes aggregate entries in descending volume/chapter
order. Numeric prefixes sort numerically, then textual suffixes and stable
identifiers break ties. Default title chapter lists use the same descending
chapter convention; global chapter feeds default to publication time.

The frontend's unpaginated tag bootstrap receives the complete taxonomy, bounded
at 10,000 entries. Exceeding that budget is an explicit service error rather than
a silently truncated filter list. Explicitly paginated tag requests retain the
normal limit and offset contract.

## Immutable protocol chapters and local editorial annotations

An imported chapter's signed event identity, publisher, original source
metadata, page descriptors, and original bytes remain owned by the original
protocol module. API metadata updates only change the instance display fields
`title`, `volume`, `chapter`, `language`, and group attribution. The DTO records
these annotations in `instance_overlay` with `scope: instance-local` and the
editing account; the catalogue audit records `overlay_update`. These are not
signed-event revisions or protocol replacements and are not federated.

Beginning a page-replacement edit for a protocol-backed chapter returns a
conflict. Changing its signed content requires the protocol's explicit deletion
and new-upload workflow. For a source-linked chapter, an authorized native DELETE
first submits an actual original signed deletion request using an explicitly bound
source identity and configured runtime. Original moderator approval remains required.
The native transaction records the real request ID and hides the instance chapter;
it does not approve source deletion or remove retained peer copies. Response headers
distinguish `moderator-review-requested` from an unassociated `instance-local-only`
deletion. Signing/configuration failures are explicit. No automatic relation to a
replacement is created.

Normal native uploads report `publication_scope: instance-local` and
`publication_status: local`. Only a successful explicitly configured original
publication runtime can supply the signed `original-protocol` status. That
runtime requires a pre-admitted publisher and an existing operator-controlled
signer/store; an account, group attribution, or local upload alone grants no
signing authority.

Once a native upload has a durable prepared or accepted source command, file edits,
removal and cancellation return a conflict. Cleanup preserves its referenced inputs.
Only the identical owner-scoped commit can retry, including after ordinary session
expiry; its typed payload hash, original inputs, session identity and current source
authority remain validated. Removing publication configuration returns unavailable
rather than silently creating a second instance-local chapter.

## Private staging and publication receipts

`GET /api/upload` returns the current owner's complete resumable session, ordered
file descriptors, private `draft` and `page_order`, version/expiry, mutation/cancel/
retry eligibility and current receipt. A browser session is required. Prepared or
accepted original commands retain their exact typed commit payload and files even
after ordinary expiry. Their owner can privately preview those retained originals;
another account, a library bearer or an anonymous caller cannot.

`PATCH /api/upload/{id}` accepts only `{version,draft,page_order}` and saves a
bounded private form independently of chapter publication. Its order must contain
every current staged file exactly once; stale versions conflict. Upload/removal
changes the session version, so callers refresh the current session before saving.
An immutable original command rejects form changes and cancellation. A committed
retry returns only a freshly authorized visible DTO or a genuinely pending,
currently source-authorized minimal owner receipt. Local tombstones, source
withdrawal or authority withdrawal never replay an old full DTO.

Native draft `Publish` returns200 with the actual version and publication scope.
When configured, it invokes the admitted, explicitly bound original signer and
durably snapshots canonical source input and original object references. Prepared
and accepted title commands reject metadata/cover/relation/state/deletion changes;
identical publication retry and exact-version rollback recovery remain possible.
Unrelated author/tag edits do not change a saved signed command. Once materialized,
native title/chapter editorial fields are explicit `instance_overlay` annotations,
not new signed revisions. Source event identities, signed metadata and originals
remain immutable.

Pending title receipt reload on private `GET /api/manga/draft/{id}` is restricted
to the actual publication-command actor with current original signer admission.
Owning the native draft or holding a staff role does not confer that source receipt
capability. It returns only native id/title/version/state and verified publication
status/identity, never historical full relationships. Public reads remain unavailable
until actual canonical projection is visible.

## Local device-library hydration

Public manga search accepts bounded repeated `id` filters; chapter filters use `id`
for chapter identities and repeated `manga` for parents. Read-only `_sort_value`
(and chapter `_sort_text`) reflect the actual SQL ordering key so a device adapter
can merge multiple bounded batches accurately. Null values sort last.

`POST /api/manga/import-mappings` maps1–400 explicit decimal MyAnimeList IDs to
current eligible publications. It returns real `{myanimelist,id,title}` records plus
`missing` and `ambiguous` IDs; an ambiguous match never invents a title association.
This read-only public lookup grants no follow, upload, signer or account authority.

## Visibility and revocation

All source-linked reads evaluate `platform.protocol_source_is_visible` using
the canonical policy generated by the original protocol facade. Platform-only
installations fail closed for non-null source identities. Original-schema
verification and policy installation occur before serving requests or starting
the importer. Importer flags are derived hints, not authority.

Root SQL predicates, nested DTO projections, library/history/export reads,
discussion/report eligibility, statistics, previews, and media eligibility use
this policy. Private/API responses are `no-store`; mutable source-derived tag
responses do not use the public response cache. Public media must revalidate
before receiving a fresh response or a 304. Previously downloaded bytes cannot
be recalled from a user's device.

Group registration does not grant ownership. A persisted, authorized group
claim review or deliberate staff membership edit is required to grant a leader.
All editorial changes use versions, transactions, validated relationships and
persisted audit records.

## Atomic staged-page replacement and abandoned sessions

`POST /api/upload/{session}/files/{file}/replace` accepts one multipart `file` and
one positive `X-WeebDex-Upload-Version`. It requires the current verified browser
owner, the existing page, an open unexpired session, and no prepared original
publication. Validation and staging finish before the locked transaction swaps
original/derivative references. The page ID and position remain stable, including
at the 500-file limit. The JSON response is the actual file DTO plus session
`version`; a byte-and-filename-identical retry does not increment it. A divergent
stale version returns 409 and leaves the current page intact. Interrupted input,
invalid content and failed transactions cannot delete the previous valid page.

Startup and the existing maintenance worker expire at most eight ordinary
abandoned sessions per batch through the partial expiry index and `FOR UPDATE
SKIP LOCKED`. Active session transactions, prepared/source-accepted commands and
pinned originals are preserved. Object deletion and cleanup-journal insertion
share the expiration transaction; at most 100 unreferenced objects and 100 queued
files are processed per maintenance pass. A file in an unfinished intake stream
is not attached without rechecking current locked session eligibility.

Public media acquires one platform request/read lease before its first protocol
or local database selection and retains that same lease through body completion
or cancellation. Falling back to local derivatives does not acquire a second
platform lease. Original-module snapshot/download ownership remains independent.

## Native staging crash/restart durability

Before an original is renamed into its final object path, and before a derivative
is written there, a non-sensitive key-only pending intent is written and synced.
The pending directory entry and newly created ancestry are synced before the
object can appear. The source file, destination object directory, and temporary
source directory are synced before a transaction may commit its file reference.
An OS file-lock lease on the intent prevents reconciliation from deleting a live
staged object, even after a long async suspension. A failed or canceled write
keeps its intent for recovery rather than relying only on destructor unlink.

Startup/maintenance inspects at most 1000 pending entries and processes at most
100 eligible intents per pass. After a five-minute conservative age threshold,
it must acquire the intent lease and the existing PostgreSQL object-key advisory
lock. A current persisted object reference retains the bytes. An unreferenced
object is unlinked and its directory synced before the marker is cleared; queued
media cleanup follows the same unlink/directory-sync/ledger-clear ordering.

Raw intake and discussion-proxy temporary files are scanned conservatively after
24 hours, bounded to 1000 inspections/100 removals per directory. Their held OS
leases preserve active intake, decode and response bodies. Proxy bytes never
become catalogue/protocol media merely because a file exists.

This contract requires a local filesystem honoring file/directory fsync and OS
file locks, plus durable PostgreSQL commits. Source and synthetic restart-window
fixtures do not establish power-loss behavior; actual crash/durability acceptance
must be recorded separately. Preexisting unjournaled artifacts from older
binaries are not silently deleted by guessing that they are unreferenced.
# Relation edits

Relations are instance-local catalogue annotations. They never modify a signed
source event. POST `/api/manga/{id}/relations` creates a relation and returns201;
an existing `(owner,related)` pair still conflicts409, including a different type.
PUT at that same path edits an existing pair and returns200. Its payload is
`{related_id,type,version}`, where `version` is the current **owner manga** version.
Missing relation is404 and stale owner version is409. A same-type PUT is a
successful no-op and leaves the version unchanged.

POST accepts an optional owner version for compatibility with the retained
request. Both successful mutation bodies are the real Relation projection with
`manga_version` (current owner version) and `relation_scope:"instance-local"`.
The ordinary Relation `version` remains the **related title** version and must
not be used as the owner precondition. DELETE accepts optional owner version,
remains204, and supplies the real updated owner version in
`x-weebdex-manga-version`. Current browser permission, title locks, canonical
source eligibility and immutable prepared-publication fences apply throughout.

## Cover extension contract

Every public Cover projection uses a dotted `ext` suffix such as `.png`, `.jpg`
or `.webp`. This matches the retained original/lightbox/editor URL expression
`/covers/{manga_id}/{cover.id}{cover.ext}`. The same projection serves cover
upload/update/get/list responses and nested manga relationships for native and
protocol-imported covers. The stored catalogue and media-object extensions stay
bare; thumbnail URLs remain `/covers/{manga_id}/{cover.id}.256.webp` and
`.512.webp`. Migration85 applies only this external DTO adaptation and preserves
the final82 canonical source, parent and tombstone checks.

Current source-bound HTTP fixtures construct the original URL from the returned
DTO, verify real bytes and media behavior, and test canonical withdrawal before
conditional media responses. They have been compiled and codegenerated; their
runtime execution remains pending the independent implementation gate.

