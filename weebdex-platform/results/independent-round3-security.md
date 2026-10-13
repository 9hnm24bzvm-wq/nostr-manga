# Independent completeness review, round 3: security and operations

Review window: 2026-10-13 04:59–05:06 UTC.

## Finite source verdict

**Five substantiated findings remain in the inspected patch set: one P1 and four P2.** Two are concrete counterexamples to the round-2 boundary/lifecycle fixes; a separate WebSocket shutdown issue is introduced by the new lifetime tracking. One additional deployment finding was reported and source-fixed during this review.

This is a finite independent source audit. It is neither an exhaustive security proof, a runtime acceptance pass, nor a production-readiness certificate. No Cargo command, runtime test, service launch, installation or production edit was performed. This report is the only file written by this reviewer. Findings were sent to the parent as they were established. Implementation owners are editing concurrently: line references and open statuses below describe the inspected source at the review window, and subsequent patches require a fresh independent review.

The genuine-original browser fixture contract was treated as setup and unrun acceptance preparation. Synthetic fixture identities/data are not production provisioning or evidence that a real runtime gate passed.

## R3-SEC-01 — P2: future-path resolution can miss a symlink after a missing parent traversal

Evidence:

- `backend/src/config.rs:527–563`: `resolved_future_path` removes trailing components until one existing ancestor can be canonicalized. It then rebuilds the suffix lexically, popping `..` and appending ordinary components. Appended components are never inspected or canonicalized again.
- The original storage and backend roots use this result for the public/private disjoint check at `config.rs:183–211,232–257`. These roots need not already exist, including in production. The production provisioning check for native `uploads_dir` does not apply to them.
- Original `../manga-protocol-lab/server/src/storage.rs:133–134` and `server/src/backend.rs:51–52` subsequently call `create_dir_all` on the configured path and canonicalize the actual created root.
- `backend/src/router.rs:242–255` serves ordinary files in the UI artifact tree.

Concrete scenario: the UI is `/base/ui`; an existing symlink `/base/alias` points to it; `/base/missing` does not exist; the configured original storage or backend directory is `/base/missing/../alias/private-runtime`. The resolver walks back to existing `/base`, reconstructs `/base/alias/private-runtime` without following `alias`, and accepts it as disjoint from `/base/ui`. The original opener can create the missing ancestry and resolves the actual target to `/base/ui/private-runtime`. The UI scan contains no symlink because `alias` is outside the UI. If the runtime user can write the artifact tree, private original/backend data becomes ordinary unauthenticated static content, bypassing original visibility/authorization. An immutable read-only service layout can stop the later creation, but the preflight itself does not enforce its claimed serving boundary.

This is not a claim that missing production group-key roots successfully start: the current publication opener separately canonicalizes matching group roots before opening them. It is a counterexample for accepted future original storage/backend roots, and the shared resolver should not be trusted for other private paths either.

Owner: configuration/catalogue plus original storage integration. Required acceptance: reject the combined missing-component/parent-traversal/existing-alias case before opening stores/backends/listeners, or resolve it with filesystem-equivalent component semantics. Rejecting parent traversal is a valid conservative policy if existing supported configurations are handled deliberately. Cover aliases to both descendants and ancestors of UI, equal roots, ordinary disjoint future roots, and inaccessible/broken ancestors. Merely resolving the first existing ancestor or normalizing the complete string lexically is insufficient.

## R3-SEC-02 — P2: background-worker descendants remain outside embedded termination ownership

Evidence:

- Original `server/src/node.rs:862–872` registers the background future directly; it does not scope it in the new owned-operation tracker.
- `node.rs:874–903` aborts only registered background handles, then waits their terminal observations. The operation drain at `node.rs:958–964` therefore cannot account for descendants of these unscoped workers.
- A normal injected-store path always creates the `blob_reclamation` worker at `node.rs:1039–1057`, which calls `UploadStore::reclaim_objects_page`.
- Original `server/src/upload.rs:655–682` creates a `JoinSet` whose jobs await `reclaim_library`. `reclaim_library` at `upload.rs:586–612` spawns a further Tokio task owning physical cleanup and durable refund. Aborting the collector drops/aborts its JoinSet jobs, which drop the further JoinHandles; those further tasks remain detached.
- Another configured path is `storage_audit` at `node.rs:1074–1084`: it awaits `upload.rs:698–730`'s detached audit task, which can in turn await `read_observed_class`'s task at `upload.rs:501–550`. That task can withdraw and quarantine an original at `upload.rs:535–543`.

Concrete scenario: an embedded background collector is awaiting a claimed original's cleanup/refund child when shutdown reaches `EmbeddedWorkerOwner::shutdown`. The registered collector becomes cancelled and its observation becomes terminal; the native listener can then report successful cleanup while the child is still using the physical volume or SQL. The platform can close its pool and runtime after native completion, abandoning that child. The existing durable ledger may permit recovery, but it does not make the reported completed termination accurate.

This reopens the lifecycle acceptance criterion from R2-SEC-06 with a specific newly inspected descendant path; the request-level guard does correctly retain many directly awaited handler tasks. Counting and abort-observing only the named parent workers is insufficient.

Owner: original lifecycle/bridge. Required acceptance: embedded ownership covers cancellation-surviving background descendants as well as HTTP/relay descendants. Stop new collector batches, preserve each admitted cleanup/refund/audit operation through terminal durable finalization, and wait actual descendants before reporting success or dropping storage/DB infrastructure. A deadline must fail truthfully. Later real runtime acceptance must pause a genuine reclamation child after its spawn, initiate shutdown, and prove the drain waits for its termination or reports uncertainty; a test with only a synthetic direct registry worker does not establish this property. Standalone behavior should remain deliberately preserved.

## R3-SEC-03 — P2: ordinary open relay sockets cannot receive the new shutdown boundary

Evidence:

- Original `server/src/node.rs:373–389,425–440` acquires an `OperationLease` for root and peer WebSocket upgrades and retains it through `relay.session`.
- `node.rs:775–791` only sets the operation admission `closed` flag and waits for active leases to become zero. `node.rs:1617–1624` closes admissions on shutdown; no cancellation/close notification is delivered to upgraded sessions.
- Original `server/src/relay.rs:99–203` selects authentication deadline, policy ticks, feed notifications and incoming socket frames. It has no shutdown branch.
- A public idle connection with no subscriptions has neither the peer-only authentication timeout nor the conditional policy-tick branch (`relay.rs:104–105`). It can retain its session lease indefinitely.

Concrete scenario: a user leaves the real browser's native public relay WebSocket open, or a public client upgrades and remains idle. On SIGTERM the HTTP listener may finish its HTTP drain, but the upgraded session remains alive and its active operation lease cannot drain. The native five-second operation timeout fails; normal deployment shutdown is reported unsuccessful and the process eventually drops the still-open session. An otherwise healthy open browser connection is enough. This differs from an honestly blocked durable mutation: there is no legitimate durable work the idle connection needs to finish.

Owner: original lifecycle/relay. Required acceptance: explicitly stop admitting new frames/publications, notify and close upgraded root and peer sessions on embedded shutdown, while retaining already-started mutation and detached policy descendants until terminal. Do not discard durable publication work by simply aborting the whole session. Later acceptance should keep an idle public socket and an authenticated/subscribed peer socket open, request shutdown, and prove clean closure plus successful bounded termination; separately block a real signed publication during that closure and prove its ownership remains intact.

## R3-SEC-04 — P1: native file ownership has unjournaled and nondurable namespace crash windows

Evidence:

- `backend/src/domains/uploads/storage.rs:215–232` renames a fully received original into the final `objects/<prefix>/<key>` tree and returns a `StoredImage` with no pending marker. Derivatives are created directly in that tree at `storage.rs:234–270`.
- The durable pending journal is only created later by `protect_commit` (`storage.rs:73–90`), for example at `backend/src/domains/uploads/sessions.rs:760–764` after object/reference insertion. Replacement and native cover upload use the same later journal pattern (`sessions.rs:705–717,1317–1321`).
- Startup recovery calls `recover_storage` at `backend/src/main.rs:86`; `backend/src/domains/uploads/mod.rs:87–90` delegates to session cleanup. `sessions.rs:1131–1164,1485–1556` reconciles pending markers, expires staged sessions and unlinks database-ledger cleanup. It never discovers an object that has neither a DB row nor a pending marker.
- Background temporary cleanup in `backend/src/runtime.rs:120–126` only visits `temporary`. The image proxy writes into `proxy-temporary` (`backend/src/domains/uploads/proxy.rs:205–216`) and relies exclusively on `ProxyFile::drop` (`proxy.rs:32–36`); that separate crash-leftover bucket is never scanned either.
- Original/derivative inode contents are synced (`storage.rs:186,267`) and marker contents are synced (`storage.rs:88`), but no containing-directory sync makes the final rename/new object entry, created object ancestry or created pending marker durable before SQL COMMIT. `confirm_persisted` removes the marker at `storage.rs:92–95` without syncing its directory.
- Protocol cover materialization repeats the final-object-before-marker and no-directory-sync sequence at `backend/src/protocol/covers.rs:185–224`, then commits its DB references at `covers.rs:239–248`.

Concrete scenarios and impact:

1. A hard process stop occurs after the original rename or derivative creation and before its pending marker/object row is committed. Drop guards do not run. After restart the bytes remain in permanent object storage with no recoverable ledger identity. They are never reclaimed by expiry, `queue_unused`, pending recovery or temporary cleanup. Repeated failures grow private disk use. The proxy's separate temporary bucket has the analogous hard-stop leak.
2. A filesystem/power crash occurs after SQL COMMIT but before final object/pending directory entries have become durable. Syncing file data does not itself establish durable rename/new-entry ordering. A committed DB reference can survive while its final filename does not, losing an accepted native upload/derivative despite the commit receipt. Conversely, a journal entry can disappear before it provides restart recovery. No actual power-crash outcome is asserted here; the missing durability barrier is visible in the source.

Owner: native storage/catalogue and protocol cover materialization. Required acceptance: give every final object a restart-recoverable ownership record before it enters an otherwise undiscoverable namespace, including all derivative/materialization paths, or use a bounded safe orphan reconciler with explicit active-operation exclusion. Establish the required directory durability ordering before the DB commit and durable marker removal afterward. Shared recovery must preserve every current DB reference, immutable publication pin and in-flight object installation, and retry failed unlinks. Reclaim only validated regular crash leftovers in temporary/proxy-temporary buckets, with bounded traversal and no links/special files. Later process-crash/fault acceptance must interrupt each creation/journal/SQL-commit boundary, restart against the same private volume, and verify retained bytes and bounded cleanup; ordinary cancellation tests alone do not cover it.

## R3-SEC-05 — P2: the documented proxy collapses every analytics client into one limiter identity

Evidence:

- `backend/src/domains/analytics/mod.rs:90–99` hashes only the `ConnectInfo` transport peer. It deliberately ignores forwarded client headers.
- `analytics/mod.rs:110–112` accepts at most 120 view submissions per minute for that peer; visitor cookies affect hourly view deduplication, not this limiter.
- `backend/src/main.rs:91–94` supplies ordinary TCP `ConnectInfo` to Axum.
- `deployment/nginx.example.conf:9–10,31–35,73–75` uses a loopback upstream, forwards an overwritten `X-Forwarded-For`, and proxies all public API visitors through that upstream. Thus their backend TCP peer is the same loopback address.

Concrete scenario: deploy the supplied proxy layout and have one visitor submit 120 otherwise-valid view events within a minute, within nginx's own per-client request allowance. Every other visitor then gets the backend busy response for view recording until the next minute, even with independent accounts, addresses and visitor cookies. Legitimate traffic itself also consumes this single tiny shared bucket. This is an identity-collapse/deployment-availability defect, not a performance benchmark or a request to trust arbitrary forwarding headers.

Owner: platform analytics/deployment. Required acceptance: define and implement an explicitly trusted ingress-client identity contract, or enforce bounded per-client analytics limits at the trusted edge with a separately intended backend-global safety limit. Direct clients must not obtain unlimited identities by supplying forged `X-Forwarded-For`; forwarded data must be ignored unless the configured trusted ingress is actually the transport peer, and ambiguity/unexpected chains must fail safely. Later proxy-backed tests must show two distinct real ingress clients are independent, malicious headers cannot evade their limit, and a deliberate global saturation response remains bounded.

## Deployment issue reported and source-closed during this review

At the initial read, the supplied nginx template had a 1 MiB generic API limit and raised limits only for `/api/upload/<10-character-id>` and `/api/cover/<10-character-id>`. The new atomic replacement endpoint was therefore rejected at the proxy for an ordinary replacement image above 1 MiB despite its 40 MiB Rust route/body budget.

The owner added the exact `/api/upload/<id>/files/<file>/replace` location during this review. The reread at `deployment/nginx.example.conf:57–65` confirms POST-only routing, 40 MiB body size, 180-second body intake and unchanged forwarding to the platform. This narrow source gap is closed. Actual nginx configuration validation and proxy-through replacement behavior remain unrun; the original Router test source with a >1 MiB valid replacement does not itself test nginx.

## Round-2 controls independently rechecked

- R2-SEC-01: `moderation/mod.rs:61–62` now requires a browser session before returning report data. Current issued library bearers cannot pass the guard, including staff-owned bearers.
- R2-SEC-02: `community/mod.rs:41–49` places the browser-session guard in both shared actor/mutation helpers; all four vote/unvote routes also require the intended permission. Public reply viewer projections discard API-client identities (`community/mod.rs:191–192,207–208`).
- R2-SEC-03: both explicitly configured group-key roots and ordinary missing descendants/symlink ancestry are now checked. R3-SEC-01 supplies a distinct combined counterexample for the shared resolver, rather than repeating the original omission.
- R2-SEC-04: `uploads/media.rs:32,70` obtains request/live-read admission before protocol source-selection SQL. A single lease follows fallback/native/original branches and is retained by actual body owners. No further preadmission DB path was found in the inspected media/proxy/preview routes.
- R2-SEC-05: `sessions.rs:1111–1134` now expires a bounded batch under session-row locks, excluding prepared/source-accepted commands and pinned original/derivative inputs, with migration 0083's relevant indexes. No repeat of the abandoned-session reference leak was established. R3-SEC-04 concerns files that never acquire a DB/session reference or durable journal, not those fixed expired rows.
- R2-SEC-06: request scopes and upgraded-session leases now cover much of the embedded work. R3-SEC-02 and R3-SEC-03 identify the specific remaining termination gaps. The new synthetic operation tests do not replace later genuine signed mutation/collector/WebSocket acceptance.

Other inspected source controls included current bearer scope/role narrowing, API-client/recovery/browser gates, expected-account constraints at optional identity resolution, password/credential/session revocation, email delivery/redeem bindings, browser-origin/duplicate-origin checks, current source visibility on public media/previews/library projections, private cache headers and computed CSP, DNS-pinned discussion-image proxy targets, streaming intake/admission budgets, atomic owner-page replacement and version checks, ordered checksummed migrations, service/proxy/private-path templates and release claim boundaries. No additional substantiated authorization or public-source visibility finding was established in this finite pass.

The previously noted runtime visibility-function DDL dependency still needs explicit operator role provisioning or a reviewed elimination of runtime DDL: `main.rs:58`, `protocol/mod.rs:36–47`. It is retained as a deployment prerequisite rather than promoted to a source defect without the intended database-role contract. Runtime/system, real mail, TLS/proxy, fresh/upgrade migration, durable-volume recovery and browser/desktop/mobile gates remain held or pending. No production-readiness conclusion is warranted.
