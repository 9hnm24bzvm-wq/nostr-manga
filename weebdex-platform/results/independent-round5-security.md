# Independent completeness review, round 5: security and operations

Review window: 2026-10-13 05:32–05:38 UTC. This was a fresh finite source-only review of platform authentication, privacy, native filesystem durability/recovery, permissions, public-source visibility, deployment boundaries and security budgets. No Cargo command, runtime test, service launch, dependency operation or production-source edit was performed. This report is the only file written by this reviewer. The parent received each actionable finding as it was established. Other owners may edit concurrently; findings below refer to the inspected snapshots and do not assert closure of subsequent changes.

## Finite verdict

**Three new substantiated in-scope gaps remain in the inspected source: a private-list visibility/content race (P2), recovery redemption unable to reach the mounted browser password-reset form (P1 functionality), and pre-change recovery links retaining password-reset authority after credential rotation (P2).** The previous protocol-cover durable-installation bypass and optional recovery personal projections are source-closed in their inspected paths. These closures do not establish browser reset completeness: the newly found reset-form blocker is a different boundary.

This is neither an exhaustive security proof nor runtime or production qualification. Required real PostgreSQL/Router, mounted-browser, proxy/TLS, migration, filesystem fault/restart and shutdown acceptance remains unrun. The independently assigned original-module process/lifecycle audit is outside this review's verdict; this reviewer does not certify its new 23-file freeze.

## R5-SEC-01 — P2: a public list GET can return contents introduced only after it becomes private

Source evidence:

- `backend/src/domains/library/lists.rs:71–90`: GET authenticates optional personal identity, starts a default transaction, reads `user_id,visibility` without a row lock, and permits a nonowner/anonymous reader when the observed visibility is public or unlisted. It subsequently calls a separate DTO query.
- `backend/src/domains/library/repository.rs:77–78`: `list_json` selects the current name, description, visibility, version and members by list ID alone. It contains no eligibility predicate.
- `lists.rs:125–149`: the owner update locks the user/list and can replace members, change name/description and switch visibility to private in the same commit. The GET's earlier unlocked SELECT does not block it. GET does not request repeatable-read isolation.

Concrete interleaving:

1. A public list exists, with no secret description/member selection.
2. An anonymous or other-account GET completes its first SELECT and authorizes the then-public list.
3. The owner atomically changes visibility to private and adds a new private name/description/member selection, then commits.
4. Under the default READ COMMITTED transaction, GET's later `list_json` obtains a new statement snapshot and returns the now-private contents. These new contents were never public, so this is more than the inevitable delivery of an already-read public snapshot.

Impact: disclosure of newly private list metadata and reading selections to a known-list-ID nonowner reader during an ordinary privacy transition. The existing private-default and direct-list owner checks establish the intended boundary.

Source acceptance: eligibility and full DTO must share a coherent authorized snapshot, preferably one query with the visibility/owner predicate, or an explicit snapshot/locking contract that prevents a later unauthorized DTO version. Preserve intended owner-library bearer access and unlisted direct access. Do not rely on no-store headers or a second identity lookup to fix the inconsistent data snapshot.

Required genuine acceptance: use real Router/PostgreSQL concurrency, pause GET after its eligibility observation, commit public/unlisted-to-private together with new private data, and resume GET. It must deny or return only the previously authorized public snapshot; it must never return the newly private data. Check anonymous, nonowner browser and nonowner scoped bearer, plus owner and unchanged-public controls. Current sequential private-list fixture assertions do not establish this race boundary.

## R5-SEC-02 — P1 functionality: valid password recovery cannot mount the browser reset form

Source evidence:

- `backend/src/domains/auth/mail.rs:235–261`: redeeming a genuine recovery link issues the purpose=`recovery` cookie, rotates the browser-flow cookie and redirects to `/settings/account`.
- `backend/src/domains/auth/preferences.rs:18–21`: `/api/auth/check` resolves that cookie, then calls `require_browser_session`. `auth/principal.rs:67–74` refuses recovery with HTTP403.
- Retained `frontend/src/lib/stores/Session.ts:12–15` hydrates `$Session` only on a successful200 response. The mounted Session overlays at `platform-integrations/frontend-overlays.mjs:143–165` retain this gate; they add refresh and account preference handling, not recovery recognition.
- Retained `frontend/src/routes/(app)/settings/+layout.svelte:28–45` hides Account when `$Session` is absent and redirects a hidden Account destination to the first settings tab.
- Retained `frontend/src/routes/(app)/settings/account/+layout.svelte:10–14` wraps the password-settings page in `Protected` and a user-role condition. `frontend/src/lib/components/ui/Protected.svelte:18–27` renders children only with `$Session`.
- The only Account overlays at `frontend-overlays.mjs:191–204` repair page script refresh. No account-layout or general Protected replacement admits limited recovery; local-library Protected replacements are restricted to library/history/library-settings.
- `backend/tests/auth_http.rs:1325–1329` still expects200 and empty permissions for `/api/auth/check` immediately after actual recovery redemption, contradicting the inspected implementation. The later private-projection rejection assertions are useful but do not remove this contradiction.

Concrete normal flow: open a valid recovery email link in a fresh browser, receive the limited cookie and redirect, then let the app initialize. `/api/auth/check` returns403; Account is hidden/redirected and its protected children cannot mount. The page's real `createBrowserSettingsFlow` and password form never become usable. Direct API calls to the settings flow can succeed, but they do not establish the requested browser recovery journey.

Impact: someone who forgot their password cannot use the actual browser UI to complete a valid password recovery. This affects a primary account-recovery feature, not an optional future feature.

Source acceptance: provide a deliberately minimal recovery identity/route capability that lets only the password-reset UI mount, or a dedicated recovery-reset route with the equivalent limited guard. Never restore full library/preferences/follows/private votes/drafts/staged-media projections to recovery to make the UI pass. Keep profile/client/domain mutation restrictions and stale/expired recovery rejection.

Required genuine acceptance: a real synthetic mail-delivery link opened in a fresh mounted browser must reach and render the reset form, submit a new password, rotate to an ordinary session, invalidate the old password/session/client grants, and show the normal account UI. Before completion, private follows/preferences/library and private social viewer data must remain inaccessible and profile changes must fail. Update the contradictory actual Router fixture expectation to match the chosen safe minimal reset capability and test its shape. A settings API-only success is insufficient.

## R5-SEC-03 — P2: unused recovery links retain reset authority after a password change

Source evidence:

- `backend/src/domains/auth/flows.rs:984–987`: successful password change updates the password and revokes every other session plus all API clients. It never invalidates outstanding recovery email tokens.
- The only token mutations in `backend/src/domains/auth/mail.rs` are expired-token cleanup/issuance at144–145, delivery status at156 and consumption of the single presented token at215–216. `auth_email_tokens` has no credential generation binding in `backend/migrations/0012_email_tokens.sql:4–20`.
- `mail.rs:193–211`: redeem accepts any unused, delivered, unexpired matching token, then checks only disabled state and unchanged email hash. A password change does not affect either eligibility condition.
- `mail.rs:235–245`: that still-eligible old link creates a fresh recovery-purpose session, which is authorized for the actual password-setting branch at `flows.rs:949–985`.

Concrete scenario: request two delivered recovery links A and B for the same account, or request B before an ordinary recent-auth password change. Complete the password change/reset (using A in the two-link case). Before B's original fifteen-minute expiry, redeem B. It creates a new recovery session and can replace the new password again despite completion revoking the other session/client credentials. The attack requires possession of the earlier link and is bounded by its remaining token lifetime; no perpetual or arbitrary account access is claimed.

Impact: a leaked pre-change password-reset capability survives the credential-rotation boundary and can undo the user's recovery during the remaining issuance window. The one-use property of A does not invalidate separate unused B.

Source acceptance: invalidate pre-change outstanding recovery capabilities atomically with both normal password changes and recovery completion, while preserving newly requested post-change recovery and correct verification semantics. Account/token/session locking or a credential-generation binding must define the overlapping issuance/redemption/rotation ordering; merely deleting in a later best-effort task is insufficient.

Required genuine acceptance: actual mail/Router/PostgreSQL fixtures should issue multiple links, complete each kind of password change, and verify all pre-change recovery links and recovery sessions fail afterward. A newly requested post-change recovery link must work. Exercise redemption/issuance paused across password rotation; no old-generation token may create a usable post-change recovery session. Do not expose raw fixture links in logs or receipts.

## Independently reread source closures

- **R4-SEC-01 source-closed:** `backend/src/protocol/covers.rs:170–188` now uses the shared `storage::store_encoded` for every variant instead of its independent late-marker writer. `storage.rs:134–159` durably prepares the key-only marker and verifies/holds its named OS lease before final creation; `storage.rs:221–246` creates/syncs final entries; `covers.rs:195–210` shares the matching object advisory lock, protects each final object before SQL COMMIT and clears intents only after acknowledged persistence. `storage.rs:250–261` retains the intent across failed/cancelled cleanup. The source-cover test at `covers.rs:237–278` covers the helper's live lease/unknown outcome contract but is not executed here and does not replace real SQL/power-loss acceptance.
- **R4-SEC-02 source-closed in the reported projection paths:** `auth/principal.rs:89–93,264–272` separates optional personal reads from the deliberately available reset identity. `community/mod.rs:193–196,211–214` excludes recovery and bearer identity from private viewer-vote projection. `catalog/read.rs:104–107,240–244` obtains required library capability for reading-status filters and `catalog/query.rs:297–303` independently checks it. `library/lists.rs:71,189–200` excludes recovery from private owner projections. Genuine recovery-cookie test assertions were added for replies, saved-state filtering and private list feed at `auth_http.rs:1330–1362`; they remain unrun, and the contradictory auth/check expectation is called out separately above.
- **Previous private-path traversal and proxy transport closures retained:** `config.rs:551–565` rejects ParentDir before future-path resolution; UI/private/signer/original/backend roots go through actual ancestry resolution and disjointness checks. `client_peer.rs:48–115` grounds identity in actual ConnectInfo, ignores forged chains from untrusted transports, normalizes mapped addresses and validates configured exact hop chains. `analytics/mod.rs:90–99` consumes the verified extension. The nginx example overwrites XFF at the reviewed edge and documents its local transport trust boundary. Actual proxy-through and path-alias acceptance remains unrun.
- **Native install/recovery scope retained:** shared durable prewrite helper/lease plus final directory syncing, current media-object checks under advisory key ownership, bounded aged pending reconciliation, temporary/proxy lease cleanup and immutable original pins remain visible in the inspected paths. No additional concrete orphan-installation or current-reference deletion counterexample was established in this review. This is not a filesystem fault qualification.

## Other inspected boundaries and explicit limits

The finite pass reread strict issued-bearer scope/role narrowing and expected-account constraints; protected account/client/report/community operations; recent-auth credential/session rotation; mail delivery/redeem/token binding; private defaults/owner list guards; catalog draft/cover/staged upload permissions; source-visible media and preview eligibility before conditional delivery; streamed request/body/media/decode/proxy bounds; DNS-pinned inline-image targets; generated-bootstrap CSP/private no-store policy; production Host/Origin handling; durable pending/expired-session/unused-object cleanup; atomic replacement and prepared-command input fencing; ordered/checksummed migrations; private systemd writable roots; exact nginx upload/replacement paths and logging exclusions; and source/release claim boundaries.

No additional substantiated arbitrary filesystem path, source visibility bypass, bearer authority escalation, SSRF target-policy bypass or proxy identity-collapse defect was established beyond the findings above. Runtime visibility-function DDL remains a documented/provisioning prerequisite rather than a new defect absent the intended operator-role contract. Real provider mail, TLS/proxy, fresh/upgrade migration, power/fault recovery, original signed processes/relay lifecycle, mounted browser/PWA and device/mobile qualification remain pending or outside this finite source pass.

## Snapshot provenance

Read-only SHA-256 snapshot at approximately05:37 UTC (subsequent owner patches require a fresh review):

- `auth/principal.rs`: `e2ece78caf36d498e31510cfe6143b609eb66b8d1aea3705e0b3d263712c164e`
- `auth/preferences.rs`: `f8b093d61cd08ee3031ce07e5e30f2748d150a5a1a11ec16aad5f87cd221befe`
- `auth/flows.rs`: `8304371e4a829cc1d79c7f76bc69816397f0335ec8d6052db04490de74f1c7a8`
- `auth/mail.rs`: `1397acbea42f0011b3c4ee6ab4046265e73fc22d9c8a9e0f2f8911ce70989c8c`
- `protocol/covers.rs`: `9e5490bcc567f6089a0bb091c61f013181279e7f7404fb120783483822f4ae16`
- `uploads/storage.rs`: `77d9b5be1739f5fc74a75637b984e47346ab46765a062e7fc0fe44a476b1c196`
- `library/lists.rs`: `58288e5e697208b8bb96aa89d7b1fea7b04bddab839f20314b4087492a93fd48`
- `library/repository.rs`: `80e4e2efdd338a190ffd47ab2419d011655a76e09e273d43274d9e54bb80ef54`
- `platform-integrations/frontend-overlays.mjs`: `32a6d12e6ab9e8ad852584e22d6c493f30dc50d8de244bce89c5161f8df9badd`
