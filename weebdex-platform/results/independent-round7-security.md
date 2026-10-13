# Independent completeness review, round 7: security, recovery and operations

Review window: 2026-10-13 05:54–06:04 UTC. This is a fresh finite source-only review of the round-6 credential-issuance fix and the current surrounding authority, privacy, recovery and operational boundaries. The review resumed after a reported capacity interruption; no verdict was based on an interrupted, unfinished pass. No Cargo/Vite command, application execution, service launch, database operation, browser execution or implementation edit was performed by this reviewer. This report is the only file written by this reviewer.

## Finite verdict

**No substantiated new security/recovery/privacy/operations finding was established in this completed finite pass. The round-6 client-issuance race is source-closed at the independently inspected current hash.** Earlier reported security fixes remain source-closed on rereading. This is a finite negative source result, not perfect security, an exhaustive proof, a passing build, executed acceptance or a production-readiness guarantee.

The review considered supported current behavior. It does not recommend speculative hardening, newly supported account/security features, style changes or unsupported state transitions. Other owners edited concurrently; the exact snapshots below bound the conclusion. The shared compiler and separate UI/protocol owners retain their own gates. Later substantive source patches require the user's requested fresh-squad review.

## Independent round-6 fix closure

Current `backend/src/domains/auth/clients.rs` SHA-256 was verified at the beginning and near the end of this pass:

`61952652ea07769fb54e5f1cdecbbaca298dc0a5f711c8234a5d5055e22ad57e`.

- Client creation calls `revalidate_credential_authority` inside the issuance transaction before its client count and insertion (`clients.rs:101–120`). Secret rotation calls the same helper before its credential update and COMMIT (`:202–209`).
- The helper requires a browser session, locks the current verified/enabled account, refreshes its current roles and permissions, and rechecks the actual required permission (`:215–230`).
- It then locks and validates the exact session ID/user binding with purpose=`normal`, no revocation, future expiry and authentication within the preceding ten minutes (`:231–236`). Those account/session locks remain held through COMMIT.
- Ordinary password change and actual recovery completion hold the same account lock before checking the current session and atomically updating the password, consuming unused recovery links and revoking other sessions and existing clients (`flows.rs:989–1011`).

The concrete ordering is closed in both directions: if rotation commits first, old-cookie issuance/secret rotation cannot pass the current-session predicate after acquiring the account lock; if issuance commits first, the later password rotation revokes the resulting grant. Refreshing the current permissions also prevents a pre-lock cached permission from independently authorizing new credentials. This conclusion is limited to the inspected source; no concurrency test ran.

Required genuine acceptance remains the Router/PostgreSQL interleaving described in round 6: hold password/recovery rotation after principal resolution and before issuance takes the account lock, then ensure stale creation and stale secret rotation fail without a surviving usable grant. Include the inverse order and a valid new-session issuance. Inspect private-library use and persisted session/client state, not only response status. Never record raw secrets or recovery links in evidence.

## Earlier security fixes independently retained

1. **Private list visibility and contents share the authorized statement.** `library/lists.rs:65–74` uses an eligible personal reader and `repository.rs:90–96` produces the DTO with its private/owner predicate in the same SQL statement. Public feeds put visibility in the DTO-producing query. The earlier split public-eligibility/newer-private-contents counterexample was not reintroduced.
2. **Recovery cookies remain reset-only.** `auth/preferences.rs:18–29` returns only the recovery capability and empty roles/permissions before querying the ordinary account projection. `flows.rs` exposes only CSRF/password settings nodes for recovery, retains flow/session binding and password-only mutation, and ordinary required principals reject recovery. Mail redemption redirects to the mounted `/auth/recovery` boundary. The current boundary source checks restricted nodes, submits once, clears the password field, and requires a fresh normal Session confirmation after success. No account identity or private library projection was restored to make recovery usable. The current Session overlays reject reset-only bodies and fence older initialization results after newer store writes. Actual mail/browser completion remains unrun here.
3. **Outstanding recovery capabilities serialize with password rotation.** Issuance locks the account before token insertion (`mail.rs:133–146`); redemption discovers the account without first taking a token lock, then locks that account and rechecks current token eligibility (`:193–206`). Password rotation consumes outstanding unused recovery links in its credential transaction. A late delivery-state update does not clear `used_at`. The old-link/overlapping issuance/redemption counterexamples remain source-closed.
4. **The unsupported parallel projection-backend route remains retired.** `config.rs:454–458` rejects non-null `publication.backend` in all modes. `protocol/publication.rs:106–110` rejects it independently at direct runtime entry. Platform startup opens only the configured supervised native listener and importer; full production requires the original native listener/backend. Original standalone semantics remain separately reviewed.

## Broader current boundary review

The finite pass reread the following actual source paths and did not establish another concrete in-scope counterexample:

- Normal cookie and issued-bearer resolution from persisted primary state; expiry/revocation; explicit owner-library bearer scope and staff/permission narrowing; expected-account constraints; ordinary browser/account/client operations; current-password hash recheck under account lock before login session issuance.
- Library follows/history/progress/lists/preferences and public user DTOs; private-owner selection; published/source-visible related resources; public source-hidden group filtering; CSV export title escaping; profile mutation fields that cannot overwrite account authority.
- Private moderation reports, owner/reviewer access, explicit moderator permissions, versioned terminal transitions and group claim-to-leadership eligibility. Public community thread/reply projections use current source visibility, and private viewer vote fields exclude recovery/API clients. Announcement DTO queries include publication/draft admission in the content-producing statement.
- Browser-only private upload stages and previews; draft/cover authority; versioned staging and immutable prepared-command inputs; source-visible media/preview selection before conditional response; bounded bytes, dimensions, frames and decode slots; read/request permits retained by streamed bodies.
- Inline-image proxy destination/port/credential checks, DNS-pinned approved public addresses, per-redirect validation, no ambient proxy or credential inheritance, limited response bytes and decoded image admission.
- Durable key-only prewrite intents, live named OS leases, file/parent-directory sync, shared native/derived/source-cover storage seam, protection before uncertain SQL COMMIT, conservative journal recovery and bounded orphan cleanup. Object advisory locks, current FK references and publication pins remain in the reviewed seams.
- Exact trusted immediate/intermediate proxy chains grounded in ConnectInfo; same-origin/Host/source checks; current private/API/auth no-store and restrictive response policy; bounded migration history/checksum validation; request drain and supervised-listener ownership; supplied service/proxy/run entrypoints and their explicit operator prerequisites.

These are inspected coverage areas, not a claim that every operation or environmental fault was executed or exhaustively enumerated. The original module's full lifecycle implementation remains the separate protocol reviewer's responsibility. As a provenance-only check, the frozen original lifecycle manifest still has SHA-256 `cacc69e88bbc4fcd6e3faad25b9ad38d3d518078654e8ad93e230e7240772b3a`; this reviewer does not upgrade its recorded source delta into process or regression evidence.

## Checked candidate that is not a new finding

The byte-preserved `frontend/src/lib/components/reader/reader.svelte:34–42` visibly interpolates JSON into a raw `svelte:head` script. Reading that attachment alone suggests a literal closing-script injection through editable titles. The actual integration already neutralizes this sink:

- `frontend-overlays.mjs:544–549` has a hash-anchored reader patch that scopes the template's `JSON.stringify` name to a serializer replacing every literal `<` with the JSON escape `\u003c`, plus U+2028/U+2029 escaping.
- Its recorded original reader hash matches the actual preserved source, `0b5ea1b1d3cb2ffbf818117e8f72d46218cd60b9b72e1539b864fb082e7ccf1d`.
- `scripts/vite.platform.config.mjs:37–38` places the shared overlay before Svelte compilation; its transform validates the source hash and exact anchor before returning transformed code. The frontend owner separately confirmed this current composition route.

Thus the proposed title-based closing-script counterexample is not supported by the actual integrated source. This closure is based on the serializer/transform source path, not on assuming that CSP would sanitize arbitrary HTML. No current bundle or browser execution was performed. A genuine rendered malicious-title regression remains useful acceptance, but no new fix is justified from this candidate.

Likewise, a possible cover eligibility/content race was checked against the actual title lifecycle: the implemented transition route only admits draft/rejected→submitted and submitted→published/rejected, and does not provide published→private draft. The later cover projection rechecks deletion and original-source eligibility. No supported public-to-private cover counterexample was established.

## Verification limits

Still unrun by this reviewer: current all-target compilation/codegen; Router/PostgreSQL auth and privacy concurrency; fresh/upgrade migrations; actual mail delivery and mounted browser reset; private-library account-switch journeys; actual browser malicious-title rendering; real HTTP proxy/TLS/WebSocket/body/range behavior; original publication/relay/process drain; restart and durable-volume faults; mobile/PWA rendering; production/load and backup/restore qualification.

Authored assertions, old bundles, source manifests, prior reports and finite negative searches are not substitutes for those gates. The latest recovery/UI sources changed during this pass and were reread where relevant to security; this report does not assert rendered UI completeness or successful build of them.

## Snapshot provenance

Near the end of the pass, 06:01–06:02 UTC:

- `auth/clients.rs`: `61952652ea07769fb54e5f1cdecbbaca298dc0a5f711c8234a5d5055e22ad57e`
- `auth/flows.rs`: `c37227be7e2fe7619987f6af0c67081daf06f2a9bb3ec81d385566cb5b681f23`
- `auth/mail.rs`: `dff688e3c8b89cf96f77fc429b39d9b5aceb94790ff9503c9540362c0400921f`
- `auth/principal.rs`: `e2ece78caf36d498e31510cfe6143b609eb66b8d1aea3705e0b3d263712c164e`
- `auth/preferences.rs`: `ce19addacf4c564653d914df47931b733fb1235452d38f41c8abe21e4e19cf41`
- `library/lists.rs`: `a5378027f1b4dc17c6deaae9c4f76ec0c34dcf2f5000e3645b769b7f6fbd5655`
- `library/repository.rs`: `bb7d7f7573ef40a76abb6634307ce1f0b084cdc1e6cc25cee2a802aaf0307499`
- `config.rs`: `395c261408e93d69a62fc29ef392e67bb54847f3b1abb3b84fba4c86c5c9e231`
- `router.rs`: `544a69ff1466f7a6231b062c6631a17f35b71a48ef5409aa8bea45489634087b`
- `main.rs`: `89072b2e12ce290a31590f48d36508d486cb6fd1d7a6b8052ab75b05c741b401`
- `runtime.rs`: `b54345fbbbd8e87194e16fa56ded99d6d5f0c510b9b3833957a9408b01af9982`
- `privacy.rs`: `58ad9202b33a151bb04ae389b73b21e25b4a6f25c5901744c82d2a2570715561`
- `uploads/storage.rs`: `0310aa66e42a376ddf7a45c2484d5ab949aee5d0fabf642a5677df28136cd0c9`
- `uploads/sessions.rs`: `46c5be9a53c836405f56b598a78f523dcae1b792d3969d8a2dc26cce904e5405`
- `uploads/proxy.rs`: `4b74ecabf8dc961c705d4657ca9f147d9d98932cbffc7227e7d0c4f6ca4659cd`
- `uploads/media.rs`: `56aa7ba41275411a871b8ca60c7928adad5e27161b7933b33d0f39479f5bd015`
- `protocol/covers.rs`: `9e5490bcc567f6089a0bb091c61f013181279e7f7404fb120783483822f4ae16`
- `protocol/media.rs`: `8af6719b3ddd38a8ebf9883409f2c7040f036e5de1b363866e4664010d6d6f94`
- `protocol/publication.rs`: `7c9e517233be37bd50763df76cff14d0444ebd5ab88f47fb210198fc60896552`
- `frontend-overlays.mjs`: `a47eeac0a472d622b578dd00a420f18c65c57f39a0f76c8324d4a52a2d504cf7`
- `frontend-behavior/RecoveryBoundary.svelte`: `6777bf183bed550fdc800b53fe2bcc568400b60cd905bbe0f356ba4240aa99f3`

The conclusion applies to these inspected snapshots and paths, not future edits.
