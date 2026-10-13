# Experimental durable replication jobs

This is a tested implementation direction, not a finalized federation standard. The accepted live discovery pagination is separate from these inventory snapshots. All examples remain loopback-only synthetic fixtures.

## Administration interface

- POST /jobs/sync with JSON peer (approved service public key), filter, inventory. Inventories are chapters, accounts and deletions; metadata filters apply only to chapters. Authenticated instance administrators receive HTTP 202 with an ID.
- GET /jobs/{id} inspects state, attempts, next attempt time, completed partitions and acknowledged imports.
- POST /jobs/{id}/cancel requests cooperative cancellation.
- POST /jobs/{id}/retry explicitly resumes a failed or cancelled job.

Every route requires the same exact-URL/method/body-bound, fresh, non-replayed NIP-98 admin signature as other administrative operations. Request examples are executable in prototype/tests/jobs.py. An active duplicate with the same exact peer/filter/inventory returns the existing job ID. There are at most 64 active jobs; retry cannot evade that cap or revive a duplicate active job.

## Durability and cancellation

SQLite persists jobs and partition work. An exclusive OS lock prevents two wrappers from owning the same instance directory. After process restart, unfinished running work returns to queued state. Completed partitions remain recorded; already admitted publications remain retained. Fully verified local blobs can be reused on retry. Partial blob downloads do not resume at byte offsets.

The worker is deliberately single-threaded at the job level. It retries failures with bounded exponential delay, up to eight consecutive failures; explicit retry clears the terminal failure. A successful partition resets that failure streak. Completed job history is currently retained without automatic pruning.

Cancellation is cooperative, checked during an async operation at roughly half-second intervals. It does not undo already admitted content and cannot revoke a blocking filesystem operation that already started. A persisted generation fence prevents old work from finishing, splitting or failing a job after cancellation/retry. Import counters are acknowledged work, not exact unique-row accounting: a crash can undercount an import. Completion follows partition work state, not the counter.

## Inventory partition extension

An optional lowercase hexadecimal prefix, length 0–64, selects the inclusive event-ID range from prefix+zeros through prefix+fs. It is bound to the reconciliation session and echoed in responses. Nonempty prefixes require the peer capability event-id-hex-prefix-v1. SQL values stay parameterized, and returned IDs are checked against the requested prefix.

Each snapshot retains its 100,000-entry and SQL-work budget. On an explicitly overfull snapshot, the worker replaces a range with its 16 next-hex-digit child ranges. A job allows at most 8,192 work records and depth 64; exceeding these bounds fails explicitly. There is no silent truncation. Separate partitions are separate snapshots, not one globally atomic view; later changes require another sweep. Existing session time/round/byte ceilings remain in force, so high-latency/disjoint sets may need smaller filters or a future adaptive timeout/partition strategy.

## Evidence and limits

The two-node partition fixture starts with 120,000 source publications and 119,998 receiver publications. They have valid signatures and actual retained original bytes, but all reference one synthetic original/manifest. Setup deliberately uses bulk SQLite and verified strfry import, bypassing HTTP admission throughput. The promoted generation-fenced build reconciled 16 partitions and imported the two missing publications in 4.58 seconds after 43.49 seconds of fixture seeding. See results/next/partition-scale.json for the precise run.

Unit tests cover exclusive ownership, restart recovery, coalescing, bounded failure backoff, cancellation/retry, stale generation rejection, active-job budget, prefix range boundaries, and maximum-depth refusal. Actual integration tests pause/restart a peer and kill/restart a receiver with work active.

This does not demonstrate 100 TB of storage, production ingestion throughput, WAN behavior, host power-loss durability, peer fairness, or full distributed consistency. The single SQLite connection, per-event strfry subprocess adapter, no partial-blob resume, lack of atomic transfer reservations, provisional wire allocations and incomplete service identity/TLS remain release gates.

The promoted optimized job build also passed a full one-hour mixed-format operational run: 3,569 cycles, 59 receiver restarts, asynchronous chapter/account/deletion jobs, periodic independent moderator decisions and membership changes. Its exact build and resource samples are recorded in results/soak-jobs-release.json. This is still a small local synthetic workload, not a target-scale certification.
