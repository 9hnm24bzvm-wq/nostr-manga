# Final source-bound verification plan

Execution starts only after the parent confirms that bridge, deployment, and
frontend implementation are frozen. No scale test precedes feature acceptance.
A successful historical test is not evidence for a changed source snapshot.

## 1. Freeze and compile

- Record SHA-256 for the complete platform Rust, SQL, configuration/template,
  test, and final frontend artifacts; record the original protocol baseline and
  every individually authorized original-source change separately using the exact final lifecycle delta manifest. Do not expand this to a blanket original-source exemption; the original52 SQL migrations remain byte-identical.
- Preserve previous receipts, lockfiles, and guard snapshots. Use one compiler,
  the existing shared Cargo target, two jobs, debug information disabled, and the
  authorized 90-second owned-process/resource guard. Never treat a deadline as
  success or restart a live compiler by guessing its PID.
- Run `cargo check --all-targets`, then bounded `cargo test --no-run` windows.
  Build the platform and original test executables serially in that target.
- Tie every later executable receipt to this source manifest and binary hash.

## 2. End-to-end browser acceptance first

Verify the browser can actually reach the owned test instance before declaring
browser tests available or blocked. Serve only the final clean frontend build
with its configured API/auth/media origins and computed exact bootstrap CSP
hash. Use synthetic accounts and genuine local image fixtures.

Check startup/hydration without CSP/bootstrap errors; registration/login/mail
and settings; search/home filters and all-unchecked reset/history semantics;
author/group/profile/follows; title creation/edit/review/publish; upload session
controls and cover selection; reader first/last/previous/next, optimized fallback,
page progress/history; forum replies/votes/quotes/links/previews; reports and group
claims; announcement listing/detail/staff actions. Confirm external analytics
script/connect/beacon requests are blocked. Record screenshots and console/
network evidence, with no credentials, tokens, private keys or personal data.

The latest user order gates new runtime suites on independent feature/wiring review.
Once that review accepts the exact source, E2E runs first against the final served
UI and newly compiled binary. Fixture readiness/migration setup is necessary
startup, not a substitute for a browser pass. System suites below follow actual
end-to-end outcomes and fixes. No old build or old screenshots count as current.

## 3. Native platform and account HTTP/database seams

Use the existing owned disposable PostgreSQL runner. It creates only a reserved
`weebdex_platform_test` database under the platform test-runtime directory,
binds a fresh loopback port with Unix sockets disabled, and records stop/cleanup
on every exit. Original runtime databases and storage volumes are never used.

Run precompiled suites serially, using fresh fixture clusters or disjoint
synthetic fixture names:

1. Platform unit/public-body seams, including CSP hashes, request body budgets,
   ranges/ETags/HEAD, stream-drop admission release, and proxy destination rules.
2. `auth_http`: native registration/login/settings/session/API-client and full
   local HTTP mail-delivery/token-redeem/recovery/reset/replay/expiry contracts,
   actual issued library bearer report/reply/vote least-authority denials with
   persisted no-effect assertions, and expected-account switch constraints.
3. `catalog_http`: all 36 catalogue operations plus Cover.List metadata, versions,
   permissions, rollback, group ownership policy, Unicode DTOs, every search
   filter, full tag bootstrap, explicit empty ratings, safe defaults, natural
   aggregate/list navigation, and source-unavailable fail-closed reads.
4. `library_http`: all observed follows/history/user/preferences/library export
   operations and additive list/page-progress contracts. Include revoked nested
   group/source entities, empty pages/counts, CSV roundtrip and privacy checks.
5. `community_http`: all 25 thread/report/statistics/analytics operations, report
   eligibility after source revocation, accepted group-claim authority/conflicts,
   real counters and additive announcement CRUD/draft visibility.
6. A complete upload HTTP/PG scenario: begin session, upload genuine PNG/GIF
   multipart bytes, sort/delete/reupload, idempotent commit, detail/list/aggregate,
   actual original and optimized reader bytes, page-order/count, cover variants,
   claim/edit/delete, owner separation, CSRF and draft privacy. Storage/helper
   tests alone do not establish this workflow. The fifth `upload_http` test
   covers atomic replacement at 500 staged files, genuine >1MiB multipart wiring,
   failed/stale/foreign replacement preservation, identical retry stability,
   maintenance abandonment reclamation, active locks/durable command pins, and
   saturated media admission before database selection. A sixth source fixture
   verifies restart-journal/reference/lease and raw/proxy residue semantics;
   synthetic filesystem states are not an actual hard-stop or power-loss pass.

## 4. Original-module bridge and publication recovery

Use separate synthetic original protocol/schema/storage fixtures with freshly generated ephemeral
synthetic test signers only. Confirm publisher admission and explicit native
identity binding; never provision production credentials.

- Run original protocol regression suites after the authorized facade/listener
  changes, especially staging/owner/grants/publication/source visibility and
  shared-store ownership.
- Exercise actual signed native publication, original hosted visibility,
  durable aliases and command timestamps, duplicate retry after native rollback,
  accepted-pending response/retry, restart/import reconciliation, and no duplicate
  event or new alias after recovery.
- Verify pool/resource limits and shared-store listener readiness, startup
  failure propagation, worker supervision and graceful stop.
- Hide/delete/quarantine/revoke the source between reads. Fresh metadata,
  nested DTOs, feeds, library/history/export, discussions/reports/statistics,
  previews, originals/optimized media and conditional 304s must fail closed.
- Metadata overlays must retain signed identities/source payload/originals;
  page replacement of protocol-backed chapters must be rejected. Local
  tombstones must survive importer sweeps. No replacement relation is inferred.
- Cancel a real client during an admitted SQL write and a media body. Preserve
  completion/rollback evidence and owned permit cleanup, without abandoning a
  transaction or claiming timeout success.

## 5. Robustness, design review and final reconciliation

Map verified public-operation labels to the normalized 107-operation inventory;
keep additive endpoints separate. Report source, current compilation, API,
original regression and browser evidence independently. Investigate every
failure, retain failed receipts, rebuild changed binaries, and rerun affected
suites before using a pass as final evidence. Confirm all owned test processes
and temporary listeners are stopped. Only then summarize acceptance, remaining
operator prerequisites and production/scale limits.

## 6. Performance hill climbing last

Only after functional E2E/system acceptance, review cancellation, retry, privacy
and module boundaries against the requested codebase-design guidance. Preserve
public seams while refactoring and rerun affected current-source evidence. Then
measure real performance/resource budgets before choosing a bounded improvement;
no scale result substitutes for a feature or authorization pass.
