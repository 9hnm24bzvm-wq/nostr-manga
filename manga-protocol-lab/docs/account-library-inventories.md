# Durable account and library inventories

Schema28 adds separate profiles and deletions domains to approved-peer reconciliation. Schema29 adds libraries. Use POST /admin/sync-jobs with an explicitly approved peer, inventory and optional filter. The default inventory remains chapters; series is also supported.

- profiles: currently projected kinds0,3,10002. Imported accounts do not acquire personal posting permission.
- deletions: signed kind5 evidence accepted by the ordinary-deletion workflow. Exact IDs, author qualifications and selected replaceable cutoffs apply. Protected chapter/series deletion still requires the receiving instance's local policy. Head absence does not authorize deletion.
- libraries: current, projected, nonquarantined kind11999 heads. Root and every referenced chunk are verified and locally retained before the new head can publish.

Publisher and created_at are available to the existing filter language in these domains. Chapter-only fields are NULL. Filters are reevaluated on the signed content before admission, rather than trusting a peer's inventory membership. Sessions are scoped to domain/peer/epoch/prefix/filter and bounded by the existing snapshot/partition budgets.

## Authority and cancellation

The durable job's lease/generation token travels with account-owned staging and commits. Permission and lease checks occur inside the final transactions; late cancellation or replaced claims cannot publish a new account/library head. Private staged bytes may remain conservatively charged after interruption. Already admitted content is not rolled back by cancellation.

The existing queue still limits active batches, pending IDs, partitions and per-quantum work. A quantum has a ten-minute overall deadline in addition to per-request deadlines. The worker renews its lease while active. Five consecutive ordinary failures stop a batch with an explicit last_error; byte/queue backpressure retains its separate retry treatment. Profiles/deletions/libraries currently reject nonempty fallback lists instead of silently ignoring them. An operator may choose another independently approved source for a new job.

## Library head churn

A stable inventory may contain a pointer replaced before its job resumes. The approved-peer-only /federation/library-routing/<event-id> endpoint can return a recently retained signed library pointer to establish its author. This is a routing hint, not authority to serve or resurrect old library data. The receiver then requests that author's current visible head and transfers its complete current objects through the normal admission path.

Old routing bodies are still subject to bounded retirement pruning. If the hint is gone, the head disappears, or objects stop being publicly retained during transfer, work fails/retries within its bounds; rerun reconciliation from a fresh snapshot if the batch fails. It does not guess the author, merge progress, resurrect a deleted root or silently accept an incomplete library. Current-head filtering can intentionally reject a newer head outside the selected timestamp range.

Repeating a successful job is an explicit reconciliation pass, not a permanent synchronization schedule. No new background relationship is silently enabled by approval alone.

## Remaining scope

Schema30 also enables automatic social inventories with bounded parent-first traversal; see `social-inventory.md`. Missing manga roots and out-of-filter parents are not imported implicitly. Library root conflicts retain the chosen timestamp/ID rule and optional local CAS; there is no global merge or consensus.

## Evidence status

account-inventory-regression.log passed the real two-node profile/deletion fixture, including protected series remaining hosted. library-inventory-regression.log caught an omitted migration registration on the repeat-inventory check. The migration is now registered and a new unit check requires every migration file to appear in sequential order with its exact body. library-inventory-regression-2.log passed the rerun, including restart from a retained old pointer, current-head convergence, cancellation during staging and local reads after origin shutdown. account-library-full-regression.log passed the full schema29 integration suite, Clippy and multi-process PostgreSQL restart checks.
