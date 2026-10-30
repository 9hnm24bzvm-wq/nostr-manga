# Reviewed native publish-now access

This is a supported **offline operator CLI**, not a public HTTP grant endpoint or an automatic startup/bootstrap action. Installing the feature does not grant anybody access. Actual delegation requires separate operator authorization.

## Scope and prerequisites

Use an already-migrated deployment, an existing enabled, verified, unrestricted native account, and the exact already-configured original operator **public** publisher key. The original publisher must already have current publisher admission through the original module's supported operator controls. This command does not create a native account, admit a publisher, generate/load/export a signing key, open original storage, or provision a group. Runtime key/storage health is not attested by a review.

A dedicated delegation permits that native account's current ordinary browser session to publish kind 1001/1002 immediately, still subject to its current native catalogue/upload permissions. It does **not** let the original signer authenticate as that account, grant native roles/permissions, connect a portable identity, authorize publishing-enabled groups, enable scheduled publication, or authorize source withdrawal/deletion. Legacy explicit signer bindings retain their historical routes; this workflow never writes them or silently replaces them. A dedicated row, including its revoked history, never falls back to a legacy binding.

The normal Upload screen reports publish-now eligibility and **separate scheduling eligibility**. Delegation-only accounts must deliberately clear any chosen future time to publish now. The server refuses unsupported future dates before allocating a schedule or original publication command. Existing legacy schedules and their read/cancel/retry paths remain separate.

## Review, then confirm exactly

Use the usual local configuration and database environment (`PLATFORM_CONFIG` and `PLATFORM_DATABASE_URL`) without printing credentials. Substitute the existing account ID and configured public publisher. Operator reference is an audit label, **not verified personal identity**; access to the trusted offline environment/database remains the administrative boundary. Use a non-sensitive reason/reference (no passwords, tokens, email addresses or private keys).

```sh
weebdex-admin publication-access review \
  --operation grant --user EXISTING_NATIVE_ACCOUNT_ID \
  --publisher CONFIGURED_PUBLIC_PUBLISHER_HEX \
  --operator 'approved change reference' --reason 'Reviewed publish-now access'
```

Review the displayed native ID/username, enabled/verified/restricted state, account version/authority/lifecycle, configured public publisher, original admission epoch, current delegation revision, capabilities and exact effect. Save the returned review ID and digest privately. The immutable review expires after ten minutes according to database time. It is **not** permission to apply silently.

```sh
weebdex-admin publication-access apply \
  --review REVIEW_UUID --digest REVIEW_SHA256 \
  --operator 'approved change reference'
```

The CLI displays the reviewed summary again. Type precisely the displayed `PUBLICATION-ACCESS APPLY REVIEW_UUID REVIEW_SHA256` line. Any other input aborts that invocation without granting access **or canceling the durable review**. Account, delegation, configuration, original admission epoch and final review expiry are rechecked under bounded serialized locks. A stale review must be explicitly canceled and replaced, not edited or force-applied.

## Unknown result, cancellation, and revocation

A lost response or absent receipt is not evidence the old request cannot commit. Do not create an unreviewed replacement or use raw SQL. Read only:

```sh
weebdex-admin publication-access receipt --review REVIEW_UUID --digest REVIEW_SHA256 --operator 'approved change reference'
weebdex-admin publication-access pending --user EXISTING_NATIVE_ACCOUNT_ID
```

`pending` recovers minimal unsettled review references, not private reasons. One unsettled review per target blocks a new review, including expired/no-op reviews. A receipt is immutable and bound to the exact ID/digest/operator reference. Repeating exact `apply` returns an existing receipt without another authority change. After an unknown result, prefer readback or explicit cancellation, not blind replay.

```sh
weebdex-admin publication-access cancel --review REVIEW_UUID --digest REVIEW_SHA256 --operator 'approved change reference'
```

Confirm the displayed exact `PUBLICATION-ACCESS CANCEL ...` line. Cancellation serializes with apply. If apply already committed, it returns the applied receipt and **does not undo** it. Otherwise an append-only cancellation receipt prevents any late old apply, even if the reviewed desired state was a no-op. Expired reviews can still be canceled.

For removal, review `--operation revoke` with the same target and stored public publisher, then inspect and apply that new exact review. Revocation changes only the dedicated native publish-now delegation. It never retracts accepted source events, changes original admission or erases historical source data. Unresolved original publication commands, source withdrawal intents or schedules block grant/regrant. A reviewed containment-only revoke remains available: its immutable review and receipt show exact unresolved counts/statuses and the irreversible-acceptance warning. It revokes only dedicated native publish-now authority, never cancels or resolves that work. If an old admission epoch prevents recovery, do not rebind the command or pretend its accepted source was unpublished; preserve its receipt/pins for operator-assisted source reconciliation. Existing native restrictions/session revocation remain independent containment controls. A prepared or accepted command never adopts a later delegation revision, publisher, original admission epoch or account lifecycle. Current-authority failure leaves its durable uncertainty/pins intact; unavailable receipts do **not** mean canceled/unpublished.

Original publisher revoke → re-admit changes its admission epoch and does not revive an old delegation. A new explicit native delegation review is required. Owner account disable revokes the delegation and records its audit atomically; administrator reactivation never restores it. Last-administrator refusal rolls back the disable, delegation revision and audit together.

## Bounded operation and audit

The CLI never opens the publication runtime or original UploadStore; it reads the public configuration and original `publisher_permissions` policy only. A global nonblocking access-change lock, account-before-delegation row order, short lock/statement deadlines, current ordinary session checks before signing and again after staging at the original publication API handoff, delegation locks held through signing and immutable command authority captures protect races. This clock check is at the API handoff, not an atomic condition inside the original module's later acceptance transaction; it does not promise that the native session cannot expire during that module's internal waits. Held/conflicting activity fails safely; inspect the receipt and retry deliberately only after it is understood. Full account secrets and native credential material are never part of reviews/receipts/audits.

Review/receipt/audit history is append-only. No-op reviewed changes are auditable but do not increment the delegation revision. Private reasons remain in operator-controlled audit history, not browser storage. Secure database backup/retention is the deployment operator's existing responsibility.

## Disposable verification only

The `publication_access_http` ignored integration target requires the repository's owned isolated `scripts/run-auth-tests.sh` PostgreSQL fixture and the coordinated Cargo slot. It exercises the actual CLI and real loopback HTTP with synthetic native accounts and ephemeral original keys/admission. Original admission setup is fixture infrastructure, **not this CLI's product behavior**. Do not point it at a deployment database. Release evidence must record the exact source/artifacts and actual test results; predecessor test receipts do not qualify this feature.
