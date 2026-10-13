# Optional reader self-registration (schema52)

The owner selectedA on8October2026: configurable self-registration, disabled by default. This is an instance-local account admission feature. Nostr identity and signed events remain separate; no passwords, email identities, private-key custody or additional authentication service are introduced.

## Configuration and endpoints

The optional `enrollment` configuration object contains:

- `enabled`: false by default
- `max_new_accounts_per_hour`:100 by default; valid range1..1000000
- `max_self_enrolled_accounts`:10000 by default; valid range1..1000000000

These defaults are conservative abuse bounds, not a tested user-capacity estimate. The hourly limit is an instance-wide, persisted fixed one-hour creation window, not a rolling per-user allowance. The lifetime counter counts successful self-enrollments, including keys later revoked. Operator-admitted keys do not consume this self-enrollment allowance. Raising a bound is an explicit operator configuration choice; no automatic counter reset or capacity upgrade occurs.

GET `/enrollment` publicly describes the configured policy, bounds and scope, even on an instance whose content reads require admission. The root capability document includes the same description. It exposes no live account counts or private member data.

POST `/enrollment`, or POST `/compat/mangadex/nostr/enrollment`, accepts an empty JSON object. The caller supplies fresh NIP98 authorization bound to the exact URL, POST method and body hash. The verified signer is the only enrollment target. A body specifying another key, roles or permissions is rejected. Disabled self-registration returns403. A missing event backend returns503 rather than claiming usable account-write admission.

A new successful admission returns201; an already admitted key returns200 with `created:false`. The response contains the public account key, admission result and permission version, plus `publisher_permission_granted:false`. Ordinary authenticated-response proofs also cover this route. Fresh HTTP authorization is required for every retry; a response lost after commit can be retried without creating another account or changing its permission epoch.

## Privilege and revocation boundaries

Self-enrollment grants only the existing personal account permission. It enables that key's author-signed metadata, social and library workflows subject to their existing budgets and policies. It does not admit a manga publisher, create a managed-group role, approve a peer, grant instance administration, publish a placeholder profile/library, or sign anything for the user.

Operator revocation sets an explicit self-enrollment block. Signup cannot reverse it, including concurrent attempts or after a service restart. An operator may deliberately re-admit a key through the existing account administration API. Disabling self-registration does not revoke previously admitted accounts.

A new unadmitted row created solely by federation can enroll with proof of its key if no operator block exists. Upgrade migration treats all pre-schema52 denied rows conservatively as blocked because their original source may be ambiguous; operators can deliberately re-admit those keys. Previously allowed rows stay allowed. No old denial is silently relaxed. A database trigger fences older binaries that revoke by updating only the original allowed column; omitted enrollment fields on old inserts default to blocked. Only explicit new peer-placeholder insertion opts out.

The creation-budget lock and the account permission row serialize new registrations and administrative races. Account creation and budget increment commit together. Database/lock failures return503 and do not leave partial admission. Exhausted creation bounds return429. Existing admitted keys can get the idempotent result even when the new-key budget is exhausted. Moving the clock backward does not reset a future persisted window.

Public reading remains anonymous unless the instance selects a restricted read policy. An instance combining open enrollment with admitted-only reading should expect any eligible new key to join and read; this is not human identity verification or Sybil resistance.

## Restore and operations

The backup restore helper disables self-registration in the restored configuration while retaining its bounds and database history, alongside its other deliberate activation resets. Review and re-enable it explicitly if wanted. Restoring an old snapshot still requires the existing source-offline and replay-barrier procedures; this feature does not change the durability limits of backups.

Private group keys, publisher admission and other operator controls retain their existing setup and confirmation requirements. No live production configuration is enabled by shipping this feature.

## Verification

`http_enrollment` covers disabled default, policy discovery on a restricted instance, exact proof/body and replay checks, concurrent duplicate enrollment without epoch/count changes, no publisher/operator grant, real signed metadata/library saves, explicit denial, new peer placeholders, both creation limits, restart and disabled-again behavior.

`enrollment_fences` exercises the exact upgrade migration, operator insert/revocation races, budget-update failure rollback, clock rollback and publisher isolation. The backup helper has a restore-policy unit test. Focused fixtures,120 Python tests and the69-group combined regression pass, including89 library units (one optional stress test skipped),Clippy and independent process/PostgreSQL restart. The optimized build passes. See `results/enrollment-focused-2.log`, `enrollment-fences.log` and `enrollment-python.log`. The initial failed fixture is retained separately; its budget-parameter integer type was corrected with an explicit BIGINT cast.
