# Independent fixed-cluster PG fault source review

Reviewed 2026-10-13 10:09–10:14 UTC. **Finite verdict: HOLD the optional PG fault boundary for BF-07 below.** One concrete terminal-provenance gap was established. The genuine dependency outage, restricted authority, independent recovery, and existing ownership machinery otherwise have no additional source-established blocker within this review's scope. This review neither authorizes runtime launch nor supplies runtime acceptance.

Only source/document/compiler-receipt reads, hashes, non-executing Python AST inspection, and this report were performed. No fixture, PostgreSQL, application, original backend, provisioner, Node, Chromium, Cargo, test binary, or network service was launched. No implementation source was changed. The reproduction below is a source-established failure sequence, not a claim that it has been executed.

## Exact reviewed boundary

- Supervisor `tests/runtime/browser_fixture.py`: `a05a0979804e17c15d06eb1528138d18a9b54a328a45c18097f6e821f748b697`
- Watchdog `tests/runtime/browser_pg_fault_watchdog.py`: `0624c7a9bdc4ea7853983a63fe1c31c0509f006703bf29e8d351989644f50b92`
- Provisioner `backend/examples/browser_fixture_provisioner.rs`: `59a3fcbbf65a096c82f61ca0b1d51c790b49543c9130026effba2eb4b2ae7845`
- Native helper `backend/examples/browser_fixture/ui_regressions.rs`: `98b2a1be4c85fcb5152ba0ad395b4e92fb5d09c0680e2333a11428f2551ee944`
- Contract `docs/browser-fixture-contract.md`: `ae04859326ed6ab9e467afcff4e856c10530dbd13411fbc8b3b14529adefb8a9`

All five hashes match `test-evidence/security/pg-home-fault-fixture-source.json`. Both Python files parse without application execution. The separate four-control source index explicitly retains its historical supervisor/contract scope; it is not a current optional-fault launch pin.

The existing candidate019 build index became available during review. Its candidate manifest hashes to `4e696538df1490f81246470a7db9ba17ffdeb8c7c42ab9fa57251ac75229ad92`. The current normal binary independently hashes to `e12349fe5f861b742b8915d3aa6265c5cd2d7fd232d4b12568492f04d46bbf33`; the provisioner independently hashes to `2871b09f202cfafe8ddd8d6de63837fdec7e7507b0750f1d774a5f0f1fe9af9b`. These match `test-evidence/final/backend-current-build.json` and its candidate019 copy. Its provisioner/helper source pins match the files above. Existing normal/example receipts independently match their indexed hashes `2a5645adeb9448ff9d270cdbdaae2d5739cc1685e60dd6ca5a54a0fa86885fa2` and `2a4bd1c0198bb709ac4f71f583d90b6fc9e9612e8183c6d7e1e574862243ee88`; both report passed, command exit0, reaped leader, no remaining owned members, no changed inputs, and no abort/cleanup error. This reviewer did not compile or execute those binaries.

## BF-07: a failed watchdog can be accepted as a clean PG lifecycle

Priority: P1, terminal evidence and zero-exit truthfulness. Owner: fixture supervisor, with the watchdog's terminal error retained.

The watchdog correctly distinguishes its own failure from the actual PG process's exit status. At watchdog line192, an unexpected PG exit raises `RuntimeError`, even when the PG process returned0. Lines195 and201 retain `error:"RuntimeError"`; lines200–201 can still emit `status:"verified_stopped"`, `terminal:true`, `forced:false`, actual PG `returncode:0`, and an empty remaining-process list when the unexpected exit was clean. Line202 returns1 because the watchdog encountered that error.

Supervisor cleanup does not retain that distinction in its success predicate:

- Supervisor lines1030–1038 join/record the owned `postgres` lifecycle slot, whose leader is now the watchdog. Its actual nonzero return code is present in `results['postgres']`, but an already-exited watchdog incurs no new forced operation.
- Lines1047–1048 check only terminal/status/actual-PG-returncode/forced fields from the watchdog receipt. They omit its `error` and the owned watchdog's exit status.
- Line1065 requires exit0 only for `platform` and `recipient`. Line1066 checks browser return codes and cumulative force, not the failed `postgres` slot. With clean platform/recipient/browser shutdown, closed ports, absent keys and no surviving owned process, all fixture success flags can therefore be true. Main line1108 can return0.

Concrete failure sequence for a later authorized runtime check:

1. Complete the real begin/resume controls so the watchdog is in its ready ownership loop and the actual replacement PG is healthy.
2. Request ordinary supervisor shutdown. Let cleanup's recovery observation at1000 finish successfully, then interrupt the actual identified replacement PG with a clean SIGINT while cleanup is closing the mail sink or draining the native services, before the supervisor signals the watchdog.
3. With no outstanding DB-dependent drain work, allow the platform/recipient to exit0. The watchdog observes PG exit0 in its ready loop, records the error/verified-stopped combination described above and exits1. All descendants and ports are genuinely stopped.
4. The supervisor records watchdog returncode1 but accepts the PG terminal receipt and can emit `final_teardown_clean:true`, `lifecycle_clean:true`, and exit0. This is a false clean-lifecycle result, not a surviving-process or mock-HTTP claim.

This sequence is bounded to the already-owned cluster. It needs no forged receipt, unrelated process selection, arbitrary command/path input, production-policy change, or hypothetical generic I/O failure. The ordinary monitor at976 catches an unexpected watchdog exit while monitor is running, but that does not close this sequence: cleanup has already replaced monitor execution.

Narrow closure: require the installed watchdog's genuine exit0 and an error-free watchdog terminal receipt before accepting a clean fault terminal result. Preserve actual PG returncode0, empty owned descendants, closed port, and nonforced teardown as separate required facts. A stopped-but-failed watchdog may remain accurately marked stopped, but it must make fixture clean-lifecycle/exit0 acceptance fail. Refresh the exact supervisor/index/contract identities and obtain fresh review of that narrow change before using the fault-enabled boundary. No new lifecycle framework is requested.

## Authority, genuine outage, and recovery checked

The optional CLI flag is separate from the four native UI controls and requires the combined provisioner (supervisor1089,1097). Only begin/resume are added to allowed actions (189–191). The private command remains bounded4096 bytes, regular/nonlinked, safe unique request ID and exact active-browser phase acknowledgment (787–793). PG actions additionally reject every field except request_id/action/browser_phase and require80 seconds in each remaining observation window (802–806). Begin requires a live `ui14-home-failures` runner, intact ready signed seed and no prior fault (804,720–723). SQL, PID, host, command, duration, credential, and path cannot be supplied by the control queue.

The watchdog independently checks the fixed private root/marker, reserved non-default port, original/supervisor PID birth/group/session identities and direct-parent relation, its own dedicated session, actual original executable and hash, exact postmaster launch argv/data/port, and unchanged bounded regular `postgresql.conf`/`pg_hba.conf` (watchdog114–148). It opens a parent pidfd, installs cancellation handlers, and publishes armed state before the supervisor stops the original PG. The restart argv is a fixed list matching supervisor337–338; it is not derived from a caller-supplied command. No HBA, authentication, network, or database policy is rewritten by this seam.

The supervisor SIGINT-stops and joins the original PG, checks genuine exit0, empty owned descendants and closed port before handing the registered lifecycle slot to the watchdog (733–741,708–718). It then observes the actual native `/api/chapter/updates?limit=1` response and requires503 with code unavailable (742–746). Production error conversion in `backend/src/error.rs` maps real pool/SQL failures to that response; the feed uses the real database and `/readyz` uses real health checks. No fabricated503 or success adapter is present in this seam.

The watchdog independently restarts after15 seconds or an earlier fixed resume marker; it rechecks binary/config hashes before using the same cluster/argv (170–185). The registered watchdog remains the owner of the actual restarted postmaster and inherits the sanitized supervisor environment. Resume observes watchdog readiness, actual `/readyz` and public read200, genuine original status/capabilities/resources, and unchanged namespace within its60-second observation boundary (supervisor763–780,237–256). `fully_ready` is false during the intentional outage and replaced by genuine status on normal recovery. Other private controls and the mail mutation surface are held until explicit recovery (801,449,732,779).

## Cancellation, parent death, and baseline ownership checked

The armed watchdog's cancellation/parent-death path is bounded. Before activation, a live parent's untouched original cluster is disarmed; exact-parent death stops only the identified original sibling/session (watchdog156–166,81–107). After original disappearance it avoids a live duplicate, restarts independently, and finally cleans the replacement. Parent-pidfd death also triggers original-session cleanup in finally (197–199). PG fast-stop is given15 seconds, followed by fixed5-second TERM/KILL stages; replacement startup/probes and final owned-descendant cleanup are bounded. The ready ownership loop has a4200-second outer limit (178–193). Source checks do not prove those runtime transitions passed.

Supervisor cleanup stops browsers first, requests recovery before native drain, serializes existing provisioner work, and grants the installed watchdog30 seconds after native drain (987–1038). Exceptions become recorded tracking errors; forced/unconfirmed cleanup stays nonzero. BF-07 concerns the omitted successful-exit/error predicates on a genuinely stopped watchdog, not the registered ownership or signal mechanism.

A non-executing AST comparison against the retained, hash-verified `70401b969911da6b44bec5f41531476b5d425fcb1c9b2a4354e46839d2930c59` supervisor found no removed Fixture method. Changes are limited to optional-control setup/dispatch, bounded recovery observations, the five fault-owner methods, browser-kind bookkeeping, and cleanup additions. The existing spawn/register/run_owned, adopted-session ancestry, pidfd signalling, locked orphan reaping, monotonic child-force ledger, immutable frontend snapshot, and signed phase-budget methods remain unchanged. BF-01–BF-05's mechanisms remain intact. BF-06's cumulative force and true Popen wait-status mechanisms also remain intact, but the new failed-watchdog acceptance path must be closed for the expanded lifecycle boundary.

## Finite remaining gates

No second concrete implementation blocker was established in this scope. Close BF-07 and review the fresh exact change; retain independent final UI/catalogue gates and exact current complete frontend manifest/index/bootstrap-CSP, backend, provisioner, runner and watchdog pins. Only then may the parent decide runtime clearance. Later evidence must show the genuine mounted home503 attempts, real recovered Retry200, automatic restart without resume, relevant cancellation/parent-death transitions, and an actual error-free exit0/nonforced/empty-descendant/closed-port watchdog terminal receipt. Source parsing, authored journeys and compiler receipts establish none of those runtime results.

Keep runtime databases, private control/authentication payloads, signer/server secrets, recovery links, licensed originals/derivatives and artwork-bearing screenshots out of checkpoint uploads. This report contains only source identities, existing non-sensitive compiler provenance, and review conclusions.
