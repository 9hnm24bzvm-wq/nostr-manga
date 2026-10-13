# Next implementation: abandoned staging and safe deletion

Status: expiry, group leases and transfer holds are implemented and verified as described in staging-reclamation.md. Schema40 implements authenticated reference-API deletion; see staged-deletion.md.

## Required invariant

Expiry or a delete request makes an unreferenced generation eligible for inspection. Only verified filesystem cleanup plus the existing durable accounting transaction can refund it. Publication/library references, valid grants, current mutation generations and outstanding external readers must remain protected.

## Proposed sequence

1. Extend the existing GC queue with an explicit staging source. Keep library/publication opt-in controls independent and never infer deletion authority merely from a queued row.
2. Add bounded, rotating selection of expired available unretained uploads. Use the existing sparse unreleased-staging-credit index rather than repeatedly scanning the hosted catalogue. Preserve a cursor across worker iterations/restarts; resume already claimed cleanup even if new selection is paused.
3. Extend eligibility to an expired unretained generation while preserving every reference, grace, publisher/account/group grant, mutation and reader check. Reuse the existing receipt-backed cleanup/refund operation instead of implementing a second deletion engine.
4. Give managed-group staging grants an explicit generation/lease policy. Do not silently make existing persistent group grants immediately invalid during migration. Document upgrade grace and test group publish races.
5. Implement authorized direct deletion through the same lifecycle. Releasing one uploader's staging interest must not remove another uploader's valid interest or any hosted original. Expose a protected/in-use result rather than pretending physical deletion happened.
6. Advertise the staging policy with the later capability registry. Existing five-minute staging behavior is an implementation default, not a universal federation rule.

## Acceptance scenarios

- Successfully uploaded, never-published expired bytes are eventually removed and both physical/staging charges are refunded exactly once.
- Active upload grants and a concurrent publication/library commit stop premature cleanup.
- Shared originals and group-scoped uploads remain protected until all relevant holds are released/expired.
- Disconnect, process death, unlink/fsync failure, stale generation and database restart cannot refund uncertain bytes.
- Disabled new cleanup still resumes durable claims safely.
- A collection larger than one selection page makes progress without starvation.
- HTTP deletion authenticates actor/action/hash and cannot become a remote moderation bypass.

Do not start another throughput qualification to implement these functional contracts. Preserve bounded queries, existing ownership fences and explicit budgets; measure performance after agreed features are covered.

## Implementation progress and remaining direct-delete contract

Schemas37–39 implement bounded abandoned-staging selection, group lease expiry and in-flight transfer protection; see staging-reclamation.md for current verification scope. The deletion endpoint is implemented with conservative shared-interest refusal; see staged-deletion.md.

Before immediate physical deletion, every uploader's temporary interest must be represented explicitly, including an administrator using the ordinary upload API. Clearing the common staging timestamp without that accounting could remove another uploader's unreferenced-but-active bytes. An explicit delete must authenticate the actor/action/hash, fence the permission epoch, refuse hosted references/active other interests/readers/transfers, and share the receipt-backed reclaim transaction. A queued or protected result must never be presented as completed physical deletion. Account/library retirement remains its existing dedicated workflow.
