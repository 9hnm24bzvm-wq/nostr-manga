# Approved-peer library snapshot transfer

`POST /admin/sync-library` accepts `{ "peer": "service-key", "publisher": "author-key" }` under operator NIP98 authentication. Both instances must explicitly approve the other's service key. The receiver verifies a nonce/endpoint-bound identity proof, reads the projected signed head, fetches the canonical root and referenced chunks with byte/hash limits, stages them locally and commits through the same complete-root admission path.

Source routes are `GET /federation/libraries/<publisher>` and `GET /federation/library/blobs/<hash>`, requiring an approved service-key NIP98 request. Public reader routes follow the configured read_policy (anonymous by default); this access gate is federation policy, not DRM or a promise that public readers cannot copy bytes.

Peer authority is separate from personal posting authority. A remote-only author row starts with posting disabled. Peer approval version/endpoint is checked before transfers, after I/O and under the final root transaction. Root replacement still follows timestamp/ID ordering, local deletion cutoffs and exact content checks. No root becomes hosted with missing chunks. An older snapshot is reported superseded rather than replacing a newer local head.

The worker shares bounded synchronization slots and a ten-minute overall request deadline. Files are transferred sequentially under existing per-object and aggregate library limits. A disconnected caller does not release the owned synchronization slot while work continues. A failed transfer may retain private staged bytes; it does not publish an incomplete head or refund uncertain storage.

## Evidence

`results/library-peer-http-regression-2.log` and `results/library-peer-http.json`: real three-node PostgreSQL/HTTP/strfry fixture, bilateral approval, signed root/chunk transfer, duplicate, no personal login grant, approval revocation while object staging is blocked, local reads after source shutdown and subsequent revocation denial. The first attempt's old chapter-only fixture expected zero stored objects after a filtered chapter; it was updated to account for the two already verified library objects. That failed log remains diagnostic evidence.

## Current limits

The original selected snapshot transfer now also has a durable library-inventory adapter in schema29. Profile and ordinary deletion-request inventories are supported too; see `account-library-inventories.md`. Automatic schedules, conflict merging and library fallback lists remain outside this profile. Source head absence never silently deletes the local library. An operator may explicitly select another independently approved holder as the source. The published root/chunk profile is experimental, not a registered Nostr standard.


## Other signed account events

`POST /admin/sync-account` accepts peer service key and exact event ID. `/federation/account-events/<id>` provides approved peers a projected profile/contact/relay-list/comment/reaction, or signed ordinary deletion evidence. Supported selected kinds are0,1,3,10002,1111,7 and5. The same peer epoch is fenced inside the final account transaction, while personal posting remains disabled for remote-only authors. Comments require their target/thread locally, so import dependencies before replies. Deletion requests follow the ordinary author-qualified policy and never automatically approve protected publications. Selected transfer tests passed in `results/account-peer-http-regression.log`.
