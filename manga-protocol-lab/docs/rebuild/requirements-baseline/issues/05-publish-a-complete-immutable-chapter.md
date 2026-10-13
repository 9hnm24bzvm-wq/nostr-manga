# 05: Publish a complete immutable chapter

**What to build:** Publish a series/chapter manifest and read its exact original pages only after full local validation and retention commit.

**Blocked by:** 03: Upload and retrieve a verified original, 04: Project signed events through strfry

**Status:** done

**Spec stories:** 7, 8, 9, 10, 11

## Shared context

This is the scale-first Nostr-based manga reference node. Use PostgreSQL with deadpool-postgres/tokio-postgres and Moka, never SQLx. Originals remain owned and verified; moderation and peer approval are local. Work directly in this chat/computer environment. Website UI and production deployment are outside this ticket.

## Acceptance criteria

- [x] Canonical CBOR, hash/version, descriptor and actual-image checks enforce the experimental profile.
- [x] Every unique original is verified, and conflicting repeated descriptors are rejected.
- [x] Deletion, eviction or revocation during preparation cannot cause stale completion.
- [x] A reader receives exact original bytes; static delivery can fall back safely while serving projections catch up.

Evidence: results/postgres-http-publication.json, results/postgres-publication-fences.json and results/postgres-publication-tests.log. Exact-original HTTP fallback is implemented; nginx/static acceleration remains ticket15. Tests use real strfry, version/permission fences, rollback and synthetic PNG originals.
