# 15: Operate scalable serving roles

**What to build:** Run the same roles in a small deployment and a multi-process fixture with bounded resources, operational metrics and direct public-original delivery.

**Blocked by:** 02: Boot an authenticated PostgreSQL node, 03: Upload and retrieve a verified original, 05: Publish a complete immutable chapter, 08: Synchronize one approved peer chapter

**Status:** done

**Spec stories:** 1, 2, 30, 33, 34, 35

## Shared context

This is the scale-first Nostr-based manga reference node. Use PostgreSQL with deadpool-postgres/tokio-postgres and Moka, never SQLx. Originals remain owned and verified; moderation and peer approval are local. Work directly in this chat/computer environment. Website UI and production deployment are outside this ticket.

## Acceptance criteria

- [x] Ordinary public image reads bypass PostgreSQL and Moka; guarded policies remain fail-closed.
- [x] Pool, worker, queue, body, decode and cache limits are explicit and observable.
- [x] Storage mutation ownership, generation fencing and API role separation do not rely on a global metadata mutex.
- [x] TLS/access-policy configuration fails safely; overload and backend outage have tested responses.

## Schema26 progress

Guarded WebSocket, bounded history/live queues, multi-key NIP42 and configurable public/authenticated/admitted read policy have focused real-node evidence. PostgreSQL feed disconnection explicitly resets continuity and reconnects. Full regression is tracked separately. Direct nginx public-original exporting remains open: do not bypass generation/reclamation safety or claim this ticket complete. See docs/guarded-relay.md.

Schema29 closes durable series/profile/deletion/library inventory adapters with full regression and release-build evidence. Library jobs recover a current complete head from a retained signed old pointer, converge on repeat, and cannot publish after mid-stage cancellation. Automatic social dependency handling and direct public-original serving remain open; the ticket is not complete.

## Schema32 progress

Social reconciliation and recurring policies are now implemented and tested. Non-expiring external-reader holds protect reclamation/repair. The managed nginx family uses private parent-child pipes, kernel pidfds, saved process identities, and boot/namespace fencing. Focused Rust cancellation and actual slow-reader/crash tests pass. Public export/withdrawal and backup/restore integration remain open; the feature is disabled and this ticket remains in_progress. See docs/direct-serving-lifetime.md for exact next acceptance conditions.

## Schema33 progress

Focused real PostgreSQL/strfry/nginx withdrawal and exact-token drain-release tests pass, including another physical-root mismatch and a forced late-commit race. Positive linking is test-seeded; automatic export visibility is still disabled. See docs/direct-serving-next-slice.md for the remaining acceptance work. Full regression/build are tracked separately, and the ticket remains in_progress.

## Positive projection progress

The explicit verified positive-link primitive and protected backend-removal acknowledgement barrier pass focused real-component tests. The main fixture no longer test-seeds the alias. A moderator-approval race prevents a blocked export from linking, and failed visibility retains its creating hold. Automatic work scheduling, quarantine, guarded startup and backup/restore remain open; no config enables direct serving.

## Offline backup progress

Derived public aliases/process records are excluded from backup; legacy snapshots containing them are rejected. Verified independent restore copies clear only target reader holds and preserve source holds. Six Python safety tests and the actual-node restore fixture pass. Automatic scheduling, whole-publication quarantine and guarded startup remain the direct-serving gates.

## Schema34 queue progress

Opt-in role registration, indexed64-row backfill, durable metadata-triggered work,16-item owned quanta, generation/version/root fences and bounded retry pass focused/full/final tests. Restore clears copied roles/queues for fresh backfill. Node activation, complete quarantine handling and guarded startup remain open; no public-serving config is enabled.

## Final implemented-profile gate,2026-10-07

The opt-in managed original reader, guarded-policy startup, failure closure and supervisor recovery pass the final full PostgreSQL suite (43 test-result groups), Clippy and process/restart checks. The source-only archive rebuilds and its unit tests pass using the pinned existing dependency cache.107Python model tests also pass. See docs/managed-public-serving.md and results/serving-role-full-final-2.log. This closes the specified reference-profile implementation/functional gate, not production readiness or100TB/300M-save deployment capacity. Conservative restart-after-fault behavior and foreign host/namespace recovery limitations remain explicit. Ticket18 stays blocked on target infrastructure.
