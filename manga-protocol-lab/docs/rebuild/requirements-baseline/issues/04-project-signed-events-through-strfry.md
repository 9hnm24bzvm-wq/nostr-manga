# 04: Project signed events through strfry

**What to build:** Submit and retrieve a regular signed event through the node while durable projection work reaches real strfry and survives backend interruption.

**Blocked by:** 02: Boot an authenticated PostgreSQL node

**Status:** done

**Spec stories:** 8, 16, 32

## Shared context

This is the scale-first Nostr-based manga reference node. Use PostgreSQL with deadpool-postgres/tokio-postgres and Moka, never SQLx. Originals remain owned and verified; moderation and peer approval are local. Work directly in this chat/computer environment. Website UI and production deployment are outside this ticket.

## Acceptance criteria

- [x] Signature and identity validation precede persistence.
- [x] Duplicate events produce one durable intent; backend failure is reported with recoverable pending work.
- [x] Projection ordering and deletion/replaceable barriers are preserved without holding a database transaction across a subprocess.
- [x] Protected kind 5 requests never enter the raw automatic-deletion import path.

Evidence: results/postgres-http-backend.json and results/postgres-backend-tests.log. The immutable slice accepts kinds1/7/1111 only; deletion, replaceable and expiring events are rejected before staging. Unexpected queued lifecycle barriers stop rather than skip. Actual publication/moderation projection belongs to05/07.
