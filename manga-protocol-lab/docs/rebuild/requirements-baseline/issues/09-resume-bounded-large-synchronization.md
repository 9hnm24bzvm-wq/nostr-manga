# 09: Resume bounded large synchronization

**What to build:** Run durable partitioned synchronization jobs across worker/process interruptions without unbounded retained state.

**Blocked by:** 08: Synchronize one approved peer chapter

**Status:** done

**Spec stories:** 22, 23, 34

## Shared context

This is the scale-first Nostr-based manga reference node. Use PostgreSQL with deadpool-postgres/tokio-postgres and Moka, never SQLx. Originals remain owned and verified; moderation and peer approval are local. Work directly in this chat/computer environment. Website UI and production deployment are outside this ticket.

## Acceptance criteria

- [x] Discovery pages remain separate from bounded inventory snapshots.
- [x] Large/equal-timestamp inventories reconcile through validated partitions and explicit resource ceilings.
- [x] Lease loss, cancellation, retry and restart fence stale work and preserve idempotent admitted results.
- [x] Backoff, fairness and queue limits produce bounded behavior.

Evidence: results/postgres-http-inventory.json, postgres-sync-queue.json, postgres-http-sync-jobs.json and postgres-sync-tests.log. Actual worker restart, persisted items, live-lease preservation, newer-generation reclamation and automatic16-way partitioning are exercised.120000 common synthetic IDs drive inventory scale while five real chapters exercise media admission. Source snapshots are shared PostgreSQL slots, so API requests may move across processes. Bulk currently covers chapter inventory; other entity lifecycle adapters remain later work.
