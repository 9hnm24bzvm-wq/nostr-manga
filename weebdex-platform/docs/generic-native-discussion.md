# Signed native catalogue discussions

This additive adapter exposes the actual persisted native discussion of a public manga, chapter or group. It does not create a thread, create a public alias, publish a protocol event, or translate protocol-source comments into native replies. Native browser-cookie comments remain separately selectable.

## Contract

- GET/POST /api/v1/nostr/{manga|chapter|group}/{public UUID}/discussion.
- GET requires expectedAccountId and expectedThreadId, supports page 1–1,000,000 and order asc/desc, and has no body.
- POST has no query. Its strict duplicate-rejecting JSON shape binds expectedAccountId, expectedTargetId, expectedThreadId and operation. Reply updates/removals/deletions bind version; votes also bind expectedVote; create may bind an actual active parent reply in the same thread.
- The existing user-profile endpoint still uses expectedUserId. Neither endpoint accepts the other's target field. Request/body limits, operation normalization and receipts share native_discussion_common.rs; there is one sealed community mutation implementation.
- Metadata binds accountId, targetKind, canonical targetId and threadId. Mutation receipts additionally bind the exact method, URI and SHA-256 request body, operation and changed flag.
- Public GET /api/v1/nostr/native-forum/threads/{actual thread}/context describes only the existing visible, preferred native mapping. It returns 404 for missing/noncatalogue/unmapped targets; it never infers an alias from a thread ID.

## Authority and concurrent changes

Original request admission and transaction-rechecked bound_account_read/write remain authoritative. Browser cookies never supply this adapter's actor. Writes take the existing bound actor NO KEY UPDATE before target locks, compatible with community audit/author/vote FK KEY SHARE. Target, parent, witness and thread contention is NOWAIT and returns 409 before mutations. No network operation occurs while resource locks are held.

Target selection holds the actual stored mapping, visible native row, and exact entity_type/entity_id thread relation. Chapters retain their parent manga; imported groups retain one deterministic visible chapter-group edge and its chapter/parent manga. That single stable visible witness suffices, without locking every chapter in a large group. Instance deletion/hiding and witness removal cannot race through these row locks.

Protocol-backed targets keep the original protocol_source_is_visible predicate authoritative. They hold sorted source event rows and sorted blob_intents rows for the exact stored publication_blobs generations, including the event's stored manifest. Missing rows, changed generations or unavailable blobs fail closed; witness collection is capped at 16,384 references. Event selection uses indexed bytea IDs. Existing event lifecycle locks protect publication references and pending-outbox insertion; blob-intent locks protect quarantine insertion and removal, including when no quarantine row exists yet. The predicate is rechecked after acquiring these fences. This does not invent a new visibility rule based on source account/group membership.

Relevant existing writers: manga-node publication.rs commit locks the event before references/outbox; moderation.rs approval locks the event before withdrawal/outbox; backend.rs updates event state before removing publication references; health.rs locks blob_intents before quarantine insertion/removal. Native target writers are catalog/mutations.rs, entity_mutations.rs, repository.rs and protocol_bridge.rs.

## UI and recovery

Title, reader, group and actual-thread routes explicitly offer Browser login or Signed account. Neither is automatically selected after a provider remount. Signing identity/target changes invalidate selection and discard drafts. Signed review displays the bound account, typed resource, actual thread, reply/version, parent and vote transition as applicable; rich comment HTML uses the retained sanitizer and typography hooks.

The common signed controller retains exact in-memory review capabilities, preflight/postflight service and account checks, per-kind/account/target/thread pending fences, cancellation and no replay. An uncertain dispatched mutation remains fenced across reload/disposal. A failed signed operation never falls back to browser-cookie authority. User-profile reporting remains on its existing, separately scoped adapter. Signed report creation, inspection and moderation decisions for manga/chapter/group native discussions are unfinished and intentionally absent from this slice. The common controller exposes reports only for kind=user; generic native browser-cookie reporting remains distinct. Discussion/reply/vote support is not generic reporting parity, and protocol-source reporting is not widened.

## Qualification boundary

Historical documentation described finite synthetic checks for this slice, but the original machine-bound receipt is withheld from this privacy-safe profile. See the [withheld-evidence notice](verification/source-profile-and-withheld-evidence.md#signed-native-discussion). This paragraph is not a new execution result and does not establish real browser/network, PostgreSQL or current-union acceptance. Any ignored integration source remains a separate, explicitly admitted runtime obligation; it must not be counted as executed by a finite source pass.

## Context transport correction

The persisted-target reader now retains one production-singleton transport slot until its actual fetch, late response body, pending read and cancellation chain settles. The 250 ms caller cleanup grace only returns an explicit unsettled error; it does not release ownership or permit another dispatch. A failed reader release remains fenced. Actual EOF never triggers retroactive abort/cancel, including later JSON/provenance/UTF-8 validation failure. A 404 is separately interpreted as absence only after its body cancellation settles and current scope is rechecked, not as a healthy EOF.

Response storage is a fixed 16 KiB buffer with independent limits of 4,096 reads and 128 empty fragments. The correction adds late fetch/read/cancel fulfillment and rejection, concurrent/retry blocking, eventual release, failed release, 404 cleanup and fragment-limit controls. The historical corrected receipt and initial receipt remain unchanged privately and are not included in this profile. See the [withheld-evidence notice](verification/source-profile-and-withheld-evidence.md#signed-native-discussion). These descriptions do not establish a new composed strict, PostgreSQL or native-browser result; current qualification requires an actual run against the final profile.
