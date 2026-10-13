# Reader enrollment: original decision and implementation plan

The owner selectedA on8October2026 at11:48UTC. The implemented contract and focused evidence are in [reader-enrollment.md](reader-enrollment.md). This document retains the original design/test plan. The default remains disabled; enabling a live instance is an operator choice.

## Recommendation

Use configurable self-registration, disabled by default for the initial small-VPS pilot. Account admission should authorize the existing personal metadata/social/library workflows only; publisher admission, managed-group roles, peer approval and instance administration remain separate.

The pending choices are:

- A. Configurable self-registration, disabled by default
- B. Self-registration enabled by default
- C. Operator admission only
- D. Invitation-based enrollment

Nostr authenticates keys and leaves relay admission to local policy. It does not require a centralized signup provider or email account. This proposal uses the existing NIP98 proof-of-key HTTP boundary; it does not add Kratos or another base service.

## Shared invariants for any new enrollment API

1. The caller proves control of its own key using fresh NIP98 authorization bound to the exact endpoint, method and body. A supplied public key alone is insufficient.
2. A successful enrollment creates only a personal account permission. It grants no publisher, peer, operator, group role or signing-key custody.
3. Re-enrolling an already admitted key is idempotent and does not increment its permission epoch. Repeated signup must not invalidate valid saves or generate unnecessary rows.
4. An existing explicitly denied/revoked key cannot self-restore permission. An operator may deliberately re-admit it through the existing administrative route. Invitations must not override that denial automatically either.
5. Enrollment and operator admission changes serialize through the same account identity/permission row. Racing with a revocation cannot leave a denied account admitted.
6. Registration creates no placeholder public profile, library or other Nostr event. Those remain separately author-signed and public under instance access policy.
7. Reader policy remains independent: public reading still needs no account unless an instance selects a restricted policy.
8. Global storage and concurrency limits remain effective. Open key enrollment is not proof of a unique human and does not solve Sybil abuse.

## Proposed request and implementation boundary

A purpose-specific enrollment endpoint would accept an empty/bounded body and take identity only from verified NIP98 authorization. It cannot reuse an account-write helper that first requires admission, because a genuinely new reader has no permission row yet. Instead, it should reuse signature, exact URL/method/body, clock-barrier and persistent replay primitives, then perform the enrollment-specific transaction.

The transaction attempts a new account row only when the selected local policy permits it. On conflict it inspects current admission; it must never implement signup as an unconditional upsert setting allowed=true. Database failure is retryable and must not be reported as successful admission. A response lost after commit is recoverable with fresh HTTP authorization; the same already-admitted key returns the admitted result without another mutation.

For optionsA/B, define explicit bounded creation-rate and total-account controls before enabling open enrollment. Account creation is cheap, but an unbounded permission table is still abuseable. Exact defaults should be presented with the selected policy rather than disguised as a verified capacity estimate. Repeated already-admitted requests should not consume a new-account allowance, but remain subject to request-rate/concurrency limits.

OptionD additionally needs a reviewed invitation lifecycle: operator issuance, a secret stored only as a hash, expiry, one-use or explicitly bounded use count, revocation and atomic consumption with account admission. Actual production invitation creation is an operator action. Do not build or deploy that mechanism unlessD is selected.

## Required functional tests

- Unauthenticated, invalid signature, wrong key hint, wrong URL/method/body and replay are denied.
- Disabled policy denies new self-enrollment and preserves existing permissions.
- Successful first enrollment grants only account writes; publisher/group/peer/operator APIs remain denied.
- Duplicate enrollment with fresh auth returns the admitted result without changing permission epoch.
- Existing denial and revocation cannot be undone by enrollment.
- Concurrent registration/registration produces one account; registration/revocation respects serialized operator policy.
- A failed database transaction creates no admission or partial invite consumption.
- Restart preserves policy state, replay protection and the resulting account permission.
- Freshly enrolled keys can perform the already-tested signed metadata/library workflows, while anonymous public reading remains unchanged.
- Exhausted registration bounds reject new keys explicitly without blocking operator management or existing admitted readers.

## Delivery boundary

After an owner choice, implement only that policy's needed path, run focused and combined PostgreSQL/HTTP tests, advertise the effective policy and limits, update the native API index, and replace the existing source/runtime Library artifacts. No production configuration or access permission is to be changed implicitly by the prototype implementation.
