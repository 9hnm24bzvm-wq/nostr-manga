# Hosted browser acceptance run 38615879103

The full `all` selection finished with **failure** on 2026-10-14. The native
PostgreSQL regression passed. Browser full-suite and pixel-parity acceptance
remain unproved; pending phases are not passing results.

## Source and official evidence

- [GitHub Actions run, attempt 1](https://github.com/9hnm24bzvm-wq/nostr-manga/actions/runs/38615879103)
- [Published source commit e94029800c1ffde6f1d7e90d36d121ef54c12651](https://github.com/9hnm24bzvm-wq/nostr-manga/commit/e94029800c1ffde6f1d7e90d36d121ef54c12651)
- Reviewed source tree: `83141fce1dba67dbd529cc1da53b065bba672133`, identical to local `2d9c80135328796856c789050fcd81b0093f4661`.
- [Official artifact 11921810806 metadata](https://api.github.com/repos/9hnm24bzvm-wq/nostr-manga/actions/artifacts/11921810806)
- Artifact name: `hosted-browser-evidence-38615879103-1`.
- GitHub artifact expiration: **2026-10-15T18:59:15Z**, as reported by official metadata. These JSON copies preserve the allowlisted evidence beyond that retention period.
- Official ZIP: 8,667 bytes; SHA-256 `ac3d15d39de5350e5fdc3910b474899e6d1df20652077e6f58fce49db59acf4a`.

`preparation.json`, `summary.json`, and `phases.json` are byte-identical copies
from the official artifact. `verification.json` records the independent ZIP
size/digest check, exact three-file allowlist, closed-schema validation, exact
summary recomputation, 1,058-input frozen-source binding, 39-phase/11-batch/51-run
planned inventory and original 676 frontend files plus three additions.
No raw logs, screenshots, signed download URLs, credentials, keys or runtime
databases are included.

## Native compact-feed result

The genuine disposable-PostgreSQL HTTP regression ran before browser batches and
passed with observed exit zero, owned terminal status, no deadline expiry and
verified process/fixture cleanup. The fixed `compact-feed505-plus10000-metadata`
scenario observed the frozen baseline query succeed in 3,450 ms and the batched
first page in 1,163 ms. These are measurements of one hosted fixture run, not a
general performance claim. The regression verified all 505 IDs were reached,
bilingual 506 preservation, canonical sample parity, visibility withdrawals and
unchanged statement timeout. The successful baseline also enabled comparison
of its complete first-page JSON against the batched result. No database budget
increase was used. Full PR28 runtime integration and restricted-role execute
grant attestation remain separate gates.

## Browser result

- All 39 unique phases were selected across 11 fresh-fixture batches, with 51
  planned executions including repeated startup and prerequisite phases.
- 36 executions ran: 26 passed and 10 failed. Across unique phases, 15 passed,
  10 failed and 14 remained pending behind earlier failed batch prerequisites.
- All 11 startup executions passed. Local-library, reader-transition, both
  compact-feed executions, feed-authority and terminal-feed passed. Account
  recovery and all recovery-lifetime checks remain passed.
- Every page-error count is zero. All 36 runners finalized reports normally,
  with no runner fatal classification; all proxies closed with zero remaining
  sockets. Six expected denied-CONNECT resets were handled.
- Every fixture final teardown was verified clean, keys removed, ports closed,
  with zero remaining owned processes. No batch reached the intentional
  PostgreSQL outage. `all_fixture_lifecycles_clean` remains false because it
  also requires successful browser-child exits.
- `full_suite_passed`, `selected_scope_passed` and `pixel_parity_established`
  remain false. No interruption or budget exhaustion occurred.

## Failure boundaries

Original step indices below are zero-based. Finite checkpoints locate observed
boundaries; they do not by themselves establish whether a product, fixture,
selector, response or ownership defect caused the failure.

- `upload`: steps 3–8 failed. Eleven of 29 finite checks passed, six failed and
  twelve remain pending. Corrupt image/archive feedback, no persisted corrupt
  pages and failed-batch feedback passed. Failed-page controls and stage-metadata
  failed; replacement, commit, revert and page-edit prerequisites then failed.
- `editorial`: steps 1–3 failed. This phase has no finer finite diagnostics.
- `signed-library`: steps 4–8 failed. Shared setup/security and auth-preview-send
  passed; auth-exact-job-retained and admission-exact-retry failed. Counts are
  24 passed, two failed and six pending.
- `signed-social`: steps 4–8 failed. All 23 shared setup/security checks passed;
  social-conversation-loaded failed and eight downstream checks remain pending.
- `signed-source-controls`: steps 3 and 4 failed. Source account admission and
  identity selection passed; source-conversation-loaded and restore-prerequisite
  failed. Counts are 25 passed, two failed and eleven pending.
- `signed-recipients`: steps 3 and 5 failed. Two-snapshot review/publication passed,
  but acceptance failed. Peers-revoked passed; peers-revoked-reads failed. Counts
  are 29 passed, two failed and three pending.
- `ui14-entity-pagination`: steps 0–6 failed, without finer finite diagnostics.
- Ordinary `reader`: only step 1 failed, without finer finite diagnostics. This
  phase is distinct from the passing `ui10-reader-transition` phase; later
  reader geometry and feedback phases remained pending.
- `ui14-account-failures`: step 6 failed. Seven title-pressure checks through
  pressure-captured passed. Capacity-ready and the five remaining checks stayed
  pending; no finite checkpoint reported failed.
- `ui14-actor-follow`: steps 1 and 8 failed. Only Group/User initial-readiness and
  status-pending checkpoints failed, after prerequisites and initial-session
  passed. Their two status-settled checks remained pending. All read-retry,
  unblock-loading/error and pending ack/lost/disposed controls passed. Counts
  are 65 passed, four failed and two pending.

The official closed diagnostics identify these boundaries without exporting raw
runtime messages. Original source assertions are unchanged by this evidence-only
checkpoint. No duplicate workflow run was started to preserve this evidence.
