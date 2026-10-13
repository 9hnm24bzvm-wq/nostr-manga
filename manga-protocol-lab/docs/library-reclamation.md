# Generation-safe obsolete library reclamation

Schema19 implements local cleanup of retired public-library roots and chunks. New automatic collection is opt-in through `library_reclamation: true`; the default is false. Already committed reclamation claims are recovered even when new collection is disabled. This is not manga-publication deletion or general orphan collection.

## Durable sequence

1. Backend-confirmed replacement retirement starts a five-minute grace and schedules a generation-specific candidate.
2. The collector obtains the filesystem hash mutation guard before database blob row locks, matching uploader lock order.
3. It rechecks the exact generation, local owner/volume, available state, current/pending library holds, grace, publication references, group staging references and fresh account staging leases. Timeouts never establish eligibility.
4. A committed reclamation marker fences new staging and retention. Charges remain intact.
5. Only that generation's final/partial paths are removed and the containing directory synced. Shared internal read pins prevent cleanup from overtaking an open node-owned reader, including after caller cancellation.
6. A sealed cleanup receipt permits an atomic database transaction to refund exactly once and remove obsolete holds, grants, summaries, reservation and charge metadata. A different generation cannot be refunded by an old receipt.
7. After a process crash, marked work resumes. An already absent file still requires cleanup confirmation and directory synchronization before refund.

The due queue is indexed and processed in bounded pages, with cleanup contenders bounded by reclamation_slots (default4). Actual active cleanup follows the [foreground-reserving volume policy](storage-fairness.md), defaulting to one background operation and rising to three under pressure with the default four-slot total. Existing claims take priority. Candidates retained by a current library are removed from the queue; a later library retirement requeues them. All selection is local-owner/volume scoped. A new stage or restage refreshes a five-minute freshness lease, protecting a formerly public object while its next root is prepared.

## Verified behavior

- `results/storage-read-pins-unit.log`: concurrent shared readers, exclusive cleanup, and cancellation with a real open file descriptor.
- `results/library-reclamation-regression-2.log`: actual PostgreSQL/strfry reference/grace/staging protection, cross-author sharing, exact-generation cleanup, simulated interruption, stale generation replay, group protection, account revocation and budget conservation.
- `results/library-gc-process-regression.log` and `results/library-gc-process-recovery.json`: a real HTTP node process was killed after two obsolete objects were unlinked but before the refund transaction committed. Charges stayed intact. Restart with new collection disabled completed the durable claims, refunded the exact bytes, preserved current readable objects and did not double refund on another restart.

The process-kill fixture uses a PostgreSQL advisory-lock trigger at the refund transaction's delete boundary, and terminates only its blocked synthetic database backends to make rollback observable. It advances fixture grace and lease timestamps rather than waiting five minutes. This is process-crash evidence, not physical power-loss certification.

The full PostgreSQL integration suite and all-target Clippy passed in `results/library-gc-full-regression-2.log`. Its first run exposed a test fixture holding an exclusive mutation guard while requesting a shared read; that fixture now releases its writer before reading. No production lock bypass was added.

## Remaining verification and scope limits

Repeat the mixed125 saves/s workload with collection active beyond the real grace period, measuring oldest overdue work, remaining charges and latency. The earlier125/s pass excluded collection overhead. Additional publication/staging races and a full-quota reuse soak remain useful.

This slice does not reclaim never-published uploads, group-only staging, protected manga publications or account data merely because admission was revoked. Old replaceable event/outbox history and empty credit-grant pruning are separate pending work. The newest per-key winner watermark must survive any history pruning to prevent old-event replay.

Filesystem credits count this store's logical object bytes. External hard links, snapshots, unrelated disk occupants and readers outside the node are not covered by internal read pins. Direct external serving must be integrated with reclamation before enabling cleanup for those files.

## Schema20 history lifecycle

Server-observed `retired_at` permits bounded removal of old library event bodies and completed outbox records after five minutes. Current heads, pending projection, pending object holds and other lifecycle references prevent pruning. Per-key winner clocks/IDs survive, so an old event cannot resurrect after its body is pruned. This shares the opt-in collection flag but does not depend on physical cleanup succeeding. The targeted regression passed in `results/library-history-regression.log`.

The ten-minute schema19 cleanup load run was interrupted at the owner's request to prioritize feature completion. It did not reach the five-minute grace; it supplies no cleanup-throughput result. Details are in `results/library-node/gc-mixed-soak-125/QUALIFICATION.txt`. Further tuning is deferred.
