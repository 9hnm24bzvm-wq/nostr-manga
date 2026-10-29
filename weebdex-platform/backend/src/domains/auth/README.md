# Native browser authentication

The unchanged WeebDex UI talks to this Rust facade using `@ory/client` browser
methods, with `PUBLIC_KRATOS_PUBLIC_API=/auth` and `PUBLIC_API_URL=/api`.
No Kratos service or fake successful login is involved.

## Deployment

- Apply the entire registered platform migration list with the platform CLI before
  starting service. Auth owns migrations 10, 11 and 12, independently of the
  retained protocol migration history.
- Set `public_origin` to the exact browser origin, including any nondefault port.
  HTTPS is required outside explicit loopback development. The Rust service binds
  loopback; terminate TLS with a correctly configured reverse proxy.
- Registration is disabled by default. An operator can explicitly set
  `registration_enabled=true`. New accounts are unverified, and public write/API
  credential permissions remain ineffective until actual email verification.
- Session cookies are HttpOnly, SameSite=Lax and Secure for HTTPS deployments.
  Cookies are host-only. Session authority, expiry and revocation always come from
  PostgreSQL; they are never cached as public derived data.
- Set up actual email delivery to make verification and recovery usable. Disabled
  delivery returns an explicit provider-required error and never creates a
  verified identity or recovery session.

Optional JSON configuration:

    "auth_mail": {
      "endpoint": "https://your-trusted-delivery-provider.example/messages",
      "from": "account-service@your-domain.example",
      "bearer_token_env": "WEEBDEX_MAIL_PROVIDER_TOKEN",
      "allow_loopback_test_sink": false
    }

The endpoint is a trusted operator-controlled HTTPS delivery adapter/webhook. It
accepts POST JSON with `from`, `to`, `subject`, and `text`, and must return a 2xx
status only after accepting the message for delivery. No redirects or HTTP
proxies are followed. The request is bounded by a three-second timeout. A
provider requiring another payload format needs a corresponding trusted adapter.
Read an existing provider credential from the named environment variable; this
implementation does not create or save a real production provider credential.

`allow_loopback_test_sink=true` is only for isolated fixtures: it requires a
loopback IP endpoint and recipients ending in the reserved `@example.invalid`
domain. It never changes the default disabled production configuration.

## Security behavior

- Argon2id password hashes have independent random salts. Password hashing uses a
  bounded blocking-worker semaphore. Unknown accounts perform equivalent adaptive
  password work. No password, token, mail body, or internal SQL detail is logged.
- Browser flows bind a random CSRF token to an HttpOnly random browser cookie and
  the current session. They expire after ten minutes, limit attempts, and reject
  reuse after completion. Invalid browser binding, CSRF or session changes fail.
- Browser mutations need matching Origin or Referer. Cross-site fetch metadata is
  rejected. Only same-origin canonical return targets are accepted.
- Persisted identifier/browser/global limits constrain login, flow and email
  attempts. Rate-limit keys are hashed. Database authority survives restart and
  is shared between instances using the same PostgreSQL database.
- Verification and recovery links are random, hash-only, one-time and expire in
  fifteen minutes. The delivery ledger stores hashes and status metadata, never
  plaintext links or message bodies. The delivery result must succeed before a
  token can be redeemed. Changed email, disabled account, expired token, replay,
  decoy account, or failed delivery cannot verify or authenticate.
- Unknown email addresses receive indistinguishable decoy messages. API responses
  do not reveal account existence or return recovery secrets.
- Recovery redemption opens a limited session solely for setting a new password.
  All ordinary domain authentication rejects that session. Settings additionally
  rejects profile changes from recovery sessions. Password completion rotates a
  normal session and revokes all other sessions and API clients. Email changes
  remove verification, revoke other sessions/clients, and rotate the current
  session. Authentication freshness is preserved across this rotation.
- Existing session age must be below ten minutes for account credential changes
  and API-client issuance/rotation. A fresh password login with `refresh=true`
  rotates/revokes the previous browser session.
- Client creation/rotation disclose the random secret once, persist only its hash,
  enforce owner scope, cap active clients at ten, and expire clients after ninety
  days. The current UI has no scope editor, so issuance uses bounded read-only
  `self.library.read` scope. This baseline owner-read grant is also recognized for
  legacy active accounts. Bearer clients cannot inherit browser staff roles,
  freshness, mutation permissions, account email, private drafts/staged media,
  or private social votes. Revocation, narrowed scopes and rotation are checked
  from primary persisted state; unsupported legacy scopes do not widen authority.
- No OIDC provider is advertised. An OIDC request fails explicitly until a real
  provider integration is added. MFA/passkey semantics are not implemented.

## Shared domain interface

Ordinary catalogue account actions have an explicit native-browser UUID surface
under `/api/v1/account`: `me`, `rating`, `rating/{uuid}`,
`report/reasons/{category}`, `report`, `{manga|group|user}/{uuid}/follow`,
`user/follows/{manga|group|user|list}`, `user/follows/manga/feed`,
`user/follows/list/{uuid}` and `list/{uuid}/follow`.
They use the existing native account, rating, follow, report and audit rows. No
protocol enrollment, signing identity, identity binding or server signing occurs.
The independent signed endpoints retain their original proof/admission rules.

These routes require exactly one normal session cookie and the captured native
account ID in `X-Weebdex-Expected-Account` (from `/api/auth/check`). Any
`Authorization` header is refused, even if a valid cookie accompanies it. The
account is locked before the session inside the actual operation transaction;
revocation, expiry, recovery-only sessions and account changes cannot be bypassed
using a cached principal. Busy authority rows fail with a retryable conflict.
`me` returns the canonical public account UUID, display name and effective
permissions, not email or credentials. It allows unverified normal accounts to
see verification guidance. Follow collection/status/feed reads preserve existing
native own-library policy, including unverified/restricted read access. Manga
and public-list follow writes are private preferences, allowing unverified
active accounts with `self.preferences.update`, exactly like private Library
saves. User/group follow mutations, ratings and reports still require verified
active accounts and their actual permissions. Signed endpoints retain their
separate verified bound-account policy. Mutations additionally require same-origin source,
the expected canonical account UUID, and the reviewed rating/follow/reason
revision. Canonical target aliases are resolved server-side, never used as auth.
All responses, including early errors, are private/no-store and vary on the
cookie, authorization and expected-account headers.

`backend/tests/native_account_actions.rs` exercises this surface with real HTTP
and isolated PostgreSQL via `scripts/run-auth-tests.sh`, including cookie-only
authority, source/permission/account fences, shared data, aliases, CAS races,
locked/revoked sessions, private headers and zero protocol identity side effects.
It also checks an unverified legacy save is visible through canonical follows.
`public_list_follows.rs` exercises genuine original public-list visibility and
head revisions with an unbound ordinary cookie account, stale/foreign CAS,
withdrawal-safe status/unfollow, source metadata privacy, and unchanged signed
proof behavior. Native status returns membership and actual tombstone version,
not a misleading absent/version-zero state after unfollow.

`require_principal(&AppState, &HeaderMap)` authenticates normal sessions or API
credentials and rejects recovery-only sessions. `optional_principal` also exposes
recovery state to the account facade. Every cookie-authenticated domain mutation
must call `validate_mutation` and enforce its actual permission/ownership rules.
Private library readers use `require_library_principal` and owner projections
check `require_library_read`. Browser-only account, draft, upload and private
social views additionally call `require_browser_session`.

`SessionPrincipal` exposes `user_id`, `username`, `email`, `verified`, `roles`,
`permissions`, `session_id`, `is_api_client`, `authenticated_age_seconds` and
`recovery_session`. `has_perm` is fail-closed for unverified protected writes and
for banned/restricted/recovery accounts and for bearer scopes outside the current
owner-library-read policy. `is_staff` means browser staff/sysop;
`is_moderator` additionally includes moderator, to preserve frontend role gates.
A role does not implicitly grant a permission. Library projects real persisted
preferences, follows and group relationships for `/api/auth/check`.

## Verification

The focused real HTTP fixtures are in `backend/tests/auth_http.rs`. Run them via
`scripts/run-auth-tests.sh` after compilation, under the approved owned-process
resource guard. That script creates a fresh reserved database on a private random
loopback port, uses the restored PostgreSQL binaries read-only, and stops the
owned process with a cleanup receipt. It refuses production/original database
DSNs and uses synthetic `example.invalid` identities only. The mail fixture uses
an actual loopback HTTP sink, with captured tokens confined to test memory.

Unit tests cover salts/password verification, token comparisons, permissions,
CSRF source validation, redirects, payload validation and secure mail config.
Focused passes do not establish complete frontend UI qualification, full load
qualification, or an independent production security review.

Recovery-cookie `/api/auth/check` returns only the reset capability
`session_purpose: recovery`, `recovery.can_reset_password: true` and empty roles/
permissions. It contains no identity, email or personal library projection. Mail
redemption redirects to `/auth/recovery`; its guarded reset UI uses the existing
password-only settings flow, whose limited DTO omits all profile traits. Successful
password reset rotates to a normal session, which the UI must freshly check.

Recovery issuance, redemption and credential rotation lock the account before
any token/session mutation. Redemption rechecks token validity after that lock.
Every password change/reset atomically consumes outstanding unused recovery links
and revokes other sessions and API clients. A pending delivery cannot revive an
already consumed link. A new post-change recovery request remains usable.
Sensitive client creation and secret rotation revalidate actual current normal,
recent, nonrevoked session and current permissions under the same account-first
lock. A principal resolved before password rotation is insufficient to mint a
new bearer afterward.
