# 18: Validate deployment capacity honestly

**What to build:** Run the agreed workload on suitable authorized infrastructure and establish resource/throughput limits with representative storage cardinality.

**Blocked by:** 16: Verify PostgreSQL behavior end to end, 17: Finalize pending discovery defaults

**Status:** blocked-infrastructure

**Spec stories:** 1, 35

## Shared context

This is the scale-first Nostr-based manga reference node. Use PostgreSQL with deadpool-postgres/tokio-postgres and Moka, never SQLx. Originals remain owned and verified; moderation and peer approval are local. Work directly in this chat/computer environment. Website UI and production deployment are outside this ticket.

## Acceptance criteria

- [ ] Reusable workload drivers and acceptance metrics are ready and locally checked.
- [ ] Cold/warm and concurrent read/write results identify hardware, dataset, network and cache assumptions.
- [ ] Actual target-scale results require suitable infrastructure and permission; no synthetic result is labeled 100 TB validation.
- [ ] Outstanding production/security/operational gates are reported rather than hidden.

## Blocker

The current local laboratory does not provide 100 TB storage or target Internet delivery infrastructure. No paid resource or public deployment is authorized by this ticket. Driver preparation can proceed after its code dependencies.
