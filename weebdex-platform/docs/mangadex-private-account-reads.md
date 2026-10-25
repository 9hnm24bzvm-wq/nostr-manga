# Request-authorized account and follow reads

This additive batch implements ten more upstream read operations under
`/api/v1`. They expose actual instance-account data and preserve the default
local-first library, signed-library mutation fence, native cookie sessions and
original protocol publication boundaries.

## Authorization and privacy

Every operation below requires a fresh original Nostr HTTP capability, bound to
the exact external URI, method, payload hash, service and nonce. The principal
must currently be admitted and explicitly bound to an active, verified native
account. Admission, identity binding and account eligibility are rechecked and
locked on the same repeatable-read transaction that selects private rows,
resolves aliases and produces the response. No UUID, cookie, bare key or
third-party principal assertion can replace this capability.

The official `/user` directory operation is authenticated. This adapter keeps
it authenticated even though some legacy native public profiles are readable
without sign-in. A browser site session alone does not grant the Nostr API
capability. An original signed-public-library identity without an explicit
native binding does not grant access to this private account library.

Responses are private/no-store and vary on Authorization and Cookie. User DTOs
contain only canonical UUID, username, public roles and actual version. Email,
preferences, password hashes, tokens, private follows and profile secrets are
not copied into user resources. No GET accepts a body. Reads do not create
aliases or mutate library state; the original request verifier still consumes
the one-time authorization proof to prevent replay.

## Operations

| Read | Contract |
| --- | --- |
| `GET /user` | Authenticated native-user directory; limit 0–100/default 10, row offset, UUID `ids[]`, username substring, `order[username]`. Disabled users are excluded. No protocol-publisher profile is invented from a native account. |
| `GET /user/me` | Entity for the exact bound native account; no query. The top-level nostr extension declares `scope:instance-account` and `identity:explicitly-bound-native-account`. |
| `GET /user/history` | Actual owner-only persisted native chapter history with canonical chapter UUIDs and stored read timestamps. No query; current chapter/parent/source visibility applies, including known adult titles. The upstream5.13.1 history array is named `ratings`; this exact field is preserved. A separate nostr extension identifies native-account history and stored-native-read-time semantics. No timestamps are inferred from original signed read markers. Explicit503 above10,000 rows instead of truncation. |
| `GET /user/follows/manga` | Actual owner-only persisted followed manga. Includes published known adult titles because this operation has no discovery rating filter. Hidden/deleted/source-revoked titles are excluded. Manga reference expansions are supported. |
| `GET /user/follows/group` | Actual owner-only persisted group follows with current group visibility and optional leader/member expansion. |
| `GET /user/follows/user` | Actual owner-only persisted user follows; disabled targets are excluded. No user attributes beyond the public profile projection. |
| `GET /user/follows/manga/{id}` | 200 `{result:ok}` only for an actual current visible native follow; otherwise 404. Canonical/imported aliases resolve by kind. No query. |
| `GET /user/follows/group/{id}` | Same status contract for group follows. |
| `GET /user/follows/user/{id}` | Same status contract for native-user follows. |
| `GET /user/follows/manga/feed` | Chapter collection scoped inside SQL to the current account's actual followed titles. Same upstream feed filters/includes/order/date/toggle vocabulary and default rating policy; default limit 100, maximum 500. Native notification preferences do not silently remove followed titles. |

Follow resource lists default to 10, require limit 1–100 and use row offsets.
They do not accept undocumented filters/order controls. Local offsets are
bounded at 10,000. Resource creation time plus native ID gives a deterministic
default order; this does not claim an upstream follow-time sort option. The
collections expose `nostr.scope:instance-account` and identify their persisted
native-account-library source. Resource attributes do not invent follow times,
reading-status fields or protocol-publication state.

Private chapter-feed selection uses an indexed owner/manga EXISTS predicate,
not an expanded array or a truncated first 10,000 followed-title snapshot.
All current public chapter/parent/source visibility predicates apply. UUID
filter resolution, count, page selection, nested projection and aliases share
one transaction. The ordinary public catalogue collection now also uses a
repeatable-read, read-only transaction for internally consistent paging and
alias resolution without extra pool acquisitions.

## Verification and remaining work

`backend/tests/public_account_reads.rs` uses real loopback HTTP, disposable
PostgreSQL, original service/admission and signed request verification. It tests
missing proofs, admitted-but-unbound/disabled/banned/unverified/unadmitted
principals, exact self identity, directory paging/filtering, another account's
private follows, adult/hidden targets, group/user visibility, all three status
checks, chapter-feed rating/exclusions, replay, wrong URI, cookie insufficiency,
alias noncreation and subsequent admission/binding revocation. Existing read
HTTP tests are rerun because the shared collection path changed.

List-follow persistence, ratings, settings/templates and API-client
compatibility are separate remaining operations. MangaDex OAuth/client secrets
must not be fabricated from original Nostr keys. This batch neither removes
their explicit unavailable behavior nor claims full API compatibility.
