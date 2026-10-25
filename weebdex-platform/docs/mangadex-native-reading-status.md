# Bound native-account reading status

Qualified on 2026-10-18 against production backend53130b4 and the test-only
runtime-prerequisite correction c96ead0. The exact compile/HTTP receipts are in
`test-evidence/native-status-combined-api-20261018/report.json`.

This adapter edits the real `platform.library_manga_follows.reading_status` for
an already-followed title. It cannot create/delete membership, alter notification,
privacy or followedAt, change follow counters, sign a protocol event or modify a
device library. Native cookie updates and imports continue to use their existing
cores and are observed by the same status revision trigger.

## Separate signed preview

`GET /api/v1/nostr/manga/{UUID}/reading-status?expectedAccountId={UserUUID}`
requires a fresh request-bound Nostr capability and an explicitly bound active,
verified native user with `self.preferences.update`. Cookies alone do not grant
this public authority. Exactly one expectedAccountId is required; no other query
or GET body is accepted. A changed binding returns409 inside the same transaction.
Only actual native, currently visible published Manga targets qualify. Resource
IDs are canonical typed persisted UUIDs; reads do not create aliases.

The response is `{result:'ok',status:string|null,nostr:{scope:'instance-account',
source:'persisted-native-account-reading-status',accountId,mangaId,followed,
version,updatedAt,versionSemantics:'reading-status-change-revision'}}`.

The six status values are reading, plan_to_read, completed, on_hold, re_reading,
and dropped. Version is an actual independent integer revision,0..2147483647.
No persisted status observation has followed=false/status=null/version=0/updatedAt=null;
this is not a claim about unknowable pre-migration history. Removed
membership retains a positive revision and its actual update time, with
followed=false/status=null. Active membership has a positive version and status.
No followedAt field is added to this minimum DTO. updatedAt is UTC with six
fractional digits; display time is never the CAS guard.

## Explicit native write

`POST /api/v1/manga/{UUID}/status` accepts only JSON up to1024bytes:
`{status,expectedAccountId,expectedMangaId,version}`. All four fields are required;
unknown/duplicate fields, malformed UUIDs, null, unsupported statuses and unsafe
integer versions fail closed. expectedMangaId must match the actual canonical
resolved Manga, even when the signed request path used a genuine imported alias.

The account binding, current grants/active policy and visible target are checked
inside the final transaction, which holds the user/binding and title locks. The
follow row and genuine status revision are compared before changing only status
and its source update time. Stale versions/rebinds/target mismatch return409;
missing/invisible targets404; absent membership409 with an explicit account
Follow prerequisite. Same-status writes are stable no-ops preserving revision
and times. Successful receipts have the preview identity/provenance and add
`nostr.changed`; known-version confirmations must agree with captured intent.

Null clear is deliberately unsupported in this scoped operation: existing
native follows require a nonnull status. It does not secretly unfollow or invent
a replacement status. Intentional Account Unfollow remains a separate command.
This is partial upstream POST support; the official null-clear semantics are not
claimed. Existing `GET /manga/status` and `GET /manga/{UUID}/status` still project
original signed public-library state, never this private account state.

## Migration and concurrency

Migration0102 takes the follow-table migration lock, baselines existing real
observations at revision1 using their stored source updatedAt (including known
membership tombstones, without inventing their former status), then observes all
INSERT/DELETE and actual reading_status UPDATE operations. Deletes retain
revision tombstones; delete/reinsert and status ABA cannot reset the CAS, even
with identical source timestamps. A same-status native metadata update does not
advance status revision; membership0100 versions remain independent. Exhaustion
fails closed and database errors roll the whole status transaction back.

Both routes, including refusal/error paths, use private,no-store and Authorization
variation at the outer response boundary. The preview also uses the private-read
admission class. The default device Save and original signed publication flows
remain separate.

## Qualification

`backend/tests/public_native_reading_status.rs` is the isolated actual TCP/
PostgreSQL fixture covering migration baseline, account policy/isolation/rebind,
canonical target capture, native cookie/import invalidation, same-clock status
changes, remove/readd tombstones, stable no-op, preserved follow fields/counters,
concurrent CAS, rollback, replay and original-library read separation. Its actual
TCP/PostgreSQL run passed1/1; the same frozen source passed139 library tests
(2ignored), all seven baseline API HTTP groups and the113-operation routing probe.
All eight database guards physically stopped with closed ports, no forced cleanup,
survivors, unknown process births or cleanup errors.

The first run exposed an incorrect test prerequisite: this native-only fixture
does not attach an original PublicationRuntime. It now asserts the actual safe503
original GET baseline both before and after native changes, never an invented
empty signed graph. Genuine original signed-graph200 status reads require the
separate `public_nostr_statuses` runtime fixture and its own final-source receipt.
Other feature-lane runtime suites and browser/load qualification remain separate.
