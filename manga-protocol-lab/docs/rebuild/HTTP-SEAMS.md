# HTTP, deletion and scheduling seam cleanup

All five reconstructed fixtures passed against isolated PostgreSQL on 2026-10-10. No production code was changed in this slice.

- ordinary_deletion (0.47s): creates a real original, manifest and protected chapter through application publication lifecycle, then verifies ordinary deletion cannot remove it. Concurrent author deletion and late replay fencing remain. Deletion requests are observed through their public lifecycle and are absent from normal visible events.
- sync_schedules (8.44s): creates and finishes 513 real scheduled batches. The oldest automatic status disappears, the newest 512 remain completed, manually coalesced history survives and completed work cannot be claimed. No synthetic history rows or private count/ledger assertions remain.
- http_publishers (1.14s): publication, scoped staging, original byte/identity retention, physical ownership and revocation/grant fences use HTTP and actual lifecycle interfaces. Private blob table counters and owner queries were replaced.
- http_public_serving (3.21s): waits on the application projection completion contract; a real filesystem withdrawal fault closes the reader and preserves exact original bytes. Startup withdrawal and orphan-reader/failed-database recovery remain covered without private hold-table outcomes.
- http_sync_jobs (22.88s): real process crash/restart preserves a live lease, expires and recovers it, and fences old authority. Physical storage is constrained to exactly the original and manifest byte total: identical upload succeeds, a distinct object fails, then succeeds after explicit quota expansion and restart. Actual live Nostr delivery observes the distinct parent, reply and reaction identities in dependency order, with exact final HTTP event bodies. Retained series bytes are fetched over HTTP. Private capacity, queue ledger, job generation and outbox-order outcome queries were removed.

SQL remains for explicit external failure/race arrangement. The existing large reconciliation section still injects 120,000 shared synthetic inventory metadata rows, then transfers one genuine missing chapter; that section is labeled synthetic and does not establish real catalogue ingestion or target traffic capacity. No claim of production readiness is made.

Commands, test logs and structured evidence: results/rebuild-http-seams. Exact pre-edit fixtures were preserved after replacement validation in retired/rebuild-http-seam-fixtures.
