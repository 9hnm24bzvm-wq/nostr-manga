# Native-user navigation and signed discussion slice

This source candidate fixes legacy native-user navigation in public UUID mode
without importing identity or granting authority from a public identifier.
Only visible, existing native accounts with durable stored public aliases resolve.
Mapped legacy users require explicit `native_public_user` provenance, the same
canonical profile, and the actual persisted user-thread relation. UUID routes
retain their existing direct lookup behavior. Protocol publishers have no invented
native account, thread, revision, cookie permission, or native discussion controls.

Two exact overlay imports route only the supplied user profile/comments pages to
the additive wrapper. All 679 tracked supplied frontend files remain untouched;
archival parity checks all 676 archive entries. Gate-off imports remain native.
Actual provider/context, URL page/order state, sanitized comment Markdown,
typography attachments, user tabs and author links are retained. Anonymous reads
omit credentials and bracket the native feed with public typed user-thread checks.
The anonymous feed is incrementally decoded under an 8 MiB wire cap and a 30s
whole-operation deadline. Healthy EOF plus final context only releases ownership,
with no abort or cancellation. Failure cancellation aborts the request, cancels the
owned body reader and awaits owned work for a bounded 250ms grace. Dependencies
that ignore cancellation produce an explicit unconfirmed-cleanup failure; their
late rejections remain observed. No native-service cancellation claim is inferred
from finite response mocks.

Signed actions deliberately establish the existing exact signer/service/catalogue/
account scope. They use `/api/v1/nostr/user/{canonicalUuid}/discussion`, not the
cookie-based native endpoints and not source-event comment conversion. The UI
reviews immutable commands against the loaded snapshot before dispatch. Create,
parent reply, edit, owner/moderator remove, moderator hard-delete, thread/reply vote
and unvote call the existing native core with genuine sealed bound-account actors.
The transaction checks current policy/admission/binding, target visibility and the
actual stored relation. Version and personal-vote captures are mandatory where
applicable. Contested target account rows fail with known pre-mutation 409 using
NOWAIT, avoiding reciprocal actor/target lock waits without changing shared auth.

Receipts bind exact transmitted JSON bytes with SHA-256 plus the HTTP method/path,
canonical account/user/thread and operation. Operation-specific reply identity,
body, revision, state and personal vote are checked. Account/service/namespace
postflight verification runs before clearing the pending safeguard. A later GET
that was not sent cannot clear the safeguard for an already sent mutation. Unknown
outcomes remain fenced across explicit reads and reopen; nothing is auto-replayed.
Native tombstone body/author privacy and moderation permissions remain intact.

Only the exact native discussion GET/POST endpoint has a larger bounded transport
budget: a query-free JSON POST accepts at most 393216 serialized bytes, and JSON
responses at most 8 MiB for twenty stored 50000-byte comments including worst-case
JSON escaping. All generic endpoint request/response caps remain unchanged.

## Qualification and remaining scope

The earlier discussion-only source freeze deliberately left native reporting as a
required separate follow-on. That adapter and its mounted controls are now
implemented in [native-user-reporting.md](native-user-reporting.md), without
source-event report widening, new schema or cookie authority fallback.

[Backend qualification evidence](../test-evidence/native-user-http-20261022/README.md)
records the independently reviewed combined source and actual disposable
HTTP/PostgreSQL execution. Final native navigation/discussion/reporting tests
passed at `1477d6b`, with compatible native community and other regression
executables unchanged from their successful `b043cf0` runs. Historical failures
and their test-only corrections remain distinct from successful runs.

UUID routes continue to mean public aliases. Existing UUIDv4 native accounts have
stored self aliases; a raw UUIDv8 native key without an identical public alias is
not recovered by this public-first resolver. Its actual durable generated alias
works. A supported import emitting raw UUIDv8 native links would require a
separate explicit typed-native migration, not ambiguous UUID/native fallback.

Earlier finite actual-module and mounted Svelte/JSDOM results remain scoped to
their recorded source freezes. Full composed finite/strict/build qualification
after this product integration is still required. No native-browser terminal or
cancellation proof, visual parity, hosted execution, real account/comment,
publication or production credentials are claimed. Account-closure policy
remains pending and unchanged.
