# Staged upload expiry (schemas37–39)

Status: focused and combined functional regressions pass. Schema40 also implements authenticated reference-API deletion; see staged-deletion.md.

## Reference policy

New successful uploads retain the existing five-minute staging window. `staging_reclamation` defaults to true in the reference node. It is independent of `library_reclamation` and `publication_reclamation`, whose previous opt-in behavior remains unchanged. Offline restore explicitly sets all three false until operator review. Already claimed cleanup still resumes safely even when new candidate selection is disabled.

The selector targets the sparse unreleased-staging-credit set and uses a durable per-owner/volume cursor, returning at most64 candidates per pass. This is a bounded candidate/result contract; physical query-plan cost at a production-sized catalogue is not yet qualified. Queue source bit4 distinguishes abandoned staging from retired library/publication data. A queued entry alone never authorizes file removal.

## Safe cleanup

The shared reclaim path checks generation, owner/volume, availability, publication/library references, retention/grace periods, active account/publisher/group grants, in-flight transfer holds and external readers. It acquires the existing hash mutation guard before claiming. Unlink/fsync must produce a cleanup receipt before either physical or staging credits are refunded. Failed physical cleanup retains its durable claim and charges; repeated successful cleanup does not double-refund.

Expired group staging grants are now generation-specific. Re-upload renews the group's lease. Existing grants receive at least a fresh five-minute migration grace, never immediate retroactive expiry. Publication commit rechecks that the manifest and every original still have valid group leases. Hosted references do not expire merely because their former staging grants expire. Temporary group protection reschedules existing retirement work rather than deleting its queue entry.

## In-flight transfers

A signed peer publication or library transfer is bounded to600seconds and holds its verified staged generations through a durable operation with a660second maximum lifetime. This permits a transfer to outlast an individual blob's five-minute staging clock without losing earlier objects. These holds grant no publishing or read authority. Success/error closes the operation and wakes existing queued cleanup; cancellation/process loss leaves only the bounded fallback expiry. Expired operation records are pruned in bounded pages. Copied operations are cleared on offline restore because no source operation owns the independently restored files.

## Evidence

`server/tests/staging_reclamation.rs` and `results/audit-transfer-holds-regression.log` cover71 orphan uploads over multiple pages, disabled selection, exact physical/staging refunds, new generations, active/expired publisher and group grants, group prepare/commit expiry, retained publications, external-reader holds, unlink failure, resumed claims with new cleanup disabled, explicit operation close and abandoned operation expiry.

Combined evidence: `results/audit-staging-full-regression.log` passes all included functional fixtures, Clippy and PostgreSQL process/restart checks. `results/audit-staging-python.log` passes109 Python tests. No performance qualification was run for this work.
