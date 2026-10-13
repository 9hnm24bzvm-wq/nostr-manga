# 07: Moderate protected publication removal

**What to build:** Receive a signed deletion request, keep the publication until local approval, and complete safe withdrawal with persistent replay protection.

**Blocked by:** 04: Project signed events through strfry, 05: Publish a complete immutable chapter

**Status:** done

**Spec stories:** 9, 16, 17, 18

## Shared context

This is the scale-first Nostr-based manga reference node. Use PostgreSQL with deadpool-postgres/tokio-postgres and Moka, never SQLx. Originals remain owned and verified; moderation and peer approval are local. Work directly in this chat/computer environment. Website UI and production deployment are outside this ticket.

## Acceptance criteria

- [x] Receipt alone does not auto-delete protected manga content.
- [x] Approval is local, deletion does not cascade automatically from series, and shared originals remain retained.
- [x] Completed withdrawal accounts for owned serving projections; interruption/retry is recoverable.
- [x] Replacement is a new upload/event, with no protocol revision/equivalence link.

Evidence: results/postgres-http-moderation.json and results/postgres-moderation-tests.log. Real strfry deletion is scan-verified; generation fences and a subprocess-held flock prevent an orphaned older import from resurrecting approved removal after a parent crash. Shared originals remain; physical orphan reclamation is ticket12 and static-serving projection acknowledgements extend this in15.
