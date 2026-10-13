# Prepared-query result: checkpoint 22

The same mixed-reader write experiment now passes its provisional local gate after fixed-query preparation was cached. This is a bounded development result, not production qualification. All failed predecessor trials are retained.

## Verified result

- 128 synthetic readers: 126 libraries with 30,000 completed chapters and two with 500,000.
- 120 scheduled saves/second for 600 seconds; all 72,000 accepted in 600.198 seconds; zero rejections and zero monitor errors.
- Schedule-to-acceptance median 228.48 ms, p95 362.19 ms, maximum 2.716 seconds.
- The 30k-reader group had p95 320.38 ms and p99 1.014 seconds. The 500k group had p95 612.09 ms and p99 1.912 seconds. Not every save was subsecond.
- Backend projection drained in 0.395 seconds. Cleanup was enabled throughout. After the five-minute grace, sampled maximum overdue age was 0.278 seconds and maximum due candidates 70. These are five-second samples, not a continuous maximum.
- Final ledger: 1,073,741,298 physical-used bytes equaled blob-intent bytes; zero conservation violations. Forty cleanup candidates remained due and none claimed at that snapshot; the queue was not claimed to be empty.

## Exact configuration

PostgreSQL connections 32; library request slots 128, processing slots 16, commit slots 8; shared storage slots 16; cleanup fan-out 8; credit lanes 16; configured Moka cache 64 MiB. Defaults were not changed. PostgreSQL fsync and synchronous_commit were enabled. The synthetic connection model adds 80 ms round-trip delay, 2 Mbps uploads and 10 Mbps response consumption; it is not real WAN/TLS or packet-loss testing.

The node, PostgreSQL, strfry and generator share the cloud host. No compilation ran during the timed workload. The source uses PostgreSQL/deadpool/tokio-postgres and Moka, not SQLx. A more capable host and higher concurrency are not free resources for the initial VPS.

## Change and correctness checks

102 fixed SQL call sites now use the existing per-connection prepared-statement cache. SQL text and argument order, transaction boundaries, locks, authorization checks, generation checks and receipt-before-refund sequencing are unchanged. The transformation was mechanically reversed to check those invariants. There is no cached authorization result or new dependency/schema migration in this change. The prior full object-reference index and bounded cleanup control remain.

All 64 functional result groups pass, including 87 library unit tests with one optional stress test skipped, Clippy with warnings denied and independent-process/PostgreSQL restart checks. 115 Python tests and the optimized two-node demo pass. The sparse 500k-row retention test verifies both forced generic and custom prepared plans remain indexed.

## Evidence and remaining work

Current results: results/library-node/query-cache-128-120-20261008/, results/query-cache-full.log, query-cache-gate.json, query-cache-release.sha256 and query-cache-transform.json. Per-reader tails are in reader-groups.json. Bulky raw samples are supplied separately with checksums and Library identities in RAW_DATA_ARCHIVES.json. See checkpoint20-capacity-check.md and process-diagnostic-20261008.md for failed runs and the diagnostic rationale.

This meets the rounded average-rate baseline for 300 million saves per 30-day month only within this ten-minute synthetic experiment. It does not certify a month, bursts, a large reader population, simultaneous catalogue/image traffic, real networks, failover, power loss, 100 TB storage or independent security. A longer steady-load run across more cleanup/checkpoint cycles is the next validation step.

The extracted source archive also passes library/binary tests and compiles the retention-plan and both cleanup/index fixtures. Application source is byte-identical to the fully tested working tree; see query-cache-archive-rebuild.log and its zero exit record.
