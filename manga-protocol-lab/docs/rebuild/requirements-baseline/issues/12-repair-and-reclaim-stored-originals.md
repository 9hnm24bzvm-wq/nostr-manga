# 12: Repair and reclaim stored originals

**What to build:** Detect missing/corrupt originals, stop offering affected content, repair it through approved sources and reclaim only eligible generations.

**Blocked by:** 03: Upload and retrieve a verified original, 05: Publish a complete immutable chapter, 07: Moderate protected publication removal, 08: Synchronize one approved peer chapter

**Status:** done

**Spec stories:** 6, 17, 21, 24

## Shared context

This is the scale-first Nostr-based manga reference node. Use PostgreSQL with deadpool-postgres/tokio-postgres and Moka, never SQLx. Originals remain owned and verified; moderation and peer approval are local. Work directly in this chat/computer environment. Website UI and production deployment are outside this ticket.

## Acceptance criteria

- [x] Known corrupt/missing bytes cannot remain advertised as healthy through stale caches.
- [x] Shared references and active reservations/pins prevent unsafe reclamation.
- [x] Cleanup is fenced and idempotent; expiry alone does not refund uncertain writes.
- [x] Rotating bounded maintenance makes progress across the collection.


## Negative health checkpoint

Generation-fenced sparse quarantine markers now gate wrapper cache reads, publication admission, discovery, public originals and inventory. A storage-health epoch prevents cached reconciliation frames from surviving health changes. Actual corrupted/shared-image, accounting, stale-generation and PostgreSQL lock-wait tests pass; see docs/storage-health.md and results/health-fenced-regression.log. No automatic repair, backend withdrawal, physical reclaim or bounded rotating auditor is claimed complete; this ticket remains in progress.


Schema25 checkpoint: current implementation and acceptance evidence are listed in graph.json and results/migration-backup-full-regression.log. Current behavior supersedes older incomplete checkpoint notes above. Remaining external-serving and production-capacity qualifications remain explicit.
