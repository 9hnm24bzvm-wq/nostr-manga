> **Historical A/C benchmark evidence.** The preserved report below describes its original revision,
> including any then-pending work, test counts and measurements. It does not
> establish the current reconstructed build's status or capacity. For current
> requirements and evidence use [rebuild status](rebuild/STATUS.md) and the
> [parity contract](rebuild/PARITY-REQUIREMENTS.md). Any “current” navigation
> inside the preserved body refers to that historical report's time.

# Public library A/C performance: 30,000 and 500,000 chapters

6 October 2026. Corrected revision after the sustained-load investigation. Synthetic, reproducible comparative experiment on the cloud computer.

**Decision:** the user explicitly selected C. **Measurement correction:** initial concurrent results included recurring full-history statistics scans, and the test client retained obsolete chunks. Those issues are fixed. This revision uses corrected paired-load timings, separately identifies the later server reference-count optimization, and includes a completed five-minute run. Earlier raw trials remain in the archive as superseded diagnostics, not final capacity evidence.

## Recommendation

**Choose C for the scale-first protocol:** one replaceable, signed library pointer with immutable, content-addressed chunks. Keep one public library per key. Use batched commits and batched full-library downloads; permit a client to fetch only the chunk it needs. A whole snapshot is simple, but at the requested reader sizes it makes a tiny progress edit retransmit, validate and rewrite megabytes of unchanged history.

C is not an automatic merge system, unlimited-capacity proof, or finished production account implementation. It adds chunk lifecycle, incomplete-upload recovery, read grace and garbage collection. Those are required engineering work, not free benefits. This experiment strongly supports that tradeoff at 30k/500k. It does not select per-chapter addressable events (the former option B).

## Definitions and test boundaries

- **A:** the whole logical library in one signed replaceable Nostr event.
- **C:** one signed replaceable event containing an immutable root hash. The root names stable hash-partitioned chapter-state chunks and an overview containing public lists/series states.
- Both contain exactly the same completion/progress state. Both use JSON in this comparison, so CBOR/binary-ID encoding is not being credited as a chunking advantage.
- The30k case has30,000 completed chapters,1,500 series and 60 active progress entries. The500k case has500,000 completed chapters,10,000 series and 200 active entries. The reader-size labels come from your requested scenarios; the series/active counts are explicit synthetic assumptions, not measured reader percentiles.
- **Accepted latency** includes client serialization/signing/compression, the HTTP request and durable PostgreSQL commit. C's referenced files and directory are synced before the pointer becomes visible.
- **Verified projection latency** below is measured from the same client start through successful real strfry import and an ID scan. It is not merely a fast enqueue response.

Final large-reader tests place PostgreSQL, strfry and chunk files on the workspace **overlay filesystem**, with fsync and synchronous_commit enabled. Earlier diagnostic runs used `/tmp`, which was discovered to be **tmpfs**; those are not disk-performance evidence and are excluded from the headline numbers. The underlying physical device and actual power-loss durability remain unverified.

## 1. Progress saves on the local connection

Isolated-save baseline before the later reference-lookup optimization; these per-event measurements did not use the flawed recurring statistics scan. Medians across two independent runs. Twelve saves per 30k option; eight per 500k option. The second run also changes history distribution. Small samples describe observed local performance, not reliable population p99s.

| Reader history | A: accepted | C: accepted | A: verified in strfry | C: verified in strfry |
|---|---:|---:|---:|---:|
|30,000 chapters|294 ms|5.8 ms|365 ms|27.7 ms|
|500,000 chapters|5.83 s|15.4 ms|6.75 s|37.4 ms|

C uses64 chunks at 30k and 256 at 500k in this table. Median compressed request payloads were about 1.24 MB versus 22 KB at 30k, and 19.67 MB versus 89 KB at 500k. The immutable Nostr pointer itself is about 433 bytes; the changed chunk/root travel outside that event.

The serialized A events were approximately 2.70 MB and 43.44 MB. **Both exceed the current1-MiB reference event policy.** The large-A timed comparison deliberately permits64-MiB events in the experimental service and strfry, plus128-MiB request envelopes. These are labeled experimental settings, not a production-policy change. Thus the result does not win for C simply by rejecting A at a smaller cap.

A payload-only control using compact completed-ID arrays, rather than an object per completion, reduced compressed A size by roughly 2% in the earlier smaller cases. Encoding improvements are useful, but do not remove whole-history transfer on every save. This control is not a second service-throughput measurement.

## 2. Constrained connection: actual requests with explicit network emulation

Profile:80-ms round-trip delay,2-Mbps upload,10-Mbps download. Bandwidth is shared per emulated client. Both designs use gzip level3. These are controlled userspace delays around real HTTP requests, not measurements from an actual phone network. One measured request per large-case/profile; do not interpret these as tail percentiles.

| Operation |30k A|30k C,64 chunks|500k A|500k C,256 chunks|
|---|---:|---:|---:|---:|
|Save a progress change|5.32 s|0.179 s|84.25 s|0.458 s|
|Restore the entire library in one response|1.16 s|1.22 s|17.49 s|18.51 s|
|Fresh client: read, change one chapter, save|6.48 s|0.456 s|101.15 s|0.824 s|

The last row was rerun using lightweight completion checks and the final reference-count service. It is **not** a full-library restore for C. It fetches the signed head, root and one relevant chunk, verifies their bindings, then publishes the updated chunk/root. A must retrieve the entire old snapshot to preserve all other entries. This distinction is particularly important for 500k histories.

C with 1024 chunks at 500k lowered the tested upload to about 60 KB:0.336 seconds to save. Full restoration remained18.47 seconds. C does not make the full history disappear; its practical gain is avoiding full-history work for ordinary updates.

The30-ms/20-Mbps-upload/100-Mbps-download profile gave approximately 0.810 seconds for a30k A save versus 0.048 seconds for C64; at 500k it was13.50 seconds for A versus 0.199 seconds for C256, or0.133 seconds for C1024.

### Batching is a requirement, not a cosmetic optimization

Earlier controlled diagnostics showed that fetching dozens/hundreds of chunks one HTTP request at a time can make C's cold start much slower than A. The measured full-library bundle removes that fan-out penalty. The C write measurements likewise use one batched commit. A separate upload-chunks/upload-root/publish-pointer three-round-trip path is not independently timed here; extra RTT cost would need to be added. Do not implement the slow multi-request path and expect the batched numbers.

## 3. Corrected concurrent tests and sustained-load result

Concurrency tests use diverse histories:30% shared popular IDs and 70% distinct deterministic selections from a larger catalog. Initial population is excluded. Multiple clients use a common start barrier, lightweight pending-only monitoring, and a bounded cache containing only their current snapshot. Completion is timed before full statistics collection, and all client processes finish the measured interval before any heavyweight aggregation.

### Paired baseline:32 readers, 30k chapters each

240 updates scheduled at 24/second over 10 seconds, four load generators, with a full/cached library read every tenth update. These paired numbers use corrected instrumentation before the server's later reference-count optimization.

|Measure|A|C64|
|---|---:|---:|
|Time through final verified projection|22.32 s|9.87 s|
|Observed completed rate over that finite window|10.75/s|24.33/s|
|Median request-to-accept latency|810 ms|7.3 ms|
|95th percentile server projection delay|11.43 s|50.1 ms|
|Maximum observed pending projections|194|4|
|Errors / pending at finish|0 / 0|0 / 0|

A needed approximately 12 seconds beyond the 10-second offered workload to drain. The earlier 34-second A result was distorted by benchmark overhead and is superseded. Projection delay starts at outbox acceptance; it must not simply be added to client accepted latency.

### Two extreme readers, 500k chapters each

Eight updates across two clients, nominally one update/second combined: A completed in 25.63 seconds and C256 in 6.06 seconds. Maximum scheduled-action-to-accept delay was18.48 seconds for A versus 18.5 ms for C. Both finished with zero errors and no pending work. Eight samples do not support a meaningful population p99 or high-user-count500k capacity claim.

### What the five-minute investigation found

After correcting monitoring and bounding the client cache, the original C storage lookup still slowed under retained history. The service checked object visibility by searching historical root-reference arrays. The optimized experimental service instead maintains atomic current-reference counts and read-grace deadlines, updates only the sorted old/new reference difference, and serves objects through a primary-key lookup. Verified staging precedes the pointer transaction. This is a real implementation optimization, not just a reporting change.

### Final five-minute run:128 readers, 30k chapters each

100 requested updates/second for 300 seconds;30,000 updates completed in **300.02 seconds**. Four load generators;10% read mix. **Zero errors, zero remaining projections, and zero reference-count audit discrepancies.**

|Metric|Median|p95|p99|
|---|---:|---:|---:|
|Request-to-accept, including client preparation|10.1 ms|17.2 ms|29.2 ms|
|Scheduled action to acceptance, including client scheduling/read delays|11.9 ms|115 ms|156 ms|
|Server projection delay|27.3 ms|46.3 ms|56.9 ms|

Peak pending projections:22. Client memory remained bounded at about 570 MiB per generator, each simulating32 readers; server RSS was about 338 MiB. These are generator/service measurements, not browser memory requirements. The final audit independently recomputed object-reference counts from current heads.

This establishes the offered100/s workload for this five-minute local run. It does not establish a maximum, a production SLA, long-term garbage-collection stability, Internet federation performance or the original8-billion-image/350-million-page-view deployment target. The initial200/s short-run figures are retained only as superseded diagnostics; they are not a corrected capacity claim here.

## 4. Database and storage work

In the corrected paired 240-update30k baseline, A generated1.883 GB of PostgreSQL WAL; C generated8.23 MB plus10.26 MB of new immutable objects. For the corrected eight 500k updates, A generated1.020 GB of WAL versus 0.849 MB for C, plus1.40 MB of new objects.

In the optimized30,000-update C run, PostgreSQL WAL was501.33 MB, database growth243.97 MB and new immutable objects1.279 GB. This includes retained history and does not pretend old objects have already been garbage-collected.

These figures depend on the fixture's retention and schema: current heads and outbox payloads are retained, and PostgreSQL also has old MVCC tuples pending vacuum. A smaller outbox, compression/storage changes and aggressive vacuum/compaction can reduce A's stored-history cost. **They do not remove its full-snapshot network transfer, hashing or mutation cost.** C's old objects likewise require collection; no free or already-completed production collector is assumed.

Measured current-library object footprint illustrates C's file-count tradeoff:

| C partitioning|Current objects|Apparent content bytes|Allocated filesystem bytes|
|---|---:|---:|---:|
|30k,64 chunks|66|2.57 MB|2.71 MB|
|30k,256 chunks|258|2.59 MB|3.26 MB|
|500k,256 chunks|258|41.42 MB|41.96 MB|
|500k,1024 chunks|1,026|41.49 MB|43.59 MB|

Finer chunks save update bandwidth but increase root/reference metadata, files and index work. In the matched500k serial sequences,1024-chunk C generated more PostgreSQL WAL than 256-chunk C despite its smaller uploads. My starting recommendation is64 chunks around 30k and 256 around 500k, with 1024 available when bandwidth is more important. The protocol should not hard-code an allegedly universal optimal bucket count. Backend object packing/storage placement remains an operational choice to validate at large account counts.

## 5. Correctness and recovery

Executable fault checks, rerun on the workspace filesystem, cover:

- Invalid signatures and interrupted HTTP bodies do not publish a new library.
- A crash after database commit but before acknowledgement replays the durable projection intent; an exact retry is idempotent.
- A C root referencing missing content is rejected.
- A crash after C object fsync but before the pointer transaction preserves the old head; retry succeeds. It leaves an orphan that requires collection.
- Corrupt chunk bytes fail verification after process cache is discarded.
- Two stale devices still lose one edit under ordinary last-writer-wins behavior in both A and C. Optional local CAS catches the stale write at one instance, but is not federation-wide consensus.

Nostr's replaceable-event rules choose the latest timestamp, with a lexical event-ID tie-break. A pointer changes the payload arrangement, not those conflict semantics. Source: [NIP-01](https://github.com/nostr-protocol/nips/blob/master/01.md). Kind11999 and the tested library shape are experimental application choices, not a registered library standard.

## 6. Environment, reproducibility and remaining limits

- Rust release build; actual Schnorr verification; PostgreSQL 18.6 via deadpool-postgres/tokio-postgres; shared64-MiB Moka immutable-body cache; real strfry import and scan. No SQLx or SQLite in this experiment.
- Nine logical CPUs visible; approximately 9.7 GiB RAM. Actual CPU quota, device model and noisy-neighbor isolation are not known.
- PostgreSQL shared_buffers128 MB, pool 16, fsync/synchronous_commit on, max_wal_size512 MB. Large snapshots caused frequent checkpoints. This is not an optimally tuned production database comparison.
- Four Rust runtime threads;16 write slots for 30k concurrency, four for giant-case/serial fixtures; bounded file-I/O slots. Timed large-A limits were raised explicitly to64-MiB events and 128-MiB envelopes.
- JSON, gzip and actual signatures for both modes. Python client CPU is recorded. This is not a mobile-browser memory/performance test.
- Cold means empty client cache; physical disk cache was not flushed. The workspace is an overlay filesystem, not a verified physical SSD. No host power-cut or cross-machine durability test is claimed.
- No TLS/HTTP2, packet loss, radio variation or Internet federation is simulated. Emulation adds RTT and shared payload bandwidth delays, excluding protocol headers and complex TCP overlap.
- The fixture uses isolated tables/storage and test-only inspection/fault controls. It does not exercise all production quota, peer approval, moderation, WebSocket fan-out or account-inventory integration. Do not deploy the benchmark service publicly. Ticket 11 remains in progress.

Reproduce with `bench/library-ac/README.md`. Raw final samples and configurations are under `results/library-ac/disk-*`; `large-analysis.json` indexes them. Earlier RAM-backed diagnostics are preserved and labeled. Source versions, build hashes, failed/aborted trial notes and complete scripts are included in the accompanying source archive.


## Source groups and implementation status

- `disk-*` single-save/full-restore runs: valid workspace-backed isolated baselines. Their original concurrency runs are superseded by the corrected groups below.
- `corrected-multi-*-1`: corrected paired comparisons, before the visibility optimization.
- `refcount-soak-30k-C64-100`: final optimized five-minute run, including independent reference-count audit.
- `final-selective-*`: rerun fresh-client read/change/save timings with lightweight completion checks.
- `disk-soak-*`, `corrected-soak-*`, `bounded-soak-*`: preserved diagnostic runs showing why short tests were insufficient.

The production canonical-CBOR codec now has strict typed chunk/root validation and verified bucket summaries. Its explicit500,000-record fixture passes, along with 45 normal unit tests and all-target Clippy. This is codec progress, not completed production account APIs, peer integration or garbage collection. The benchmark remains an isolated experimental service.
