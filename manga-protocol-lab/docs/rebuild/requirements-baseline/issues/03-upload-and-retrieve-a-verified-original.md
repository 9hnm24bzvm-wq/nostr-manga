# 03: Upload and retrieve a verified original

**What to build:** Accept an authorized upload through the real HTTP path, reserve its budgets, issue a durable generation-bound receipt, and retrieve exactly the accepted bytes.

**Blocked by:** 02: Boot an authenticated PostgreSQL node

**Status:** done

**Spec stories:** 3, 4, 6, 10, 11

## Shared context

This is the scale-first Nostr-based manga reference node. Use PostgreSQL with deadpool-postgres/tokio-postgres and Moka, never SQLx. Originals remain owned and verified; moderation and peer approval are local. Work directly in this chat/computer environment. Website UI and production deployment are outside this ticket.

## Acceptance criteria

- [x] Invalid authorization is rejected before unbounded body receipt.
- [x] Hash/size failures, interrupted writes and duplicate retries cannot falsely register or double-charge an object.
- [x] Space is reserved atomically and only confirmed outcomes settle/refund it.
- [x] Stale cleanup cannot delete a newer generation; unregistered/staged objects are not anonymously exposed.

Evidence: results/postgres-http-upload.json, results/postgres-blob-receipts.json, results/postgres-http-upload-tests.log. Scope: bounded operator-staged blobs; publication validation, retained references, full Blossom route compatibility and public static serving remain later tickets.
