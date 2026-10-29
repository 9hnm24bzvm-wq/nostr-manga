# Native administrator bootstrap and safe handoff

This runbook manages **this site's native accounts only**. It never creates an
original-service operator, publisher, signing key or global administrative secret.
Do not run these commands on a live deployment without its operator's approval.
The automated acceptance runner uses only a disposable loopback database.

## Prepare an existing, verified account

1. Install the reviewed platform build and run its ordinary explicit migration
   command (`weebdex-platform migrate`) using the deployment's existing database
   configuration. Serving the application does not install an administrator.
2. Register through the site's normal browser registration form. Verify the email
   using the normal one-time verification link, then sign in normally.
3. Identify the exact **native** account ID from the signed-in account session
   (`/api/auth/check`, `identity.id`). Match the username with its owner. A public
   signer key, public alias, email address or copied session token is not an ID
   selector and is never proof of administrator authority.
4. Confirm that no enabled, verified, unrestricted account already has both the
   native `sysop` role and `admin.roles.manage`. The CLI checks this itself. If one
   exists, use the ordinary Users and security workspace instead of bootstrap.

No SQL edits, fixture flags, hidden HTTP endpoint or automatic startup grant are
required or supported by this procedure.

## One-shot offline bootstrap

Use the same reviewed configuration and database connection variables as the
ordinary platform command. Keep the database credential in its existing private
configuration/environment; never paste it into a screenshot, ticket or audit reason.

```sh
# Build through the project's approved build workflow; the separate binary is:
# cargo build --manifest-path backend/Cargo.toml --bin weebdex-admin
weebdex-admin bootstrap --user EXISTING_NATIVE_ACCOUNT_ID \
  --reason 'Initial local security administration approved by the instance owner'
```

The CLI verifies the migration history, reads the existing account and prints:

- exact native ID, username and current authority revision;
- current and proposed roles, bundle names and exact grants;
- the reason and the local-only effect;
- a digest binding that complete reviewed summary.

It displays **no email address, password/hash, session token/hash, API secret,
profile body or runtime configuration**. Read the entire change, including removal
of previous nonbaseline/custom access. To proceed, type the displayed exact line:

```text
BOOTSTRAP EXISTING_NATIVE_ACCOUNT_ID EXACT_REVIEW_DIGEST
```

Anything else cancels without changing authority. The CLI does not hold a database
transaction while you review. At confirmation it takes the same bounded global
lock as normal management, rechecks no eligible administrator exists, locks and
re-reads the exact account and rejects a changed summary. A successful transaction
appends its security audit record and prints a durable audit ID and new authority
revision. Concurrent bootstrap/handoff cannot create an unreviewed duplicate.

Bootstrap grants only **Manage staff access**, **Manage local restrictions**,
**Revoke native sessions**, and **Read security history**, plus the seventeen
ordinary account permissions. It adds no report-review, announcement, catalogue-
editing or account-reactivation grants and no original-service control. The
existing native hierarchy is preserved: `sysop` satisfies legacy staff/moderator
role gates. Baseline permissions can combine with those gates, for example
`reply.create` permits replying in a locked discussion. Review this inherited
behavior as well as the exact additional grants before confirming bootstrap.

If the CLI exits unexpectedly after confirmation, do **not** assume failure or
rerun blindly. Sign in and inspect `/staff/users` and security history. A second
bootstrap refuses while any eligible roles administrator exists. If the account
or revision changed before confirmation, start a new explicit review.

## First browser handoff

1. Sign in as the bootstrapped account and open **Staff workspace → Users and
   security** (`/staff/users`). Current authority is shown separately from original
   service authority, which is never implied.
2. Ask the successor to register and verify their own native account. Search for
   that exact username, then verify the displayed native ID with its owner.
3. Choose **Change staff access** and select only the named bundles needed. To
   create a second administrator who can safely receive the handoff, include
   **Manage staff access**. Granting security administration is not the same as
   granting every moderation or editorial action.
4. Enter a bounded reason. Review exact ID, username, authority revision, roles
   and grant additions/removals. Legacy/custom authority requires explicit
   acknowledgement before replacement; it is never silently merged or removed.
5. If prompted, use **Sign in again**. The ordinary login form requires the native
   password. Returning restores saved fields for read-only inspection, **not** an
   automatic write. Review the current account again and confirm explicitly.
6. Have the successor sign in separately and verify their own actual authority.
   Only then deliberately remove the predecessor's access if appropriate.

The final eligible roles administrator cannot be demoted, restricted, disabled,
unverified or deleted. This applies to ordinary account closure and email changes
as well as the staff UI. A refusal rolls back the entire operation. Before changing
the sole administrator's email, appoint another enabled, verified, unrestricted
roles administrator. **Password changes and password recovery remain available**;
they do not require removing verification or management authority.

## Restrictions, credentials and reactivation

- **Restrict locally** lasts until an administrator explicitly lifts it. There is
  no hidden expiry. **Lift local restriction / ban** clears local restriction
  flags only; it does not verify or enable a disabled account and does not change
  source-service admission.
- **Sign out native sessions** revokes only the selected account's native ordinary
  and recovery sessions, pending recovery links and native API credentials. The
  UI/audit show counts, not credential values. Passwords and original/operator
  credentials are not deleted. Revoking your own sessions signs you out; sign in
  normally again to read your original receipt.
- **Reactivate retained account** needs its separate named bundle and the exact
  `admin.accounts.manage` permission. It works only on a retained owner-disabled
  account, requires a reason and acknowledgement of the owner's request, and
  never recreates purged data or clears independent restrictions/verification.
  Old native sessions/API credentials remain revoked. Revoked identity bindings
  are not restored. The owner must sign in normally and deliberately re-establish
  any separately authorized association.

## Interrupted or uncertain result

A missing receipt, equal authority revision, reload, Back, Close or expired login
is **not proof that an earlier request cannot apply**. The workspace preserves
an account/site-bound exact command fence and blocks a replacement review.

- **Check exact receipt** is read-only. If committed, it displays the original
  receipt without repeating the operation.
- **Retry only this unchanged command** is an explicit resend of the identical
  actor/body/request ID. A committed command returns its original receipt; it
  cannot duplicate the effect. Expired reviews/current authority still fail safe.
- **Cancel only if not already applied** takes the global authority lock. It either
  returns the already applied receipt, without undoing it, or installs a durable
  no-effect tombstone that prevents a late original request from applying.

Even a demoted or locally restricted actor with a normal native cookie can read
or settle their own exact receipt; that grants no new security authority. If the
session was revoked, sign in normally first. Expired reviews are converted to
cancellation receipts before bounded cleanup, never silently forgotten.

## Error recovery and audit

- Busy account/session/global lock: no blind repeat. Refresh, inspect the original
  receipt if uncertain, and review again deliberately. Operations have explicit
  lock and statement deadlines and do not wait forever.
- Stale target or actor authority: load current access and create a fresh review.
- Missing role/exact grant: ask an existing eligible administrator for the needed
  reviewed bundle; public signing, API credentials and recovery cookies cannot
  substitute for a normal native administrator session.
- Last-administrator refusal: perform a verified browser handoff first. Do not
  disable database guards or edit grants manually to bypass it.

Security audit and command receipts are append-only. List/detail/history surfaces
use explicit minimal fields and never expose account email, password material,
profile JSON, tokens, secret hashes or runtime configuration.

## Isolated verification

The owned fixture runner creates a new `weebdex_platform_test` database on private
loopback, records ownership, and shuts it down after the command. Never substitute
a production DSN. Coordinate the repository's single Cargo/build slot.

```sh
scripts/run-auth-tests.sh cargo test --manifest-path backend/Cargo.toml \
  --test staff_authority_http -- --ignored --test-threads=1 --nocapture
node --test tests/ci/test_admin_authority_client.mjs
```

The HTTP test registers eight independent principals through ordinary browser APIs,
verifies seven through the real local mail sink, follows this exact CLI's cancel
and confirmed-bootstrap prompts, and exercises permission controls, handoff,
fresh-login recovery, sole-admin email rollback/password recovery, CAS, tombstones,
credential revocation, bounded locks, redaction and last-admin concurrency. The
finite client test is synthetic transport coverage, not a substitute for HTTP or
mounted browser acceptance. Record exact results and source/build hashes with the
release evidence; do not claim unrun stages passed.
