# Application queue fixture reconstruction

The active PostgreSQL foundation, publication fence, HTTP restore and separate-process fixtures use actual application sync batches: SyncSpec, enqueue_sync, claim_sync, save_missing, finish_sync_quantum, cancel_sync and sync_status. No generic Repository enqueue/claim/complete/cancel callers remain in the active server source or tests. The server refactor owner was notified that their removal is safe after these replacement gates passed.

Verified 2026-10-10 against isolated real PostgreSQL on port 55437:

- postgres_foundation: concurrent explicit migration, exact immutable event reads, transactional failure visibility, removal fencing, real sync coalescing and parallel claims, expired/stale/cancelled work, reconnect and pool exhaustion recovery.
- publication_fences: prepared generation/permission/removal fences, faulted publication retry and competing commits, cancelled/expired real sync lease rejection followed by successful recovered publication.
- http_migration: real HTTP/strfry migration and Python backup/restore; stable identities, retained original bytes and readable library, tombstone and group/account revocations, stale authorization/blob tokens and worker lease rejection, fresh worker recovery, independent copied files, paused serving/signup/schedules. Enrollment preservation is checked through real limit errors rather than budget-table counts.
- multiprocess: 12 writer processes admit one event; four real sync workers complete exactly 32 distinct batches; an immediate PostgreSQL stop fails closed, restart restores authoritative reads, and a fresh process reads the exact expected event.

Final assertions use actual public lifecycle return values, immutable event contents, sync status, HTTP and CLI/filesystem outcomes. SQL is limited to external fault/resource arrangement; cache checks no longer depend on internal counters or Arc identity.

Logs and structured results: results/rebuild-queue. The initial process runner failed because ephemeral port/socket overrides were not persisted for PostgreSQL restart. The corrected persistent configuration passed; this was a harness configuration failure, not a production fix. Both logs are retained.

Replaced historical files are outside the active test tree in retired/rebuild-queue-fixtures. This slice does not establish target traffic capacity, power-loss safety, or production readiness.
