# Hosted browser acceptance run 38626003249

The full `all` selection finished with **failure** on 2026-10-14. The genuine native
PostgreSQL regression passed. Browser full-suite and pixel-parity acceptance remain
unproved; pending phases are not passing results.

## Source and official evidence

- [GitHub Actions run, attempt 1](https://github.com/9hnm24bzvm-wq/nostr-manga/actions/runs/38626003249)
- [Published source commit 820537725404b5f6592b0072922bcecca5905c56](https://github.com/9hnm24bzvm-wq/nostr-manga/commit/820537725404b5f6592b0072922bcecca5905c56)
- Reviewed source tree: `56056bd473639d7aa342a390b123368811f67e2d`, identical to local `53c04b1ce5e0c394c3707a1ae2a441f57a5839da`.
- [Official artifact 11925093096 metadata](https://api.github.com/repos/9hnm24bzvm-wq/nostr-manga/actions/artifacts/11925093096)
- Artifact name: `hosted-browser-evidence-38626003249-1`.
- GitHub artifact expiration: **2026-10-15T20:12:58Z**, as reported by official metadata. These JSON copies preserve the allowlisted evidence beyond that retention period.
- Official ZIP: 9,423 bytes; SHA-256 `d5fd557eae1e30a7a6cd913aeca4728c709c521c0837c24dff029066701dcb6e`.

`preparation.json`, `summary.json`, and `phases.json` are byte-identical copies
from the official artifact. `verification.json` records the independent ZIP
size/digest check, exact three-file allowlist, closed-schema validation, exact
summary recomputation, 1,060-input frozen-source binding, 39-phase/11-batch/51-run
planned inventory and original 676 frontend files plus three additions.
No raw logs, screenshots, signed download URLs, credentials, keys or runtime
databases are included.

## Native compact-feed result and limits

The genuine disposable-PostgreSQL HTTP regression ran before browser batches and
passed with observed exit zero, owned terminal status, no deadline expiry and
verified process/fixture cleanup. In the fixed
`compact-feed505-plus10000-metadata` scenario, the frozen baseline query was
canceled at 5,001 ms; the batched first-page HTTP result completed in 2,378 ms.
There was **no complete baseline first-page JSON comparison** in this run because
the baseline did not return a result. The unconditional canonical per-page samples,
all 505 IDs, bilingual 506, visibility-withdrawal checks and unchanged statement
timeout proof passed. These are observations from one fixed fixture, not a general
performance claim. Full PR28 runtime/role-grant attestation remains separate.

Both browser compact-feed phases still returned unavailable503/query-canceled.
The native fixture success therefore does not establish browser-fixture feed
availability or identify the remaining cancellation cause.

## Browser result

- All 39 unique phases were selected across 11 fresh-fixture batches, with 51
  planned executions including repeated startup and prerequisite phases.
- 34 executions ran: 25 passed and nine failed. Across unique phases, 15 passed,
  eight failed and 16 remained pending behind earlier failed batch prerequisites.
- All 11 startup executions passed. Signed-library and signed-profile passed;
  local-library, account recovery, reader-transition, recovery lifetimes and the
  account-failure phase passed. Ordinary reader and entity pagination were pending
  behind the failed compact-feed prerequisite, so their new changes were untested.
- Every page-error count is zero. All 34 runners finalized reports normally with
  no runner fatal classification. All proxies closed with zero remaining sockets;
  seven expected denied-CONNECT resets were handled.
- Every fixture final teardown was verified clean, keys removed, ports closed,
  with zero remaining owned processes. The ui-home batch reached its intentional
  PostgreSQL outage and verified terminal cleanup. `all_fixture_lifecycles_clean`
  remains false because it also requires successful browser-child exits.
- `full_suite_passed`, `selected_scope_passed` and `pixel_parity_established`
  remain false. No interruption or budget exhaustion occurred.

## Exact failure boundaries

Original step indices below are zero-based. Finite diagnostics expose reached
boundaries, without proving whether a product, fixture, response, selector or
ownership defect caused the failure.

- `upload`: steps 6–8 failed. All 29 finite checks through commit-response passed,
  including failure controls/removal, numbered metadata, durable stage/reload,
  invalid replacement retention and lost-reply replacement durability.
  Commit-receipt failed; revert and page-edit prerequisites failed. The published
  reader and later controls remained pending (29 passed, three failed, three pending).
- `editorial`: steps 1–3 failed. Account-ready, author-ready and create-controls
  passed; author-control failed before artist/cover/create requests. Transition
  and publish prerequisites then failed (three passed, three failed, 17 pending).
- `signed-social`: steps 4–8 failed. All shared setup/security checks and the
  target-controls/mapping-response/root-response/feed-response checkpoints passed.
  Source-visible and original conversation-loaded failed; eight later checks
  remained pending (27 passed, two failed, eight pending).
- `signed-source-controls`: steps 3 and 4 failed. Account admission and identity
  selection plus the same response checkpoints passed. Source-visible,
  conversation-loaded and restore-prerequisite failed (29 passed, three failed,
  eleven pending). Passing response checkpoints does not establish source rendering.
- `signed-recipients`: steps 3 and 5 failed. Two-snapshot review and publication
  checkpoints passed; accepted and job-retained failed. Peers-published-prerequisite
  failed before revoke/read/restore actions (28 passed, three failed, seven pending).
- `ui12-compact-feed`: step 0 failed in both ui-feed and ui-reader batches. The
  fixture response arrived but HTTP200/fixture-total failed with unavailable503
  and finite database category query-canceled. Geometry and pagination were
  pending (three passed, two failed, 16 pending in each execution).
- `ui14-home-failures`: only step 3 failed. All PostgreSQL outage, repeated-denial,
  recovery-retry and recovery-read checks passed. Private403 native denial and
  widget feedback passed; single-read failed before retry and repeated-denial
  checkpoints (25 passed, one failed, two pending).
- `ui14-actor-follow`: steps 1 and 8 failed. Group/User pending Follow visibility,
  disabled state, Block-hidden and no-write proofs all passed. Only the original
  initial-readiness and status-settled checkpoints failed (75 passed, four failed,
  zero pending). The closed immediate observations showed a current route, one
  header, expected prerequisite actor, held native204, delivery still pending,
  disabled Follow, absent Block/Retry controls, many GETs and zero mutations for
  each entity kind. Read-retry, unblock and pending ack/lost/disposed controls all
  passed. These observations do not by themselves prove the settlement root cause.

This evidence-only checkpoint changes no original assertion and starts no duplicate
workflow run. New source fixes still require actual browser acceptance.
