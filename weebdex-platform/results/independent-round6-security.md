# Independent completeness review, round 6: security, recovery and operations

Review window: 2026-10-13 05:44–05:52 UTC. Fresh finite source-only review after the round-5 fixes. No Cargo command, runtime test, service launch, dependency operation or implementation edit was performed by this reviewer. This report is the only file written by this reviewer. Findings were reported to the parent as established.

## Finite verdict

**One new substantiated P2 credential-issuance race was found and promptly reported. The owner patched it during this review; independent rereading source-closes the specific counterexample. No other substantive security/recovery/operations issue was established in this finite pass.** The three round-5 security findings and the alternate platform-backend lifecycle route are source-closed at the inspected snapshots, with browser recovery qualification explicitly remaining a separate mounted-UI/runtime gate.

The new patch requires the user's requested fresh-squad audit after this report. This review does not substitute a same-review reread for that independent round. It is not an exhaustive security proof, compiler pass, runtime acceptance or production-readiness guarantee. Authored fixtures are not executed evidence. Other owners edited concurrently; the initial and closure snapshots are separated below.

## R6-SEC-01 — P2, found then source-closed: a revoked browser session can mint a new surviving API client

### Initial inspected source

Initial `backend/src/domains/auth/clients.rs` SHA-256: `1bd83b094502827106ef8b2f1195d2a75d5b37ca4550fb834d7e3b827a9f5580`.

- `clients.rs:76–79` resolved the browser session and checked its cached permission/recent authentication before opening the issuance transaction.
- `clients.rs:101–109` subsequently locked the verified, enabled account, but did not recheck that the original session was still current, normal, unrevoked, unexpired and recent under that lock.
- `clients.rs:123–128` inserted and committed a new API client using the previously resolved principal. The SQL did not depend on the session's continued eligibility.
- `auth/flows.rs:989–1011` password/recovery rotation serialized on that same account lock, updated the password, invalidated unused recovery links and revoked all other sessions and all then-existing API clients.
- `migrations/0011_api_clients.sql:10–12` gives an issued client a default 90-day expiry. `auth/principal.rs:192–195` resolves that grant independently of the original browser session; its granted authority is read-only owner library, not full browser or publisher authority.

Concrete interleaving:

1. A verified account has a recent normal cookie. A copied-cookie `POST /api/client` resolves that still-valid principal.
2. A normal password change or recovery completion wins/holds the account row lock. The client-creation request reaches the account lock and waits.
3. Rotation revokes the old cookie and all currently existing API clients, then commits.
4. The waiting client request obtains the account lock, sees the account is still verified/enabled and inserts a new, active client after the revocation sweep. Its returned bearer remains usable for private owner-library reads despite the old cookie being revoked.

This is specifically reusable authority being created after the credential revocation boundary, not a claim that every already-admitted ordinary mutation must be retroactively canceled. It requires possession of a recent normal cookie and an overlapping issuance request; no recovery-purpose cookie can directly call the endpoint. Short lock deadlines bound the overlap but do not remove an interleaving in which rotation completes within that deadline.

Impact: a leaked pre-change browser capability can be converted into longer-lived private-library access after the user completes password rotation/recovery.

### Owner patch independently reread

Closure snapshot at approximately 05:50 UTC: `clients.rs` SHA-256 `61952652ea07769fb54e5f1cdecbbaca298dc0a5f711c8234a5d5055e22ad57e`.

- `clients.rs:101` now calls `revalidate_credential_authority` inside the creation transaction before counting or inserting clients; `:204` applies the same contract to secret rotation.
- `clients.rs:215–237` requires browser purpose, locks the current verified/enabled account, refreshes roles and permissions from that locked account and rechecks the required permission.
- `clients.rs:231–236` then locks and verifies the exact current session ID/user binding, purpose=`normal`, unrevoked state, expiry and recent authentication. A principal resolved before password rotation cannot pass after it wins the account lock.
- The account/session locks remain held through `clients.rs:120` or `:209` COMMIT. If issuance wins first, later password rotation revokes the resulting grant; if password rotation wins first, stale issuance fails. The inspected paths use consistent account-before-session ordering.

**Source disposition: closed for the reported interleaving.** No runtime race test was run here.

Required genuine acceptance: actual Router/PostgreSQL concurrency must pause client creation after principal resolution while normal password rotation or recovery completion holds the account lock. After rotation commits, stale creation must fail and leave no active newly issued client. Include the opposite ordering, where creation commits first and is then revoked by rotation, plus valid post-change creation. Inspect the actual grant's library use and session revocation, not only an HTTP status. Also exercise secret rotation with a stale cookie to preserve the shared contract. Test evidence must not record raw client secrets or recovery links.

## Independently reread prior source closures

### R5-SEC-01: list visibility and content share an authorized statement snapshot

`library/lists.rs:65–74` resolves only eligible personal identity and delegates directly to `repository::authorized_list_json`. `library/repository.rs:77,90–96` selects the complete DTO and applies `(visibility <> 'private' OR user_id = reader)` in the same SQL statement. There is no earlier public eligibility decision followed by an unqualified newer DTO. Owner library access and public/unlisted direct access remain deliberate. Public feeds also apply their visibility predicate in the DTO-producing statement (`lists.rs:183–186`); separate totals can differ under concurrency without exposing an unauthorized private DTO. Real PostgreSQL concurrent privacy-transition acceptance remains unrun.

### R5-SEC-02: deliberately restricted browser reset route now exists

`auth/preferences.rs:18–29` returns a purpose/capability-only recovery body with empty roles/permissions before querying any account projection. It contains no identity, follows, preferences or groups. `auth/flows.rs:319–345` exposes only CSRF/password nodes for recovery settings, and `:390–393,495–503,632–640,960–963` retain recovery purpose, flow/session binding and password-only mutation boundaries. `auth/mail.rs:238–265` redirects the actual redeemed recovery cookie to `/auth/recovery`.

The recovery route overlay at `platform-integrations/frontend-overlays.mjs:1425–1431` mounts `RecoveryBoundary`. The later Session overlays at `:1434–1463` accept only a normal identity-shaped response, clear reset-only200 responses instead of hydrating them as a user, and reject a stale initialization response after a newer store write. `frontend-behavior/RecoveryBoundary.svelte:33–49` checks the reset-only capability, admits only restricted settings nodes, and retains component generation/cancellation state; `:51–79` submits the password once, clears submitted password bytes and confirms a normal Session after success. No private projection was restored to make the route work. The independent round-6 UI reviewer additionally inspected actual composed Session source and confirmed the reset-only response is not stored as a normal Session.

This closes the identified source admission/mount blocker. It does **not** establish genuine browser/mail recovery completion, repeated/expired/error behavior, rendered accessibility or a passing build. Those remain assigned to the independent UI/compiler/runtime owners. Before completion, whoami still calls `require_browser_session` (`flows.rs:1197–1205`), required ordinary principals reject recovery (`principal.rs:275–287`), private library/reading-status projections require library capability and optional social vote projections exclude recovery. The actual mail Router fixture now asserts the restricted body's absence of personal fields and password-only settings nodes (`backend/tests/auth_http.rs:1325–1337,1381–1405`); it was read, not run.

### R5-SEC-03: old recovery capabilities serialize with credential rotation

`auth/mail.rs:133–146` locks the account before token issuance. Redemption at `:193–206` discovers the target without first locking its token, locks the account and then rechecks/locks current token eligibility. `auth/flows.rs:989–1011` holds that same account lock while invalidating every unused recovery token and revoking older sessions/clients in the password transaction. Both ordinary password settings and actual recovery completion use this branch. The asynchronous delivery-state update at `mail.rs:156` does not clear `used_at`, so late delivery cannot reactivate an invalidated issuance. The account-first ordering closes the previously described old-link and overlapping issuance/redemption counterexamples. Genuine Router/mail/PostgreSQL ordering acceptance remains unrun.

### R3-PROTO-01: unsupported projection-backend alternative is retired

`config.rs:454–458` rejects non-null `publication.backend` for all modes. `protocol/publication.rs:106–110` independently rejects it at direct runtime entry before schema, signer, store or worker opening. The old platform backend field/worker and raw projection spawn are absent from `PublicationRuntime` and the startup worker path. `main.rs:61–73` opens the configured supervised native listener and only the platform importer, not an alternate projection backend. Production still requires the original native listener/backend (`config.rs:459–522`). The original standalone behavior is not modified by this retirement. Configuration rejection/direct-open tests and genuine supervised startup/shutdown acceptance remain unrun.

### Earlier storage/privacy/transport fixes retained

The pass reread source cover variants using shared `storage::store_encoded`, key advisory ownership, `protect_commit` before SQL COMMIT and intent clearing after acknowledged persistence (`protocol/covers.rs:170–210`). Shared storage continues durable prewrite key intent, verified live OS lease, file/directory sync and retained uncertain-outcome journals (`uploads/storage.rs:130–185,193–261`). Temporary/proxy files retain named OS leases; aged temporary cleanup does not delete a held lease. Unused-media removal respects current FK references and original publication pins (`uploads/sessions.rs:1100–1169`). No new concrete current-reference deletion or unjournaled final-installation counterexample was established.

Current path preflight rejects ParentDir before future-path ancestry resolution and checks private roots disjoint from the scanned immutable public UI tree (`config.rs:119–260,551–613`). Proxy identity still starts from actual ConnectInfo and validates only explicitly trusted exact hop chains (`client_peer.rs:48–115`). The nginx example overwrites XFF at the edge and omits query strings/credentials from access logs. These are source closures, not actual filesystem-fault, alias/proxy or installed-service qualifications.

## Other inspected boundaries and limits

The finite pass reread strict issued-bearer scope/role narrowing, required browser/account operations, expected-account binding, public/private list feeds, public user DTOs and source-hidden group filtering, report ownership/moderator guards and claim-to-authority transitions, private social viewer projections, cover/draft/staged-media guards, source-visible image/preview eligibility before conditional delivery, streamed intake/media/decode limits, DNS-pinned inline-image proxy redirects/address policy, trusted bootstrap CSP, same-origin/Host/no-store boundaries, ordered/checksummed explicit migrations, request/listener drain ownership and the supplied service/proxy/run entrypoints. No additional substantiated authority escalation, private projection, filesystem path, SSRF destination, source-visibility or operational counterexample was established in those inspected paths.

This review does not recertify every original-module lifecycle implementation. The separately assigned original protocol/process reviewer owns those semantics. As a provenance check only, all 23 files listed in the frozen `protocol-round2-lifecycle-delta.json` matched their recorded `after_sha256`; manifest SHA-256 is `cacc69e88bbc4fcd6e3faad25b9ad38d3d518078654e8ad93e230e7240772b3a`. A matching source manifest is not terminal process, compiler or regression evidence.

Still unrun here: current all-target compilation/codegen, actual Router/PostgreSQL concurrency and fresh/upgrade migration, real mail delivery and mounted browser reset, private-library/browser account-switch journeys, original signed publication/relay/process drain, real proxy/TLS/WS/body/range behavior, restart and durable-volume fault acceptance, supported mobile/PWA behavior and production/load qualification. Existing reports, authored assertions and finite negative source searches do not establish those gates.

## Snapshot provenance

At approximately 05:47 UTC, except the separately identified final client closure snapshot:

- `auth/flows.rs`: `c37227be7e2fe7619987f6af0c67081daf06f2a9bb3ec81d385566cb5b681f23`
- `auth/mail.rs`: `dff688e3c8b89cf96f77fc429b39d9b5aceb94790ff9503c9540362c0400921f`
- `auth/principal.rs`: `e2ece78caf36d498e31510cfe6143b609eb66b8d1aea3705e0b3d263712c164e`
- `auth/preferences.rs`: `ce19addacf4c564653d914df47931b733fb1235452d38f41c8abe21e4e19cf41`
- `library/lists.rs`: `a5378027f1b4dc17c6deaae9c4f76ec0c34dcf2f5000e3645b769b7f6fbd5655`
- `library/repository.rs`: `bb7d7f7573ef40a76abb6634307ce1f0b084cdc1e6cc25cee2a802aaf0307499`
- `config.rs`: `395c261408e93d69a62fc29ef392e67bb54847f3b1abb3b84fba4c86c5c9e231`
- `main.rs`: `89072b2e12ce290a31590f48d36508d486cb6fd1d7a6b8052ab75b05c741b401`
- `protocol/publication.rs`: `7c9e517233be37bd50763df76cff14d0444ebd5ab88f47fb210198fc60896552`
- `frontend-overlays.mjs`: `35233ac867ad3dc82d042cbf3ad2bdd7a057b6c72ecbb59610c0307b0b3c6076`
- `frontend-behavior/RecoveryBoundary.svelte`: `0af3bb50299dec5b9d23d8992312df1184daed378b29cb63434e4fb768a93573`
- `frontend-behavior/RetainedRecovery.svelte`: `20ba760776b71854e74c17aef228abd234dd5bded650c4fe863a1a94e1115fcf`

Later source changes require another review; no report wording asserts that these hashes cover future edits.
