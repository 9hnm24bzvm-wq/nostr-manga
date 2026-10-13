# Bounded public-original export work

Status: the opt-in queue and focused real-component fixture pass. No node configuration starts a public-serving role yet; whole-publication quarantine and guarded startup remain open.

Schema34 records one physical-root-bound role per logical storage owner/volume. Ordinary nodes have no role row and do not accumulate export work. An initial cursor scans at most64 blob-intent rows per quantum using the owner/volume/hash index. Only referenced image originals and pre-existing export holds are queued; unrelated staged bytes are excluded. Repeated role configuration preserves the cursor. Live metadata triggers cover changes that occur before or after the cursor passes a hash.

Publication-reference changes, event lifecycle changes and completed backend projections enqueue work transactionally. Creating holds also ensure pending work exists without endlessly bumping a worker's own version. A queue item has a blob generation and monotonically changing per-row version; completion and retry match both, plus the physical root. Recreating a hash in a new blob generation cannot inherit an old completion.

A maintenance quantum processes at most16 due items. It rechecks whether a complete, healthy, projected publication retains the original. Eligible work calls the verified positive projection; other work calls durable namespace withdrawal. Errors retain the item with bounded exponential backoff capped at60seconds and a bounded diagnostic string. Concurrent metadata changes advance the version, so an older quantum cannot remove the new work. The storage owner serializes owned quanta, preserving their cleanup responsibility if a caller disconnects.

The role's initial scan and queue emptiness are separate readiness conditions. A delayed retry is still pending work; checking only currently due items is not a readiness proof. Reader draining remains a distinct bounded operation protecting physical reclamation after withdrawal.

The focused fixture uses real PostgreSQL, strfry and nginx. It covers a73-object catalog with64-row cursor progression, exclusion of unpublished staging, shared-original retention, last-publication withdrawal, publication after backfill, verification failure/backoff/retry, and stale version/generation completion fences. The last two fences use test-only SQL mutations to force the exact races. This is functional evidence, not catalog-scale performance evidence.

Offline restore clears only the copied target's export role, cursor and work queue, together with copied reader holds. It leaves the source untouched and does not enable serving. A later explicit role setup must backfill the restored physical volume.

## Remaining gates

- Quarantine/repair must schedule every affected publication's required namespace changes, including shared originals, with bounded fan-out.
- A guarded/private-read startup must recover old public reader families before accepting requests.
- Public listeners must not expose stale aliases during initial recovery/backfill.
- The node must wire the supervised image role and bounded maintenance loop only after those gates pass.

Source: server/migrations/0034_public_export_queue.sql, server/src/export_queue.rs and server/tests/public_export_queue.rs. Results: results/public-export-queue-regression.log and public-export-queue.json. Aggregate checks and the release build are tracked separately.

Aggregate evidence now passes: public-export-queue-full-regression.log. After indexed cursor/due-cutoff refinements, public-export-queue-final-regression.log and public-export-queue-final-release.log also passed. The standard runner still skips the optional large codec stress unit; this is not target-scale validation.
