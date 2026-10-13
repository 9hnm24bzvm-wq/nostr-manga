# Image admission and metadata contention

Full image decoding originally ran while the single metadata connection was locked. A bounded local test published 64 distinct synthetic 1536×1536 PNG originals (665,801 compressed bytes, 150,994,944 total pixels) while probing a one-result metadata query and an already-hosted image.

On the frozen optimized baseline, admission took 89.9 ms. The one query probe that began during admission waited 81.3 ms. Direct nginx image requests continued, with an admission-period maximum of 3.74 ms. These are a handful of local samples, not p95 estimates or capacity guarantees.

## Two-stage validation

The current implementation serializes decoding with a separate per-instance validation mutex. It briefly reads the candidate identity, releases the database mutex, then validates manifest bytes and fully decodes each unique original. It does not reserve or mark those files hosted during preparation.

A final database transaction rechecks the publication identity and deletion state, manifest hash, blob registration, size/MIME declarations, and current original bytes against the already-decoded hashes. It then evaluates the filter and commits references/hosted state. A file evicted or corrupted during preparation causes rejection; a removed publication cannot be resurrected by stale preparation. Identical verified bytes re-uploaded after eviction can still be admitted. Preparation is not cached across requests. The separate validation mutex preserves single-decoder concurrency rather than multiplying peak decoder memory across requests.

The optimized candidate admitted the same fixture in 90.5 ms. Seven query probes began during admission: median 0.69 ms and maximum 14.4 ms. Direct-image maximum was 2.26 ms. This indicates reduced metadata blocking in this fixture, not faster image decoding or elimination of all control-plane contention. Native backend commands, file hashing and final database/filesystem work can still block metadata.

Seven module tests cover stale preparation versus deletion, eviction, corruption, manifest replacement, valid re-registration, filter rollback and the distinct mutexes. The full integration/crash suite passed, including 92 Rust tests, 91 Python tests and 40 kill/restart cycles; results are recorded in admission-unlocked-deep-validation.log. A separate optimized five-minute run completed 300 cycles, four receiver restarts and 12 per-node accounting audits. The older frozen hour-long run does not include this change.

## Remaining limits

This is not a fair scheduler, multi-worker transfer reservation system, cumulative decoder-work budget, or process-wide memory guarantee. Large chapters with many unique high-resolution pages still consume cumulative CPU. Disk changes outside the service and power-loss durability need stronger operational controls. Direct nginx delivery can continue independently, but guarded image reads and metadata operations still depend on the control service.

## Read-class admission (rebuild parity)

`read_admission` is an optional node configuration object. Defaults are:

```json
{"overall":4,"wait_ms":200,"catalogue":{"active":2,"waiting":8},"library_head":{"active":2,"waiting":8},"live":{"active":2,"waiting":8}}
```

For an omitted configuration and a pre-existing pool smaller than four, the
default overall cap is the pool size and class caps are at most two. Explicit
configuration is validated rather than silently reduced.

`overall` must be 1–1024 and no greater than `database_connections`; each class's
active limit is 1–overall, waiting limit 0–1024, and wait deadline 1–5000ms.
Unknown fields are rejected. No pool-size or durability setting changes are
implied. The overall limit caps admitted reads; writes/background jobs retain
their own limits and share the configured PostgreSQL pool.

HTTP admission happens before authentication and policy database reads. Public
library heads, selective library reads/objects and their federation counterparts
use `library_head`; Nostr upgrades, AUTH, REQ, live lookup/delivery authorization
and periodic policy checks use `live`; other HTTP reads use `catalogue`.
Long-lived image bodies keep separate download count/byte leases. Library saves
remain under separate mutation admission.

Class and overall active credits are acquired atomically. Queued requests own
neither active credit. Waiting is FIFO within each class; another class can use
free overall capacity while the first class is full. Limits are caps, not strict
reserved shares: two classes together can occupy the overall limit. Queue-full
or deadline-expired HTTP reads return 429 with `Retry-After: 1`; Nostr command
responses identify retryable rate limiting. Cancellation removes a waiting position. Admitted HTTP work owns both active
credits in one bounded handler task, so a disconnected client does not release
them while submitted SQL or verified I/O is still running. Credits return only
when that owned handler terminates.

Live dispatch uses one authoritative lookup per notification, then bounded
filtered queues (64 events per session, at most40 registered sessions). The raw
notification backlog remains bounded to1024 hints. Unsubscribed instances skip
lookups. Short admission pressure is retried within a two-second lookup/delivery
authorization deadline; periodic policy checks defer while busy. Permission is
checked before delivery. Actual feed loss, overflow, database failure or a
sustained deadline breach invalidates the affected live stream and requires
resubscription; silent event loss is not treated as success. History registers
its live filters before querying, so concurrent arrivals remain queued; duplicate
history/live delivery remains possible. The five-second socket-send deadline
bounds slow consumers independently of other session queues.

The live response deadline does not pretend that dropping a PostgreSQL future
stops server work. A source lookup owns a single per-dispatcher work position and
its read permit/client until the existing PostgreSQL lock/statement and pool
deadlines finish it, even if the caller has already timed out. Policy reads are
spawned only after obtaining admission, so outstanding owned policy work cannot
exceed active read credits. These credits end before socket sends. Existing
server settings remain `lock_timeout=1s` and `statement_timeout=5s`; no new broad
database-timeout service or pool increase is introduced.


HTTP library-view response deadlines (native10s, reader15s) preserve their typed
error responses without cancelling the already-started bounded operation. The
same owned handler sends the deadline response and drains that operation before
returning its error; later handler steps do not execute. No extra deadline task
is spawned. A timed-out traversal can finish its bounded work while retaining
its original class/global credits. Inbound request-body timeouts are unchanged.

Response-proof middleware wraps this early-response boundary, with CORS still
outermost. Authenticated timeout responses therefore keep the normal proof and
CORS policy. Federation proof capacity is unchanged, but its slot may now be
held during the bounded read-admission wait; no authentication/policy SQL runs
before read admission. Image response bodies retain separate byte/count leases.
