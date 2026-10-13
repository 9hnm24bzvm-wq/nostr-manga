# Independent completeness review, round 8: reset-only and private-read boundaries

Review window: 2026-10-13 06:48–06:53 UTC. This is a fresh, finite, source-only pass over the current client-list browser guard, its touched genuine-mail recovery assertion, and the surrounding authentication/private optional-read projection paths. No Cargo command, compiler, application execution, service launch, database operation or browser execution was performed by this reviewer. No implementation or test source was edited. This report is the only file written by this reviewer.

## Finite verdict

**No substantiated remaining security finding was established in the scoped current-source pass. The client GET browser-capability guard is correct at the inspected hash, reset-only cookies do not acquire ordinary private/account projections through the reviewed call paths, and the existing credential-issuance/revocation fence remains source-closed.** One concrete touched-test method/route mismatch was reported immediately and independently verified source-corrected during this pass.

This is a finite negative source result, not an exhaustive security proof, passing build, executed Router/PostgreSQL acceptance, or production-readiness guarantee. The earlier broad historical claim that a recovery cookie leaked the private client DTO is not supported by this review: the unchanged `require_principal` helper already rejects recovery cookies. The owner also confirmed that the previous client-list implementation explicitly rejected API clients. The current change should therefore be described as a defensive refactor to the shared browser-capability guard, not a newly demonstrated production disclosure fix.

## Current client GET boundary

At `backend/src/domains/auth/clients.rs:50–66`, `GET /api/client` resolves `require_principal`, then calls `require_browser_session` before the private SQL query. The first helper rejects recovery cookies (`principal.rs:275–287`); the second rejects both API clients and recovery capabilities (`:67–75`). The successful browser path remains owner-bound by `WHERE user_id=$1`, returns active clients only, is bounded to ten rows, and invokes `dto(row, None)`, so this read cannot emit a secret key. Its response remains `no-store`.

The inspected client source SHA-256 is `3537f7634a74d83d4a9416ebdb0dd9eb76d6d5d0fe14e730772b5a968bda172d` and was checked at the beginning and near the end of the pass.

## Reset-only and optional-projection inventory

The review inventoried the actual consumers of `optional_principal`, `optional_personal_principal`, `require_principal` and `require_library_principal` throughout `backend/src`, then reread their relevant authority/projection branches. No changed call path required repeating the unchanged original protocol lifecycle review.

- **Persisted resolution and capability checks:** `principal.rs:175–229` obtains current unrevoked/unexpired sessions or clients from persisted state and preserves recovery purpose separately. Browser-only and library-read helpers reject recovery; ordinary required principals reject it before domain code. `has_perm`, `is_staff` and `is_moderator` cannot grant recovery authority. `has_role` is only used for banned/restricted rejection in a community path that already requires an ordinary browser session; it does not open a recovery grant.
- **Minimal recovery check:** `auth/preferences.rs:18–29` returns only recovery purpose, password-reset capability and empty roles/permissions before any ordinary library/session projection is queried. It emits no identity, email, username, preferences, follows or groups. The normal `/api/auth/check` branch subsequently requires a browser session, also denying scoped API clients.
- **Password-only settings:** `flows.rs:390–395`, `:495–507` and `:632–640` deny a recovery principal for non-settings browser flows and deny API clients. Settings flow reads/submissions retain browser-hash and exact session binding. `flow_body` suppresses profile/email/username nodes for recovery (`:319–345`), including error responses; `settings` rejects a non-password recovery method (`:960–963`). The accepted password mutation revalidates the live session under lock and rotates to a normal session only after the credential transaction (`:981–1011`, `:1079–1102`). The pre-reset cookie is never independently used to obtain a normal Session DTO.
- **Whoami and auxiliary auth GETs:** `/auth/sessions/whoami` requires the shared browser boundary before returning identity (`flows.rs:1197–1205`). Logout may revoke the current reset capability but returns only its browser-bound logout token/URL or redirect. The generic flow-error route loads the exact browser-bound flow and returns a flow ID and generic error, with no account projection. Email-token GET redemption requires its own current one-time delivered token and account check; recovery redemption mints purpose=`recovery` and redirects to `/auth/recovery` (`mail.rs:193–267`). A recovery cookie itself confers no verification-token authority.
- **Private library GETs:** follows, export, chapter feeds/updates, follow-status GETs, history, progress and own lists call `require_library_principal` before reading their owner data. The required helper rejects recovery through `require_principal` before performing the library-read check. Normal browser owner-library access and verified active bearer clients with the current explicit `self.library.read` scope remain allowed. API clients do not inherit staff, account, draft, upload or mutation grants; removing their scope prevents optional identity from being exposed (`principal.rs:198–224`).
- **Private list optional-owner projections:** list detail and user-list feed use `optional_personal_principal` (`library/lists.rs:71`, `:171`), which filters recovery using `can_read_personal_data`. Private detail eligibility and DTO contents remain in the same SQL statement (`repository.rs:90–96`). A recovery viewer can see an independently public/unlisted direct-access list according to its existing public rules, but cannot borrow its account identity to see a private list or private owner feed.
- **Public reply viewer votes:** reply detail/feed first filter through the personal boundary and then require a browser session for the viewer identity (`community/mod.rs:193–200`, `:211–217`). Recovery and library bearer readers receive public reply projections without the private `vote_state`. The separate private thread-vote GET requires an ordinary browser session (`:127–131`).
- **Personal reading-status filters:** ordinary/top manga feeds and random manga obtain `require_library_principal` whenever `reading_status` is present (`catalog/read.rs:104–109`, `:240–245`). The filter builder independently requires library-read authority before binding the owner (`catalog/query.rs:297–303`). Recovery cannot use these paths as an owner-library oracle; unfiltered published catalog reads remain public.
- **Other optional private branches:** announcement detail opens draft DTOs only through `is_staff` plus explicit announcement permission (`announcements/mod.rs:34–35`, `:93–96`). Private cover metadata/media require owner plus `manga.create`, or moderator plus `manga.cover.edit` (`uploads/sessions.rs:1181–1192`, `uploads/media.rs:79–91`); recovery fails those capability checks. Draft titles/covers, upload sessions/previews, private reports and community mutations use required ordinary principals with their browser/permission boundaries. Public profiles, statistics and previews do not select personal data based on the cookie.

## Credential-issuance/revocation fence retained

The client-list refactor does not touch creation or rotation transactions. Creation and secret rotation invoke `revalidate_credential_authority` before issuing/updating a credential and committing (`clients.rs:99–118`, `:202–207`). The helper requires a browser session, locks the current verified/enabled account, refreshes its current roles and permissions, and checks the actual issuance permission. It then locks/rechecks the exact session ID/user binding with purpose=`normal`, no revocation, future expiry and authentication in the preceding ten minutes (`:218–235`). Those locks remain held through issuance COMMIT.

Password change and recovery completion take the same account lock before the live-session predicate and atomically update the password, consume unused recovery links, revoke other sessions and revoke existing API clients (`flows.rs:989–1011`). The current submitting session is also revoked when replaced (`:1086–1090`). If rotation wins the account lock first, stale issuance cannot pass its later session check; if issuance commits first, subsequent rotation revokes that grant. No regression in this account-first fence was established. This remains a source conclusion; no interleaving test was run here.

## Touched acceptance-test defect found and source-corrected

The starting `backend/tests/auth_http.rs` SHA-256 was `7ab6cd3f124076a803a270ddf33e5a479874e76171b3f8ea5e992d057c0a1d3e`. Its private-GET loop at then-lines 1362–1364 included `GET /api/settings` and asserted HTTP 403. The only `/settings` registration is PUT (`library/mod.rs:29`); the local vendored Axum method-router source defaults an unsupported method to HTTP 405. The touched genuine-mail recovery test would consequently fail at that unsupported GET before checking a recovery principal. This was an acceptance-fixture defect, not a demonstrated private-data exposure.

Reproduction at that starting hash: run the genuine-mail test `actual_email_delivery_one_time_verification_and_restricted_recovery` under the project's isolated auth Router/PostgreSQL harness; after redeeming its real recovery link, the loop reaches `GET /api/settings`, receives unsupported-method 405, and fails the expected 403 assertion. This reproduction was established from router and dependency source, not executed.

The owner corrected the loop to the supported `GET /api/user/follows/manga`; this reviewer reread it at current `auth_http.rs:1362–1368`. The registered path invokes `follows::search`, whose first authority step is `require_library_principal` (`follows.rs:69–74`), so its expected recovery 403 is source-correct. The final touched-test hash is `f8f2644713905fe3b0601ab1490706cea7123c5fa49275cc4fd3013729eef558`.

Acceptance must genuinely execute the corrected mail test, including: normal-browser client-list success before reset; real recovery-cookie denial of client list, private follows and whoami; minimal recovery check without personal projections; anonymous public replies without viewer votes; denial of personal reading filters/private own lists and credential creation; password-only settings; successful reset producing a fresh normal session; revocation of the old browser and clients. The existing bearer matrix at `auth_http.rs:565–575`, `:782–798` separately asserts intended owner-library reads and denial of browser/private surfaces, including `/api/client`. Authored assertions are not a passing result.

## Verification limits and provenance

Still unrun here: current compilation/codegen; the corrected Router/PostgreSQL auth tests; actual mail delivery and mounted reset UI; bearer/private-library browser journeys; credential issuance/password-rotation concurrency; fresh/upgrade migrations; and deployment, load, restart or backup/restore qualification. This finite scope does not upgrade round-7 evidence or repeat the unchanged original protocol/process review.

Additional source snapshots checked near the end of this pass:

- `auth/flows.rs`: `c37227be7e2fe7619987f6af0c67081daf06f2a9bb3ec81d385566cb5b681f23`
- `auth/mail.rs`: `dff688e3c8b89cf96f77fc429b39d9b5aceb94790ff9503c9540362c0400921f`
- `auth/principal.rs`: `e2ece78caf36d498e31510cfe6143b609eb66b8d1aea3705e0b3d263712c164e`
- `auth/preferences.rs`: `ce19addacf4c564653d914df47931b733fb1235452d38f41c8abe21e4e19cf41`
- `library/mod.rs`: `c5b744e4f7c18bf605615f5ff90f50f93f0aba7516c88d71127a6c2fc0c973d6`
- `library/follows.rs`: `d644bf03b443431195f22156be691a1bd6919143c27e438542f30ae78c4f7447`
- `library/lists.rs`: `a5378027f1b4dc17c6deaae9c4f76ec0c34dcf2f5000e3645b769b7f6fbd5655`
- `library/repository.rs`: `bb7d7f7573ef40a76abb6634307ce1f0b084cdc1e6cc25cee2a802aaf0307499`
- `community/mod.rs`: `9c64d255f012a4f6a672e8aa653ea43237576e337ea5631a5ded0057cba9cfb2`
- `announcements/mod.rs`: `6fb358c5dc3972c4e43e7752b3c2ec1e3ccb5463d92faad7ac855ccb09f69bc9`
- `catalog/read.rs`: `330cd648a8a8001667ae40fc5a254beacca1cfd049f8e4b4cd8751f158e0d07b`
- `catalog/query.rs`: `4ac89db5fc44f3cacc92f5075a8be77bec1f1ddcfc0b42d2e8f2843302e220e9`
- `uploads/media.rs`: `56aa7ba41275411a871b8ca60c7928adad5e27161b7933b33d0f39479f5bd015`
- `uploads/sessions.rs`: `46c5be9a53c836405f56b598a78f523dcae1b792d3969d8a2dc26cce904e5405`
- `router.rs`: `544a69ff1466f7a6231b062c6631a17f35b71a48ef5409aa8bea45489634087b`

The conclusion applies to the inspected snapshots and supported paths, not future changes.
