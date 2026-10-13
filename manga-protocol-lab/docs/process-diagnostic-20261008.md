> **Historical October 8 process diagnostic.** The preserved report below describes its original revision,
> including any then-pending work, test counts and measurements. It does not
> establish the current reconstructed build's status or capacity. For current
> requirements and evidence use [rebuild status](rebuild/STATUS.md) and the
> [parity contract](rebuild/PARITY-REQUIREMENTS.md). Any “current” navigation
> inside the preserved body refers to that historical report's time.

# Process and lock diagnostic, 8 October 2026

This follows the saved schema49/source checkpoint21. It is a diagnostic run with sampled server stage traces and frequent PostgreSQL activity sampling, not a capacity certificate or a fair latency comparison with unprofiled runs.

Exact data: results/library-node/reclamation-index-process-profile-20261008/. Configuration: results/reclamation-index-diagnostic-config.json. The run accepted43,200 writes without rejection but took432.15seconds for360seconds of scheduled arrivals. Existing performance failure remains unresolved.

## Observations

- Mean observed PostgreSQL CPU was approximately3.3 cores before cleanup and4.0 during the selected active-cleanup window; node CPU approximately1.2–1.3 cores. These sampled lower bounds miss processes that begin/end between samples. They do not establish the host's dedicated CPU quota.
- Account-admission and capacity-grant queries appeared frequently waiting on transaction/tuple locks. Query-presence samples are not execution counts, time attribution or a complete blocking graph. No single blocker is proved by those samples alone.
- Sampled median cleanup claim was7.23ms and refund13.86ms, versus0.24ms for filesystem cleanup. These are wall-stage timings including waits; adding medians is not a measured end-to-end median. Storage credit/retention work also dominated sampled foreground stages relative to file sync.
- Summed PostgreSQL RSS includes shared mappings multiple times. It is not physical host memory consumption and must not be compared directly with node RSS as if both measured private memory.

## Next controlled hypothesis

A ten-minute unprofiled run of the same application binary uses64 credit lanes instead of16, retaining32 database connections,16 processing slots,8 commits,16 shared storage slots and8 cleanup workers. The existing lane implementation and64-to16 rebalance tests are unchanged. More lanes may reduce shared-grant contention but can strand more idle credits until rebalance; no default is changed and no throughput improvement is assumed. Process sampling remains enabled at the ordinary five-second interval. Compare final accounting, rejected attempts, scheduled latency and cleanup debt as well as throughput.

Repeated SQL parsing/planning and backend acknowledgement lock duration are additional candidates for investigation if needed. Do not weaken admission, lock ordering, generation checks or receipt-backed accounting to obtain a faster benchmark.
