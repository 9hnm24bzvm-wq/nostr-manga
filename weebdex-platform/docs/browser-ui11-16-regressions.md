# Independent UI11–16 browser regression journeys

Prepared 2026-10-13. **Source-only deliverable; real browser acceptance is unrun.**
Only `node --check scripts/browser-ui11-16-journeys.mjs` has been executed by this
author. No application, browser, server, database, build or fixture was launched.
Importing the journey module launches none of them. The parent's final source,
artifact, fixture and runtime gate must clear before execution. Syntax validity
does not establish working selectors, runtime behavior, timings or acceptance.

## Source and integration boundary

The authored assertions trace the actual mounted controls and native contracts
described in `round11-ui-source-audit.md` through `round16-ui-source-audit.md`.
Inspection used the actual composed source516 / Build17 review copy, frozen at
`2026-10-13T09:14:26Z`, with 541 composed source files and composed inventory SHA
`08807db37f25dd398631fb1a48bcabfa2481b1a6e5855947a51e77d8634adb1b`.
Its 62-file integration inventory SHA was
`4cc4d72c2e998cb1a89762645bf114466447d3b5491ae796e1ef9d352f8c2e0b`.
That review found two subsequent UI17 issues. These journeys do not sign off
those issues or replace the parent's fresh final-source review/build binding.

The new module exports the exact shared-runner interface:

```js
ui11To16JourneyPhases
runUi11To16Journeys({
  phase, page, context, origin, fixture, step, insist, open, report, redactions
})
```

The frontend owner separately wires these exports into `browser-acceptance.mjs`.
This author has not edited that runner, existing journey modules or product
source. Every ordinary supervised phase must have a deadline of at most 300
seconds. The module stops useful work at 270 seconds, leaving cleanup reserve;
individual polling and local requests have explicit finite bounds. A stalled or
failed assertion is a failure, not evidence of success.

## Ordered finite phases and authored evidence

Each phase has a fresh browser context and registers its own synthetic verified
account through real visible forms when needed. No shared UI10 account object,
product helper import or mocked successful feed is used.

| Phase | Actual UI/backend evidence authored |
| --- | --- |
| `ui11-device-navigation` | Create a real custom device list and use its actual link. Delay the native final IndexedDB snapshot acknowledgement or committed mode-transaction completion. Navigate away, Back/Forward, or dispose the reader, then release obsolete work. Assert that newer routes survive within the same document. |
| `ui11-device-progress` | Real backup/export/preview/replace UI removes a previous chapter prerequisite. Hold the genuine progress timer or exclusive device-write browser lock during device→account→device selection at unchanged pages 1 and 3. Assert no obsolete write, durable current page/completion after reload, and a stable revision rather than a save loop. |
| `ui12-compact-feed` | Real 505-chapter Spanish compact scenario stays trimmed while the visible footer continues beyond 200 rows. Assert the finite automatic burst (initial page plus four continuations), actual Load more resume, unique serialized page reads, all 505 title-specific chapter IDs, true exhaustion, resize and Back. |
| `ui13-terminal-feed` | Hold the actual second French page before forwarding. A named production deletion changes 101→100; the native second page is truly empty. Assert that already acknowledged rows remain and restore coherently through Back. Separately, a genuine initial zero-match Uzbek feed keeps its original empty state. |
| `ui13-feed-authority` | Delay real public feed and protocol-capabilities deliveries; observe actual pending scope and settle to verified or failed-delivery/offline. Assert a usable public terminal state. Hold a genuine private account feed, select the empty device library, and reject obsolete account rows. |
| `ui14-entity-pagination` | Actual group, author and user Uploads with 101 rows, both ascending and descending, receive one real last-page row and retain the ordinary page-1 link. Real named deletion shrinks the count to 100; the empty high group page retains its earlier-page recovery. |
| `ui14-account-failures` | Actual chapter read, card follow and title status→None controls receive genuine 403, 429 and 401. Assert owned feedback, one write per gesture, no replay and unchanged durable native follows/history. |
| `ui14-home-failures` | Real public Grid admission saturation gives final native 429 attempts and visible error/Retry recovery. A reviewed real fixed-cluster PG outage gives native 503, followed by preserved-cluster readiness/read 200 and current UI Retry. Cached account My Updates receives real cookie-withdrawn 401, and real restricted-recovery-cookie 403 with an explicit currently denied Retry. |
| `ui15-chapter-filter` | Real bilingual title chapters use English→Spanish→English with the existing visible Apply controls, then Back/Forward and Reset. Assert applied fields, baseline enablement and actual chapter links, within one SPA document. |
| `ui15-reader-feedback` | Genuine automatic account-history 200 persists once across rerenders. Real 403/429/401 automatic-history denials give feedback without replay. An actually committed 200 whose delivery is aborted reports current uncertainty; disposal suppresses obsolete feedback while honestly retaining the actual server commit. |
| `ui15-reader-geometry` | Real four-portrait and eight-mixed-page chapters, offsets 0/1 and LTR/RTL. Select every advertised spread and check raw-page title/URL, label and decoded real image count; check Single page 2 separately. Actual closed-left-panel tap rectangle works at widths 1200, 800 and 390 in both directions. |
| `ui15-16-empty-tabs` | Genuine acknowledged-empty Recommendations settles and repeats its current read after Back. An actual authorized last-cover DELETE 204 leaves the stale parent Covers tab visible; opening it receives real Cover.List 200 `[]` and settles empty without changing the parent lifetime. |
| `ui16-home-preferences` | Real saved account ratings refetch both home queries, exclude actual formerly admitted adult targets, honor Safe-off/all-off `__none__`, and widen after acknowledged empty. Real analytics POST 204 establishes actual ranking activity. Visible original-language and tag-blacklist controls change native queries/results; release an older acknowledged home response after current exclusion and preserve the same SPA through Back/Forward. |
| `ui16-search-cap` | Real 10,000 metadata-only matching titles, visible items-per-page sizes 28/42/56/70/84/98, and the ordinary last-page link. Assert native last-page count, including page 103's four rows at limit 98, plus Back/Forward agreement. |

Run pagination/compact checks before activating bilingual/readers. The exported
order does this. On a later rerun, reader activation adds one genuine Spanish
chapter, so global Spanish feed total can be 506; the compact title itself stays
505. Assertions derive the actual permitted total from phase/descriptor state
and prove all 505 title-specific IDs instead of pretending the additional row
does not exist.

## Genuine fixture dependencies

Use the disposable combined supervisor and the opt-in
`--ui-regression-controls` capability documented in
`browser-fixture-contract.md`. This extension uses persisted native titles,
production chapter publication/deletion writers, bounded search metadata and
actual verified original-page objects. It makes no signed-protocol publication
claim for native regression-only rows. It does not seed on normal startup.

The exact private queue actions, plus ordinary `request_id`, `action` and the
currently recorded owned `browser_phase`, are:

- `ui-regressions-seed` with `scenario: "pagination"`, 120-second deadline
- `ui-regressions-readers` with no scenario, 120-second deadline
- `ui-regressions-status` with no scenario, 30-second deadline
- `ui-regressions-withdraw` with `scenario: "terminal"` or `"entity"`, 30-second deadline

No arbitrary ID, path, URL, SQL or account credential is submitted to this
private control seam. Seed/readers require at least 140 seconds remaining in the
active phase and fixture; status/withdraw require 50. The module checks owned
0700 runtime root, exact command/result paths, recorded browser PID/phase,
reserved database ownership, atomic bounded 0600 commands, finite receipts and
unchanged frontend/service identity. Missing controls/data throw an explicit
`UNIMPLEMENTED FIXTURE DEPENDENCY`, with no skip-to-green fallback.

`fixture.ui_regressions` must explicitly identify the actual native-only fixture
and publish compact, terminal, entity, search and preference descriptors. The
preference descriptor requires a real tagged Spanish title, `tag_id` and
`tagged_title_id`; rankings require actual successful native analytics requests.
The separate readers action publishes actual English/Spanish chapter IDs and
four-portrait/eight-mixed-page descriptors. Native Chapter.Get dimensions must
match those descriptors before geometry assertions proceed.

The genuine 503 phase additionally requires `--pg-home-fault-controls`, its
`fixture.fixture_control.pg_home_fault.enabled` capability and exact named
`pg-home-fault-begin` / `pg-home-fault-resume` actions. Each has a 60-second
receipt deadline plus 20-second reserve. Begin is restricted to the owned
`ui14-home-failures` browser kind and proves the actual native
`/api/chapter/updates?limit=1` response 503/code `unavailable`, active outage and
independent 15-second restart watchdog. Resume proves actual readiness/read 200,
same cluster, executable and configuration. This is real stop/start, not an HBA
rewrite or a fabricated browser response. The watchdog remains the owned live
replacement PostgreSQL supervisor after resume: its terminal stop proof must
come from final `cleanup.json.pg_home_fault_terminal`, not a false claim that
resume already terminated it. Only one such fault is allowed per fixture.

Genuine private-read403 needs no new permission or fault extension. The test
uses the real visible recovery-mail request while logged out, authenticates a
normal browser session, then redeems the genuine one-time link in the same
browser's request context without following its 303 redirect. The server-issued
restricted cookie has no identity/follows/preferences/roles/permissions. The
retained SPA still owns its cached normal Session; its mounted My Updates then
receives native403. No token, cookie or permission is forged; no password reset
is performed. Native principal `require_library_read` rejects this real
restricted recovery capability. Ordinary readonly owner clients are used for
mutation403 only, because they genuinely have owner-library read scope.

## Native delay/failure seams and truthfulness

- Network holds either withhold forwarding, or delay delivery after a genuine
  backend response. They fulfill only the original actual response. Delivery
  aborts are explicitly browser-delivery failures, never invented server codes.
- Native IndexedDB gates delay the last readonly request callback or a real
  already-committed transaction completion. A committed mode choice is not
  claimed to have rolled back after navigation. Browser locks/timers hold the
  real platform operation, not an internal implementation function.
- Device DB inspection is readonly. Chapter prerequisite removal uses the
  actual user-visible backup workflow; no direct IDB mutation supplies success.
- Real mutation403 comes from the same owner's actual server-issued readonly
  API client. Real401 withdraws actual cookies while retaining cached UI state.
- Real429 owns at most 32 incomplete native JSON requests to the exact loopback
  fixture. An actual native probe must prove code `capacity_busy` before the UI
  action proceeds. The 9-second watchdog is armed before connection; all request
  and socket objects are closed in `finally`, and actual socket-close events are
  awaited. Recovery is a real native read. No success mutation completes on
  these anonymous incomplete uploads, and cleanup cannot be held indefinitely.
- Account-history lost-delivery tests explicitly prove a durable committed
  200 before aborting delivery. Cancellation or absent feedback is never
  presented as proof that the original server write did not happen.

## Isolation, finite scope and remaining runtime evidence

These are finite reproduction journeys for the reported UI11–16 defects, not
the complete combinatorial acceptance matrices in the audit documents. Currently
unimplemented breadth includes entity sets of exactly 200/201, other compact
viewports/group combinations, every authority transition and restart path,
all transport/503 failures on every mutation, nonempty Recommendation/cover
loading-error variants, eight-portrait/other offset spreads, open/right-panel or
Scroll tap combinations, preference hydration races, and search below/above
10,000 plus jump-prompt/library-uncapped matrices. Those are not silently counted
as executed or passed. No fake successful feed can substitute for missing rows.

All runtime rows, timing/selector correctness, actual decoded images, native
failure observations, admitted-response races, suite budget and final lifecycle
remain unexecuted. Final accepted frontend/fixture hashes must bind the future
browser receipts. A passed syntax check or a passed individual finite phase
cannot stand in for the entire audit matrix or pixel parity with an independently
runnable original-render baseline.

Terminal/entity withdrawal and last-cover deletion deliberately mutate the
disposable fixture. Run unrelated baseline/cover-dependent checks beforehand,
or use a fresh reviewed native seed/schema between suites. Repeating destructive
phases against already-shrunk data is not supported as fresh acceptance.
Readers activate only after pagination; do not combine both 120-second seed
operations in one ordinary phase.

Credentials, one-time links, CSRF values and API keys remain in memory and are
registered with harness redactions. Only reserved `example.invalid` synthetic
recipients and exact verified loopback origins are used. API-response collections
remain in memory; returned observations contain public IDs, counts/statuses and
booleans, not raw authentication responses. The module creates no screenshots,
uploads, production accounts, external publication or runtime listener.

The owned runtime may reuse licensed Ubunchu originals from its verified local
test cache or locally generated valid fixture pages; licensed originals,
derivatives and artwork-bearing screenshots must remain excluded from source
checkpoints and uploaded evidence. Exclude runtime databases, private queue/mail
receipts, cookies, credentials and signing keys. Final supervisor cleanup must
prove the owned browsers/listeners/database process descendants and watchdog
terminated cleanly; a successful UI assertion is not that cleanup proof.

## Allowed validation performed

```sh
node --check scripts/browser-ui11-16-journeys.mjs
```

Result: syntax check exit 0. **No runtime result is claimed.**
