# Ordinary account deletion (experimental profile)

`POST /account-deletions` accepts a signed Nostr kind5 request with NIP98 account authentication. The signer must own the request and have current local posting permission. Requests are public; `GET /account-deletions/<request-id>` returns their signed evidence. Raw kind5 is never sent through strfry's automatic deletion handling.

Exact `e` targets are author-qualified. Known own ordinary events (kinds0,1,3,7,1111,10002,11999) withdraw immediately from the wrapper, then durable backend jobs remove their IDs. Unknown targets establish a persistent author-qualified tombstone so delayed delivery cannot resurrect them. Another author's matching ID is not affected. Deleting a deletion request has no effect.

Supported `a` coordinates are `kind:author:` for local profile/contact/relay-list/library kinds0,3,10002,11999. The server records a persistent timestamp cutoff. Versions at or below the signed deletion timestamp are rejected; a newer version may be published. This is not a global guarantee that another instance deletes anything.

Removing a library clears its current head but preserves its winning clock. Current object holds become pending until actual backend acknowledgement, then enter normal read grace and optional generation-safe cleanup. Shared objects and newly reused objects stay protected. Social replies are not cascaded when their parent is deleted.

Protected chapter/series events1001/1002 remain under the separate local moderation workflow, including when named in a mixed request. Recording an ordinary request does not invoke their automatic approval option; moderators can use the existing approval API on the recorded request/targets. There is no revision/equivalence linking.

Bounds: at most100 distinct explicit/expanded targets,256 bounded tags,64KiB reason and128KiB HTTP body. Supported coordinate expansion only acts on currently hosted versions; already withdrawn versions retain their existing lifecycle.

## Correctness fences

Account authority locks serialize head changes/deletion. Transaction-scoped event-ID locks also fence ordinary late event delivery when no target row exists yet. Hash collisions merely serialize unrelated requests. Tombstones and coordinate cutoffs are checked by personal social/library/metadata admission and the low-level regular event ingest path. Protected publication admission remains separate.

## Evidence status

`results/ordinary-library-delete-regression.log` verifies real HTTP library deletion by coordinate, read disappearance, rejection at the cutoff and a newer root reusing the same stored bytes. The full social/deletion regression passed in `results/social-deletion-full-regression.log`, including exact-ID social deletion and32 concurrent delivery/deletion cases. `results/ordinary-deletion-races.json` records their scope.

Protocol reference, checked2026-10-07: https://github.com/nostr-protocol/nips/blob/master/09.md . NIP09 is draft/optional; this is a bounded local profile. Request evidence is served through the guarded wrapper rather than blindly importing kind5 into strfry.
