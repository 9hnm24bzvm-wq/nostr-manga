# Native-account follows of signed public protocol lists

Migration0099 adds private native-account follow membership for the real typed
protocol-list identities already exposed by `/api/v1/list/{uuid}`. It does not
create a list, copy a private native list into a public graph, bind a signer to
an account automatically, or publish/change any original signed library.

The follower is an explicitly bound, active, verified native account. The list
owner is its actual original protocol publisher. Their User UUIDs remain
different typed identities even when the account is bound to that same pubkey.
Native account-owned `library_lists` UUIDs and wrong-kind UUIDs return404 at
these protocol-list endpoints. Existing private native ownership is preserved.

## Authenticated read operations

All operations require a fresh original Nostr proof bound to the exact external
URI, method, service, nonce and body, followed by a transaction-owned explicit
native-account binding recheck. Cookies alone and a public signed-library read
do not grant this private native follow authority. Responses are no-store and
vary on authorization/cookies.

- GET `/user/follows/list` returns the standard collection envelope, with
  `limit`1–100 (default10), `offset`0–10000 and no other query/body fields.
  Current visible followed lists are ordered by actual follow time descending,
  then canonical public UUID ascending. `total` counts current visible follows,
  before pagination. It does not confuse owned public lists with followed lists.
- Each list is the existing truthful original signed-list DTO. Its private
  `nostr.follow` extension contains actual `version`, `followedAt`, `updatedAt`
  and `sourceHeadAtFollow`. The top-level extension includes canonical native
  `accountId`, scope/source and `unavailableFollowCount`.
- GET `/user/follows/list/{uuid}` returns200 `{result:ok,nostr:{...}}` only when
  this account actually follows that currently visible signed list. Otherwise
  it returns404. Its extension includes canonical `listId`, `accountId`, actual
  membership version/times and the current source head.

Every source list uses the original signed visibility/snapshot APIs and the
same completed alias projection as the existing public list detail. A pending
new source projection returns503, rather than stale metadata or a fake empty
collection. Source-head changes detected during assembly return409. Original
source deletion/withdrawal hides its metadata but preserves the account's own
membership until an explicit unfollow.

## Actual follow and unfollow writes

POST `/list/{uuid}/follow` and DELETE on that path persist/remove this native
account's membership. They require its current `self.preferences.update`
permission. Empty bodies implement the upstream operations. An optional strict
JSON body adds application preconditions:

`{"version":N,"expectedAccountId":"canonical native user UUID"}`

POST additionally accepts `expectedHead` for the exact original signed source
head preview. A changed head409s. DELETE rejects `expectedHead`: removing one's
own follow must remain possible after source withdrawal/deletion. No unknown
fields, duplicate fields, body over1024bytes or query parameters are accepted.
All preconditions are within the exact signed body, never an unsigned header.

The transaction locks the original admission/binding and native account.
`expectedAccountId` freezes that account across a UI preview and submit; a
pubkey rebind409s rather than mutating another account with a colliding version.
Version0 means no persisted record, then state changes increment revisions.
Unfollow tombstones preserve those revisions; identical fresh-proof follow or
unfollow commands do not change version/clocks. A supplied stale version409s.
Exact proof replay401s. Unknown/withdrawn sources cannot be newly followed, but
an existing typed alias can still be unfollowed after its source disappears.

Successful mutations return `{result:ok,nostr:{scope,source,accountId,listId,
version,changed,followed,head}}`. The head is current source provenance for a
follow, not a pinned immutable copy of the list. Existing follows automatically
show real signed name/member changes after the source is projected. A client
should always send its captured account ID, and send a version when available
from an active follow. After404, callers must not assume version0: a prior
unfollow tombstone can exist.

## Bounded work and verification

The application permits at most1000 active followed public lists per account.
Reads validate all candidate sources before returning truthful visible totals;
they never silently truncate that total. DTOs are retained only for the selected
page, source head rechecks do not retain unselected member expansions, and a
page exceeding5000 Manga relationships returns400 asking for a smaller limit.
The complete private read has a30second budget and fails503 on exhaustion.
Original per-view/chunk/member budgets remain in force. These bounds are not a
claim of load or production qualification.

The real HTTP fixture uses the actual original signed library storage and
verified pinned strfry binary, with an owned loopback PostgreSQL cluster. It
checks private isolation, typed identity separation, canonical/imported aliases,
pending projections, actual source rename/replacement/withdrawal/deletion,
unfollow after disappearance, version/idempotency and admission/permission
revocation. No network publication or real user account is involved.
