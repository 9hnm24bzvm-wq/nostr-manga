# Bounded social dependency reconciliation

Schema30 adds inventory:"social" to approved-peer durable sync jobs. It covers notes(kind1), comments/replies(kind1111) and reactions(kind7). Existing selected account-event transfer remains available. Publisher/timestamp filters retain their SQL NULL semantics for chapter-only fields.

A reply or reaction can sort before its parent in the ID queue. The social worker therefore traverses missing dependencies before publishing the target. It verifies each signed event, checks its exact requested ID, applies the selected filter, and permits only social event kinds in this traversal. Events are committed in parent-first order through the existing social validation and peer/job-lease fences. When the same dependency is reached later as a queued item, idempotency prevents duplicate projection.

Manga roots must already be hosted locally. The dependency walker does not fetch chapter originals, restore a locally removed publication or broaden the selected inventory. A missing parent excluded by the filter is an explicit error; an already hosted parent can be referenced without reimporting it. To resolve an excluded dependency, an operator can import it through an independently authorized selection or broaden the social filter deliberately.

The traversal permits at most64 previously missing social events and2MiB of aggregate serialized event data. Per-response limits, peer deadlines and the durable quantum's ten-minute ceiling still apply. Cycles, wrong identities, unavailable roots, out-of-filter dependencies and unsupported kinds fail closed. Local tombstones are not bypassed. Partial earlier admissions can remain after a later failure; cancellation prevents future fenced commits and does not roll back previously hosted content.

## Evidence

social-inventory-regression.log and social-inventory.json passed a real two-node PostgreSQL/strfry fixture. Signed IDs were deliberately selected so the queue encounters reaction first, then reply, then parent. The receiver's outbox records parent-before-reply-before-reaction exactly once. A separate child needing an excluded-author parent is rejected; neither missing event becomes hosted. The fixture also retains prior series/profile/deletion/library and crash-recovery checks.

social-inventory-full-regression.log passed the integrated schema30 suite, Clippy and PostgreSQL immediate-restart checks. Targeted passes do not establish deployment capacity or an unbounded dependency graph guarantee. Continuous reconciliation schedules and direct static-original serving remain separate operational work.
