# Capacity checks after checkpoint 20

These are historical pre-query-cache experiments on the available shared cloud host, after the agreed reference backend feature milestone. The later passing local experiment is documented in query-cache-results.md. All sustained capacity gates below failed. Zero rejected writes did not mean that requests arrived or completed on schedule. No production target is certified.

## Workload and evidence

- 128 reader libraries: 126 with 30,000 completed chapters and two with 500,000. Each includes active progress, series and lists; see `bench/library-node/README.md` for exact assumptions.
- 120 scheduled saves/second approximates 300 million saves in a 30-day month. This is an average baseline, not a peak SLO agreed with the owner.
- One sequential client per reader. Schedule-to-acceptance latency includes client backlog. Failed attempts are retained and not retried.
- The application-paced connection model adds 80 ms round-trip delay, streams uploads at 2 Mbps and consumes responses at 10 Mbps. It is not a real WAN, TLS, packet-loss or congestion-control experiment.
- Library cleanup enabled; five-minute grace unchanged. PostgreSQL fsync and synchronous_commit enabled; statistics were not manually analyzed after setup. All data and keys are synthetic, listeners loopback-only, storage on the workspace overlay.
- Node, PostgreSQL, strfry and generator share the host. It reports nine available CPUs and about 9.7 GiB memory; no cgroup quota information is exposed. This does not establish dedicated resources. No builds ran concurrently with the sustained measurements.

Serial smoke checks also passed for both library sizes, with 12 saves per case and complete hash/count-verified restores. Paced median acceptance was 189.33 ms for 30k and 440.08 ms for 500k. Twelve samples are not a robust tail estimate.

## Sustained results

| Variant | Scheduled window | Accepted / rejected | Completion window | Scheduled-save p95 | Due cleanup candidates at final ledger snapshot |
|---|---:|---:|---:|---:|---:|
| Shipped checkpoint 20, default limits | 360 s | 43,200 / 0 | 505.46 s | 108.17 s | 10,708 |
| Shipped checkpoint 20, wider limits | 360 s | 43,200 / 0 | 373.82 s | 7.14 s | 8,313 |
| Cleanup fan-out candidate, schema 48 | 600 s | 72,000 / 0 | 703.03 s | 74.08 s | 4,140 |
| All-state index candidate, schema 49 | 600 s | 72,000 / 0 | 752.42 s | 101.21 s | 20 |

Every row failed the provisional latency/offered-window gate. All had zero monitor errors, fully drained backend outboxes at final verification and matching physical-used/blob-intent bytes with zero budget conservation violations. A due cleanup candidate is not necessarily removable: reference and lease checks still apply. Backend projection drain is separate from cleanup drain. Twenty due candidates is nearly caught up, not an empty queue.

The index candidate was slower overall in this run despite its better lookup plan. Extra index maintenance and shared-host variability are possible factors, not established causes. It must not be advertised as an end-to-end throughput fix. Its final ledger snapshot was 782,941,374 physical-used bytes, equal to blob-intent bytes; zero claimed cleanup jobs and 20 due candidates. Peak node RSS was 127.58 MiB, excluding database, generator, strfry and caches.

## Configuration comparison

All profiles retained 128 request slots, a 64 MiB configured Moka cache, 16 credit lanes and the existing request/body/receipt budgets.

| Variant | Database connections | Processing slots | Commit slots | Shared storage slots | Cleanup fan-out |
|---|---:|---:|---:|---:|---:|
| Default | 16 | 8 | 4 | 4 | fixed 4 |
| Wider | 32 | 16 | 8 | 8 | fixed 4 |
| Fan-out/index candidates | 32 | 16 | 8 | 16 | configured 8 |

Two separate 30-second sampled diagnostics found that raising only processing slots from 8 to 16 did not fix the missed rate. In the latter run, sampled median commit-slot wait was about 65 ms. This identifies contention, not a single exclusive cause. File sync was smaller than sampled database-related work. No diagnostic tracing was enabled for the long comparison runs.

## Changes tested

### Bounded cleanup fan-out

`storage.reclamation_slots` replaces the fixed four-job page fan-out. Default 4 and range 1–16; existing configurations keep their behavior. Every cleanup still acquires the shared storage semaphore and generation-specific mutation guard, checks references/leases and obtains the filesystem receipt before refunding charges. More workers can compete with foreground requests; the setting is not a free capacity increase.

A dedicated fixture expires 96 synthetic blobs, runs 16 cleanup workers behind two storage permits, and verifies complete collection, returned permits, zero used/reserved charges and exact budget conservation. The existing reference/lease/generation fence fixture exercises eight cleanup slots behind four storage permits. Opt-in sampled traces record cleanup queue, claim, filesystem and refund stages without object IDs or payloads.

### Complete object-reference index

The foreign key from `library_object_holds(hash,generation)` to `blob_intents` searches all reference states during deletion. Existing object-leading indexes were partial active/grace indexes; the foreign-key lookup has no state predicate.

On a synthetic 500,000-reference/128-publisher reproduction, PostgreSQL 18 used 129 searches through the publisher-leading primary key and 602 shared buffer accesses. Adding a complete `(hash,generation)` index reduced this to one search and three buffers. This was an index skip scan, not a claimed full-table sequential scan. The fixture also records the real parent-delete foreign-key trigger.

Migration 0049 adds `library_holds_object_reference`; the fixture verifies the production index is valid, ready, nonpartial and object-leading. The index bounds lookup work but adds space and write cost. No deletion check or ordering is weakened. Index construction can block writes; use a controlled migration and backup procedure rather than an unplanned live rollout. Older binaries intentionally reject a newer schema.

## Exact evidence

`results/reclamation-index-comparison.json` collects the four configurations, outcomes and exact binary/driver hashes. Complete per-run records remain under `results/library-node/`:

- `checkpoint20-serial-20261008`
- `checkpoint20-mixed-128-120-20261008`
- `checkpoint20-diagnostic-128-120-20261008`
- `checkpoint20-work16-diagnostic-20261008`
- `checkpoint20-wide-128-120-20261008`
- `reclamation-candidate-128-120-20261008`
- `reclamation-index-128-120-20261008`

Plan/FK evidence: `results/reclamation-reference-index.json`. Earlier drivers are frozen under `bench/library-node/versions/` so later monitoring changes are not imputed to these results.

## Remaining work and limits

Process-level CPU/RSS diagnostics have been added and checked with three unit tests plus a real two-reader smoke run. CPU sampling misses short-lived processes between samples; summed RSS double-counts shared mappings and is not host physical memory use. These observations were not present in the sustained runs above.

Further tuning should first measure database/cleanup and process costs, preserving all safety fences. More concurrency alone has not demonstrated sustainable capacity. Image/page traffic, the 100 TB working set, real bandwidth, regional failover, independent security and production deployment hardware remain unqualified. The source and runnable reference backend are functional development deliverables, not a promise of these traffic targets.
