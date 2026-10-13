# Bounded library throughput controls

Current scheduling update: [storage fairness](storage-fairness.md) adds one shared volume-work budget, foreground reservation and headroom-responsive background allowance. Measurements below are historical and do not qualify the changed scheduler.

The full correctness suite passed in results/library-throughput-regression.log, including real PostgreSQL/strfry, all-target lint and process/restart checks. The release load rerun is still pending; correctness does not establish capacity.

## Admission and phase scheduling

The original node admitted four combined write requests, holding those slots during network receipt, decoding, staging and root commit. The initial concurrent measurements exposed request rejection before the required120 saves/s baseline.

The refinement separates:

- Up to128 admitted combined/batch requests, retained until their owned processing finishes.
- A64MiB weighted compressed-wire/aggregation budget. Each request reserves twice its declared wire length plus64KiB. An absent length reserves the full8MiB wire limit; at most three such worst-case requests fit concurrently. The actual body reader uses the declared limit, so the header cannot understate accepted bytes.
- Eight processing operations after receipt, each still subject to the8MiB decompressed-envelope and6MiB decoded-object bounds.
- Four storage operations and four root commits. Acquisition waits fairly for at most two seconds rather than immediately rejecting merely because another bounded phase is active.

The byte lease and request count move into the blocking decode and owned storage/commit task. Disconnects cannot release their permits while work continues. Body receipt retains its existing30-second deadline. Queue timeout remains explicit overload, not an unbounded queue. These are application payload/concurrency controls, not a claim that whole-process/TCP memory equals64MiB.

An intermediate implementation queued uploads but left commit-root reads on immediate rejection. Under load this was worse:169 of2,400 writes accepted. Both read and write phases now use bounded fair acquisition. The failed intermediate run is preserved rather than reported as a performance improvement.

Unit checks cover receipt/work independence, count/byte exhaustion, large or unknown lengths, cancellation and credit recovery. The actual storage fixture holds all four storage permits and verifies a root/object read waits, then succeeds after release instead of returning an immediate busy error.

## Projection batching

The original projector stopped at individual deletion barriers, repeatedly starting strfry for each replacement pair. The refinement collects at most64 outbox rows with the existing import-byte bound. It admits at most one import per replaceable author/kind address, imports the bounded current set, deletes explicit predecessor IDs, then scans to verify expected presence and absence before acknowledgement.

A version-stale job is never used as authority for a newer event generation. Retirement author locks are acquired in sorted order before event/hold updates. Predecessor holds remain pending until verified deletion; the transaction then starts their grace period. A failed or interrupted command leaves durable work for retry. No database transaction spans the subprocess work. Raw kind5 is still excluded from strfry.

The new real-strfry regression stages32 authors' replacement pairs before starting the projector, verifies all current heads, removed predecessors and64 retired holds, and checks the work uses bounded multi-event import/delete groups rather than32 singleton pairs. Existing timestamp-tie, overlapping replacement, permission, moderation and restart cases remain required in the full suite.

After the first five-minute run, the admitted count changed from64 to128 to absorb a measured subsecond slowdown. The byte and processing limits remain unchanged. Stage-specific overload responses distinguish request-count, byte-budget and work-queue pressure. This refinement awaits its sustained rerun.
