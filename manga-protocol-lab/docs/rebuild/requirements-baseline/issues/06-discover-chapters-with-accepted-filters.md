# 06: Discover chapters with accepted filters

**What to build:** Query verified chapter metadata over HTTP with the accepted expression grammar, pagination and comparison semantics.

**Blocked by:** 05: Publish a complete immutable chapter

**Status:** done

**Spec stories:** 12, 13, 14, 15

## Shared context

This is the scale-first Nostr-based manga reference node. Use PostgreSQL with deadpool-postgres/tokio-postgres and Moka, never SQLx. Originals remain owned and verified; moderation and peer approval are local. Work directly in this chat/computer environment. Website UI and production deployment are outside this ticket.

## Acceptance criteria

- [x] Live offset/limit and inclusive signed-created_at bounds work together.
- [x] Exact non-language comparisons, RFC 4647 basic ranges, unknown-under-NOT and explicit exists/missing agree across node and database evaluation.
- [x] Invalid, overbudget or oversized queries fail explicitly without partial-page success.
- [x] Explicit sort directions work; Q93/Q94 defaults remain provisional until answered.

Evidence: results/postgres-http-discovery.json and results/postgres-discovery-tests.log. Query semantics use a synthetic signed metadata fixture plus a query of a genuinely admitted chapter in the publication fixture. Q93/Q94 remain open; no target-capacity claim.
