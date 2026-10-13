> **Historical node benchmark evidence.** The preserved report below describes its original revision,
> including any then-pending work, test counts and measurements. It does not
> establish the current reconstructed build's status or capacity. For current
> requirements and evidence use [rebuild status](rebuild/STATUS.md) and the
> [parity contract](rebuild/PARITY-REQUIREMENTS.md). Any “current” navigation
> inside the preserved body refers to that historical report's time.

# Actual production-node library measurements

Status: initial serial measurements complete; concurrent-load experiments in progress. These results do not certify production readiness or the eventual traffic/storage target.

## Corrected serial run

Source: `results/library-node/serial-02/summary.json`, immutable release binary hash in the same directory. This exercises the real node API and lifecycle, unlike the earlier A/C comparison service. PostgreSQL and objects were stored on the workspace overlay with fsync/synchronous commit enabled. The underlying physical device and power-loss semantics are unknown.

| Completed chapter history | Local median accepted save | Paced median accepted save | Paced complete restore | Paced fresh selective update |
|---|---:|---:|---:|---:|
| 30,000 | 22.45 ms | 187.18 ms | 1.519 s | 468.04 ms |
| 500,000 | 28.40 ms | 441.35 ms | 18.662 s | 789.41 ms |

Each save median covers only twelve operations, all accepted. The restore and selective operations are one observation per profile, not latency distributions. The paced profile models 80 ms round-trip delay, 2 Mbps upload and 10 Mbps download using application-level streaming. It excludes TLS, packet loss, real Internet routing and congestion control.

The complete 30k fixture contains 31,712 records across chapter history, active chapters, series, public lists and list members. The 500k fixture contains 511,202 records. A complete restore downloaded and verified every current chunk and root. Fresh selective update uses four requests: head, root, changed bucket, combined signed commit. No protocol rule allows editing immutable chapters; these are replaceable public reader-library pointers.

Median client-start-to-observed-projection was 138.91 ms (30k local), 242.98 ms (30k paced), 148.01 ms (500k local), and 450.62 ms (500k paced). The observer queried the local head at 20 ms intervals; these figures include observation delay/traffic and are not exact backend commit timestamps.

The first diagnostic run (`serial-01`) had a double-counted projection timer. Its acceptance/restore figures were unaffected, but use corrected `serial-02` for the reported projection figures. Both the defect and original artifact are retained.

## Interpretation

A warm page-progress save does not grow in proportion to the entire history. It uploads the changed hash bucket and a new root. A complete restore still necessarily transfers the entire public library, so 500k history restoration is materially slower. These results support approach C's intended tradeoff; they do not establish concurrent capacity.

See `bench/library-node/README.md` for reproduction, dataset assumptions and remaining measurement limits.

## Explicit throughput requirement

On 2026-10-07 the user clarified that the requirement is **300 million library/progress saves per month**, not page views or image requests. Over a 30-day month this averages 115.74 successful saves/second. The rounded baseline test target is 120/s, with higher peak tests reported separately. Neither peak multiplier nor a formal latency SLO has been chosen by the user. A short run at 120/s is not evidence of month-long durability, cost or availability.

## Initial concurrent baseline, before optimization

Each reader has 30,000 completed chapters plus the additional records described above. These were 20-second request intervals, not sustained soaks.

| Readers / offered saves per second | Network | Accepted / attempted | Median accepted request | Backlog after traffic |
|---|---|---:|---:|---:|
| 32 / 24 | Local | 480 / 480 | 32.64 ms | 5 jobs |
| 32 / 24 | Paced | 480 / 480 | 201.59 ms | 6 jobs |
| 128 / 100 | Local | 1,441 / 2,000 | 50.86 ms | 1,317 jobs |
| 128 / 100 | Paced | 1,170 / 2,000 | 212.46 ms | 905 jobs |

All high-load rejections were explicit HTTP429 `library_busy`; no retries concealed them. The local100 run needed about15.28 seconds of final drain; paced100 needed12.94 seconds. Final outbox counts were zero. The baseline therefore does **not** meet even the average300M/month requirement. Successful-request latency alone would have hidden this failure.

The code held one of four processing slots while receiving the network body, and backend replacement projection handled deletion barriers individually. Bounded receipt/processing separation and mixed import/delete batching are under development and require fresh regression/load evidence before being credited as fixes.


## Batching alone was insufficient

The bounded-phase plus batched-projector120/s run (`batched-paced-128-120`) accepted1,378 of2,400 attempts. Projection had no pending work at the immediate end snapshot, but accepted request latency had grown to a1.099-second median and1,022 requests were rejected. This fails the baseline. A separate diagnostic sampled shared capacity-grant lock waits; storage credit lanes are being verified next. None of these intermediate runs qualifies as300M/month capacity.


## First successful120/s gate

With16 durable credit lanes and the preceding admission/projector changes, `lanes-paced-128-120` accepted all2,400/2,400 writes during the20-second offered interval. All128 readers had30,000 completed chapters. Paced acceptance median was213.84ms, p95 258.69ms, maximum302.09ms. Schedule-to-acceptance p95 was261.65ms, so the client was not hiding a growing launch queue. The closing requests completed0.203s after the offered interval; final outbox drain took0.198s and ended at zero.

Five-second snapshots observed at most22 pending outbox jobs and oldest pending age0.086s. Those are sampled maxima, not continuous bounds. Peak sampled node RSS was33,708KiB, excluding PostgreSQL, strfry children, the generator and kernel caches. No monitor errors occurred.

This is a successful short baseline gate, **not** month-long capacity certification. The five-minute soak is next. During these2,400 saves, apparent object-store bytes increased55,477,798 and PostgreSQL size increased27,082,752 bytes; physical reclamation and metadata-history pruning remain required before continuous production operation.

## Write amplification to carry into capacity planning

The successful20-second30k-history trial generated about39,972 WAL bytes and23,116 apparent object-store bytes per accepted save. Multiplying those observations by300 million gives roughly12.0TB of WAL generated and6.93TB of object writes per month for this specific stress workload. These are arithmetic extrapolations, not month-long measurements or retained-WAL requirements: PostgreSQL normally recycles WAL, and a correct object reclaimer should bound obsolete-root/chunk retention. Current code has no physical reclaimer, so its obsolete files do accumulate. Full filesystem block overhead, backups, replicas and initial library imports are additional costs. Do not infer the same figures for smaller histories or a different reader mix.

## First five-minute run: nearly sustained, still a failed zero-rejection gate

`lanes-soak-128-120` attempted36,000 saves over300 seconds and accepted35,990. The ten HTTP429 responses clustered around scheduled seconds212.7–213.4. Accepted latency median was211.23ms, p95 278.88ms and maximum784.86ms; schedule-to-accept p95 was281.45ms. Final drain took0.255s. Five-second snapshots saw at most34 pending jobs and oldest age0.136s; final pending count was zero. Node RSS rose from28,532 to37,704KiB. No observer failures occurred.

The short rejection interval and available wire-byte budget point to exhaustion of the64 admitted-request slots during a subsecond slowdown, rather than a two-second work timeout. Stage-specific overload diagnostics have been added, and the finite count limit is increased to128 while retaining the64MiB wire budget, eight processors and two-second queue deadlines. A fresh five-minute rerun is required before calling this fixed.

PostgreSQL logged frequent WAL-driven checkpoints with this fixture's512MiB max-WAL setting. They are a plausible contributor to transient latency; the test does not isolate their causal effect. Database durability/settings remain unchanged for the queue rerun. The longer run generated2,507,274,976 WAL bytes,832,124,970 apparent object bytes and348,348,416 bytes of database growth. This reinforces that short-run write-amplification extrapolations are provisional and checkpoint/history management must be tested.

## Queue rerun and planner diagnosis

The larger-queue five-minute rerun (`burst-soak-128-120`) accepted all36,000 saves but failed latency: request median966.54ms/p951,887.20ms, schedule-to-accept median8.10s/p9516.96s. It finished at300.275s with zero final backend work. Accepting everything is not sufficient when readers queue this long.

That rerun also used faster four-reader parallel fixture preparation. A diagnostic exposed an important setup/import sensitivity: PostgreSQL had not yet refreshed planner statistics after the bulk insert. In a fresh diagnostic with explicit ANALYZE (`analyzed-profile-retry-128-120`), all3,600 attempts at120/s over30s completed with219.45ms median and253.39ms p95. Sampled retention-accounting stage median fell from about40ms to6.46ms. Before/after plans in that same fixture changed whole-table join work to indexed lookups. The earlier analyzed attempt was interrupted by an environment restart and is explicitly excluded from completed evidence.

To improve fresh-load behavior rather than requiring a favorable warm-up, migration0018 adds a sparse unreleased-credit index and the accounting path now uses separate bounded primary-key lookups. Owner/availability/generation checks remain under caller-held blob locks; staging transfer is still in the publication transaction. A redundant active-credit index from0017 is removed because0004 already defined its equivalent. Fresh-load and sustained reruns are required for this version. Normal operational ANALYZE/autovacuum remains important even with targeted indexes.


## Current measured baseline:125/s mixed steady-state pass

`wide-mixed-soak-125` completed all37,500 saves from a300-second125/s schedule in300.204 seconds, without rejection, retry or monitor error. The128 synthetic readers were126 with30,000 completed chapters and two with500,000. PostgreSQL statistics were explicitly refreshed after setup. Application pacing remained80ms round-trip,2Mbps upload and10Mbps download. This is an initialized steady-state result, not an unqualified cold-start claim.

Configuration:32 database pool connections,256 admitted requests,16 processing operations,8 root commits,8 storage operations,16 credit lanes,64MiB weighted wire budget and64MiB response budget. The same release node retains fsync/synchronous commit and actual strfry verification. Limits/configuration and binary hashes are recorded with the result.

- Overall acceptance median218.485ms, p95256.960ms, maximum727.328ms.
- Schedule-to-acceptance p95259.951ms, maximum736.796ms: the client did not conceal a growing launch queue.
-30k readers:36,914 operations, median218.319ms, p95249.338ms.
-500k readers:586 operations, median484.934ms, p95521.370ms.
- Final drain0.226s, zero final pending work. Five-second snapshots saw at most44 pending jobs and oldest age0.185s; these are sampled maxima.
- Peak sampled node RSS119,696KiB, excluding database, generator, subprocesses and kernel memory.

125 saves/s exceeds the300M/28-day arithmetic average of124.008/s. This five-minute result establishes that average-rate milestone on this test host. It does not certify calendar-month availability, all-extreme readers, real WAN behavior, public deployment or the image/page-view targets. Obsolete bytes and metadata still accumulate because physical reclamation/pruning are unfinished.

## Peak tests remain failures

All7,200 writes from a30-second240/s schedule eventually succeeded, but defaults took61.641s, the larger16-credit-lane configuration took49.379s, and64 credit lanes took48.483s. Schedule-to-acceptance p95 was25.217s,12.556s and11.224s respectively. None sustained240/s. More queue room or eventual completion must not be mistaken for capacity. The64-lane change was not a meaningful peak fix; defaults remain16 lanes. Profiling found quota-grant lock contention, while filesystem sync was a small median component.

The180-second interval diagnostic at120/s completed21,600 writes with349ms p95 but did not reproduce the earlier five-minute slowdown. It is not evidence that the earlier tail problem disappeared. All slower results remain in this report/evidence set.

## Generator memory and operator limits

Later trials use compact generator state, retaining decoded active buckets rather than all completed-history records. The separate equivalence check verified five identical updates for both histories: retained decoded records fell from31,712 to469 and511,202 to1,843. Server-side libraries remained complete. This reduces synthetic client memory; it is not a smaller server dataset.

Storage credit lane count is now configurable (power of two1..256, default16). Rebalancing recognizes historical higher lanes when reducing the setting. Full correctness regression, including quota pressure and64-to16 downsizing, passed in results/credit-lanes-config-regression.log. Library worker limits have their own validated bounds; see library-concurrency.md. Increasing them expands worst-case processing memory and does not guarantee proportional throughput.
