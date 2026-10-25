# Visible native-user memberships

The supplied native overview displays publicly visible group memberships in
native database `group.name, group.id` order and marks the target user's leader
or officer edge. The public profile DTO intentionally has no expanded groups.
This additive gate-on presentation restores that feature without importing
native cookie actors into public UUID or signed bound-account controls.

The pinned MangaDex 5.13.1 inventory declares a query-free `GET /user/{id}` and
no user-memberships operation. The new read is explicitly an application
extension: `GET /api/v1/nostr/user/{publicUserUuid}/memberships?limit=32&after=…`.
It accepts an actual, active native public user, not a protocol publisher,
account binding, signing key, or arbitrary native identifier. The request may
use a stored alias; the envelope identifies the canonical public user. The
mounted overview uses the canonical public profile UUID.

Each read uses a read-only repeatable-read transaction, selecting at most 32+1
visible membership rows. A group must not be deleted and must satisfy the
existing `protocol_group_is_visible` predicate. Inactive public groups remain
visible, matching the native overview. Missing preferred aliases fail closed,
including on continuation. The response projects only public group UUID, name,
version and the target user's two membership flags. It does not invoke full
group/member projections, leak private counts, profile/contact data, native
IDs or lifecycle metadata, or grant permissions from a group flag.

Continuation preserves the native database collation/order, including same-name
groups. Its canonical base64url cursor contains only version, canonical public
user UUID, public group UUID and public group name. It is not a secret or an
authorization credential. Before continuing, the server resolves the anchor's
preferred group alias and rechecks that same target's visible membership and
unchanged name. A moved, removed, foreign-user or invalid anchor requires an
explicit refresh. The cursor is capped at 2048 bytes on encode/decode, covering
all permitted 200-character/800-UTF8-byte names, including JSON escaping.
Responses are bounded at 64 KiB; the browser independently bounds streamed
bytes, UTF-8 decoding and a 15-second per-page deadline. A fixed 64 KiB buffer plus 8192-fragment
and 128-empty-fragment caps bound retained bytes and pathological stream work.

Pages are live, not an atomic cross-request snapshot. No total or snapshot is
invented. Changes before the anchor may require Refresh to update earlier
results. The UI explains this and does not sort pages independently. It retains
only one page of at most 32 groups. Next replaces the current page, the page
number is explicit, and Refresh returns to the first page. It renders the native
avatar/link/leader/officer treatment. Requests
omit credentials and never invoke a signer. A new target/refresh/unmount aborts
the owned request and fences late results. At most one replacement is queued
behind an active read. Failure cleanup waits for owned fetch, read and body
cancellation to settle for a bounded 250 ms grace. If they do not settle, the
module-level transport slot stays unavailable, including across reopened views,
until actual late settlement and lock release. Repeated Refresh cannot stack
unsettled requests. Healthy EOF releases without abort/cancel. These controls
do not claim remote service cancellation completion. The gate-off supplied page remains byte-for-byte
unchanged. Protocol publishers receive no fabricated native memberships.

## Qualification

The source-only backend privacy/order/cursor review identified and corrected a
cursor escape-budget mismatch before qualification. The initial d32f7d9 UI/transport candidate was then rejected
for unawaited cleanup and unbounded cross-page accumulation. Its exact rejected
transport is retained only for an executed counterfactual. The corrected actual
module/lifecycle tests pass 49/49, and compiled Svelte/JSDOM tests using the
supplied overview, exact public overlay, production component and production
adapter pass 13/13. The final navigation correction clears old target rows
synchronously even while their request cleanup remains pending. Current evidence
is in `test-evidence/native-user-memberships-final-20261022`; the earlier evidence
remains associated only with its rejected source freeze. Standalone adapter TypeScript no-emit checking passes. Mounted leaf
controls, Fetch responses and DOM are finite test substitutes, not browser or
live database evidence.

The synthetic TCP/PostgreSQL test target `public_native_user_memberships` is
authored but NOT compiled or executed in this source checkpoint. It requires
the existing isolated `scripts/run-auth-tests.sh` fixture, reviewed migrations
and an assigned heavy-test slot. It covers DB collation and same-name paging,
maximum escaped/UTF-8 names with server-emitted continuation, visible flags,
canonical aliases, disabled/publisher/private projection rejection, stale
anchors and missing aliases. Authored assertions are not a passing HTTP result.
Full composed type/build qualification and actual browser acceptance remain
separate. No production account, hosted workflow, deployment or publication was
used by these tests.
