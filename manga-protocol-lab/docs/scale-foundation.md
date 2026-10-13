# Scale first node architecture

## Accepted implementation direction

On 6 October 2026 the user selected PostgreSQL, deadpool-postgres with tokio-postgres, and Moka, explicitly excluded SQLx, and required the prototype to be the foundation of the eventual service. The 200-GB VPS deployment must use the same core architecture as a larger deployment, with smaller configured resource limits. SQLite is retained only as historical behavioral and regression evidence during migration. This choice belongs to the reference implementation, not the federation wire protocol.

The new implementation is being developed in `server/`. It is not yet a feature-complete replacement. The existing tests provide contracts to port, not evidence that the PostgreSQL implementation already passes them.

## Workload and boundaries

The stated targets are 8 billion monthly image requests, 350 million monthly page views and at least 100 TB of originals. Over a 30-day month these average about 3,086 image requests/s and 135 page views/s. Page views can generate multiple API calls; average rates are not peak design rates. At an illustrative 500 KB per image, image delivery is about 4 PB/month or 12.35 Gbit/s average before overhead. Real distributions, cache misses, bursts and regional traffic must be measured.

- Original bytes remain in operator-controlled blob storage and are served through nginx/CDN delivery. Ordinary public image reads should not require a PostgreSQL or Moka lookup.
- PostgreSQL owns searchable metadata, local hosting/moderation state, references, quota reservations, replay records, durable work and the strfry outbox.
- Moka stores bounded per-process derived reads. It is neither the authoritative database nor a distributed cache.
- strfry remains behind an adapter. Protected kind-5 requests remain outside its automatic deletion path.
- Protocol validation and signatures remain independent of database and cache implementations.

## Deployment shape

Keep API handling, admission/validation, replication work, strfry projection, and blob-storage mutation as separable roles. The pilot can run them together on one VPS; larger installations can add API and worker processes without relying on one process's memory or database mutex for correctness. A storage volume has an explicit mutation owner and fencing discipline. Adding API replicas must not silently allow arbitrary concurrent filesystem mutation.

A PostgreSQL connection pool has a bounded per-process size, acquisition deadline and overall deployment connection budget. [deadpool-postgres](https://docs.rs/deadpool-postgres/latest/deadpool_postgres/) provides asynchronous pooling and prepared-statement caching around tokio-postgres. Cache only a finite set of fixed prepared SQL shapes; arbitrary user-generated filter shapes must not create an unbounded statement cache.

Use short asynchronous transactions. Do not hold pooled connections or row locks while downloading originals, decoding images, invoking strfry, or waiting on unrelated network operations. [PostgreSQL MVCC](https://www.postgresql.org/docs/current/mvcc-intro.html) supports concurrent sessions, but it does not remove application lock contention, poor indexes or hot counter rows.

## Multi-writer correctness

- Event IDs and content hashes have binary fixed-length validation and database uniqueness constraints. Repeated submissions are idempotent.
- Publication completion requires verified durable blob receipts and a final transaction checking current state, references and deletion decisions. Stale workers cannot resurrect removed content.
- Blob generations and fenced storage operations prevent an old cleanup worker from deleting a newly registered object with the same hash. Metadata expiry alone is not sufficient proof that physical work stopped.
- Acquire overlapping row locks in a documented stable order, use compare-and-set versions where appropriate, and retry bounded serialization/deadlock failures only for idempotent transactions.
- Durably record outbox work in the same transaction as the state change. strfry work is ordered per backend, with deletion/replaceable barriers preserved. No database transaction spans a native subprocess.
- Multi-worker jobs use bounded claims, ownership tokens, lease renewal and generation-fenced completion. Restarting one process must not reset jobs owned by another live worker.
- Quotas are enforced by durable reservations and actual accounting, never cache estimates. Per-volume/per-owner allocation limits must avoid making one global counter the serialization point for every object. Reservation reclamation requires fencing and reconciliation of possibly completed physical writes, not blind lease expiration.
- Moderation withdrawal needs an explicit completion boundary: discovery can hide content immediately, but a successful completed-withdrawal response must account for owned serving projections. Retained/shared originals must not be deleted accidentally.

These are implementation requirements. Their transaction and failure tests are release gates; merely porting SQLite triggers to PostgreSQL does not satisfy them.

## Moka cache policy

Start with verified immutable manifest/event bodies keyed by content hash or event ID, with the instance context isolated. Cached body bytes never prove current hosting availability or authorization. Moderation, membership, replay protection and quota decisions stay authoritative.

Use the asynchronous [Moka cache](https://docs.rs/moka/latest/moka/future/struct.Cache.html), shared values such as Arc, weighted capacity, individual entry limits, and coalesced loading. Its capacity is best-effort, so the configured weight is not a hard process-RSS guarantee. Do not wrap it in a global mutex.

Mutable query-result caching requires a separate consistency contract: versioned entries, post-commit invalidation, protection against stale in-flight loaders and recovery after missed invalidations. PostgreSQL notifications can wake consumers, while durable outbox/version state supplies recovery. Do not rely on TTL alone for revocation or deletion. Cold-cache operation must remain correct and bounded.

## Schema and query design

Use explicit typed columns and indexes for common filters; keep signed event representations and extension data separate from indexed projections. Store 32-byte IDs/hashes as validated binary values. Preserve exact non-language comparisons, three-valued missing fields and RFC 4647 language filtering.

Live offset/limit and timestamp filtering remain the accepted discovery contract. Retain query deadlines, response limits and appropriate ascending/descending indexes. Deep offsets still cost work on PostgreSQL; changing the driver does not make them constant-time. Default ordering and exact-count behavior remain pending Q93–94.

Use explicit versioned migrations with checksums and a deployment migration step. Avoid expensive schema work in every API process startup. Partition only where retention, index size or measured access patterns justify it; schema keys and storage placement should allow growth without assuming one filesystem or one API process.

## Validation gates

1. Port the existing signature, CBOR, publication, independent moderation, replication, group, library and recovery contracts.
2. Test competing writes from separate pools/processes: duplicate publication, last-reference removal, quota competition, job lease loss and stale-worker completion.
3. Test cache misses, stampedes, loader/invalidation races, disconnected invalidation consumers, revocation and read-after-write behavior.
4. Test PostgreSQL restart, slow queries, pool exhaustion, statement cancellation, backend writer failure and blob-store outages.
5. Benchmark representative mixed reads/writes with realistic cardinality, cold/warm caches, p95/p99 latency, throughput, pool wait, lock wait, I/O, replication lag and resource use.
6. Keep durability enabled during tests. A small local pass is not a 100-TB or monthly-traffic certification.

## Migration approach

Preserve the runnable SQLite checkpoint and its synthetic evidence. Build and test the PostgreSQL path as the main foundation, then port vertical slices with feature-parity and concurrency tests. Existing data migration should be explicit and offline initially, validating IDs, original hashes, reference counts, moderation tombstones and quota totals before switching. Do not maintain two diverging production storage engines or silently change federation semantics during the port.
