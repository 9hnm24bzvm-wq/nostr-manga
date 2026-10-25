# Actual-node library benchmark

This exercises the production `manga-node` HTTP routes, account admission, canonical library codecs, PostgreSQL retention/accounting transactions, filesystem receipts and the actual strfry projector. It is separate from the older `library-ac` comparative fixture.

All keys and records are synthetic. Fixtures bind only loopback. Never deploy these keys/configurations. Run from a built checkout with the pinned local PostgreSQL/strfry binaries, release `manga-node`, and Python dependencies in `vendor/pydeps`:

```
bench/library-node/fixture.sh serial-02
NODE_USERS=32 NODE_RATE=24 NODE_DURATION=20 bench/library-node/load-fixture.sh local-32-24
```

The fixture creates a disposable PostgreSQL cluster and object store under the workspace overlay, not `/tmp` RAM storage. PostgreSQL fsync and synchronous commit remain enabled. Success removes only that fixture; failure retains it for diagnosis. Results include binary/driver hashes, database settings, filesystem type and final disk usage. Overlay durability is not a physical power-loss guarantee.

## Workload

The 30,000-completed-chapter reader also has 60 active chapters, 1,500 series, two lists and 150 list members. The 500,000 reader has 200 active chapters, 10,000 series, two lists and 1,000 list members. These are explicit synthetic workload assumptions, not a claim about typical reader demographics. Chapter identifiers differ between readers; series identifiers are shared. All four typed families are sorted and hash-bucketed according to the production codec.

An update advances a page in one active chapter, encodes that bucket and a new root, and sends both with a signed replaceable pointer using the combined commit route. Initial uploads use bounded batches. Whole restore verifies every downloaded object's hash, size and record count. Fresh selective update fetches the head, root and affected bucket before committing: four HTTP requests. The selected record itself is loaded from downloaded bytes, rather than its old cached copy.

Each serial profile measures twelve saves. Such a small sample is useful for smoke/latency comparison; its p95 is effectively the maximum and is not a robust tail estimate. Projection observation polls the head at 20 ms intervals over an unshaped local connection; that adds observer traffic and observation delay.

The concurrent fixture schedules a fixed total number of attempts at the requested aggregate rate, with one sequential client per reader. It records schedule-to-acceptance latency so client backlog cannot be hidden by reporting only successful request duration. Failures remain visible and are not retried. It does not poll projection for each write; after traffic, it verifies every final head and measures backlog drain. Superseded intermediate pointers need not be observed as current by clients.

## Network model and limits

The shaped profile adds 40 ms before sending and 40 ms after receiving, streams request bodies at 250,000 bytes/s (2 Mbps), and consumes response bodies at 1,250,000 bytes/s (10 Mbps). Upload pacing happens while the server receives the body, rather than sleeping before a fast local upload. JSON write bodies use gzip level 3 and NIP-98 binds the exact compressed wire bytes. Read batches request gzip; raw object routes may be uncompressed.

This is an application-level stream model, not a real WAN, TLS connection, packet-loss simulation or congestion-control test. TCP/kernel buffering can hide receiver backpressure. It does not certify slow-socket memory limits. Full connection memory, many machines, failover, physical disk failure and the 100 TB / eight-billion-image-request target need separate tests.

`serial-01` is retained as a diagnostic: its projection timer double-counted polling duration. Acceptance and restore metrics were unaffected. The corrected driver is used from `serial-02` onward.

The first concurrent fixture measured final-head projection separately from predecessor retirement. `paced-32-24` therefore has one pending deletion in its immediate post-head snapshot, although the later fixture snapshot confirms zero. Subsequent driver versions explicitly drain the entire outbox. Frozen driver versions preserve the exact hashes used in diagnostic runs. The initial local32 run also included up to one second of RSS-monitor shutdown in its reported drain duration; accepted-save timings were unaffected.

The historical120 successful durable library saves/s baseline is a synthetic original-node network-write experiment. The integrated local-first300M logical progress saves per30-day month averages115.74 device saves/s and does not imply HTTP/database writes. Optional public-library publication cadence remains an unresolved separate input. `gate.py` uses provisional experiment criteria (zero rejected attempts, p95 schedule-to-accept below1s, final drain below2s, no monitor failures and no more than2s closing delay). These are research gates, not a user-approved production SLO. `NODE_SAMPLE_OUTBOX=1` records an indexed pending-outbox count/oldest age every five seconds during load; it never scans historical JSON payloads. All generator, database and node work share the test host. Node RSS excludes PostgreSQL, generator, strfry children and kernel caches.

Later runs may set `NODE_EXTREME_USERS=2` to replace two of the normal histories with500,000-completed-chapter histories; the summary explicitly records this count. Setup now admits up to four libraries in parallel before timing starts. This changes fixture preparation, not the timed request schedule. Later drivers also flush live five-second metrics and progress counts so a failed long run does not lose all observations. Older exact drivers are retained under versions/ and in the delivered source revisions.

## Cleanup concurrency experiment

`NODE_RECLAMATION_SLOTS` sets the candidate storage cleanup fan-out (default4, bounded1..16). It shares `NODE_STORAGE_SLOTS` with foreground work; increasing cleanup fan-out is not an independent storage-capacity increase. The checkpoint20 fixture is frozen under versions/load-fixture-checkpoint20.sh. All measurements made with the later fixture record the new setting and their exact release/driver hashes. Always report cleanup queue age/debt separately from the historical request-latency gate; that gate alone does not certify sustainable reclamation.

## Process diagnostics

The current monitor records observed CPU deltas and summed RSS separately for the node, PostgreSQL processes, strfry and the Python generator, using process names and /proc stat fields only. No command lines or credentials are collected. PID/start-time pairs prevent attributing reused process IDs to an old process. CPU is a sampled lower bound: short-lived processes between samples are absent. Summed RSS double-counts shared mappings and must not be reported as physical host memory. Missing/exited stat entries are counted. Three parser/delta tests and an actual two-reader node/PostgreSQL/generator smoke check pass. Earlier drivers and their hashes remain frozen under versions/; these new measurements must not be imputed to old runs.
