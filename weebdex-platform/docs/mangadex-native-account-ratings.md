# Persisted instance-account ratings

Official contract reference: [MangaDex 5.13.1 OpenAPI](https://gitlab.com/mangadex-pub/mangadex-api-docs/-/raw/main/static/api.yaml),
GET `/rating` and POST/DELETE `/rating/{mangaId}`. The native instance account
is the observation owner. Existing aggregate counters never create invented
per-account ratings, and no Nostr library event or central MangaDex account is
implied by a rating.

## Public UUID boundary

Every request needs a fresh original Nostr proof bound to the exact external
URI, method, service, nonce and actual body bytes. An explicit active, verified
native-account binding is rechecked and locked inside the transaction. A site
cookie, public UUID or signed-library principal alone is insufficient. Writes
also require the account's existing `self.preferences.update` permission,
rechecked from current native policy inside the same transaction. Responses are
no-store and vary on authorization/cookies.

- GET `/api/v1/rating?manga[]=UUID` accepts 1–100 typed Manga UUIDs, including
  canonical/imported aliases. Repeated `manga=UUID` is also accepted for the
  upstream array parameter spelling; mixing spellings is rejected. No other
  query parameters or body are accepted. Unknown/wrong-kind/hidden titles404.
  Missing personal votes yield an empty `ratings` map rather than a guessed
  zero. Map keys are always persisted preferred public UUIDs.
- POST `/api/v1/rating/{uuid}` takes `{"rating":1..10}`. An optional nonnegative
  integer `version` is a compare-and-set precondition. Unknown JSON fields,
  duplicate fields, fractional/string ratings, invalid versions and bodies over
  1024 bytes are rejected. Ratings for known adult titles remain accessible.
- DELETE on the same path accepts an empty body, or the application extension
  `{"version":N}`. The body and precondition are covered by the Nostr proof.

GET and mutation `nostr.accountId` is the actual canonical native-account user
UUID. POST/DELETE also accept optional `expectedAccountId` in their exact signed
JSON body. The transaction-locked account must match it, otherwise the command
returns409 before any vote or aggregate change. A client should always capture
this ID with its preview and supply it alongside `version`; a pubkey rebind can
otherwise select a different native account with the same per-account version.
An unsigned header is not used as this binding precondition.

GET entries preserve the real first active vote's `createdAt`; `nostr.version`
and `nostr.updatedAt` identify its actual stored revision. Top-level
`nostr.versions` also exposes owner-only tombstone versions; zero explicitly
means no persisted record. Successful mutations return `result:ok` and a native
provenance extension with canonical `mangaId`, `version` and `changed`.

Versions start at 1 and increment only when state changes. Identical fresh-proof
POSTs and repeated empty DELETEs do not change revision or clocks. A supplied
stale version409s, including after deletion/recreation. Deletion retains a
minimal version tombstone, so a stale create cannot reset an old rating to
version 1. A recreated vote gets a new actual `createdAt`. Exact proof replay401s
even when the underlying state command would be idempotent.

## Native site boundary and shared transaction core

GET `/api/rating?manga[]=native-id` reads the same owner records with a genuine
site session or existing explicitly scoped owner-library reader. POST/DELETE
`/api/rating/{native-id}` require a genuine verified browser session, current
permission and same-origin mutation validation. Read-only API clients cannot
write. Native POST/DELETE accept the same optional version precondition.
Native IDs stay within this legacy native boundary.

Both mutation adapters call one transaction-owned core that accepts only the
existing sealed genuine-actor capability. It never authenticates an arbitrary
user string or fabricates a session. User policy and the visible title are
locked before observing/modifying state. Different owners of one title
serialize on that title, so concurrent committed votes cannot lose aggregate
updates. Every state change and exact mean update commits atomically.

## Actual aggregate statistics

`/statistics/manga` and `/statistics/manga/{uuid}` now expose the actual mean,
ten-bin distribution and observed count of persisted instance-account votes.
No observations means `average:null` and ten real zero bins. A legacy cached
rating alone does not become a vote or distribution. Submitted observations
remain historical records if their account later becomes inactive; inactive
accounts cannot submit new mutations. Hidden titles remain excluded.

The extension explicitly identifies this native population. No global
MangaDex Bayesian model or score is invented; `bayesian` is omitted and the
reason is recorded under `rating.nostr`. This is an intentional model boundary,
not a claim of central MangaDex statistics equivalence. On each actual vote
change, the exact native mean also updates `catalog_manga_counters.rating` for
existing native/public catalogue ordering; unrelated follows/views stay intact.

Migration0098 adds only the observation/tombstone table and bounded aggregate
index. It does not backfill votes from cached averages, alter credentials,
publish a protocol event or change device-local library defaults.
