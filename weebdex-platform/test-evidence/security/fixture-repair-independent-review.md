# Independent fixture repair source review

Reviewed 2026-10-13 06:58–07:05 UTC. **Finite verdict: HOLD combined fixture/browser runtime clearance for BF-05 below.** The four mechanisms reported in `fixture-independent-review.md` are repaired in the inspected source. One concrete wait-status ownership race in the new subreaper implementation remains. Final UI build10 review and its fresh complete artifact/index identities are also pending external gates, not additional fixture-code findings.

This review inspected source, installed dependency files, hashes and existing compiler receipts only. It did not execute the fixture, any target application/module, Node, Chromium, PostgreSQL, strfry, Cargo, tests or network services. The review report is the only file this reviewer wrote. No source repair was made here. Source closure, authored journeys, compiler receipts and installed browser bytes do not establish runtime acceptance.

## Exact inspected inputs

- Supervisor: `tests/runtime/browser_fixture.py`, SHA256 `8a055f8af6c7d4e55e8fdb432055ffc52adb5b76e7caa32e9ac09a8a0c2fc04b`
- Rust test provisioner: `backend/examples/browser_fixture_provisioner.rs`, `1ee707435069e7c8925efb8247c6b99244d043935cc2b5470c4fb58c556e842a`
- Contract: `docs/browser-fixture-contract.md`, `42b09cb0f06b7ea9479bf4ea8972babb50f6db8c6570e7980ad19ceb01072e9f`
- Browser runner: `scripts/browser-acceptance.mjs`, `b4c1ed2dc34d32d5a964d56f662085e7ac7cd5efec85c798ce53884ac5b308e0`
- Signed journey module: `scripts/browser-signed-journeys.mjs`, `af01c43b9753d8cc4d3764afef8adfd25d4762cd5bb8b978c40e8359bbbd19d5`
- Native journey module: `scripts/browser-journeys.mjs`, `65f5e5d115f35d15608e4075957172354978727cead70cfbeea6b64a0c0e5efc`
- Installed original strfry bytes: `559791b46763f5c129231bd9347c8fb39cfad84e2ee3d4f4218cef8df8c27e56`. This is an independently observed installed identity, not a new compiler/provenance approval.

The frontend owner confirmed the two inspected runner/signed-module hashes are frozen, the eight-phase extension is finished, and no further signed-module edits are pending. UI build10 is separate and was still in progress at review. The presently observed frontend manifest `54a5beebe98eeaf591edb7f8454c4c50343df731a661ce1feca5854f0178166b` and index `012ce47d8b7c42294a5caca50e394996cfd7c24be6990b5eb1ba835b62ddaef0` belong to the stale build9 boundary and are **not admitted as final build10 input by this report**. The supervisor accepts the supplied exact reviewed manifest hash; it does not hardcode build9.

Independently hashed existing candidate013 binaries match `test-evidence/final/backend-current-build.json`:

- Normal platform: `946210110155338ade9501e4152ce39c343b3bff573dca6f7cbca8d8e136106d`
- Provisioner: `8844296fa09e7423ea4814beac93412d0763628c787ba26f211acf0e821f3928`
- Normal compiler receipt: `cargo-build-normal-003-result.json`, `93a83d3dfb36a747f2a84b1417deddee0042204e5c39e6d88f654659df4a262b`
- Example compiler receipt: `cargo-build-example-003-result.json`, `4ffd0e18a900ed960983083898313aeeccff1431d8a31373e92104e377c19ce1`

These existing receipts report compiler exit0, unchanged frozen compiler inputs and no surviving owned compiler members. This reviewer did not rebuild, run the binaries or independently reproduce source-to-binary compilation. The source index now matches the supervisor/provisioner/contract hashes above and the candidate013 provisioner identity.

## BF-05: orphan reaping can steal an active direct child's exit status

Priority: P1, truthful provisioning/terminal receipts. Owner: fixture supervisor.

The new Linux subreaper correctly broadens ownership beyond leader PGIDs, but `reap_owned()` at supervisor lines519–529 treats a direct-child zombie absent from a **previously captured** `leaders` set as an adopted orphan and calls `os.waitpid(pid, WNOHANG)` directly. That is insufficient to establish that no `Popen` waiter owns its exit status.

There are two source-established paths to the same race:

1. `psql()` at259–265 uses an unregistered `subprocess.run(..., capture_output=True, timeout=15)`. The threaded `/fixture/provision` handler calls it at434 while the main supervisor can enter browser/control/cleanup `reap_owned()`. The psql child is a direct child of the supervisor, so `owned_members()` marks it adopted at513; when it becomes a zombie the orphan reaper can consume its status before `subprocess.run`'s `Popen` does.
2. `reap_owned()` releases `process_ownership_lock` after capturing `leaders` at520. A sink admission thread can subsequently spawn/register a provisioner via243–258. The following `owned_members()` call takes a fresh root snapshot and can include that newly registered root, yet the orphan exclusion still tests the older `leaders` snapshot. If the new direct child is already terminal, its status can similarly be stolen. `spawn()` also creates the `Popen` before acquiring the registration lock at256, leaving an additional registration gap.

The observed installed `/usr/lib/python3.13/subprocess.py` confirms the consequence without a runtime experiment: `_internal_poll` at2009–2015 sets `returncode=0` on ECHILD, and `_try_wait` at2025–2030 converts `ChildProcessError` to status0. Thus an actual direct-child failure may be recorded/handled as exit0 after the fixture consumes its wait status. Genuine receipt validation may catch a missing/invalid payload afterward, but it does not restore the lost actual exit status; `psql()`'s failure handling specifically depends on that return code.

Required narrow closure: coordinate direct-child creation/registration and orphan reaping under the ownership lock, exclude every still-owned `Popen`/direct waiter immediately before `waitpid`, and bring the concurrently invoked psql child into that exclusion/ownership path. Continue reaping real adopted descendants and retain exact PID birth checks. No unrelated process selection, synthetic exit code or generic replacement framework is needed. Re-review the changed exact supervisor hash before launch. Later genuine lifecycle evidence must retain actual nonzero/forced results and terminal descendants.

## Previously reported repair closure

### BF-01: changed-PGID original backend descendants

Addressed in source, subject to BF-05's wait-status repair. The supervisor requires child-subreaper/pidfd support at79–84, records leader PID/start ticks/session at251–257, discovers inherited-session and observed descendant ancestry at486–518, and signals individual pidfds only after rechecking exact identity at530–546. The original backend still uses `command.process_group(0)` in `manga-protocol-lab/server/src/backend.rs:264–266`; those commands inherit the fixture child session and are now included despite their different PGID. Global cleanup at844–868 includes adopted/historical descendants. Remaining members or recorded signal errors prevent `verified_stopped`; `clean_drain` separately rejects forced cleanup. Offline listener mutation requires joined exit0, empty descendant sets, closed listener ports, no unassigned adopted children and no tracking errors at594–617. No process-name sweep was introduced.

### BF-02: phase inventory and restart budget

Closed in source. `SIGNED_PHASES` at37 and the signed module's exported inventory at10 contain the same eight explicit phases, including source controls, namespace, recipients and restart. Queue validation at728–729 grants those exact runner phases up to600seconds; ordinary phases remain bounded300seconds. Restart controls preserve the430second active-phase and fixture-window checks at649–650, explicit active-phase acknowledgment, explicit restart acknowledgment,220second drain and bounded action/readiness waits. The Node private control helper at108–137 uses the recorded owned phase and finite control/result deadlines. A late command is truthfully rejected; the runner must execute restart-bearing steps early enough in the allowed phase.

### BF-03: Playwright resolution under the sanitized environment

Closed in source. The child environment at92–96 continues stripping NODE_PATH and live credential variables. Runner13–24 loads only the explicit installed Playwright absolute entry, checks real paths, pins package/index SHA256 and requires official package metadata/version1.62.1. Independently observed package/index hashes match the pins:

- package.json: `ca170ec143a88ed3043ac953eb3b2377b2b97304104f4e1e23316684ce2c35af`
- index.js: `4e98f65f0a9d9bcab8cffc0c5cfdd87fd3f5b0be74fc494ff166a466b204cd44`

The entry requires adjacent `playwright-core`, which is present on the installed runtime's ordinary module lookup path. This closes the prior deterministic NODE_PATH-only lookup failure. It does not assert that real Chromium can launch in the owned execution namespace; launch failures remain nonzero and cannot pass acceptance.

### BF-04: complete frontend executable snapshot

Closed as a source mechanism. Supervisor97–140 requires the supplied complete manifest SHA256, validates bounded unique safe members, checks summary totals, verifies exact source directory/file membership and every length/hash, copies into the private snapshot, verifies it again, and makes files0400/directories0500. The manifest must contain a bounded index and the independent supplied index hash must match. The same verified expected membership is rechecked at listener restart572 and browser phase719. The receipt binds the manifest/index identities and byte/membership checks. Main requires the complete manifest and index hashes at884. Missing, added or changed chunks/service-worker files are rejected, including after copying. The parent must supply the newly reviewed build10 manifest/index, not the stale observed build9 pair.

## Frozen signed integration and original-authority safeguards

- Runner42–54 and signed module55–62 validate one or two distinct exact loopback HTTP/root-WS recipient services with valid public64hex keys and matching primary identity. HTTP routes and WS routes admit only those exact declared services; external traffic is denied by the owned proxy. There is no generic loopback or wildcard recipient expansion.
- Signed module21–39 pins the existing Noble provenance manifest, official archive identities and each admitted installed source file before importing genuine Schnorr. The manifest observed hash matches its hardcoded reviewed identity. Its private signer remains ephemeral Node memory; only public key/templates/signed outputs are exposed to the selected page. It is not a password identity, protocol response adapter or fake signature. Finally484–485 clears the signer secret and page bridge. No signer secret or signed authorization body is intentionally persisted in report output.
- The genuine source-control phase at255–274 uses actual owner moderation, waits for public404/current mapping absence, tests both stale signed reviews, then explicitly requests joined exact restore and rechecks identical source/native alias/namespace and preserved device records.
- The genuine namespace phase at276–294 uses the actual disposable platform-schema migration seam. It checks the new migration-generated namespace and stable original service/source, retains old saved-device rows/backup scope and independent canonical records, and tests reload/current-title migration fencing. Rust596–634 drops only disposable `platform`, calls real migrations and requires a genuinely different generated namespace; no fixture UUID UPDATE is used.
- The restart phase at295–302 requires genuine joined listener receipts, a new platform PID, stable identity/origin/namespace/frontend and exact retained outbox/recovery/device records. It does not equate page reload with a process restart.
- The recipient phase at303–327 independently verifies/remembers the second actual service, requires both real recipient outcomes and exact original counterparts, invalidates review on recipient selection change, verifies exact-body retry remains bound to its approved original service, and changes actual mutual peer policy only for the two recorded services. Browser account authority remains distinct from federation service-peer authority.
- Source setup/admission/publication and restoration still call actual original `Repository`/`PublicationRuntime` methods. Rust81–169 confines the provisioner to the private direct reserved root and exact password-free disposable database/actual PG port. Distinct0700/0600 service/operator/storage roots are retained for the two services; listener ownership and exclusive original restoration ownership remain separate from native fixture SQL.
- Fault routes abort transport or forward real responses before deliberate loss. No successful original acknowledgment is manufactured. Signed/canonical/library projections are checked against genuine original counterpart bytes/signatures. Runner153 requires at least one actual step, all steps passed and no harness error; a missing browser/control/recipient cannot count as passing coverage.
- Unique private temporary receipt files at45–54 prevent concurrent writers replacing the same temporary pathname; cleanup snapshots child iteration at798/814 and ownership dictionaries are read under the ownership lock. These are useful concurrency repairs, but do not by themselves close BF-05.

## Finite follow-through boundary

After BF-05 source closure, refresh the exact supervisor/source index and combine this review with the independent final UI review. Bind the already compiled candidate013 platform/provisioner identities, reviewed strfry identity, frozen runner/signed module, fresh build10 complete artifact/index and matching bootstrap-CSP evidence before the parent opens runtime. The existing contract/index's statement that new signed calls remain unauthored is stale documentation; the frozen module now contains them. Updating that remaining-scope description is a provenance cleanup, not another implementation blocker.

Then obtain actual combined startup/projection, all relevant visible-control journeys, service proof/signature/delivery/recovery, two real recipients, source/namespace fences, exact restore, joined restart and truthful graceful/forced descendant/port/key receipts. None has passed in this review. Actual reviewed nginx >1MiB replacement remains its separate pending proxy gate.

Keep runtime databases, private controls, server/signer keys, authentication proof/request bodies, recovery links, licensed PNG/derivatives and artwork-bearing screenshots out of source/checkpoint/evidence uploads. A source or artifact change outside the reviewed boundary requires matching identity/review refresh; it does not invalidate unrelated proven mechanisms or justify inventing additional blockers.
