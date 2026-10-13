# Independent fixture terminal-evidence review

Reviewed 2026-10-13 07:19–07:23 UTC. **Finite source verdict: BF-06 is closed in the exact inspected supervisor. No additional bounded, source-established terminal-provenance or direct-child wait-ownership finding was substantiated.** BF-01 through BF-05 remain intact at the inspected boundary. This is source clearance for these mechanisms, not permission to launch and not runtime acceptance.

No fixture, PostgreSQL, platform, original backend, provisioner, Node, Chromium, Cargo or tests were launched. Operations were source/document reads, hashes and writing this report. No implementation source was changed. The final UI build still needs its own fresh complete artifact/index/bootstrap-CSP identities and review before a combined run.

## Exact inspected boundary

- `tests/runtime/browser_fixture.py`: `70401b969911da6b44bec5f41531476b5d425fcb1c9b2a4354e46839d2930c59`
- `backend/examples/browser_fixture_provisioner.rs`: `1ee707435069e7c8925efb8247c6b99244d043935cc2b5470c4fb58c556e842a`
- `docs/browser-fixture-contract.md`: `39bcf7bcdd1c1b0b85d6262d99bca4c24d3c9d08b971ba5c4c9edae5e5d42071`
- `scripts/browser-acceptance.mjs`: `b4c1ed2dc34d32d5a964d56f662085e7ac7cd5efec85c798ce53884ac5b308e0`
- `scripts/browser-signed-journeys.mjs`: `af01c43b9753d8cc4d3764afef8adfd25d4762cd5bb8b978c40e8359bbbd19d5`
- `scripts/browser-journeys.mjs`: `65f5e5d115f35d15608e4075957172354978727cead70cfbeea6b64a0c0e5efc`
- `test-evidence/security/combined-browser-fixture-source.json`: `7a2a11a9c0f9cadb732e7bc6ff2fdc5d4019901d60dd9aa12d80267ecbd711eb`

The index's supervisor/provisioner/contract hashes match these independently read bytes. The provisioner and all three browser modules retain the hashes in the preceding `fixture-exit-ownership-review.md`; the changed supervisor and contract were read afresh. The index reports prior compilation, not compilation reproduced by this reviewer. Its build10 frontend entries do not approve a subsequent UI build. Its residual statement that signed control calls remain unauthored is stale documentation: the inspected frozen signed module contains those calls/assertions. This documentation inconsistency does not invent runtime evidence or introduce another implementation blocker.

## BF-06 closure: cumulative force is monotonic

`Fixture.__init__` initializes fixture-wide `lifecycle_ever_forced` at75. Registration at257–263 initializes a new child generation's `forced:false` and `ever_forced:false`; it does not reinitialize the fixture-wide flag.

The new `update_child_receipt()` at264–272 takes the ownership lock, ORs the previous `ever_forced`, previous `forced` and new `forced` at269, writes that accumulated fact to both child flags at270, and ORs it into fixture history at271. All terminal child updates use it: provisioner158, synchronous commands295, intentional listener drain653, browser phase754 and final cleanup850/867/886. No later direct assignment resetting a terminal child's force flag remains.

Source walk of the original BF-06 browser scenario:

1. A live browser expires at744–747. It is signalled/joined, descendant cleanup is completed and actual status plus `forced:true` is published at748–756. The shared update sets the fixture-wide flag.
2. A later ordinary stop reaches cleanup after the browser is already terminal. Local final-only `forced` remains false at843–849, but the shared update at850 preserves `forced:true` and `ever_forced:true`; it separately records `teardown_forced:false`.
3. Final shutdown can truthfully have `final_teardown_clean:true` at907 while `lifecycle_clean:false` at908. Main at945 then returns1. Neither the final child summary nor the exit status erases the earlier forced phase.

The same merge covers previously forced provisioner and synchronous commands. A synchronous timeout sets local `forced:true` at285 and raises at289; its `finally` receipt295 preserves that fact. A provisioner timeout ORs the residual-group result at156 before its receipt158. Normal synchronous completion with unresolved live descendants at290–291 also fails and retains force. Browser residual descendants are accumulated at752–754.

Both replacement paths archive the old child receipt before a new generation is registered: generic spawn249 and listener restart625. Their copied flags retain that generation's provenance, and fixture-wide history survives new registration. Global final cleanup separately ORs its forced outcome at889–890. Thus fresh per-generation fields do not clear the full-fixture history.

## Final-only shutdown, lifecycle and pass/failure remain distinct

Cleanup computes:

- `verified_stopped` at906 from every final child result, every declared closed port, absent synthetic server keys, no remaining owned process and no tracking error.
- `final_teardown_clean` at907 additionally requires no global/final-only forced work and exit0 for the final platform/recipient listeners.
- `lifecycle_clean` at908 additionally requires no earlier forced work, no captured fixture exception and exit0 for every recorded browser child.

These values, the cumulative fixture flag, remaining descendants, signal/reaping history and actual child results are included in the receipt at909–916. Cleanup writes `cleanup.json`, updates the supervisor status, prints the terminal receipt and **returns it** at917–920. Main's only normal return at945 requires `verified_stopped`, both clean flags and no pre-cleanup error. An ordinary stop followed by an unconfirmed survivor, signal-tracking error, open declared port or retained server key therefore cannot take exit0.

A naturally terminated browser with a genuine nonzero status is recorded at753–755, even though the monitor can continue for later phases. Final cleanup retains that actual Popen status and the browser predicate908 rejects a clean lifecycle; main returns1. A deadline-forced browser is rejected by cumulative history even if its script handles SIGTERM and returns0. An active browser interrupted by a supervisor stop is marked forced in final cleanup843–850. The final cleanup receipt is its terminal evidence if the monitor did not reach the per-phase publication path; a separate per-phase file must not be assumed to exist on that interrupted path.

The runner does not equate process cleanup with feature acceptance. `browser-acceptance.mjs:69–75` records each actual step as passed/failed. Its final153–156 requires a completed report, no harness error, at least one step and all steps passing; otherwise it sets exit1. The signed module's private control128–135 requires a matching successful actual result and, for restart, joined exit0/nonforced listeners with no remaining members. A false, missing or pending result is not accepted. Process lifecycle flags alone still do not prove that the requested journeys ran.

Nonzero readiness probes and rejected/malformed fixture requests are not automatically browser-acceptance failures; their callers interpret them. The changed contract164–169 correctly defines `lifecycle_clean` as no earlier force, fixture error or failed browser phase, rather than a universal assertion that every deliberately negative subprocess/request returned success.

## Return and exception paths

Argument errors at930–935 occur before a fixture or child is created and exit through argparse's error path. Constructor setup at61–91 creates no child process; constructor/setup-support failure is an unhandled nonzero failure, not a clean run. Signal handlers are installed before `start()`938–940.

After successful construction, startup, operational checks and monitoring share the `try` at940. Any `BaseException` is captured and reported as `FIXTURE_ERROR` at941; `finally` calls cleanup with that error at942. This includes failed readiness, SQL/migration failure, accepted-control failure and fixture deadline expiry. Accepted controls emit `success:false` and `cleanup_required:true` then re-raise at728–740. A normal stop or stop.request still passes through the same cleanup and the result-based exit gate.

`run_owned()` publishes a terminal child record and closes pipes in `finally`294–297. Provisioner errors occurring after its normal join/descendant cleanup already have a child receipt158–160; exceptional interruption before that publication is still covered by top-level final cleanup of registered children. No successful default return or fallback status was introduced.

Receipt production is not guaranteed if teardown itself throws, for example on an I/O error while removing a key or persisting a receipt: `cleanup()` is not wrapped in a second best-effort receipt framework. Such an exception escapes main's `finally` and produces a nonzero process failure; it does not take the successful return945. A missing/incomplete final receipt in such a run must remain unconfirmed evidence. This source review does not turn exceptional receipt absence into a verified shutdown or propose an unrelated generic framework.

## BF-05 remains closed: actual Popen status is retained

The only two `subprocess.Popen` sites remain254 and279. Both hold `process_ownership_lock` continuously through birth and `register_process()` at255/280. Registration captures the direct child, PID/start ticks/group/session and root ledger at259–263 before lock release. Every synchronous psql/initdb/pg_isready/migration uses `run_owned()`298–344. No bare subprocess.run/call/check invocation was found in the supervisor.

`reap_owned()`558–572 holds that same reentrant lock through selection of all historical Popen leaders563, owning-Popen polling564, owned-member scan565, exact leader exclusion566, current-identity recheck567–568 and orphan-only waitpid570. A concurrent sink request cannot insert a new direct waiter between the exclusion snapshot and orphan wait. Historical roots remain excluded too. Its only direct `os.waitpid` call targets a confirmed adopted zombie that is not one of those exact registered leaders.

The installed `/usr/lib/python3.13/subprocess.py:1978–2017` was read: Popen.poll uses its own waitpid lock and retains an actual collected exit status. Its ECHILD fallback remains an upstream behavior, but the fixture's orphan reaper no longer steals the registered direct child's status to trigger it. communicate283, provisioner wait151 and blocking listener drains648/882 are outside the fixture lock. The fixture's newly added receipt merge does not alter Popen returncode or introduce another wait collector.

## Preserved ownership, budgets and immutable inputs

- BF-01: Linux subreaper and pidfd support are required82–84. Root birth identity and inherited-session/ancestry/adopted ownership525–557 remain intact. Signals573–589 reopen a pidfd and check the exact current identity; no process-name sweep is used. Historical/global cleanup889–891 checks detached/adopted descendants. Offline control drains637–659 require real exit0, empty owned sets and closed ports before mutation. The original backend's separate command PGID at `manga-protocol-lab/server/src/backend.rs:265` does not escape the remembered owned session.
- BF-02: the eight phases37 match signed module10. Only those exact signed runner options get the600second bound771–772; ordinary phases remain300. Restart controls require explicit consent and430seconds in both relevant windows689–693, with bounded real drain/readiness.
- BF-03: the child environment92–96 still strips NODE_PATH/live credential variables. The unchanged runner13–24 loads its exact absolute reviewed Playwright package with path, hash, version and repository checks. This was not a Playwright/Chromium execution test.
- BF-04: complete manifest and independent index pins, exact safe member/byte identities and source/snapshot checks remain97–141. Snapshot readonly permissions120–121 and rechecks at615/762 remain. They require fresh supplied identities for the final UI generation; this review does not silently promote build10 to the final UI.

## Genuine controls, private keys and exact origins remain intact

The unchanged provisioner still verifies a direct private disposable root/marker81–122 and exact password-free reserved loopback DSNs140–159. Server keys are newly generated/private, with600 file modes188–206; public counterpart identities are distinct244–248/658–663. The supervisor's sanitized environment92–96 and bounded exact two-service private discovery overrides171–196 are unchanged. They do not globally enable private destinations.

Source revoke invokes actual signed kind5/deletion approval484–500; exact restore acquires the actual exclusive owner, restores real lifecycle and republishes the recorded canonical/image bytes and original creation time502–577. Catalogue rebuild596–634 drops only disposable platform schema and invokes real migrations, requiring a different migration-generated namespace. Separate recipient635–742 uses distinct original database/storage/key/backend roots and the actual injected-store listener. Peer policy756 onward changes the two recorded counterparts through actual repository methods. No successful original-response adapter, fixture UUID rewrite or browser-account federation grant was introduced.

The unchanged signed module76–88/162 verifies event IDs and genuine Schnorr signatures from actual original counterparts; direct observations/admission stay on exact recorded origins with redirects disabled83–99. Private controls108–137 require genuine private ownership, the current recorded browser PID/phase, real terminal results and stable service/UI identities. The runner validates one/two distinct loopback HTTP/root-WS destinations42–54 and retains exact routing plus an owned external-deny proxy79–100. Source inspection is not a runtime network trace.

The signer remains ephemeral Node memory71, restricted to the selected page at168 onward, and is cleared484–485. Mail links remain sink memory431–452 and are cleared895; native account fixture operations do not change password/verification479. Final cleanup removes both operator/service key pairs896–901 and makes their absence a verification requirement906. Provisioner arbitrary secret-bearing failures remain suppressed902–910. No runtime database, private key, authentication proof, recovery link or licensed image was read or packaged by this reviewer.

## Finite stopping point

The exact BF-06 patch is source-clean for the requested terminal truthfulness/ownership boundary. A new patchset requires another fresh review. Keep runtime held until the independent final UI/source gates and exact complete manifest/index/bootstrap-CSP identities are coordinated. Later actual acceptance must produce real startup/projection, visible-control journeys, signed delivery/recovery/source/namespace/two-recipient/restart observations and detailed graceful/forced/unconfirmed cleanup evidence. None of those runtime results was produced here. Reviewed nginx >1MiB replacement remains a separate runtime gate.

Keep runtime databases, private control payloads, server/signer secrets, authentication proof/request bodies, recovery links, licensed PNG/derivatives and artwork-bearing screenshots out of checkpoint uploads. This report contains source identities and review conclusions only.
