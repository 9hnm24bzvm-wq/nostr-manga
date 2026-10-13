> **Historical SQLite prototype coverage.** The preserved report below describes its original revision,
> including any then-pending work, test counts and measurements. It does not
> establish the current reconstructed build's status or capacity. For current
> requirements and evidence use [rebuild status](rebuild/STATUS.md) and the
> [parity contract](rebuild/PARITY-REQUIREMENTS.md). Any “current” navigation
> inside the preserved body refers to that historical report's time.

This page describes the historical SQLite behavioral prototype. It is not acceptance evidence for the selected PostgreSQL server. See START-HERE.md and the PostgreSQL ticket graph for current status.

# Prototype coverage and release gates

This inventory distinguishes working research behavior from the final protocol and production readiness. Q90–92 are now selected and implemented; Q93–94 remain pending.

## Exercised end to end

- Individual and instance-held group signatures; member authorization; series/chapter group publication.
- Upload original and deterministic manifest, then publish immutable event.
- Hash, declared byte length, format, dimensions, actual PNG decode, schema version and required-feature checks.
- Complete-before-hosted discovery and self-hosted peer replication.
- HTTP body/URL/method-bound authentication, persistent replay rejection.
- WebSocket NIP-42 challenge and basic NIP-01 retrieval.
- Real Negentropy sessions with repeat-sync convergence and explicit resource release.
- Separate reconciliation of deletion requests; no automatic protected-publication deletion.
- Local approval and persistent replay tombstones; explicit restore.
- Shared image references and independent retention on another instance.
- Original nginx byte/range delivery; last-reference withdrawal.
- Detected corruption withdrawal followed by verified repair.
- Three-node manifest/image fallback after primary disk faults; changed service key at the same fallback endpoint is rejected.
- Whole replaceable public library event replication.
- Experimental admin-only durable sync queue, restart recovery, cancellation/retry, and adaptive prefix partitioning beyond 100,000 inventory records.
- Ordinary exact-ID and replaceable-address deletion, including deletion-before-target.
- Group-key deletion request signing with moderator approval remaining separate.

## Tested at module/model level

- Bounded CBOR preflight, canonical byte equality, duplicate keys and malformed-input corpus.
- Stale prepared admission cannot resurrect a removed event or host an evicted/corrupt blob; final filter rejection rolls back metadata.
- SQL injection strings remain bound values; complexity caps and typed field whitelist.
- Exact case-sensitive non-language matching and three-valued absence are enforced; earlier alternative-mode tests remain research evidence.
- Compact reconciliation snapshot independent of discovery limit; overbudget rejection.
- Blob byte accounting stays constant on duplicate upload; pressure eviction excludes hosted originals.
- Persistent scrub cursor advances beyond the first thousand records.
- Small executable lifecycle model includes simulated transaction rollback and staging expiry.
- SQLite reader/writer experiment demonstrates WAL retention during a held snapshot.

## Implemented but needing stronger adversarial/multi-peer testing

- More extensive fallback/retry timing and concurrent-failure cases beyond the passing three-node fixture.
- Request body/header, event, image and reconciliation resource ceilings.
- Recoverable event-store outbox, bounded immutable-event batches, deletion/replaceable barriers and per-record readback verification.
- Foreign-key ownership cleanup and staged object lifecycle.
- Group series/deletion API and conservative admin/publisher role matrix.
- Basic language filtering and well-formed BCP47 metadata admission.

## Not finished

- Production TLS and channel-bound service identity, automatic first-key remembrance, signed domain migration.
- Fair multi-worker transfer reservations and byte-level resumable concurrent large imports; a bounded durable single-worker queue/backoff is implemented. Configurable staging above 32 MiB has passed a real 37.8 MB chapter test.
- Production strfry write adapter instead of per-event subprocesses.
- Full Nostr relay interoperability and all protocol extensions.
- Complete comments/reactions API semantics and library/client schema validation.
- Categories, rich series search, all metadata fields and MangaDex-compatible migration/API.
- Final ordering/count wire choices, full query-language coverage and representative concurrent deep-offset load beyond the offline-seeded 500,000-row fixture.
- On-disk migration between prototype versions, production schema migrations, recovery from every filesystem/DB crash point.
- Production online backups, key vault, operational monitoring and complete audit logs. An offline synthetic whole-instance restore drill passed; see recovery.md.
- Independent security review, abuse testing, production endpoint exposure.
- 100-TB physical storage and representative cold-working-set tests.
- Sustained distributed Internet traffic at 8B images/350M pages per month.
- Website UI and anime streaming/transcoding.

## Before calling this production-ready

Resolve protocol choices, select final kind/schema/endpoint allocations, close the above gaps, run conformance tests against independent implementations, benchmark on the intended infrastructure, exercise realistic outages/rebuilds/deletions, and review authentication/key handling. The research prototype is evidence for architectural feasibility, not proof these release gates have been met.

## Additional regression findings

- The original 64-round reconciliation limit was too low for legitimate equal-timestamp 100,000-ID inventories. The 256-round/16-MiB bounded replacement passed an actual 87-round authenticated HTTP exchange.
- An observed missing/corrupt shared original now withdraws every hosted local publication referencing it, including records outside the current 128-item scrub batch.
- Re-uploading a staging blob renews its expiration rather than leaving it immediately eligible for expiry.
- Public metadata browsing is anonymous by default; optional metadata authentication does not turn the public-image demo into a private-instance implementation.


## Further implementation fixes

- Early body-independent token checks now reject unauthenticated upload/control requests without waiting for the body; bounded probes passed on direct and nginx paths.
- JPEG and WebP publication/replication preserve the exact original bytes. A receiver can restrict itself to PNG, reject those manifests before fetching originals, then explicitly change policy and retry.
- New group members can upload and publish through that group without global individual-publisher access. Revocation removes the scope, and restarting does not reseed revoked bootstrap membership.
- Failed physical staging deletion preserves ownership and accounted bytes; protected deletion responds with a policy rejection.
- NIP-11 responses use the requested application/nostr+json media type and advertise implemented NIP-42 support.

A valid-file/missing-index crash-window regression exposed and then verified repair of a false-hosted advertisement. Indexed blob size/MIME are now checked before hosted-state commit; existing valid bytes are re-admitted through accounting rather than blindly skipped. See the before/after recovery reports.

The current regression suite also covers recognized temporary-file cleanup without deleting valid unindexed originals, side-effect-free per-identity quota rejection, actual live discovery pagination/time bounds and strict selected text/null semantics.
