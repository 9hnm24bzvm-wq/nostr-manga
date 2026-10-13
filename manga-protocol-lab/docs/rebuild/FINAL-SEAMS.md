# Final queue, receipt and recovery seam cleanup

Verified against isolated real PostgreSQL on 2026-10-10:

- sync_queue, 4.55s: the actual queue survives more than five backpressure retries, releases precisely enough room on cancellation, exposes all sixteen split prefixes through real claims, completes all child partitions, and recovers its full 200,000-item capacity after cancellation and terminal retry failure. Internal pending/failure counts and prefix-list SQL are gone.
- sync_skips, 2.40s: public status retains bounded skip details and correct terminal semantics; a transient failure produces delayed claimable work rather than a skip. Successful retry completion, skip completion and cancellation leave the entire queue capacity reusable through real save_missing requests and an overflow rejection.
- blob_receipts, 0.14s: actual runtime generation-bound filesystem receipts and blob transactions preserve quota across writing, settlement, duplicate receipt, abort and replacement. Exact admission probes replace internal grant balance reads; stale receipts and cleanup cannot refund a replacement.
- upload_recovery, 0.18s: reads prove durable bytes survive while absent bytes stay unavailable; repeated full remaining-capacity put/delete probes prove only cleaned generations are refunded and repeated recovery does not over-refund.
- audit_gaps: diagnostic HTTP observations completed with abandoned-byte visibility and actual exact-capacity stage/delete probes replacing private GC-queue existence. It remains explicitly a diagnostic report, not a conformance pass for every observed response.

No production code changed. SQL remains solely for explicit fault, expiry and historical-state arrangement. Historical fixture copies were preserved after green in retired/rebuild-final-seam-fixtures. Evidence and commands are in results/rebuild-final-seams.

The final http_sync_jobs wait audit also passed (1 test, 22.16s). Synchronization now follows actual pg_blocking_pids relationships instead of matching implementation query text. The one lease-row read is explicitly external stale-authority fault-target capture; HTTP/runtime calls assert the outcome. The initial rerun hit a full /tmp filesystem before execution; the successful rerun used workspace-backed PostgreSQL and TMPDIR. See causal-sync-run.log and http_sync_jobs.log in the evidence directory.
