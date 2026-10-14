# Hosted browser acceptance run 38554048951

The full `all` selection finished with **failure** on 2026-10-14. This is an
observed browser result, not a full-suite or pixel-parity pass.

## Source and official evidence

- [GitHub Actions run, attempt 1](https://github.com/9hnm24bzvm-wq/nostr-manga/actions/runs/38554048951)
- [Published source commit 5743275621c3869e7edc4219e9e9a755636a01ca](https://github.com/9hnm24bzvm-wq/nostr-manga/commit/5743275621c3869e7edc4219e9e9a755636a01ca)
- Reviewed source tree: `0103a59fdbbab71750ddbda27b3ac9b58843ccbb`, identical to local `01a593b7d167add9a4c8e7936d4e86abd10fcf21`.
- [Official artifact 11890278354 metadata](https://api.github.com/repos/9hnm24bzvm-wq/nostr-manga/actions/artifacts/11890278354)
- Artifact name: `hosted-browser-evidence-38554048951-1`.
- GitHub artifact expiration: **2026-10-15T11:31:06Z**, as reported by the artifact metadata. The committed JSON copies preserve the allowlisted evidence beyond that retention period.
- Official ZIP: 6,730 bytes; SHA-256 `46208733ddf092c6877ad27789a6d1b7a3b35c4fe156a00ffcf88bcbe58666b3`.

`preparation.json`, `summary.json`, and `phases.json` are byte-identical copies
of the three files in the official artifact. `verification.json` records the
independent ZIP size/digest check, exact three-file allowlist, closed-schema
validation, exact recomputed summary, frozen-source snapshot binding and
preservation of the original 676 frontend files plus three reviewed additions.
No raw logs, screenshots, signed download URLs, credentials, keys or runtime
databases are included.

## Result

- All 38 unique phases were selected across ten fresh-fixture batches, with 49
  planned executions including repeated startup and prerequisite phases.
- 27 executions ran: 17 passed and 10 failed. Across unique phases, 8 passed,
  9 failed and 21 remained pending behind earlier failed batch prerequisites.
- All ten startup executions passed. All 27 runners finalized their reports,
  with no fatal runner error or page error. Every proxy closed with zero
  remaining sockets. Six expected denied-CONNECT resets were handled.
- `ui10-recovery-lifetimes` passed all 14 recovery checks. Recovery beyond the
  device-library prerequisite in `account-recovery` was not reached.
- Every fixture's final teardown was verified clean, with keys removed, ports
  closed and zero remaining owned processes. `all_fixture_lifecycles_clean`
  remains false because the supervisor also requires successful browser-child
  exits. The later intentional PostgreSQL outage phase was not reached.
- `full_suite_passed`, `selected_scope_passed` and `pixel_parity_established`
  remain false. There was no interruption or budget exhaustion.

## Failure boundaries

- `account-recovery`: device-library seeding failed; later recovery checks pending.
- `local-library`: device/account follow-trigger visibility/uniqueness failed;
  custom-list membership and signer-dismissal visibility also failed.
- `signed-library`, `signed-social`, `signed-source-controls`, `signed-recipients`:
  shared mapped-title follow-trigger visibility/uniqueness and signer-selection
  denial checks failed; later specific signed journeys were not established.
- `ui10-reader-transition`: the controlled previous-detail 403/404 cases,
  delayed previous-navigation case and normal automatic previous-navigation
  case failed after the real chapter/device-mode prerequisite passed.
- `ui12-compact-feed`: repeated in both feed and reader batches, real fixture
  requests returned the finite outcome `unavailable503`; compact geometry and
  later pagination assertions remained pending.
- `ui14-account-failures`: only step 6, title-unfollow under genuine 429, failed;
  the other eight denial gesture cases and registration prerequisite passed.

These finite results identify boundaries; they do not by themselves establish
whether each remaining failure is product behavior, fixture state, ownership or
selector behavior. Original assertions remain unchanged by this checkpoint.
