# Foreground and maintenance storage fairness

`storage.upload_slots` remains the overall active-volume-work limit (1–16, default4) and bounds upload request/body receipts separately. `storage.reclamation_slots` remains a validated upper limit on cleanup-page contenders (1–16, default4); it is no longer permission for cleanup to consume every upload receipt or volume-work slot. Existing JSON and CLI settings remain accepted.

A volume-owned admission module schedules mutations and verified observations together. It admits the oldest eligible contender; a background contender whose class allowance is occupied does not block foreground contenders behind it. Admission is atomic across the overall and background limits. There are no nested total/class semaphores or reserved-but-idle total permits. Hash guards are acquired before admission and database generation locks, with no operation recursively acquiring a second work permit. All filesystem work within that observation/mutation reuses its permit.

For overall limit N≥2:

- Normal background allowance is min(reclamation_slots, max(1, floor(N/4)), N−1).
- Under low headroom it rises to min(reclamation_slots, N−1).
- At least one active-work place therefore remains available to foreground work even under pressure. Foreground can use all N places when background work is absent.
- Lowering the allowance never aborts an active operation; its owned permit is released only after completion, and new background work waits until within the new allowance.

The previously valid N=1 configuration keeps exactly one place and FIFO interleaving. Cleanup and foreground both progress, but simultaneous foreground reservation is mathematically impossible with one place. No implicit capacity increase or new invalid configuration is introduced.

## Two distinct headroom signals

The existing bounded collector page samples both signals once, without another timer or polling service:

1. Physical-byte quota headroom: configured physical budget minus charged used bytes and in-flight reserved bytes. Allocated-but-unused credit-lane bytes remain usable headroom.
2. Actual filesystem headroom: fstatvfs on the live volume-owner descriptor, using blocks available to the process rather than privileged reserved blocks.

Pressure starts when either signal has at most10% free. It clears only when both have more than20% free, providing hysteresis. An unavailable filesystem sample is logged and cannot clear an already active pressure state. Headroom changes scheduling only: quota admission, generation checks, ownership, leases, quarantine, strong references, fsync, durable reclaim claims and receipt-backed refunds still decide whether an operation may succeed. A disk-space signal never authorizes deleting protected content or overcommitting quota.

Recovery, reclamation, audit observations, repair and public-export/withdrawal maintenance use the background class. Uploads, library staging, explicit authorized staged deletion and download observations use foreground. A mandatory quarantine withdrawal discovered by an in-flight observation retains that observation's existing permit. Upload receipt slots and download response-byte/count credits remain independent: a slow response body holds its download credits, not its already-finished volume observation.

## Original and manifest receipt admission

The signed `PUT /upload/{hash}` and Blossom `PUT /upload` adapters join the
same FIFO receipt semaphore used by library staging. This includes CBOR
publication manifests uploaded through the signed hash route. At most
`upload_slots` original/manifest requests wait for a receipt slot, for at most
two seconds each. A full waiting budget or an expired wait returns the existing
HTTP429 `upload_slots_busy` response. This bounded wait prevents an already
queued stream of library stages from systematically excluding occasional
original uploads; it is not a guarantee that overloaded requests always succeed.

Waiting does not aggregate a request body or hold a PostgreSQL connection.
The existing shared active-slot count, per-body `max_blob_bytes` limit and
30-second body-receipt deadline remain unchanged. Authorization, exact hash
verification and current publisher grants remain mandatory. Once mutation
begins, its owned task retains the shared receipt permit through completion.

Dropping an admission future removes its waiting ticket and FIFO position.
An HTTP disconnect is not proof that Hyper has canceled that future: before
body polling, even a closed peer may keep its handler alive. Such a waiting
request still releases its reservation within the two-second admission deadline.
No immediate TCP-disconnect detection is promised. Other independent mutation
adapters retain their existing admission policies.

## Cancellation and verification

Queued admission is removed on cancellation and wakes remaining contenders. Once cleanup has acquired its mutation, its owned task retains admission and generation ownership through file cleanup and refund even if the requesting future is dropped. Blocking filesystem closures clone the same permit and hash/volume ownership, so canceling an async caller cannot free work capacity while its physical I/O continues.

`server/tests/storage_fairness.rs` uses public UploadStore and LibraryObjects lifecycle operations with real PostgreSQL advisory-lock triggers solely to arrange stalled cleanup. It checks foreground staging under normal and pressured cleanup, headroom-responsive cleanup progress, and one-slot queued/admitted cancellation. Outcome assertions use public staging, reads and cleanup results; they do not inspect private counters or database balances. Existing filesystem lease, health-fence, quota, recovery and download fixtures remain required regression coverage.

`server/tests/http_upload_admission.rs` uses actual HTTP images/manifests and
externally stalled library grants to reproduce receipt starvation. It covers
both upload adapters, finite waiting capacity/deadline, disconnected-waiter
recovery, publisher revocation and owned completion after client disconnect.
HTTP `Expect: 100-continue` exposes when body receipt begins, independently of
the lower filesystem-work scheduler. Evidence is in
`results/rebuild-upload-admission/`; the immediate-disconnect assumption failures
are preserved separately from the genuine starvation red and its green.

The reservation and headroom tests each have recorded failing-before/passing-after evidence in results/rebuild/storage-fairness-{red,green}.log and storage-pressure-{red,green}.log. Additional lifecycle coverage is regression validation; it is not claimed as a new failing-before slice. These focused fixtures do not establish sustained throughput,100TB capacity or a full final-source gate.
