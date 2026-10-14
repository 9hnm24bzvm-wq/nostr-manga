# Hosted browser acceptance run 38584846100

The full `all` selection finished with **failure** on 2026-10-14. This is an
observed browser result; full-suite and pixel-parity acceptance remain unproved.

## Source and official evidence

- [GitHub Actions run, attempt 1](https://github.com/9hnm24bzvm-wq/nostr-manga/actions/runs/38584846100)
- [Published source commit e1ea103f2d8c0d8091eb63acf11f240c1e350591](https://github.com/9hnm24bzvm-wq/nostr-manga/commit/e1ea103f2d8c0d8091eb63acf11f240c1e350591)
- Reviewed source tree: `6950c9b7a819874d630831e61ea1907a0c8c40fc`, identical to local `94c00b8b0688b121499743ed213e465eec69bc37`.
- [Official artifact 11907995328 metadata](https://api.github.com/repos/9hnm24bzvm-wq/nostr-manga/actions/artifacts/11907995328)
- Artifact name: `hosted-browser-evidence-38584846100-1`.
- GitHub artifact expiration: **2026-10-15T15:27:57Z**, as reported by official metadata. These JSON copies preserve the allowlisted evidence beyond that retention period.
- Official ZIP: 7,464 bytes; SHA-256 `3d697a94b31ea7390019a290bd01c7c096d1c28dfe4ee24661f0ca25d65f53f8`.

`preparation.json`, `summary.json`, and `phases.json` are byte-identical copies
from the official artifact. `verification.json` records the independent ZIP
size/digest check, exact three-file allowlist, closed-schema validation, exact
summary recomputation, 1,038-input frozen-source binding, 39-phase/11-batch/51-run
planned inventory and original 676 frontend files plus three additions.
No raw logs, screenshots, signed download URLs, credentials, keys or runtime
databases are included.

## Result

- All 39 unique phases were selected across 11 fresh-fixture batches, with 51
  planned executions including repeated startup and prerequisite phases.
- 31 executions ran: 20 passed and 11 failed. Across unique phases, 10 passed,
  10 failed and 19 remained pending behind earlier failed batch prerequisites.
- All 11 startup executions passed. Account recovery and account failure-denial
  phases now pass. All 14 recovery-lifetime checks remain passed.
- Every page-error count is zero. Thirty runners finalized reports normally;
  the upload runner instead reports `unhandled-rejection` / `unknown` during
  `running-checks`. Its report was not finalized and proxy shutdown was not
  started, although its observed remaining socket count was zero. The other
  30 proxies closed normally. Three expected denied-CONNECT resets were handled.
- Every fixture final teardown was verified clean, keys removed, ports closed,
  with zero remaining owned processes. The ui-home batch reached its intentional
  PostgreSQL outage and verified terminal cleanup. `all_fixture_lifecycles_clean`
  remains false because successful browser-child exits are also required.
- `full_suite_passed`, `selected_scope_passed` and `pixel_parity_established`
  remain false. No interruption or budget exhaustion occurred.

## Failure boundaries

- `upload`: original steps 2–5 failed; step 6 unresolved; runner fatal noted above.
- `local-library`: 24 of 25 finite diagnostics pass, including device/account
  follow controls, custom-list/member persistence, backup and signer dismissal.
  Only `local-library-account-import-reviewed` fails (original step 3).
- Four signed phases pass all 23 shared setup/security diagnostics each, including
  mapped/native title follow, reader progress, exact native controls, signer
  refusal/connection, recipient fences and recipient selection. Later original
  steps fail: library 4–8, social 4–8, source-controls 3–4, recipients 3 and 5.
  Passing shared setup does not establish those later signed journey results.
- `ui10-reader-transition`: all seven checkpoints in each 403, 404 and delayed
  case pass. Normal open/page/image/settings/focus/navigation pass; only the
  normal destination checkpoint fails (original step 4).
- `ui12-compact-feed`, repeated in feed and reader batches: the real fixture
  responds `unavailable503`; finite database classification is `query-canceled`.
  Geometry and later pagination assertions remain pending. This identifies the
  database boundary, not the underlying query-plan/cancellation cause.
- `ui14-home-failures`: original steps 1 and 3 fail.
- `ui14-actor-follow`: account registration and Group/User unblock-loading/error
  pass. Initial readiness, read-retry and pending-ack/lost/disposed fail for both
  entity types (10 failed checks, 5 passed). These browser failures do not by
  themselves distinguish product, retained-control selector or ownership cause.

The official closed diagnostics identify failure boundaries without exporting
raw runtime messages. Original assertions are unchanged by this evidence-only
checkpoint; pending phases have not been counted as passing.
