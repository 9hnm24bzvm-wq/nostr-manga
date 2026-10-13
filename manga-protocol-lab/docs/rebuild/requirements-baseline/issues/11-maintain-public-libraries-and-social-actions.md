# 11: Maintain public libraries and social actions

**What to build:** Publish and replicate the single public library per key plus supported public comments/reactions using explicit Nostr event rules.

**Blocked by:** 04: Project signed events through strfry, 08: Synchronize one approved peer chapter

**Status:** done

**Spec stories:** 28, 29, 30

## Shared context

This is the scale-first Nostr-based manga reference node. Use PostgreSQL with deadpool-postgres/tokio-postgres and Moka, never SQLx. Originals remain owned and verified; moderation and peer approval are local. Work directly in this chat/computer environment. Website UI and production deployment are outside this ticket.

## Acceptance criteria

- [x] Replaceable snapshots and timestamp/ID ties behave deterministically.
- [x] Ordinary library/social NIP-09 handling stays separate from protected-publication moderation.
- [x] Progress references exact publication IDs and public exposure is explicit.
- [x] No private-library or master-key subsystem is introduced.

## Q95 performance investigation

The user prefers one whole-library snapshot (A), but allows consideration of a chunked external-manifest pointer (C) if realistic measurements justify it. The requested reader sizes are30,000 and500,000 completed chapters. The experimental implementation/results live under `bench/library-ac/`, `server/examples/library_ac.rs`, and `results/library-ac/`. This evidence does not mark the production library/social acceptance criteria complete. Keep ordinary NIP09 separate from protected manga publication moderation.

### Decision resolved

At20:45UTC the user explicitly chose C. Implement the chunked signed-root library format in the production node, rather than leaving it only in the experimental service. Preserve one public library per key. Prioritize verified local ownership, bounded upload/commit, replacement/deletion ordering, read grace, and generation-safe reclamation. Do not claim Ticket11 complete from benchmark-only code.

Implementation invariants and encoding draft: docs/adr/0002-chunked-public-libraries.md.


## Account foundation checkpoint

Migration0012, accounts.rs and the initial account HTTP routes now provide explicit local admission, revocation epochs, deterministic replaceable metadata heads, authenticated personal posting and public head lookup. An actual-strfry test exercises32 competing timestamp ties; HTTP tests cover replay, author mismatch, replacement and revocation. Full regression including process/PostgreSQL restart and all-target Clippy passed in results/accounts-http-full-regression-2.log. Library kind11999 remains rejected here until object admission is integrated. See docs/account-replacement.md and docs/library-retention-design.md. Ticket remains in progress.


## Library staging checkpoint

Migration0013 and library_storage.rs add canonical bounded uploads, durable-generation summary records and account-specific staging grants. The HTTP endpoints bind the account signer/body and do not expose staged objects anonymously. Tests passed for deduplication, private access, post-I/O revocation, corrupt-file rejection and full existing regressions. Root hosting, retention/public serving and ordinary deletion remain incomplete. See docs/library-staging.md.


## Shared-object accounting prerequisite

Migration0015 separates one-time staging credit release from receipt/health rows. Existing manga publication uses compatible shared health locks and only locks unreleased accounting rows; actual competing-publication and ledger-conservation tests pass. An independent indexed-hold experiment excludes500,000 expired historical reference rows without a sequential scan. This is groundwork for root commit, not completion of root publication. See docs/storage-retention-credits.md and docs/library-retention-design.md.


## Public root checkpoint

Migration0016 and library.rs now admit signed complete roots, maintain indexed per-author delta holds, preserve predecessor bytes through actual strfry retirement, expose guarded public reads and apply exact-retry/expected-head semantics. The full HTTP/PG/strfry regression passes; see docs/public-library-api.md and results/library-root-http-regression.log. Batching, ordinary deletion and integrated large-reader load testing remain open. Ticket11 is not marked complete.


## Batched transport checkpoint

Combined changed-object/root commit, bounded staging batches, gzip public bundles and response-memory/deadline controls pass the full regression. Exact wire-body NIP98 binding and mixed public/private batch rejection are tested. See docs/library-batch-api.md. Integrated30k/500k HTTP measurements and ordinary deletion remain open.

## Integrated performance checkpoint

The actual release node now completes both full reader sizes through production HTTP/storage/accounting/strfry paths. Corrected single-reader results live in results/library-node/serial-02 and docs/library-node-performance.md. Concurrent paced-reader measurements are in progress; do not substitute older experimental service throughput as node capacity. Physical reclamation, ordinary-deletion policy and library peer replication remain incomplete.


Latest checkpoint: selected approved-peer library/account transfer, public social actions, ordinary author deletion and Q93D/Q94A pass the combined schema23 regression in results/discovery-peer-repair-full-regression-2.log. Bulk account inventories remain an explicit follow-on beyond selected snapshot/event transfer.
