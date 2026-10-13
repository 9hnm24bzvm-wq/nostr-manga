# Independent combined-browser-fixture source review

Reviewed 2026-10-13 06:38–06:45 UTC, before runtime. Verdict: **HOLD combined browser runtime clearance until the four concrete fixture/harness findings below are closed and the final journey integration is frozen.** The fixture uses the actual original module and does not substitute successful protocol responses. This is a source review, not passing browser, listener-drain, delivery, recovery, or deployment evidence.

This reviewer performed only file/source inspection and hashing. No Node, Chromium, PostgreSQL, provisioner, platform, Cargo, test, dependency installation, network service, or production action was executed. This report is the only file written by this reviewer. Fixes belong to the fixture/frontend owners through the parent; this reviewer did not modify application or fixture source.

## Inspected identities and moving-source boundary

Frozen inputs verified against `combined-browser-fixture-source.json`:

- `tests/runtime/browser_fixture.py`: `cab3b2eea0248be7683baa9d6c3e981d5fe3e6cbf2d3771fa656e30cd9556dbb`
- `backend/examples/browser_fixture_provisioner.rs`: `1ee707435069e7c8925efb8247c6b99244d043935cc2b5470c4fb58c556e842a`
- `docs/browser-fixture-contract.md`: `98a6dfa64880785bfec14910a887fd7c44f9bebd3aea1f1c7b212862133835df`
- Existing compiled provisioner bytes: `c24bafab7a11a60cf9a80cc329c8c184d2fc520070acba2a0a22fa3f3de25435`. The recorded compiler receipt maps this binary to the frozen Rust source; this reviewer did not compile or execute it.
- `docs/PROTOCOL_BROWSER_FIXTURE.md`: `04ec0b0cc34f2bbf75d17e00337293c9c6f4f842cefa0752f69755125f62a285`
- Inspected runner before its owner integration: `scripts/browser-acceptance.mjs`, `b8dfa63cee39816ae01b5bc77404517166caa0d421f7e5caaf8f931388a42b20`
- `scripts/browser-journeys.mjs`: `65f5e5d115f35d15608e4075957172354978727cead70cfbeea6b64a0c0e5efc`
- Installed original strfry bytes observed: `559791b46763f5c129231bd9347c8fb39cfad84e2ee3d4f4218cef8df8c27e56`. This is an observed identity, not independent compiler/provenance clearance.

The signed-journey owner is actively extending the module. Its first inspected hash was `6eee74d7dcac17526b0994eea191d4aad08f0aada142e1a0e27c369d02cc0547`; a later read was `9cb4ef6987f376a37f17c8822153234d7228b026bf0efcbd3e267a30f7a6c254`. Neither is a final review freeze. The owner confirmed new real-control/two-recipient phases and the exact bounded-recipient proof guard are in progress. Findings do not claim those moving edits are absent from a future freeze.

## Findings requiring closure

### BF-01: terminal cleanup observes only leader process groups, missing original backend groups

Priority: P1, owned lifecycle/receipt accuracy. Owner: fixture supervisor.

Evidence: Python `group_members()` at lines427–436 checks `/proc` membership only by PGID. `finish_owned_group()` at437–449 signals/checks that same leader PGID. `stop_listeners()` at478–498 and `cleanup()` at677–747 use that predicate; cleanup can emit `status: verified_stopped` when the leader groups and listener ports are gone. The actual embedded original backend intentionally creates a separate process group for each `timeout`/`flock`/strfry operation at `manga-protocol-lab/server/src/backend.rs:242–268`, specifically `command.process_group(0)` at264–266.

Concrete failure path: if the platform or recipient is killed while an original backend command is active, killing/checking its leader PGID does not kill or observe the backend PGID. SIGKILL also prevents Rust `OwnedCommand::Drop` from running. The subprocess may remain alive until its own bounded timeout while the fixture declares the leader group stopped and all public ports closed. A false terminal receipt is possible even though `clean_drain` correctly distinguishes a forced parent kill. Normal exit0 drain does have an original in-process command join; this finding concerns the supervisor's forced-terminal guarantee.

Required closure: retain exact owned session/descendant identities, including process start identity where appropriate, and verify all admitted backend/Chromium descendants/groups are terminal before a restart, ownership change, or successful cleanup receipt. Original backend groups inherit the owned child session despite changing PGID, so same-session ownership can close this specific gap; a dedicated owned subreaper/descendant ledger is also acceptable. Never select or kill processes by executable name, broad PID search, or unrelated session. When terminal ownership cannot be established, emit `cleanup_unconfirmed` and stop mutation/restart. Validate graceful and forced paths in the later genuine lifecycle run.

### BF-02: new genuine-control phase names cannot obtain the restart budget

Priority: P1, deterministic acceptance blocker. Owner: fixture supervisor with signed-journey owner.

Evidence: Python609–610 grants up to600seconds only when `BROWSER_PHASE` is one of `signed-library`, `signed-profile`, `signed-social`, `signed-context`. Every other phase caps300seconds. Restart controls at529–532 require430seconds remaining in the active browser phase and fixture window. The signed-journey owner confirmed it is adding `signed-source-controls`, `signed-namespace`, `signed-recipients`, and `signed-restart`.

Reproduction from source: queue a new restart-bearing phase at600seconds and it is rejected by the browser command validator; queue it at300seconds and any `source-restore`, `catalogue-rebuild`, or `runtime-restart` command is rejected by the430second remaining-budget fence. No successful runtime can satisfy both predicates.

Required closure: coordinate the final explicit phase inventory and allow the intended bounded600second genuine-control phases. Preserve the430second remaining-window requirement, phase acknowledgment, restart acknowledgment, joined listeners, and finite action/readiness timeouts. Freeze the exact names and queue budgets with the finished signed module before launch.

### BF-03: secret-sanitized child environment removes the only installed Playwright resolution route

Priority: P1, deterministic owned-browser startup blocker. Owners: runner and fixture supervisor.

Evidence: Python76–80 deliberately excludes `NODE_PATH`; browser queue613–619 uses this sanitized environment. Runner12–13 creates a require relative to `.build/frontend-restore3/package.json` and calls `require("playwright")`. There is no Playwright package in that directory's normal `node_modules` or ancestor package lookup paths. The installed runtime Playwright1.62.1 exists at `/opt/codex/runtimes/codex-primary-runtime/dependencies/node/node_modules/playwright`; the current executor exposes that directory only through `NODE_PATH`.

Consequently the owned runner will fail module resolution before constructing its report, entering its try/finally, or launching Chromium. An earlier historical runner used inherited environment and reached a browser launch, so that historical attempt does not validate the new sanitized-queue path.

Required closure: resolve the installed reviewed runtime package through an explicit narrow trusted path, or provide a validated browser-only module path. Keep other child environments secret-free and do not inherit arbitrary environment, provider credentials, upstream packages, or an unreviewed fallback. Preserve failure/nonzero receipts when an installed package or real Chromium capability is unavailable. This source finding was established through filesystem/environment inspection; Node was not run.

### BF-04: frontend index hash does not pin the complete executable snapshot

Priority: P1, exact-source acceptance/provenance gate. Owner: fixture supervisor, reusing frontend manifest.

Evidence: Python81–95 bounds frontend membership and rejects symlinks, but hashes only `index.html` before `copytree()`. It then records `immutable_private_snapshot: true` without verifying the complete copied file membership/byte hashes or making the snapshot read-only. The existing `test-evidence/frontend/artifact-manifest.json` records complete file paths, lengths, and SHA-256 values; the supervisor does not consume or verify it. Backend/provisioner executable hashes are required and checked, while a changed same-named JS chunk or service worker with unchanged index passes the current UI check.

Required closure: accept/pin the reviewed existing complete artifact-manifest identity, verify exact file membership and each byte hash against the source and copied snapshot, and bind that result to the runtime receipt before app/browser launch. Preserve the existing symlink/size/file-count bounds. Make the private served snapshot immutable for the run, and keep the actual served index/CSP bootstrap hashes consistent with that verified snapshot. Reject missing/extra/changed files, including service-worker bytes. The parent confirmed no independent runtime guard has already closed this gate; this is a concrete missing fixture check, not a request for a new manifest framework.

## Owner integration dependencies, not additional frozen-source findings

- The inspected runner's HTTP/WS routing guard admitted only the primary protocol origin/root relay. The second recipient in `protocol_recipients` would be blocked. The signed owner confirmed bounded exact recipient/proof-origin validation is being added, and the frontend owner is assigned the matching HTTP/WS guard. Re-review the finished integration before launch. Accept only the recorded max2 distinct loopback services with exact root HTTP/WS endpoints, valid public service keys, matching primary identity, and matching actual capabilities/CSP policy; do not expand to generic loopback or wildcard origins.
- The final signed control module, phase inventory, harness source, UI artifact manifest, bootstrap-CSP evidence, backend binary/source mapping, provisioner mapping, and reviewed strfry identity need one final freeze. A changed source or artifact invalidates prior clearance.
- `docs/browser-fixture-contract.md` and `combined-browser-fixture-source.json` correctly state runtime is held and pending/control compilation is not browser acceptance. Update source-hash and remaining-scope receipts after fixes, without promoting authored assertions to passes.

## Safeguards verified in source

- The guard is additive test-only code, not a production moderation/rebuild endpoint. Rust81–169 limits requests to a direct private `test-runtime/browser-final-*` child, a fixture marker, exact reserved loopback DSNs/databases/actual PG port, no password/options, and bounded private inputs. Python creates the root0700 and binds the services only to127.0.0.1; Unix PostgreSQL sockets are disabled.
- Rust208–259 calls actual `Repository::migrate`/`verify_schema` and publisher admission, uses distinct synthetic operator/service identities, private0600 key files, original storage/backend/group-key roots, public read policy, exact UI CORS, disabled unrelated synchronization/discovery workers, and records actual strfry identity. Native SQL seeds native fixtures only; it does not create original canonical/publication authority.
- Rust294–395 publishes typed canonical1002/1001 bytes through actual `PublicationRuntime::publish_operator`, saves exact verified source/CBOR/input paths privately, and drops its exclusive owner before the real platform starts. Python181–195 requires real original projection/visibility/import aliases and actual capabilities/resource-map responses, rather than trusting accepted/pending receipts.
- Optional recipient635–798 has separate original database, storage owner, keys, backend data directory, listener, and actual mutual `Repository::set_peer` policy only for the two recorded services. It serves through the real injected-store original listener. Its readiness checks public identity/nonce/endpoint; genuine service-signature proof is a browser acceptance responsibility, as the contract states.
- Native password accounts and external signing authority remain separate. HTTP admission accepts only a bounded64hex public key and explicit boolean, caps16 distinct accounts, and calls original account admission. The signer is genuine Schnorr/NIP07 emulation in Node memory with an explicit UI selection; secret bytes are not passed to Chromium or the provisioner and are zeroed at teardown. No token/request-proof/signer body is intentionally persisted to evidence.
- Private controls have bounded JSON, safe unique IDs, a fixed action set, recorded canonical event targets, explicit phase/restart acknowledgments and finite windows. Source revoke uses actual owner-signed kind5/deletion/operator approval and then checks terminal removal, genuine public404 and absence from the actual resource map. Restore joins listeners, acquires the actual exclusive owner, republishes exact canonical/original bytes at original created_at and checks identical event ID. Catalogue rebuild drops only disposable platform schema and applies actual migrations, requiring a different migration-generated namespace; it preserves original schema/service/storage identities. No fixture UUID UPDATE or fabricated acknowledgment was found.
- Existing signed fault routes abort requests or forward actual responses before intentionally losing them; they do not manufacture a successful original protocol receipt. Native journey negative fixtures fabricate errors only; forwarded delayed native responses are genuine. Source assertions, private fixture setup, browser reload and actual platform/listener restart are correctly distinguished.
- Browser report logic requires real steps, all steps passed and no harness error. Browser launch failure is not a pass. The historical `test-evidence/frontend/browser/startup/acceptance.json` has zero steps and `passed:false`; it does not prove current launch capabilities or current artifact acceptance. Real Chromium must launch in the same owned execution namespace as the loopback services. No browser launch was attempted in this review, and availability must not be inferred from a binary file existing.
- Cleanup intends to stop browsers first, withdraw fixture mutation/mail, serially join provisioner work, drain platform/recipient, stop PG, clear mail memory, remove both operator/service key pairs and verify owned listeners/ports. It records force-kill and `clean_drain` separately. BF-01 must be closed before its terminal-process guarantee can be accepted.

## Required runtime evidence after source closure

Run genuine E2E against the final verified snapshot/binaries in one owned namespace, including real signature/service proof, admission denial/approval, library/profile/social delivery and counterpart verification, two actual recipient outcomes/races, namespace/source fencing with old saved-device rows retained, exact restoration, actual process restart and terminal group/session/port/key receipts. Record forced or incomplete drain truthfully. Readiness/provisioning is not browser acceptance. Keep runtime databases, private controls, server/signer secrets, authentication proof bodies, recovery links, licensed image derivatives and artwork-bearing screenshots out of source/checkpoints/evidence uploads. Actual reviewed nginx >1MiB replacement remains a separate pending proxy acceptance gate.
