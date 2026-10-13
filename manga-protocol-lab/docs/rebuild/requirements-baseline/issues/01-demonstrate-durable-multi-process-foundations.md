# 01: Demonstrate durable multi-process foundations

**What to build:** Prove the selected PostgreSQL, driver, cache and job/accounting primitives through independent clients and process/restart fixtures.

**Blocked by:** None

**Status:** done

**Spec stories:** 1, 2, 4, 23, 33

## Shared context

This is the scale-first Nostr-based manga reference node. Use PostgreSQL with deadpool-postgres/tokio-postgres and Moka, never SQLx. Originals remain owned and verified; moderation and peer approval are local. Work directly in this chat/computer environment. Website UI and production deployment are outside this ticket.

## Acceptance criteria

- [x] Independent processes produce one event and one projection intent for duplicate submission.
- [x] Warm cached bodies cannot bypass current removal state or a database outage.
- [x] Worker claims and quota reservations survive contention with conservation and stale-owner rejection.

## Evidence

The real PostgreSQL foundation, quota-credit and separate-process/restart fixtures pass. This does not claim that higher-level manga workflows have already been ported.
