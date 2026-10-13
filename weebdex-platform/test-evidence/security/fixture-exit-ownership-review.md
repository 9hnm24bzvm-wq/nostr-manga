# Independent fixture exit-ownership review

Reviewed 2026-10-13 07:10–07:15 UTC. **Finite verdict: BF-05 is closed in the inspected source; HOLD final fixture runtime clearance for the bounded terminal-provenance finding BF-06 below.** No remaining direct-child wait-status theft was substantiated. BF-01 through BF-04 remain closed as source mechanisms. Final UI source repairs and their future fresh complete artifact/index pins remain independent gates.

This is source inspection, not execution proof. This reviewer did not launch the fixture, PostgreSQL, the platform, provisioner, original backend, Node, Chromium, Cargo or tests, and made no source edits. Local reads/hashes and this report are the only review operations. Authored journeys and existing compiler receipts do not establish runtime acceptance.

## Exact inspected boundary

- `tests/runtime/browser_fixture.py`: `f40102d5be154241646f8b7753d83863c5a5978e2cdc9fb95dfde9629f1f0124`
- `backend/examples/browser_fixture_provisioner.rs`: `1ee707435069e7c8925efb8247c6b99244d043935cc2b5470c4fb58c556e842a`
- `docs/browser-fixture-contract.md`: `6c1780e016f6a006b0533a76ea97016adecd9e029ac3d96f5e09cfb69e2181d5`
- `scripts/browser-acceptance.mjs`: `b4c1ed2dc34d32d5a964d56f662085e7ac7cd5efec85c798ce53884ac5b308e0`
- `scripts/browser-signed-journeys.mjs`: `af01c43b9753d8cc4d3764afef8adfd25d4762cd5bb8b978c40e8359bbbd19d5`
- `scripts/browser-journeys.mjs`: `65f5e5d115f35d15608e4075957172354978727cead70cfbeea6b64a0c0e5efc`
- Source index `test-evidence/security/combined-browser-fixture-source.json`: `c213ef2c855340280b8ce298a9cbffaf4573a70c744f631074d1598a91b9fcf6`; its supervisor/provisioner/contract entries match the inspected hashes.

Observed build10 complete frontend manifest: `51f8a91b9c3cc331bb98a1260c47720e43ea38ee774985abf8a17d44e828c6b8`; observed index: `7e81d63c5ecdb27ca5b0e1e5f77edb19d511d46c087cabf2d758007269b60b3b`. The existing build10 receipt reports compiler exit0, unchanged inputs and no surviving owned members. Bootstrap-CSP evidence names that same index. These are the current build10 observations, **not final approval for subsequent UI repairs**. Obtain and review fresh artifact/index/CSP identities after those repairs.

The existing backend index names candidate013 platform `946210110155338ade9501e4152ce39c343b3bff573dca6f7cbca8d8e136106d` and provisioner `8844296fa09e7423ea4814beac93412d0763628c787ba26f211acf0e821f3928`; compilation was not reproduced here. Independently observed installed original strfry identity remains `559791b46763f5c129231bd9347c8fb39cfad84e2ee3d4f4218cef8df8c27e56`.

## BF-05 closure: direct Popen owns its actual wait status

Both remaining `subprocess.Popen` sites are registered ownership paths. `spawn()` at243–256 holds `process_ownership_lock` continuously across creation254 and registration255. `run_owned()` at264–288 does the same at268–271. `register_process()` stores the child, exact PID/start ticks/session and root identity at259–263 before either caller releases the lock. No `subprocess.run` remains. psql, initdb, pg_isready and migration all use the registered synchronous path at289–335.

`reap_owned()` at549–563 holds this same reentrant ownership lock through root/leader selection554, direct Popen polling555, the owned-member scan556, the direct-leader exclusion557, exact current-identity check558–559 and orphan `waitpid`561. A concurrent sink admission cannot insert a new direct waiter between the exclusion snapshot and orphan wait. Even an immediately exiting child remains excluded by its exact birth identity. Historical Popen roots also remain in the exclusion ledger. Genuine adopted descendant zombies can still be reaped.

The owning Popen poll/wait/communicate uses Python's own `_waitpid_lock`; the inspected installed `/usr/lib/python3.13/subprocess.py:1987–2017` prevents simultaneous Popen wait collection. `communicate()`274 and blocking waits/drains run outside the fixture ownership lock. There is no fabricated fallback return code in the fixture. Nonzero psql/initdb/migrate statuses still take their failure paths292–294/313/335; timeout and unresolved-descendant synchronous commands fail275–283. BF-05's two specific races are therefore closed in this source.

## BF-06: final terminal summary loses cumulative forced provenance and can exit0 on unconfirmed cleanup

Priority: P2, final evidence/exit-status truthfulness. Owner: fixture supervisor. This is a bounded terminal-provenance finding; it does not reopen BF-05 or claim that the harness turns failed browser steps into passing acceptance.

### Previously forced children are relabelled in the final summary

Source-established scenario:

1. An owned browser exceeds its deadline. `browser_commands()` sets `forced=True` and signals it at736–740, completes descendant cleanup and writes its actual returncode plus `forced:true` to the phase receipt and `self.receipt['children']` at743–747.
2. A later ordinary stop request enters cleanup after that browser and its descendants are terminal. Cleanup initializes `forced=False` at834. It skips the signal branch because the Popen is already terminal, and `finish_owned_group()` returns false for the empty member set at840. The final `results[name]` therefore says `forced:false` at841.
3. If native listeners drain with exit0 and no other final forced action, line901 can then emit unqualified `clean_drain:true`. The final per-child summary contains no cumulative flag distinguishing this already-forced phase from a normally finished phase.

The same recomputation occurs for previously forced provisioner/synchronous children at850–858, even though their earlier receipt was recorded at158 or286. The per-phase browser receipt and `BROWSER_PHASE_FINISHED` retain the actual failure/forced fact. Runner `scripts/browser-acceptance.mjs:153–156` additionally requires actual terminal successful steps and emits nonzero when acceptance is false. Thus failure is still surfaced; **the gap is inconsistent cumulative final provenance**, not manufactured passing browser coverage.

Narrow closure: preserve an explicit cumulative forced flag/previous terminal result for every final child, including browser/provisioner/synchronous paths, and retain phase failure separately. If `clean_drain` is intentionally final-teardown-only, label that scope clearly and do not present it as an entire-lifecycle clean/pass result. A separate final-only forced action field is fine; it must not erase prior forced evidence.

### Unconfirmed final cleanup does not affect the top-level exit code

`cleanup()` honestly computes `status:'cleanup_unconfirmed'` at896 when a final owned member, signal-tracking error, open listener port or retained key prevents confirmation, writes that receipt at904–906, but returns no status. `main()` at925–929 returns1 only for its pre-cleanup `error`.

Consequently, an ordinary `stop.request` with no preceding exception followed by a final unconfirmed owned survivor/tracking result can emit `cleanup_unconfirmed` and still exit0. The receipt remains explicit; process exit alone is insufficient evidence. Narrow closure: return/propagate the final cleanup result and make unconfirmed final cleanup nonzero. Do not replace the actual per-child return codes or conflate forced-but-confirmed teardown with unconfirmed teardown.

Re-review the repaired exact supervisor/contract/source-index hashes before launch. This reviewer did not alter either path.

## Previously fixed mechanisms remain intact

- **Complete owned descendants (BF-01):** subreaper/pidfd support is required82–84; exact root birth/session registration259–263, inherited-session and remembered ancestry/adoption516–548, and individually identity-rechecked pidfd signalling564–580 include original backend changed-PGID commands. The original backend still uses `process_group(0)` at `manga-protocol-lab/server/src/backend.rs:264–266`, without changing its inherited fixture session. Global cleanup includes historical/adopted descendants880–902. Survivors and tracking errors prevent `verified_stopped`. Offline mutation requires joined exit0, empty descendants, closed ports and no unassigned adopted/tracking error at628–650. No executable-name or unrelated-process sweep is used.
- **Eight finite signed budgets (BF-02):** supervisor37 and signed module10 have the identical eight-phase inventory. Queue762–763 grants600seconds only to those exact runner phases; ordinary phases remain300seconds. Restart controls require explicit phase/restart acknowledgment and430seconds remaining in both windows680–684, with220second listener join,90second action and90second readiness bounds. Signed privateControl108–137 checks genuine private ownership and terminal joined results. A late command fails explicitly.
- **Absolute Playwright (BF-03):** the safe environment92–96 still omits NODE_PATH and live credential variables. Runner13–24 loads the explicit installed official package, checking exact real paths, metadata/version1.62.1 and package/index hashes. Independently hashed installed bytes match `ca170ec143a88ed3043ac953eb3b2377b2b97304104f4e1e23316684ce2c35af` and `4e98f65f0a9d9bcab8cffc0c5cfdd87fd3f5b0be74fc494ff166a466b204cd44`. Chromium launch is still unexecuted.
- **Complete pinned UI snapshot (BF-04):** supervisor97–141 requires the supplied complete-manifest and independent index identities, exact safe file/directory membership, each byte length/hash, bounded totals, independent source/snapshot checks and read-only private snapshot. It rechecks before listener startup606 and browser phase753. Subsequent UI repairs require a new reviewed complete manifest/index; the mechanism does not silently admit stale chunks or service workers.

## Genuine controls, two recipients and evidence privacy

The frozen signed module has authored calls/assertions for actual source withdrawal/exact restore255–274, real migration-generated namespace rebuild276–294, joined process restart295–302 and two real recipient outcomes/policy303–327. It validates actual public counterparts with genuine Schnorr/event-ID checks76–88/162, and retains old saved-device/outbox data where required. Source controls use Rust's real owner-signed deletion/admission/publication methods484–577. Catalogue rebuild drops only disposable `platform`, applies real migrations and requires a different generated namespace596–634. No fixture namespace UUID update or successful original-response adapter was introduced.

The optional second recipient has a separate guarded disposable database, service/operator keys, storage/backend roots and actual injected-store original listener635–742. Peer policy756 onward changes only the two recorded services through actual Repository methods. Browser public account admission remains distinct from service-peer authority. Supervisor protocol_setup171–196 pins private discovery exceptions to the two recorded origins; it does not weaken production defaults globally.

Runner42–54 and signed module55–62 validate one or two distinct exact loopback HTTP/root-WS services/public keys and the matching primary identity. Runner79–100 denies external HTTP/CONNECT through its owned proxy and admits page HTTP/root-WS traffic only to the declared fixture destinations. Direct signed observations/admission83–99 explicitly check recorded origins and disable redirects. No external service was contacted by this review; these are inspected traffic controls, not a runtime network trace.

Noble provenance manifest and official archive hashes independently match the frozen module pins21–39. The private signer is ephemeral Node memory71; only public identity/templates/signed results cross its selected-page bridge168 onward; its secret is cleared484–485. Supervisor environment strips live credential variables92–96; mail links remain sink memory417–443 and are cleared885. Rust provisioner stdout consists of public receipts; arbitrary secret-bearing errors are deliberately suppressed902–910. Native runner passwords/email/link token values enter its redaction set before relevant use, and runner URL/error logging removes authentication query strings60–65. No intentional secret/request-body/cookie persistence was substantiated in the inspected receipt/log paths. This is not a claim to have inspected generated runtime logs, which do not exist for this review.

## Finite follow-through

Close only BF-06's terminal-provenance/exit path, refresh the exact changed source hashes and obtain a fresh independent check. Keep frozen runner/signed-module identities and candidate013 compiler identity receipts. Complete the separate UI repairs/review, then bind their new complete manifest/index and matching bootstrap-CSP evidence before parent runtime clearance.

Later actual combined acceptance must establish startup/projection, genuine visible-control journeys, signatures/service proof/delivery/recovery, two real recipients, source/namespace fences, exact restore, joined restart and truthful graceful/forced/unconfirmed descendants/ports/key receipts. None passed in this review. Reviewed nginx >1MiB replacement remains its separate pending runtime gate.

Keep runtime databases, private controls, server/signer keys, authentication proof/request bodies, recovery links, licensed PNG/derivatives and artwork-bearing screenshots out of source/checkpoint/evidence uploads. Existing stale statements that the new signed calls remain unauthored are documentation provenance cleanup, not another code blocker.
