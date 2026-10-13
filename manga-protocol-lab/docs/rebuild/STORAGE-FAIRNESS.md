# Storage fairness refactor evidence

Requirement3 in PARITY-REQUIREMENTS.md is implemented in the volume-work lifecycle. See ../storage-fairness.md for the exact scheduler/headroom contract and one-slot compatibility exception.

Runtime changes:
- src/storage_admission.rs: atomic total/background admission, FIFO eligible ordering, cancellation-safe wait removal, permit lifetime ownership, headroom hysteresis.
- src/storage.rs: one volume scheduler replaces the lower I/O semaphore; owned mutation/observation guards carry their permit into blocking I/O. fstatvfs samples the actual opened volume.
- src/upload.rs: background reclamation/recovery/audit/repair/export/withdrawal uses the background class, while upload receipt and download response credits remain separate. Existing cleanup pages sample charged+reserved physical quota and filesystem headroom.
- src/public_exports.rs: existing mutation argument updated to the owned mutation type. No reference, authorization or receipt checks removed.
- Cargo.toml/lock: libc is a direct dependency for fstatvfs; its already-locked/vendor version was reused offline.

No database migration, configuration removal or node-route edit was needed. upload_slots and reclamation_slots retain1..16 accepted bounds and existing defaults. No new background polling service, cloud task, deployment or application load test was started.

## Red to green

1. Public LibraryObjects staging during externally stalled cleanup: storage-fairness-red.log fails with the intended foreground staging deadline. storage-fairness-green.log passes after the single volume scheduler and foreground reservation.
2. Real UploadStore staging consumes more than90% of its physical quota while a first cleanup is externally stalled. storage-pressure-red.log fails because a second cleanup cannot progress with a fixed allowance. storage-pressure-green.log passes after headroom-based allowance adjustment, preserving foreground reads.

These test assertions use application calls. PostgreSQL advisory-lock triggers and lock-wait observation only arrange/establish the external fault, and expiry SQL only advances fixture time. There are no private scheduler counters or database-balance outcome assertions.

## Focused regression results (2026-10-10)

- storage_fairness:4 passed, including normal/pressured foreground staging and one-slot queued/admitted cancellation. 0.91s.
- storage_lifecycle:7 passed, including canceled real Linux-lease-blocked reads/recovery, volume ownership, receipts, stale generations and corruption. 0.20s.
- storage_health_lifecycle:1 passed, canceled health persistence versus repair fence. 0.28s.
- reclamation_parallelism:1 passed, public exact-capacity refund probes. 1.93s.
- upload_recovery:1 passed, durable read/recovery and exact-capacity probes. 0.21s.
- library_staging:1 passed, private durable grants, epoch fences, deduplication and corruption behavior. 1.56s.

Clippy passed with -D warnings for the library and storage_fairness/storage_lifecycle targets; evidence: results/rebuild/storage-fairness-clippy.log.

Logs: results/rebuild/<fixture>-fairness-regression.log. Tests use isolated PostgreSQL18 on loopback port55438, project-local Rust1.90 and workspace TMPDIR. Additional cancellation coverage was green-first regression work, not retrospectively labeled TDD.

This document records focused evidence, not the final combined-source/optimized gate. Low actual filesystem free space is sampled in these tests but not force-created; the pressure transition test consumes the configured physical-byte quota. No sustained throughput,100TB operating capacity, or physical power-loss claim follows from these results.
