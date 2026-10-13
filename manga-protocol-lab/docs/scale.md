# Capacity: demonstrated behavior and unproven requirements

## User targets

100 TB original-image library, 8 billion image requests/month, 350 million page views/month. Using a 30-day month:

- Images: 3,086 requests/second average.
- Page views: 135/second average; each page can generate multiple API requests.
- At 100 KB/image: 800 TB/month, 2.47 Gbit/s average payload.
- At 500 KB/image: 4 PB/month, 12.35 Gbit/s average payload.
- At 1 MB/image: 8 PB/month, 24.69 Gbit/s average payload.

These use decimal bytes, exclude headers/TLS/retries, and are arithmetic scenarios, not observed traffic. A 5x traffic peak at 500 KB/image means roughly 61.7 Gbit/s payload. Replication, backups, and cold-cache fills consume additional bandwidth.

A 200 GB initial disk holds approximately 400,000 500 KB originals before metadata, filesystem overhead, staging headroom, and replicas. It is not a miniature 100 TB deployment; storage must actually grow.

## Measured local serving behavior

See results/serving-benchmark.json. One warm 492,053-byte synthetic PNG, no TLS, loopback, one nginx worker, Python generator sharing the same computer:

- Direct nginx, concurrency 16: 5,872 requests/s, 23.11 Gbit/s payload, p95 3.71 ms.
- Guarded Rust-to-nginx internal redirect, concurrency 16: 5,730 requests/s, p95 4.10 ms.
- At concurrency 1, guarded reads were materially slower: 1,506/s versus direct 3,965/s.
- At concurrency 64, both got slower, consistent with a shared generator/server resource ceiling. No bottleneck attribution was established.
- Zero HTTP errors during these short two-second trials.
- Admitted static images still served after the Rust process stopped. This proves control/data separation for already admitted files, not host failure tolerance or fresh moderation availability.

The absolute numbers cannot certify any of the user's target-scale requirements. The meaningful architectural result is that public reads can literally be nginx static-file reads, without per-image Rust signature verification.

A later pass used the hash-sharded filesystem layout with six 30-second trials (180 seconds total, 730,538 requests, zero HTTP errors). Direct nginx at concurrency 16 delivered 5,369/s, p95 3.83 ms; guarded delivery delivered 5,181/s, p95 4.30 ms. Concurrency-1 rates were 4,081/s and 1,598/s; concurrency-64 rates were 4,103/s and 4,016/s. These remain warm loopback measurements. Binary checksums and raw outputs are retained in results/serving-30s-binaries.sha256 and results/serving-benchmark-30s.json.

## Metadata experiment

One million synthetic rows, 399 MB SQLite database. Indexed exact-series lookup returning 1,000 IDs: warm median 1.22 ms. A broader language/category query returning 133,334 IDs: around 1.79 seconds. These exclude HTTP/signatures, use a separate research schema, and do not prove the wrapper's API latency. Full outputs include execution plans.

The stark difference supports bounded queries, suitable composite indexes, cardinality-aware admission, and pagination rather than promising every expressive query is fast. Long-lived snapshots retain WAL pages; session lifetime and total outstanding memory/WAL must be bounded.

## Necessary production evolution

1. Keep the image data path on nginx/object storage/CDN owned or controlled by that instance. No runtime peer leeching.
2. Shard blob storage behind deterministic content hashes. Each instance independently accounts for and retains every original it advertises.
3. Replace per-event subprocess ingestion with a durable batch/private relay adapter while preserving the deletion gate. Verify commits rather than trusting process exit status.
4. Move metadata and inventory work off the single serialized connection. Add indexed query plans, quotas and measured backpressure; harden the experimental partitioned durable job queue for multi-worker fairness and reservations.
5. Test full/cold working sets, filesystem inode density, directory sharding, disk rebuild and corrupted/missing originals, cache expiry, and approved-deletion propagation to edge caches.
6. Validate on the intended machines with real network constraints, TLS, representative images and filters, sustained load, peer outages, failover, and real storage occupancy.

No distributed system can guarantee being impossible to take down. Federation can remove individual operational dependencies; it does not remove content obligations, operator policies, bandwidth bills, or correlated infrastructure failures.

## Anime is a separate workload

At an illustrative 5 Mbit/s per viewer, 10,000 simultaneous viewers need 50 Gbit/s payload; 30,000 need 150 Gbit/s. Segment delivery can use static nginx paths, but transcoding, renditions, segment schemas and video admission are not implemented in this manga prototype.

## Actual HTTP metadata path

With 100,000 offline-injected synthetic signed-event rows, the real anonymous nginx/Rust/SQLite endpoint returned 100 events (44,390 response bytes) per selective query. In the debug build, concurrency 16 reached about 709 requests/s with p95 32.4 ms; concurrency 64 stayed near 709/s while p95 rose to 121 ms. This shows queuing, not more capacity from more concurrency. The broad unfiltered query rejected explicitly rather than returning a truncated catalogue. This experiment bypassed media admission and strfry ingestion deliberately and cannot be used to claim 100,000 hosted chapters.

## Reconciliation round-budget regression

The in-process Negentropy test exposed a real integration limit: with 100,000 IDs per side sharing one timestamp, even a 10,000-ID difference per side needed 87 round trips at the selected 64-KiB frame size. Fully disjoint 100,000-ID sets needed 99. The original 64-round cap therefore aborted valid large inventories.

The prototype now permits at most 256 rounds and 16 MiB cumulative binary reconciliation bytes, while keeping the 30-second session lifetime and 100,000-entry snapshot cap. A real NIP-98-authenticated loopback test with the 10,000-ID difference completed all 87 rounds, identified the exact differences, and closed its session. The initial measured duration was 9.71 seconds in a debug build. This does not eliminate the need for inventory partitioning/resumption on high-latency links. Timestamp distribution materially changes this test: identical 100,000-entry sets with distributed timestamps reconciled in one round and 324 protocol bytes.

## Operational failure tests

Forty deterministic-timing SIGKILL/restart cycles exercised publication and moderator approval. After each restart, SQLite integrity and foreign keys were checked, blob byte accounting matched stored rows, and the durable strfry outbox drained. Acknowledged events remained retrievable; a committed last-reference removal stayed inaccessible. SIGKILL is not a power-loss, disk-flush, or whole-machine recovery simulation.

A separate real 37,809,577-byte chapter (three original PNGs) was rejected by a receiver's 32-MiB staging policy without being advertised as hosted. After explicitly raising that receiver's staging allowance to 64 MiB, retry completed and all original hashes matched. This demonstrates configurable admission policy, not a completed multi-worker reservation scheduler.


## Optimized-build comparison

The optimized release build completed six more 30-second warm-file trials: 703,704 requests, zero HTTP errors. At concurrency 16, direct nginx delivered 4,969 requests/s (p95 4.02 ms), and guarded delivery delivered 4,880/s (p95 4.22 ms). At concurrency 1, they delivered 3,464/s and 2,248/s. Shared-machine and generator variability prevents attributing cross-run differences to the compiler alone.

The actual HTTP metadata benchmark in the release build returned 100 events per query at 2,900 requests/s, p95 10.65 ms with concurrency 16; concurrency 64 reached 2,543/s, p95 39.53 ms. The index was seeded offline and deliberately did not exercise image admission or strfry ingestion. These remain loopback research results, not Internet or target-scale claims.

## Duration-test chronology

A one-hour release-build operational soak was started on 6 October 2026 around 06:36 UTC. It continuously publishes, replicates and reads originals; reconciles independently moderated deletions; updates public libraries; revokes/restores a group member periodically; and restarts the receiver every minute. It stops on a failed assertion and records status/RSS samples in results/soak.json. This historical baseline finished successfully; final results and build distinctions are recorded below.


The current debug build subsequently passed a five-minute mixed PNG/JPEG/WebP operational run: 300 publication/replication cycles and four receiver restarts, with periodic moderation, library updates and member revocation/restoration. The test records its exact binary checksum in results/soak-current.json. This shorter current-build run is separate from the earlier frozen release's one-hour test. It is not valid to present the two binaries as the same soak result.

## Functional scaling blockers, beyond missing benchmark hardware

The current reference prototype is not merely untested at 100 TB; it also has explicit implementation limits that must be removed or worked around:

- Each reconciliation snapshot still rejects above 100,000 entries. The experimental async job now splits overfull event-ID prefixes recursively; the synchronous /sync route still needs a suitably narrow filter. The partition work budget and 30-second per-session deadline remain explicit limits.
- Q90B now provides live offset/limit discovery with timestamp filtering. Deep offsets still cost work and may hit explicit query budgets; concurrent changes can repeat or skip records.
- The async /jobs/sync route now persists work independently of the HTTP request and resumes unfinished partitions after a process restart. Production-scale imports still need fair concurrency, byte reservations and partial-blob resume; current retries reuse fully verified local objects.
- Per-event strfry subprocess ingestion and one serialized metadata connection are correctness-oriented integration choices, not the final write/query architecture.
- A complete MangaDex-compatible catalogue and account/list API is not implemented by the offline migration preview.

The prototype now contains an experimental event-ID-prefix partition extension and durable local administration queue, documented in jobs.md. Immutable IDs make partition membership stable, but a sweep of separate snapshots is not one globally atomic snapshot. The extension remains provisional and does not decide discovery pagination. Image serving remains a direct nginx path.


The frozen-release baseline hour subsequently finished successfully: 3,594 cycles, 59 receiver restarts, no failed assertion. Sampled peak RSS was about 11.3 MB and 11.5 MB for the two Rust processes. These are process samples from a small synthetic dataset, not a 100-TB memory estimate. The final record is results/soak.json with the baseline binary checksum alongside it.

After rebuilding the current source and rerunning validation, a separate optimized mixed-format one-hour run was started around 07:40 UTC. Its result is tracked in results/soak-release-current.json. It subsequently completed as recorded below.

The later optimized mixed-format hour completed successfully: 3,594 cycles and 59 receiver restarts. A subsequently discovered file/index crash-window defect was repaired afterward, so this hour is not evidence for that fix. The repaired optimized build passed the targeted orphan-recovery integration plus five minutes of mixed-format operation (300 cycles/four receiver restarts). This chronology is intentional: passing a soak did not prove absence of crash-consistency defects.

## Batched private strfry adapter experiment

A separate local adapter benchmark used 256 synthetic ordinary signed events per trial, three trials per batch size, fresh LMDB databases, and exact-ID readback after every import batch. Median rates were about 60 events/s for one event per import/readback pair, 862/s for batches of 16, 2,842/s for 64, and 7,077/s for 256. This includes subprocess startup and readback but excludes signature generation, wrapper admission, media transfer, SQL indexing and moderation. It ran on the shared research machine while the operational soak was active; it is not an isolated production capacity measurement.

A mixed valid/invalid-signature batch exited with status zero while retaining only the valid event. Exact expected-ID verification remains necessary before acknowledging durable work. Source, raw trial durations and scope are in prototype/tests/strfry_batch_benchmark.py and results/strfry-batch-benchmark.json.

This supports batching as a concrete next write-path experiment. It does not mean the wrapper now achieves those rates. The current wrapper still immediately drains per-event jobs. A proper batch adapter must preserve publication/approval ordering, treat deletion barriers carefully, bound batch bytes and delay, handle replaceable-event winners, isolate a rejected record without starving valid jobs, verify stored results and remain idempotent across crashes. A later bounded backlog adapter was implemented and fault-tested separately; see backend-outbox.md. This benchmark alone was not used to claim its correctness or live publication throughput.

## Repaired build with durable-job hour

The optimized generation-fenced job build subsequently passed a full 3,600.01-second mixed PNG/JPEG/WebP run using the asynchronous queue for chapter, account and deletion synchronization: 3,569 cycles and 59 receiver restarts, no failed assertion. It includes the earlier orphan-index repair. Sampled peak RSS was 12,177,408 and 12,464,128 bytes for the two Rust processes. Binary SHA-256: ad71e42e96126ed4a714e8e34116fadcb76a732dae3753606f6275917d7efb0f. See results/soak-jobs-release.json. The small synthetic workload and process restarts do not certify cold-storage, WAN, host-power-loss or target monthly capacity.

## Repeated-original admission optimization

A later targeted test exposed redundant validation: a 256-page manifest referring to the same 1,536×1,536 synthetic PNG decoded that original 256 times. On the tested optimized builds, median end-to-end publication time across three trials fell from 2.158 seconds to 0.0304 seconds after validating each unique original once per admission. Single-occurrence timings were 0.0294 and 0.0333 seconds, so this is an optimization for repetition, not a claim that ordinary unique-page chapters are 71 times faster.

Ordered page occurrences, reported page count and logical byte-size totals remain unchanged. Reusing a hash with conflicting size/MIME/dimensions is still rejected. Original bytes remain exact. The change passed 58 Rust tests, strict clippy, full integration validation and the targeted before/after test; it was made after the recorded hour-long async-job soak and is not covered by that older binary's hour. Raw build hashes and timings are in results/repeated-originals-before.json and results/repeated-originals-after.json.

This removes one inexpensive amplification case. It does not solve total decode budgets, fair scheduling or the fact that admission currently holds the serialized metadata lock while validating unique images.

## Retained-history accounting

The per-upload staging check previously scaled with retained ownership history. A 200,000-row actual-schema fixture measured about 465 ms for that lookup. Transactionally maintained counters now replace it; an actual small-blob HTTP comparison measured about 1.087 seconds before versus 2.90 ms after. See accounting.md for the seeded-metadata-only scope, counter correctness tests and migration cost. This does not establish full media-ingestion or target-capacity throughput.
