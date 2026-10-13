# Independent PG watchdog terminal source review

Reviewed 2026-10-13 10:16–10:21 UTC. **Finite verdict: BF-07 is closed in the inspected supervisor; HOLD the optional PG-fault boundary for the single bounded cancellation-order finding BF-08 below.** No further concrete source blocker was established in this review. Existing final UI/catalogue/source gates and parent runtime clearance remain separate.

This is a finite source review, not runtime acceptance. Only source/document/compiler-receipt reads, hashes, non-executing Python AST parsing/comparison, and this report were performed. No fixture, PostgreSQL, platform, provisioner, original backend, Node, Chromium, Cargo, test binary, or network service was launched. No implementation source was edited. The sequence below is source-established; it is not reported as executed.

## Exact reviewed boundary

- Supervisor `tests/runtime/browser_fixture.py`: `2f5caccde5292d226c2616a9566cb711741bfb6f1589100950700d63419bf434`
- Watchdog `tests/runtime/browser_pg_fault_watchdog.py`: `0624c7a9bdc4ea7853983a63fe1c31c0509f006703bf29e8d351989644f50b92`
- Provisioner `backend/examples/browser_fixture_provisioner.rs`: `59a3fcbbf65a096c82f61ca0b1d51c790b49543c9130026effba2eb4b2ae7845`
- Native helper `backend/examples/browser_fixture/ui_regressions.rs`: `98b2a1be4c85fcb5152ba0ad395b4e92fb5d09c0680e2333a11428f2551ee944`
- Contract `docs/browser-fixture-contract.md`: `3f498c668e277b359991de8d4acdd9b63de53b2e4cc04cd6c12384b1c846b1bb`
- Optional-fault source index `test-evidence/security/pg-home-fault-fixture-source.json`: `1af2984c5e97ebcab071714265503b49cfd3bd37f83ee535e09535ad2da5ad88`

All indexed source hashes match the actual files. Both Python sources parse without importing or executing their application code. The index correctly keeps runtime execution false and its parent runtime gate closed.

The candidate019 build index independently hashes to the indexed `6c7b406860ba73233cab51274ba5850462694fbbf643e8ae2424068fb1c130e7`. Current normal/provisioner binaries independently hash to `e12349fe5f861b742b8915d3aa6265c5cd2d7fd232d4b12568492f04d46bbf33` and `2871b09f202cfafe8ddd8d6de63837fdec7e7507b0750f1d774a5f0f1fe9af9b`. Existing normal/example receipt bytes independently match their indexed `2a5645adeb9448ff9d270cdbdaae2d5739cc1685e60dd6ca5a54a0fa86885fa2` and `2a4bd1c0198bb709ac4f71f583d90b6fc9e9612e8183c6d7e1e574862243ee88`. These are existing compiler provenance, not executions by this reviewer or real migration85 evidence.

## BF-07 closure: a reported watchdog failure cannot become clean fixture success

Supervisor1047–1052 now separately reads the watchdog's terminal receipt and polls the genuine registered watchdog Popen. Acceptance requires all of:

- `terminal:true` and `status:"verified_stopped"`
- Actual replacement PG `returncode:0`
- `forced:false` and exactly empty `remaining_owned_processes`
- Explicit presence of `error` with `error:null`
- Actual registered watchdog `watchdog_returncode:0`

The supervisor adds that genuine Popen return code to the copied evidence. Missing, stale-nonterminal, failed, forced, survivor-bearing or nonzero evidence causes `pg_fault_terminal_unconfirmed` to be appended at1053. The error list is required empty at1068, so both clean flags are false and main1112 returns nonzero. A watchdog that observes unexpected PG0 at its192 and emits error/exit1 can no longer be accepted merely because the actual PG and descendants stopped. This closes the exact BF-07 sequence in `pg-fault-independent-review.md`.

## BF-08: cancellation can bypass an already-exited PG check

Priority: P2, bounded terminal truthfulness. Owner: the fixture-only PG watchdog. This does not claim surviving resources, forged evidence, manufactured browser success, or an arbitrary command/control seam.

The watchdog's ready ownership loop191–193 evaluates `not cancelled` and parent liveness before executing its PG poll. The cancellation handler150–152 only sets `cancelled=True`. At final cleanup65, it polls the PG before deciding whether to request SIGINT. An already-exited PG skips the entire PG-stop branch66–69; that known pre-stop exit does not set an error.

Concrete source-established ordering:

1. The genuine planned begin/resume succeeds and the watchdog is in its ready ownership loop. Cleanup's recovery observation has finished successfully; native services can drain with genuine exit0.
2. During the watchdog's `.1`-second sleep193, the exact replacement PG is interrupted cleanly and finishes with actual exit0 and no remaining descendants. This is the same bounded owned-cluster interruption used in the prior BF-07 assessment.
3. Before the watchdog reaches its next body poll192, the supervisor sends its ordinary cleanup SIGINT1032. The handler sets `cancelled=True`; the next loop condition191 exits without checking the PG.
4. Finally calls `cleanup(leader)`199. Its first `leader.poll()`65 observes genuine PG0 before any watchdog-issued PG stop, so it skips sending SIGINT. With an empty owned set, cleanup74 returns unforced/empty. Since no error was set, terminal201 is `verified_stopped`, actual PG0, `error:null`, unforced/empty; run202 returns0.
5. The supervisor's new predicates all pass. With otherwise clean services, closed ports, absent keys and completed browsers, it can emit both clean flags and return0.

Unlike BF-07, this path does not discard a reported error: the watchdog fails to classify an exit it actually observes before issuing the PG-stop request. The contract219–220 expressly separates expected watchdog-requested teardown from a PG exit observed before that stop. Step4 is therefore a concrete contradiction of that separation. No fabricated receipt or speculative I/O failure is required.

Narrow closure: when entering actual replacement-PG cleanup, treat a PG already terminal before the watchdog requests its stop as an unexpected exit, retaining that error and nonzero watchdog outcome. Preserve the existing actual PG code, force/descendant/port facts and disarmed-with-original-preserved path. This can be a small local pre-stop state check; no new lifecycle framework or claim of perfect signal-time ordering is requested. Refresh the exact watchdog/index/contract identities and obtain a fresh finite source check. A later authorized runtime check should cover this precise cancellation ordering.

## Other terminal paths checked

- **Normal expected teardown:** the supervisor signals only the registered watchdog leader and gives30 seconds after native drain1026–1034. A live replacement is SIGINT-stopped and joined for15 seconds by watchdog65–69; actual PG0, no later forced descendant cleanup, error-free receipt and watchdog0 are necessary for supervisor success. BF-08 concerns only the already-terminal pre-stop branch.
- **Unexpected PG exit normally observed:** startup180 or ready-loop192 raises, error195/201 remains explicit and watchdog202 exits1. The repaired supervisor rejects it even if PG itself returned0. Nonzero actual PG return status is independently rejected1052.
- **Fault/recovery errors:** private begin failures propagate through fixture_controls883–886 to main1108. Recovery exceptions during final cleanup append tracking errors1000–1001. Terminal reading/validation errors append tracking errors1053. None can produce clean lifecycle/exit0.
- **Cancellation before activation:** when the original is still alive and no start marker exists, watchdog158–165 disarms without stopping it; terminal status is `disarmed`, not an installed replacement-PG success receipt. An interrupted begin cannot complete its installed-owner handoff as a successful control. The supervisor retains the original lifecycle slot and drains it normally; any accepted control exception remains the main error.
- **Parent death before activation:** watchdog159–166 and197–198 stop only the identified original sibling/session and retain the error/forced/survivor facts. After activation, pidfd death bounds the outage/ready loop and final cleanup stops the owned replacement. A dead parent cannot emit a fresh clean main receipt; watchdog cleanup evidence is not browser or fixture acceptance.
- **Forced/unconfirmed cleanup:** watchdog69–79 and199–202 retain force/survivors/error; supervisor owns the registered root, historical descendants and global cleanup1039–1043. Receipt failures or an uncaught cleanup exception produce nonzero process termination, rather than a valid success summary.

## BF-01–BF-06 and private authority remain intact

A non-executing AST comparison to the retained, independently hash-verified supervisor `70401b969911da6b44bec5f41531476b5d425fcb1c9b2a4354e46839d2930c59` found no removed Fixture method. Changes are limited to optional-control setup/dispatch, bounded recovery observations, five fault-owner methods, browser-kind bookkeeping and cleanup. Existing `spawn`, `register_process`, `run_owned`, immutable snapshot verification, ownership/reaping/signalling, signed phase budgets, native drain and monotonic child receipts are AST-identical to that retained boundary.

The two Popen creation paths257–309 still lock birth through registration. Reaper572–586 holds the same lock, polls owning Popen instances and excludes exact registered leaders from orphan waitpid. Session/ancestry/adopted ownership539–570 and pidfd identity-rechecked signalling587–603 remain. Cumulative `forced`/`ever_forced` and lifecycle flags278–286 survive final receipt merges; main1112 still makes forced/unconfirmed/failed-browser lifecycle nonzero. BF-01–BF-06's established mechanisms are not reopened by this review.

The fault remains opt-in, one-shot and restricted to the active `ui14-home-failures` phase with80 seconds in both windows802–806. Exact private control fields/identity/phase gates787–806 reject caller-supplied PID, path, command, SQL, host or duration. Watchdog114–145 independently fences the private fixture marker/root, fixed original executable/hash, exact original/supervisor birth and parent/session identity, exact launch argv/data/port and unchanged bounded configs. Begin requires actual original PG0, empty descendants and a closed port733–741 before handoff, then observes a real native503. Resume checks genuine PG/native readiness/read and unchanged original mapping/namespace763–780. No authentication/HBA/network-policy rewrite, unrelated process sweep, successful-response adapter or production-source change was found in this seam.

## Finite remaining gates

Close only BF-08's known pre-stop terminal branch, then review the fresh exact change. No additional implementation finding is asserted. Keep the independent final UI/catalogue gates, exact complete frontend manifest/index/bootstrap-CSP and backend/provisioner/runner/watchdog identities, real migration and parent runtime clearance requirements. Later runtime acceptance must establish real home503/recovered Retry200, automatic restart without resume, relevant cancellation/parent-death paths and truthful terminal receipts. This report establishes none of those runtime results.

Exclude runtime databases, credentials/private keys, private control/authentication payloads, recovery links, licensed artwork/derivatives and artwork-bearing screenshots from checkpoint uploads. This report contains only source/compiler identities and review conclusions.
