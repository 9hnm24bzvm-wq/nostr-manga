# Public social API (experimental profile)

`POST /social` accepts an author-signed Nostr kind1111 comment or kind7 reaction, with NIP98 request/body binding and an explicitly admitted personal account. The personal signer must own the event. Revocation is rechecked transactionally. Exact duplicate events do not add another projection job.

Comments use NIP22 exact event-ID root/parent references. Roots are locally hosted manga chapter/series publications (1001/1002); replies point to the root or an indexed comment in that same thread. Uppercase/lowercase author and kind tags are checked against the known targets. External URL/address scopes are outside this profile. Content is plaintext and must not be interpreted as trusted HTML.

Reactions use NIP25 kind7, targeting the last `e` tag. Optional last `p`/`k` hints must match the known target. `+` or empty content represents an upvote, `-` a downvote; other reaction text is preserved. No vote aggregation or one-person-one-vote claim is made: public keys are identities, not verified people. Clients may choose an aggregation/display policy later.

`GET /social?target=<event-id>&limit=50&offset=0&since=0&until=<timestamp>` returns hosted, projected events under the instance read policy (anonymous on public instances) rooted at the target. Bounds: limit1..100, offset0..10000, inclusive timestamp range. Ordering is descending event timestamp and ascending ID for ties. This social ordering is independent of the explicit chapter-query ordering contract.

Content is limited to64KiB for comments or256bytes for reactions, with bounded tags and request bodies. Ordinary operator `/events` remains a legacy low-level event ingest seam; the personal social API provides validated thread indexing. Unknown target hints are never fetched automatically.

## Evidence

`results/social-http-regression.log` passed the real HTTP/PostgreSQL/strfry fixture: deny-before-admission, public comment, nested reply, upvote, anonymous query, duplicate, malformed author tags, query bounds and revocation. Deletion tests pass in `results/social-deletion-full-regression.log`, including exact author deletion, idempotent replay, unknown-target tombstones and preserving replies/foreign protected chapters.

## Protocol sources

- NIP22: https://github.com/nostr-protocol/nips/blob/master/22.md
- NIP25: https://github.com/nostr-protocol/nips/blob/master/25.md

Sources checked2026-10-07. These are draft/optional Nostr extensions; this document describes the node's narrower manga profile, not universal support for every NIP22 scope.
