# Protected-original reclamation

Schema24 extends the existing generation-safe collector to locally retired publication manifests and originals. `publication_reclamation` is a separate opt-in flag, default false. Enabling library cleanup alone does not select publication-only candidates. Previously committed cleanup claims recover even when both new-collection flags are disabled.

An author deletion request alone never schedules protected cleanup. Moderator-approved withdrawal must first be acknowledged by actual strfry. That transaction records five-minute generation-specific grace before releasing publication references. Current shared publications, current/pending libraries and group staging remain strong references.

New or repeated verified uploads refresh a five-minute staging lease in receipt finalization. This includes the full-quota duplicate fast path, preventing a collector from racing preparation of a new publication using a formerly retained original. The first two focused tests caught the missing fast-path lease refresh; the corrected receipt path passed the third run.

Eligible cleanup uses the same exclusive filesystem mutation guard, durable claim, exact-generation removal/directory sync and sealed cleanup receipt as library collection. Credits are refunded only after confirmed cleanup. Publication events and moderation tombstones are retained; re-uploading bytes cannot resurrect a removed event.

Failed cleanup remains charged, receives a retry delay and does not monopolize the queue. A bounded page reserves room for both retryable claims and new candidates. An empty-directory fault fixture demonstrates that another eligible original can be reclaimed while the failed generation remains unavailable and charged.

Evidence: `results/publication-reclamation-regression-3.log`, `results/publication-reclamation.json`, and the complete suite `results/publication-reclamation-full-regression.log`. These use generated originals and real PostgreSQL/strfry; grace is advanced with fixture SQL. Internal read pins are covered by separate read/cancellation tests.

Limits: no automatic never-published staging expiry; group-held objects remain protected; no inferred backfill of historical removed-publication ownership. External nginx file-descriptor lifetimes are not covered by internal pins, so direct external serving must be integrated before enabling collection for its files. Logical storage credits are not a guarantee about snapshots, hard links or unrelated disk occupants.
