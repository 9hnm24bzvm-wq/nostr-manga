# 16: Verify PostgreSQL behavior end to end

**What to build:** Run the full reference-profile contract suite against PostgreSQL, real strfry, owned storage and nginx, with independent process and fault scenarios.

**Blocked by:** 06: Discover chapters with accepted filters, 07: Moderate protected publication removal, 09: Resume bounded large synchronization, 10: Publish for groups through member keys, 11: Maintain public libraries and social actions, 12: Repair and reclaim stored originals, 13: Import MangaDex-shaped exports, 14: Restore an interrupted instance, 15: Operate scalable serving roles

**Status:** done

**Spec stories:** 1, 2, 7, 16, 22, 24, 32, 33, 34, 35

## Shared context

This is the scale-first Nostr-based manga reference node. Use PostgreSQL with deadpool-postgres/tokio-postgres and Moka, never SQLx. Originals remain owned and verified; moderation and peer approval are local. Work directly in this chat/computer environment. Website UI and production deployment are outside this ticket.

## Acceptance criteria

- [x] Legacy behavior contracts are rerun on the new stack rather than credited from SQLite results.
- [x] Cold/warm caches, competing writes, interrupted projections, corruption and restart are covered.
- [x] Experimental profile limits and remaining unsupported behavior are documented accurately.
- [x] The delivered archive rebuilds and its dependency graph contains no SQLx or SQLite in the selected server.

## Final implemented-profile gate,2026-10-07

The opt-in managed original reader, guarded-policy startup, failure closure and supervisor recovery pass the final full PostgreSQL suite (43 test-result groups), Clippy and process/restart checks. The source-only archive rebuilds and its unit tests pass using the pinned existing dependency cache.107Python model tests also pass. See docs/managed-public-serving.md and results/serving-role-full-final-2.log. This closes the specified reference-profile implementation/functional gate, not production readiness or100TB/300M-save deployment capacity. Conservative restart-after-fault behavior and foreign host/namespace recovery limitations remain explicit. Ticket18 stays blocked on target infrastructure.
