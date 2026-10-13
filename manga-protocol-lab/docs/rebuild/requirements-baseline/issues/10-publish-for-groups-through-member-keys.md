# 10: Publish for groups through member keys

**What to build:** Authorize members personal keys for purpose-specific operations performed by an instance-held ordinary group signing key.

**Blocked by:** 02: Boot an authenticated PostgreSQL node, 05: Publish a complete immutable chapter, 07: Moderate protected publication removal

**Status:** done

**Spec stories:** 25, 26, 27

## Shared context

This is the scale-first Nostr-based manga reference node. Use PostgreSQL with deadpool-postgres/tokio-postgres and Moka, never SQLx. Originals remain owned and verified; moderation and peer approval are local. Work directly in this chat/computer environment. Website UI and production deployment are outside this ticket.

## Acceptance criteria

- [x] Publish and deletion-request signing use current role authorization, without exposing an arbitrary signing oracle.
- [x] Revocation persists after restart and is not reseeded from bootstrap configuration.
- [x] Signer references do not require plaintext private keys in PostgreSQL.
- [x] Key-only recovery preserves identity but does not recreate missing membership policy.

Evidence: results/postgres-http-groups.json, postgres-group-fences.json and postgres-groups-tests.log. Keys live in restricted local files, never PostgreSQL. Group-specific grants protect private staging. Real restart preserves revocation; key-only recovery creates no member policy without explicit bootstrap.
