# Capacity lifecycle replacement

Verified against isolated real PostgreSQL and filesystem storage on 2026-10-10. The final run after removing obsolete wrappers passed all three fixtures:

- capacity, 0.78s: actual staging exhaustion leaves physical space reusable; 16 simultaneous duplicate uploads retain the same immutable generation; mismatched bytes fail; physical deletion receipts return exact quota without double refunds; injected admission failure rolls back; an injected receipt-settlement failure leaves durable bytes charged until actual reopen/recovery; 32 competing uploads across two independently pooled stores admit exactly four objects under a shared staging budget; released full capacity is reusable by either volume.
- credit_lanes, 2.08s: 64 real PNG/manifest chapter publications exceed the small reusable staging allowance while exactly filling physical capacity. After a 64-to-16 lane reopen, high-lane originals remain exact and charged, four concurrent chapter lifecycles complete, another byte is rejected, and staged deletion cannot remove retained originals.
- reclamation_parallelism, 1.75s: 96 expired staged blobs become unavailable through storage reads. Repeated full-capacity staged upload/delete probes prove all physical space is reusable and repeated cleanup cannot over-refund. No semaphore counts, private tables or ledger totals are acceptance outcomes.

Production change is narrowly limited to removing the three unused public methods reserve_capacity, settle_verified_capacity and refund_confirmed_charge (27 lines). Runtime reserve_tx, settle_tx and refund_tx are unchanged. No new test-only public wrapper was introduced.

Fault setup uses external PostgreSQL triggers or expiry timestamps. Final outcomes use actual UploadStore/publication/deletion/recovery interfaces and exact-byte quota probes. Reads in the lane fixture are authenticated owner storage reads; strfry/public-serving readiness is covered elsewhere.

Two initial fixture issues are recorded: the first concurrency assertion expected one error spelling although an exhausted concurrent allocation may surface a constraint rejection; the first lane read attempted public serving before projection. They were corrected without production behavior changes. Initial and final logs remain in results/rebuild-capacity-seams.

Historical source was copied to retired/rebuild-capacity-seam-fixtures only after replacements passed. These checks are correctness coverage, not traffic-capacity or production-readiness certification.

Targeted Clippy with warnings denied and rustfmt checks passed after the final test run. Source hashes are recorded in results/rebuild-capacity-seams/source-sha256.txt.
