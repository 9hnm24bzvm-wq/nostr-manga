# Independent UI10 regression journeys

Prepared 2026-10-13 against the source-frozen 444-definition UI10 candidate.
The separate module is `scripts/browser-ui10-journeys.mjs`. It exports only
`ui10JourneyPhases` and `runUi10Journeys(h)` and has no top-level runtime startup.
It does not edit the supplied frontend, product overlays or older journey modules.

## Verification state and gate

**All 30 application journey steps are unrun.** No browser, application process,
listener, database, mail server or fixture was launched by this author. No passing
runtime result, visual parity or production-readiness claim is made.

Source-only validation: `node --check scripts/browser-ui10-journeys.mjs` passed.
That is a JavaScript syntax check, not execution or import of this module.
The parent owns execution approval, fresh independent source clearance, reviewed
binary/artifact hashes and the sole supervised fixture launch. All five phases
use the existing ordinary finite 300-second supervisor phase bound; a timeout
must remain a failed/incomplete result, never a substituted pass.

The source manifest and compiler receipt used to identify the accepted candidate
are `test-evidence/frontend/round10-owner-source-repairs/source-manifest.json`
and the neighboring `summary.json`. A later source/bundle change requires renewed
matching evidence. Owner build/compile evidence is separate from these tests.

## Harness and real fixture dependencies

The module consumes the existing harness object:
`{ phase, page, context, origin, fixture, step, insist, open, redactions }`.
It neither resolves another browser runtime nor creates another supervisor.
The integration owner wires its exports into `browser-acceptance.mjs` and the
sole fixture owner's phase allowlist before any approved execution.

Required for every phase:

- Reviewed installed Playwright 1.62.1 and system Chromium, as pinned by the
  existing runner; its exact loopback HTTP/WS allowlist and external-deny proxy
- Genuine native Rust platform, its real authentication/library/client handlers
  and real disposable PostgreSQL database, started and drained by the owned
  fixture; no successful API response is fabricated by this module
- The immutable artifact copied and verified from the current complete frontend
  manifest, with the accepted UI10 transformations actually mounted
- A fresh browser context per phase, no preseeded login cookie, no supplied user
  credentials, no prior device-progress completion
- The real fixture contract in `docs/browser-fixture-contract.md`; genuine
  instance resources needed to verify the device-library catalogue scope

Additional dependencies:

- Account phases: exact verified `mail_origin` or `control_origin`; actual UI
  signup and verification, `/fixture/provision` granting only the synthetic
  native account's fixture permissions, and genuine `/mail/latest` delivery
- Recovery: actual one-time recovery redemption, restricted settings flow,
  CSRF-bound password reset and server-issued session rotation
- Client: actual recent browser authentication; `client.create`, owner
  update/delete and owner-library read permissions; Chromium's real in-memory
  clipboard at the fixture origin, with no access to a user's local clipboard
- Preferences: actual account preference versions; fresh document tag cache;
  a second genuine browser page in the same fixture context for the competing
  UI write, not an API-fabricated competing success
- Reader: recorded `ids.manga`, `ids.chapter`, `ids.next_chapter` and
  `ids.alternative_language_chapter`. All three chapter identities must differ
  and each real detail must have at least three ordered actual image pages.
  The unrelated target is the distinct Japanese alternative, not the failed
  English previous target. The test does not claim a distinct-series fixture.

## Coverage matrix

Every row below is **unrun**. The phase strings are the exact exported names.

| Exact phase | UI10 boundary | Actual controls and observed persistence | Controlled timing/error | Steps |
| --- | --- | --- | --- | ---: |
| `ui10-api-client` | UI10-04 creation success contract | Actual Create; real201; exactly one POST and durable list row; real Copy Client Secret; clipboard equals one-time response key; owner-scope read200; reload omits copy/key; actual Delete confirmation204; durable list absence and revoked bearer401 | None; genuine backend success and revocation throughout | 3 |
| `ui10-auth-lifetimes` | UI10-01 login/registration and settings ownership | Real browser flow IDs and CSRF fields; Search link, Back and same-route reopen; registration/login writes remain acknowledged once; current login reaches `/settings/account`; initial settings check cannot reclaim Search | First actual flow-create and existing-flow-read response withheld for both forms; actual registration acknowledgement and subsequent Session refresh withheld separately; same for login; actual settings readiness withheld | 11 |
| `ui10-recovery-lifetimes` | UI10-01 anonymous recovery and reset-only ownership | Real request form reentry and CSRF; actual local mail remains delivered; redeemed capability exposes no identity/preferences/follows/roles/permissions; password-only current form; actual private-library403; password reset sent once, old login400/401, new login succeeds and link replay400 | Actual recovery create/read and mail-request acknowledgement withheld; actual restricted settings-flow creation withheld across Search/Back; actual successful reset's Session refresh withheld across Search | 6 |
| `ui10-preferences` | UI10-03 late tag readiness and dirty baseline | Actual rating checkbox and Save/Revert; unsaved edit remains dirty; server version/value unchanged before explicit Revert; real Save200 increments once and reload hydrates; competing actual second-tab Save200 followed by first-tab stale Save409; dirty draft and rival persisted version retained | Actual tag GET fetched then browser delivery delayed or intentionally failed; no successful tag/backend response invented; real preference version conflict | 5 |
| `ui10-reader-transition` | UI10-02 destination-owned last-page intent | Actual ArrowLeft previous-page keyboard control, single/LTR/auto-advance controls, error-page Home and real title/chapter links; same-JS-realm sentinel; unrelated URL/title/page-one plus readonly persisted IndexedDB page1/completed=false; Back/Forward preserves page1; ordinary successful previous navigation reaches exact intended last page and its real completed progress, then unrelated page1 stays incomplete | Real previous detail fetched200 then delivered to UI as controlled403 and404 in separate cases; real previous-detail200 delayed across newer title/chapter selection | 5 |

## Exact step names

The runner reports these names without any credential values.

### `ui10-api-client`

1. UI10 client prerequisite: actual registration and local verification
2. UI10 client: real Create201 commits once and exposes its one-time copy control
3. UI10 client: real clipboard copy, reload secrecy and UI revoke invalidate the credential

### `ui10-auth-lifetimes`

1. UI10 login: delayed real flow creation survives leave and Back reentry
2. UI10 login: delayed existing-flow read cannot replace a newer same-route flow
3. UI10 registration: delayed real flow creation survives leave and Back reentry
4. UI10 registration: delayed existing-flow read cannot replace a newer same-route flow
5. UI10 registration: delayed real success acknowledgement cannot redirect newer Search or replay signup
6. UI10 registration: delayed successful Session refresh cannot redirect newer Search
7. UI10 auth prerequisite: verify the genuinely registered account
8. UI10 login: delayed real success acknowledgement cannot redirect newer Search
9. UI10 login: delayed real Session refresh cannot redirect newer Search
10. UI10 settings: delayed initial Session readiness cannot reclaim a departed route
11. UI10 auth: current login still authenticates once and reaches its chosen return target

### `ui10-recovery-lifetimes`

1. UI10 recovery: delayed anonymous flow creation survives leave and Back reentry
2. UI10 recovery: delayed existing-flow read cannot replace a newer recovery flow
3. UI10 recovery prerequisite: actual verified account and real logout
4. UI10 recovery: delayed real mail-request response cannot overwrite a reopened request form
5. UI10 recovery: restricted settings-flow read is owned by the current reset-only form
6. UI10 recovery: delayed acknowledged reset Session refresh preserves newer navigation and actual password outcome

### `ui10-preferences`

1. UI10 preferences prerequisite: actual verified synthetic account
2. UI10 preferences: late real tag read preserves an edited rating draft
3. UI10 preferences: failed real tag read preserves an edited rating draft
4. UI10 preferences: genuine current Save persists the reviewed rating once and reload hydrates it
5. UI10 preferences: two actual tabs produce a real409 without silently rebasing the dirty draft

### `ui10-reader-transition`

1. UI10 reader prerequisite: real ordered chapters and anonymous device-only authority
2. UI10 reader: controlled previous-detail403 cannot make an unrelated chapter open last or completed
3. UI10 reader: controlled previous-detail404 cannot make an unrelated chapter open last or completed
4. UI10 reader: delayed genuine previous navigation cannot supersede unrelated page-one reentry
5. UI10 reader: normal automatic previous chapter opens only its intended last page

## Source-checked selectors and limits

Selectors use retained public roles, labels, input names, button titles, checkbox
IDs and real links, rather than product stores, internal callbacks or arbitrary
component positions. Source-confirmed details include `Revert` (the preference
reset control), `Left-to-right`, `Single`, `#auto_advance`, `#safe`,
`#suggestive`, `Client Name (required)`, and the actual `Copy Client Secret`,
`Regenerate Client Secret`, `Delete Client`, `Create`, `Delete`, `Sign in`,
`Create Account`, `Send Recovery Link` and `Reset Password` controls.

The ordinary title entry links to `/chapter/<id>`; the real reader installs its
one-based `/1` URL. Assertions verify the resulting exact `/chapter/<id>/1`,
Page1 document title and persisted page1. The module does not edit that link or
invoke internal navigation. Error traversal uses the root error page's actual
Home control. A same-document sentinel makes unexpected hard reloads fail,
because a reload would conceal the former module-global last-page defect.

The readonly IndexedDB observation is the persisted device-library public record
format, not a call to an internal product helper; it never changes database
records. Preference and credential durability are observed through actual
read-only native endpoints. All writes under test come from actual forms and
controls, aside from the explicit test-only fixture account provisioning seam.

The controlled chapter403/404 are browser-delivery errors after a real detail200,
not a claim that backend moderation made that chapter unavailable. Held-route
delivery reports `delivered` or `browser-cancelled`; neither means the already
sent server write was absent or reversed. Successful registration, login and
password-reset outcomes are reconciled against actual server reads/login.

Passwords, CSRF values, flow IDs, mail links and one-time client keys are held
only in test memory and added to the harness redaction set before risky calls.
The test returns only counts, status codes and boolean observations. It saves no
screenshots, traces, HARs, storage states, request/response bodies or credentials.
The actual headless clipboard is cleared in `finally` and its temporary fixture
permissions are removed. Authentication secrets are never included in evidence.

This finite suite does not claim CAPTCHA, credential aging, client-limit/rate
exhaustion, actual moderation/rebuild, account-switch preferences, language/tag
selector variants, offline/PWA or full pixel-parity coverage. Those separate
existing or proposed gates retain their own status and authority requirements.
