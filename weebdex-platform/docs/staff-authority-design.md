# Native authority management — ratified design

Scope: local native accounts only. No original-service keys, publisher admission,
protocol signing authority, or global service credentials are managed here.
The owner ratified this design on 2026-10-28; it does not grant production authority.

## Review and mutation contract

An authenticated normal native cookie, exact expected actor, same-origin browser
source and JSON request are required. The actor's current account and session are
re-read and locked in the mutation transaction. Security changes require a verified,
unrestricted sysop and the exact action grant, plus a password login within ten
minutes. A public signer, API credential, recovery session, or cached UI role is
never an administrator capability.

The review identifies exact target ID, username, current authority revision,
selected named bundles, local effect, reason, and the fact that no source service
changes. A command carries a unique request ID and that target revision. Receipts
are bound to actor and full canonical body. A lost response is reconciled by exact
receipt; no fresh command is automatically sent. Reauthentication preserves an
account/site-bound intent for read-only resume, then requires a new review.

## Serialization and last administrator

A single transaction-scoped advisory lock serializes all authority changes and
bootstrap. A database guard covers roles, grants, verified/disabled status and
account deletion, including older account-disable code paths. An eligible
administrator is enabled, verified, neither banned nor restricted, has sysop,
and has admin.roles.manage. The final eligible administrator cannot be removed,
demoted, disabled, unverified or restricted. New authority revisions invalidate
stale management forms. A failed transaction changes neither audit nor authority.

## Redaction

List/detail/history select explicit bounded fields. Never select password hashes,
email, profile JSON, session identifiers/tokens/hashes, API secrets, recovery links,
or runtime configuration. Session revocation exposes only affected counts. Audit
is append-only and records authority effects, reason, actor and exact target.

## Reviewed fixed bundle proposal

All accounts retain the seventeen normal registration permissions. `verified` is
derived from the verified account bit, never selectable. Restriction flags are
changed separately and are preserved when selecting a role bundle.

| Bundle | Explicit roles | Additional grants |
|---|---|---|
| Roles manager | sysop | admin.users.read, admin.roles.manage, admin.audit.read |
| Restriction manager | sysop | admin.users.read, admin.restrictions.manage, admin.audit.read |
| Session manager | sysop | admin.users.read, admin.sessions.revoke, admin.audit.read |
| Security auditor | sysop | admin.audit.read |
| Report reviewer | moderator | report.update |
| Report coordinator | moderator | report.update, report.assign, report.history |
| Local moderator | moderator | report.update, report.delete, manga.delete, chapter.remove, scanlation_group.update |
| Decision corrector | staff, moderator | report.update, report.history, report.correct, manga.restore, chapter.restore, group.leadership.revoke |
| Profile editor | staff | user.update |
| Operations reader | staff | operations.read |

Multiple named bundles may be reviewed together. The product never accepts raw
role or permission strings. Replacing a selection deliberately replaces prior
nonbaseline authority and warns about legacy/custom grants. Bootstrap proposes
only the four administrative bundles, not additional content-moderation grants or source operation.
Restriction v1 lasts until an administrator explicitly lifts it; no expiry is
implied. Runtime instance policy/contact remains separately config-owned.

## Ratification and completed inventory

Owner ratified this design on 2026-10-28, including the existing role hierarchy:
`staff` and `sysop` satisfy legacy moderator role tests, but **never** grant an
exact permission by themselves. New security routes require `sysop` and the
specific `admin.*` grant. The policy module's metadata and UI explain that
hierarchy instead of globally changing shared legacy helpers. Baseline permissions
can still combine with inherited role gates, such as replying in a locked discussion
with `reply.create`. Security-only bootstrap describes the additional grants, not
the absence of all inherited native role behavior.

Additional reviewed bundles cover the existing privileged native surface:

| Bundle | Explicit roles | Additional grants |
|---|---|---|
| Account reactivator | sysop | admin.users.read, admin.accounts.manage, admin.audit.read |
| Announcement editor | staff | announcement.create, announcement.update, announcement.delete |
| Editorial reviewer | moderator | manga.state.update |
| Catalogue editor | moderator | manga.update, manga.title.edit, manga.tag.edit, manga.author.edit, manga.artist.edit, manga.link.edit, manga.relation.edit, manga.cover.edit, author.update, scanlation_group.update, chapter.update |
| Catalogue moderator | moderator | manga.lock.update, manga.delete, author.lock.update, author.delete, scanlation_group.delete, cover.delete, chapter.remove |
| Discussion moderator | moderator | reply.update, reply.remove, reply.delete |

Source inventory: `domains/announcements/mod.rs`,
`domains/catalog/{mutations,entity_mutations,repository}.rs`,
`domains/community/{mod,mutations}.rs`, `domains/moderation/mod.rs`,
`domains/staff_workspace/*`, and native public API adapters. Normal creation and
self-owner permissions are already among the seventeen baseline capabilities.
No `instance.update`, protocol operator, publisher or administrator-secret
capability is invented. Public instance policy/contact is configuration-owned.

### Uncertain-result barrier

An absent receipt or equal authority revision never unlocks a pending command.
The client persists the exact account/site/body/request ID before dispatch and
blocks new security reviews while uncertain. Read-only reconciliation, explicit
exact-ID/body retry, and an explicit cancellation barrier are separate actions.
The barrier takes the same global try-lock, proves current normal-cookie actor
ownership of the original review/body, and either returns the applied receipt or
inserts an immutable no-effect cancellation receipt. A late original request
therefore cannot apply, including when its desired state is a no-op. Cancellation
of an already applied action is not an undo. Demoted/restricted ordinary actors
may read/settle their own receipts without regaining administrator authority.

Every review re-reads actor authority; final confirmation checks the recorded
actor authority revision, current exact role/grant, fresh session, target CAS,
and unchanged body. Expired review cleanup is indexed and batch-bounded (1,000),
and converts records to cancellation receipts before deletion. The security audit
and command receipts remain append-only. No expired review is silently forgotten.
