# 17: Finalize pending discovery defaults

**What to build:** Apply the owners answers to Q93 default ordering and Q94 exact-total behavior, preserving already accepted explicit query semantics.

**Blocked by:** 06: Discover chapters with accepted filters

**Status:** done

**Spec stories:** 13, 15

## Shared context

This is the scale-first Nostr-based manga reference node. Use PostgreSQL with deadpool-postgres/tokio-postgres and Moka, never SQLx. Originals remain owned and verified; moderation and peer approval are local. Work directly in this chat/computer environment. Website UI and production deployment are outside this ticket.

## Acceptance criteria

- [x] The owner answers Q93 and Q94; recommendations are not mistaken for acceptance.
- [x] Implementation, documentation and conformance tests match the chosen defaults.

## Blocker

Q93D/Q94A accepted on2026-10-07: explicit order is mandatory; exact totals are optional. Discovery HTTP tests pass; combined regression is being rerun after updating all fixture callers to include order.


Latest checkpoint: selected approved-peer library/account transfer, public social actions, ordinary author deletion and Q93D/Q94A pass the combined schema23 regression in results/discovery-peer-repair-full-regression-2.log. Bulk account inventories remain an explicit follow-on beyond selected snapshot/event transfer.
