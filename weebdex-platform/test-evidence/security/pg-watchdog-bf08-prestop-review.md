# Independent BF-08 pre-stop watchdog review

Reviewed 2026-10-13 12:06–12:07 UTC. **PASS: BF-08 is closed at the frozen source boundary below.** No actionable source defect was found in this bounded fix. This is source acceptance only; it establishes no runtime or E2E result.

## Exact boundary and checks

- Watchdog `tests/runtime/browser_pg_fault_watchdog.py`: `8ba6cc309eef8242da35b7d843c29e4349081c3f7eab1a9073b3358c93bbedef`
- Unchanged supervisor `tests/runtime/browser_fixture.py`: `2f5caccde5292d226c2616a9566cb711741bfb6f1589100950700d63419bf434`
- Contract `docs/browser-fixture-contract.md`: `4fa6aed771dad1e6a082dc71ece5c96556203c5b6ed302d897f12943dc27f3de`
- Optional source index `test-evidence/security/pg-home-fault-fixture-source.json`: `55525f2999e4814b8ae6a3ccb819ea05c8a7903dc20db7ea042c2f836f1d6057`

All source hashes recorded in that index match their actual files. Both Python files parse with `ast.parse` without importing or executing their application code. Review used source/document reads, hashes, non-executing AST inspection and this report only. No runtime, PostgreSQL, browser, backend, Cargo or test executable was launched. No implementation source was edited.

## Concrete closure

The previously reported ordering remains possible: PG exits during the ready loop's sleep, then cancellation makes its next loop condition false before body poll196. It can no longer yield clean lifecycle evidence:

1. Watchdog cleanup63–68 unconditionally makes the initial replacement-PG poll when `leader` exists. An already-terminal PG sets `exited_before_stop=True`, including actual returncode0, before any watchdog stop request.
2. Both cleanup returns78/83 retain that exact flag while separately preserving force and remaining-process facts.
3. Finally203–204 receives the flag and sets `error='RuntimeError'` only if no earlier error exists. Terminal receipt206 retains this error and the genuine PG returncode. Watchdog207 returns1, delivered as its real process exit by209–213.
4. Supervisor1047–1053 requires both explicit `error:null` and the registered watchdog Popen's genuine returncode0. Either observed failure rejects the terminal provenance and appends `pg_fault_terminal_unconfirmed`. That tracking error makes `verified_stopped`, `final_teardown_clean` and `lifecycle_clean` false at1068–1070; main1112 returns1.

Thus the BF-08 already-terminal pre-stop case cannot be accepted as `lifecycle_clean`, even when cancellation bypasses the ready-loop poll and actual PG exit is0 with no descendants.

## Preserved expected paths

- Normal requested shutdown: a live PG at initial poll68 leaves the flag false. Cleanup69–73 requests SIGINT and joins the genuine PG; a subsequent genuine exit0, unforced empty owned set and closed port still permit error-free watchdog0 and the supervisor's existing clean acceptance. The flag is not recomputed after requested shutdown, so ordinary requested PG termination is not misclassified.
- Earlier failures: finally204 does not overwrite an existing error. Actual returncode, cumulative force and survivor evidence remain independent.
- Disarmed original preserved: run168–169 still raises `Disarmed` with `leader=None`. Cleanup then leaves the flag false; disarmed status and original-preserved evidence205–207 remain available without this guard stopping the original PG.

The contract221–223 and optional index accurately describe the pre-stop guard. BF-01–BF-07, UI19 and the unchanged broad app boundary were not reopened. Proceed to separately authorized, exact-pinned runtime/E2E checks; this report grants no runtime clearance.
