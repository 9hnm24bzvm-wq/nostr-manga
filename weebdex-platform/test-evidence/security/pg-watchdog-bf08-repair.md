# BF-08 pre-stop PG terminal repair freeze

Prepared 2026-10-13T12:04:27.946209+00:00. **Owner repair complete; fresh independent source review and parent runtime clearance remain pending.** This is a finite source repair and non-executing validation report, not an independent verdict or runtime acceptance.

## Narrow change

The source-established cancellation ordering in `pg-watchdog-terminal-review.md` can skip the ready-loop PG poll. Watchdog cleanup now classifies its single first replacement-PG poll at lines63–73. A PG already terminal there sets `exited_before_stop:true`; cleanup retains that flag on both its empty-owned and remaining-owned return paths (78/83) while continuing the existing descendant reap/TERM/KILL stages. Finally at 203–204 retains `error:"RuntimeError"` for that observation without replacing an earlier error. The actual PG return code, force flag, survivors, closed-port/status facts and receipt schema remain unchanged. The existing watchdog exit predicate at 207 therefore returns1 even for actual PG exit0/unforced/no descendants.

The supervisor is byte-identical. Its existing BF-07 predicates require explicit `error:null` and the registered watchdog's actual exit0 in addition to the actual PG exit0/unforced/empty-owned terminal facts. The new failure consequently cannot take the existing clean fixture path. `verified_stopped` still describes the stopped resources when true; it does not erase the retained error or failed watchdog exit.

No stronger claim of perfect signal-time ordering is made. The fixed boundary is the known already-terminal observation at the pre-stop cleanup poll.

## Preserved paths and mechanisms

- A live replacement at the pre-stop poll takes the AST-identical existing identity-rechecked SIGINT and wait(15) branch. Descendant forced/survivor evidence is retained.
- A disarmed watchdog with no replacement sets no pre-stop exit flag; the original-preserved disarm behavior is unchanged.
- Existing startup/ready-loop failures keep their earlier error; parent-death and original-sibling cleanup are unchanged.
- The supervisor's locked Popen birth/registration/orphan reaping, exact session/ancestry/pidfd ownership, cumulative forced history, immutable frontend snapshot, phase budgets and final nonzero lifecycle gate are unchanged. BF-01–BF-07 mechanisms were not edited.
- No production source, Rust provisioner/helper, runner/journey source, frontend artifact or current binary changed. The optional fault index and Build19 startup plan remain runtime-held. Historical combined/four-control indexes retain their explicitly historical scope.

## Non-executing checks

Both Python files parse through Python AST without importing or executing fixture code. Exact AST comparison against the pre-change watchdog establishes only the expected cleanup pre-stop flag/returned tuple and finally error guard changed. The live replacement stop/join branch and all other watchdog module statements remain AST-identical. The supervisor is byte-identical. All 752 existing backend source/migration/example and frontend source/static files match their captured pre-change hashes. Existing Build19 plan command argv and startup queue payload are unchanged. All current optional-fault source pins and current Build19 plan source pins match actual bytes. Current backend/provisioner binary hashes match the existing plan/build identities.

Only local source/document reads, hashes, standard-library AST/data checks and the listed file writes were performed. No fixture, PostgreSQL, provisioner, application, original backend, Node, Chromium, Cargo, test binary, service or network action was launched; fast mode was not enabled. No runtime database, credential/private key, private control payload, authentication body, recovery link or licensed artwork was read or packaged by this repair.

## Frozen review boundary

- `tests/runtime/browser_fixture.py`: `2f5caccde5292d226c2616a9566cb711741bfb6f1589100950700d63419bf434`
- `tests/runtime/browser_pg_fault_watchdog.py`: `8ba6cc309eef8242da35b7d843c29e4349081c3f7eab1a9073b3358c93bbedef`
- `backend/examples/browser_fixture_provisioner.rs`: `59a3fcbbf65a096c82f61ca0b1d51c790b49543c9130026effba2eb4b2ae7845`
- `backend/examples/browser_fixture/ui_regressions.rs`: `98b2a1be4c85fcb5152ba0ad395b4e92fb5d09c0680e2333a11428f2551ee944`
- `docs/browser-fixture-contract.md`: `4fa6aed771dad1e6a082dc71ece5c96556203c5b6ed302d897f12943dc27f3de`
- `test-evidence/security/pg-home-fault-fixture-source.json`: `55525f2999e4814b8ae6a3ccb819ea05c8a7903dc20db7ea042c2f836f1d6057`
- `test-evidence/frontend/browser-startup-plan-build19.json`: `eb06edc33636383117f4bfa3801e0069267abf081d0b883faf06a9a75974ec41`
- `test-evidence/final/backend-current-build.json`: `6c7b406860ba73233cab51274ba5850462694fbbf643e8ae2424068fb1c130e7`
- `test-evidence/frontend/artifact-manifest.json`: `56dab08d19e57db750a5bbfcb4acb90a17605995bc29aa02f638ea2cc14af429`
- `artifacts/frontend/index.html`: `4a7f1cc552277aba231db044ca0d2616d70c4f3fe02ebc8d4974b4a5589d004f`

Current candidate019 normal binary: `e12349fe5f861b742b8915d3aa6265c5cd2d7fd232d4b12568492f04d46bbf33`.
Current candidate019 provisioner binary: `2871b09f202cfafe8ddd8d6de63837fdec7e7507b0750f1d774a5f0f1fe9af9b`.
Both are existing compiler outputs, not rebuilt or executed here. Build19 complete frontend manifest/index remain the pins above; startup plan keeps `execution_allowed:false` and zero application cases executed.

Changed files: the watchdog, `docs/browser-fixture-contract.md`, the optional PG source index, the Build19 startup plan and this report. The prior independent report remains unchanged. The optional index preserves the prior BF-07 source revision, adds the bounded BF-08 implementation fact, and explicitly marks fresh review pending. The current startup plan now pins the watchdog/contract/optional index in addition to the unchanged supervisor/runner/provisioner/frontend identities.

## Remaining acceptance

Obtain a fresh finite independent review of this exact change before parent launch clearance. In the later authorized owned runtime, establish the precise BF-08 ordering with genuine replacement PG0 followed by cancellation before the ready-loop poll, and require retained watchdog error/exit1 plus fixture nonclean/nonzero evidence. Also retain normal teardown, automatic restart/no-resume, relevant cancellation/parent-death and genuine home503/recovered Retry200 evidence requirements. No such runtime result was produced by this repair.

Exclude runtime databases, credentials/private keys, personal data, private controls/authentication payloads, recovery links, licensed artwork/derivatives and artwork-bearing screenshots from checkpoint uploads.
