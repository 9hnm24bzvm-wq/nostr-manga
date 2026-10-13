# 08: Synchronize one approved peer chapter

**What to build:** Synchronize a selected chapter between two real nodes after expected-key verification and explicit peer approval.

**Blocked by:** 05: Publish a complete immutable chapter, 06: Discover chapters with accepted filters, 07: Moderate protected publication removal

**Status:** done

**Spec stories:** 5, 19, 20, 21

## Shared context

This is the scale-first Nostr-based manga reference node. Use PostgreSQL with deadpool-postgres/tokio-postgres and Moka, never SQLx. Originals remain owned and verified; moderation and peer approval are local. Work directly in this chat/computer environment. Website UI and production deployment are outside this ticket.

## Acceptance criteria

- [x] An unapproved peer or changed service key does not inherit access.
- [x] Signed event and manifest metadata are checked before downloading out-of-filter originals.
- [x] Only complete verified local originals become hosted on the receiver.
- [x] An independently approved fallback can repair a failed origin; arbitrary endpoints are not silently trusted.

Evidence: results/postgres-http-peers.json and results/postgres-peer-tests.log. Three actual local HTTP/PostgreSQL/strfry nodes verify bilateral approval, service-key proof, filter-before-original transfer, distinct-author retention and an independently approved mirror with the origin offline. Both sources then stop while the receiver serves its own originals. Bulk/resumable synchronization is09.
