# 02: Boot an authenticated PostgreSQL node

**What to build:** Start the actual node against explicit migrations, bind it to the expected service key, and exercise public identity plus protected operator requests over HTTP.

**Blocked by:** 01: Demonstrate durable multi-process foundations

**Status:** done

**Spec stories:** 1, 2, 5, 32, 34

## Shared context

This is the scale-first Nostr-based manga reference node. Use PostgreSQL with deadpool-postgres/tokio-postgres and Moka, never SQLx. Originals remain owned and verified; moderation and peer approval are local. Work directly in this chat/computer environment. Website UI and production deployment are outside this ticket.

## Acceptance criteria

- [x] A second process can use the same intended instance database; a mismatched service key fails startup.
- [x] Identity proof binds nonce and configured endpoint; secrets are not returned or logged.
- [x] NIP-98 URL/method/body/freshness/replay checks reject unauthorized requests at the public HTTP seam.
- [x] Missing/newer/inconsistent schema fails clearly instead of silently mutating startup state.

## Evidence

The two-process HTTP fixture passed identity, authorization/body/freshness/replay, early rejection and schema-startup failure checks, followed by the complete PostgreSQL suite and strict lint checks.
