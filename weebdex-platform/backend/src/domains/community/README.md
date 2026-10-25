# Local discussion and report integration

This domain implements the twelve original Thread API operations. Roots are
created by catalog publication and native account creation. Replies and votes
are PostgreSQL instance state; nothing here writes or mutates signed protocol
records or performs relay publication.

Replies are plain text/Markdown JSON. Creation requires the persisted
`reply.create` permission; editing/removal requires the corresponding self
permission for the author or an explicit moderator permission. Hard deletion
requires both moderator role and `reply.delete`. Numeric reply IDs are checked
against their thread on every operation. Edits require the current positive
version. Soft removal leaves a source-compatible tombstone without body/author.
Hard deletion compacts reply ordinals under the thread lock because the retained
frontend computes deep-link pagination using total replies and reply_number.
Votes are unique per voter/resource; changing and removing a vote are idempotent.

Reports support every source entity category and the source's category, object,
reason, status and order filters. Authenticated ordinary users only see their
own reports. Reviewing all reports requires moderator role plus `report.update`;
removal requires `report.delete`. Review status transitions are waiting to
accepted/refused and append audit provenance. Report creation returns 204,
matching the supplied ReportDialog.

The group claim dialog looks up the literal reason `Group claim request`.
Submission captures the unclaimed group's version and records the submitted
proof; it confers no ownership. Explicit acceptance requires both report review
and group-update permissions, a verified active claimant, an unlocked unclaimed
group and unchanged target version. It atomically grants that claimant a leader
membership, increments the group version and writes linked catalog/report
audits. Generic report acceptance never grants roles or membership. Changed or
already claimed groups return a conflict and leave the report waiting. Proof
URLs are for moderator review and are never automatically fetched.

Analytics records one event per visitor, entity and UTC hour bucket and updates
real catalog counters transactionally. Anonymous visitor IDs are random,
HttpOnly cookies; the database stores only hashes. A transport-peer cap of 120
submissions per minute limits abuse; forwarded-IP headers are not trusted.
Existing catalog ranking APIs use those real events for daily/weekly/monthly
windows. User points are the net persisted votes received on visible active
replies; this local formula is explicit, rather than fabricated imported
reputation. No original notification inbox, push or email API was present; the
source's title bell controls the library notification flag and chapter feeds.

Compilation and HTTP/PostgreSQL tests are separate gates. The comprehensive
community_http.rs scenario uses only an owned disposable loopback database and
synthetic accounts; it has not been run unless the progress/evidence receipts
say so. Original frontend HTML/CSS/TypeScript files were not edited by this
implementation.

## Signed native-user discussion adapter

`GET/POST /api/v1/nostr/user/{publicUuid}/discussion` is an explicit
instance-account adapter, not a publisher discussion or browser-session bridge.
It resolves only native public-user IDs and locks the actual active user's stored
thread relation. NIP-98 admission, service, bound native account and persisted
permissions are rechecked in the same transaction as every mutation. Cookie-only
requests confer no authority. Reads are private/no-store and contain the native
thread, a fixed 20-reply page, personal vote state and operation capabilities.

Commands capture canonical account/user UUIDs and the actual thread ID. Reply
changes require the current reply version; votes require the captured personal
vote, and reply votes also require the reply version. Duplicate, unknown, null
and irrelevant fields fail closed. Bodies use native Unicode trimming and the
actual persisted 50000-byte UTF-8 limit. Both authenticated native routes and this
adapter call `mutations.rs`, preserving moderator/owner permissions, tombstone
privacy, thread locking, dense reply ordinals and native audit provenance. Native
unvote/removal permissions remain unchanged, including clearing votes after a
thread is locked. Signed success does not authorize replay of an uncertain write.

The synthetic TCP test target is `public_native_user_discussion`; it must run in
the owned disposable PostgreSQL runner. Existing `community_http` remains the
native compatibility regression gate. Source-only checks do not establish that
either HTTP/PostgreSQL gate passed.
