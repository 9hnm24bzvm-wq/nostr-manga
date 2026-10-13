# Configurable library concurrency

The default pipeline remains128 admitted requests,8 processing tasks and4 concurrent root commits. Optional node configuration `library_concurrency` accepts `request_slots`, `processing_slots` and `commit_slots`. Processing is bounded1–32; commits1–16 and no greater than processing; admitted requests must be at least processing and no greater than512. Unknown fields are rejected.

The64MiB weighted wire budget, decompressed-envelope/object limits,64MiB response budget and two-second queue deadlines remain unchanged. Raising concurrency increases the possible decoded working set, so it requires memory headroom; it is not a free throughput switch. Storage I/O slots and PostgreSQL connections are separately configured.

Historical larger profiles used256 admitted requests,16 processors,8 commits,8 storage slots and32 database connections. Current checkpoint20 measurements are recorded in checkpoint20-capacity-check.md and failed the sustained gate once cleanup was active. This is not evidence that the user's initial small VPS can sustain the same rate. Default settings have not been silently changed.

The benchmark writes node-limits.json so results identify the actual configuration rather than relying only on defaults. Fixture environment knobs are NODE_LIBRARY_REQUESTS, NODE_LIBRARY_WORK, NODE_LIBRARY_COMMITS, NODE_STORAGE_SLOTS and NODE_DB_CONNECTIONS.

Storage configuration also accepts reclamation_slots (default4, range1..16), bounding cleanup-page fan-out and the maximum background share. Actual volume work now uses one fair admission owner under upload_slots: normal cleanup has a smaller allowance and low quota/filesystem headroom can raise it, while reserving foreground capacity for totals of at least2. A one-slot configuration interleaves FIFO without increasing its capacity. Cleanup no longer consumes upload request/body receipt permits. See [storage fairness](storage-fairness.md) for exact limits, headroom hysteresis and cancellation behavior. The NODE_RECLAMATION_SLOTS fixture knob records the configured maximum. The historical ten-minute8-cleanup/16-storage-slot experiment failed the capacity gate and does not qualify this changed source.

## Previous accepted validation

`library_concurrency.validation_cache_bytes` defaults to 8 MiB and
`validation_cache_entries` to 256. Set both to zero to disable reuse. Otherwise
bytes must be 1..=64 MiB and entries 1..=4096. Unknown fields and partially disabled
configurations are rejected. This is an instance-local Moka eviction budget,
using conservative allocation weights and a minimum charged weight per entry to
bound both memory and author count. It is not a bound on total process RSS:
admitted in-flight roots retain their own finite working set.

Only a successfully committed root can populate this cache. Lookup requires the
exact previously accepted event ID. Root decoding is reused only for identical
verified hash/size; unchanged descriptors reuse summaries only with the same
family/bucket position and exact current generation. Every commit still performs
current receipt/grant reads, sorted blob locks, current health/authority checks,
root aggregate/bucket validation and the in-transaction head/CAS/winner checks.
Root bytes still pass actual verified storage reads. A stale entry, cold process,
eviction or disabled cache falls back to full validation. No cached fact grants
authority, proves current visibility or advances a generation.

The current accepted hold set also makes unchanged generation credit transfer a
no-op. Newly added objects still use the complete durable staging-to-retention
accounting operation; removed objects retain backend retirement and grace rules.
Cold/changed roots may require an extra summary query compared with the previous
single joined preparation query. The reduced immutable work is established by
public lifecycle fault tests and numeric diagnostics, not a general throughput
claim.

Opt-in `MANGA_BENCH_PROFILE=1` emits anonymous numeric stage/count diagnostics.
`MANGA_BENCH_PROFILE_EVERY` selects one sample per 1..=256 operations (default16);
setting1 is useful for a small controlled diagnostic fixture. It remains disabled
by default. Counters include decoded-root reuse, requested/reused summaries and
new retention objects. They are diagnostics, not acceptance outcome oracles.
