# Independent completeness review, round 2: security and operations

Review window: 2026-10-13 04:36–04:43 UTC.

## Finite verdict

**Six substantiated in-scope findings reopen implementation completeness: three P1 and three P2.** This is a finite source review, not exhaustive security proof, a runtime acceptance pass, or a production-readiness certificate. Findings were sent immediately to the parent and responsible implementation owner. Owners may change source concurrently; line references below describe the inspected source, and this report does not approve subsequent fixes. A fresh independent squad must review the completed patch set.

No runtime tests, Cargo commands, installations, service launches or production edits were performed. This report is the only deliverable written by this reviewer. The prior audit's 17 closed findings were treated as leads, not proof.

## R2-SEC-01 — P1: owner-library bearer reads private reports

Evidence:

- `backend/src/domains/moderation/mod.rs:61–68,94`: GET `/api/report` uses `require_principal`, applies a submitter filter to nonreviewers, then returns `platform.report_json`. It never requires a browser session.
- `backend/src/domains/auth/principal.rs:168–218,257–269`: an issued, eligible `self.library.read` bearer resolves to a normal nonrecovery principal; `require_principal` does not reject API clients.
- `backend/migrations/0050_moderation.sql:42–53`: the still-current report DTO includes private report body, moderator notes and relationships. Migration 0081 replaces target visibility predicates but does not replace this DTO or add bearer authorization.

Scenario: a user gives an integration a read-only owner-library API credential. That integration requests `/api/report` and receives the user's private abuse reports or group-claim evidence and review notes. Removing ambient staff roles does not close this owner-resource leak.

Owner: catalogue/auth. Required acceptance: a genuinely issued owner-library bearer, including one owned by staff, gets a forbidden response with no private report data. Browser owners still see their own reports; current authorized reviewers still see the intended queue. Include narrowed/revoked credential cases and response-content assertions through the real Router.

## R2-SEC-02 — P1: read-only bearer creates and deletes votes

Evidence:

- `backend/src/domains/community/mod.rs:46–55`: `mutable_actor` checks verification and banned/restricted exclusions, but neither browser authority nor a mutation permission.
- Thread vote/unvote invoke it at lines 143 and 169, then write at 160 and 172–175.
- Reply vote/unvote invoke it at lines 354 and 377, then write at 368 and 392–395.
- `backend/src/domains/auth/principal.rs:274–280`: bearer callers bypass browser-origin mutation validation, as intended for nonambient credentials. The missing capability check is therefore decisive.

Scenario: an eligible `self.library.read` credential submits POST/DELETE to a visible nonself thread/reply vote route. It passes the helper and mutates the owner's social votes. The existing GET vote browser gate at community line 124 does not protect these writes.

Owner: catalogue/community. Required acceptance: the same actual issued bearer fails all four vote mutations with no SQL effect, while authorized browser votes/unvotes remain functional. Place the browser-only policy in the shared mutation helper so every caller is covered.

## R2-SEC-03 — P1: configured managed-group keys may be publicly served

Evidence:

- `backend/src/config.rs:163–226`: runtime preflight checks publication operator key, original service key, original storage and backend roots against `ui_dir`, but never checks `protocol_publication.group_keys_dir` or `protocol_publication.native_node.group_keys_dir`.
- `backend/src/protocol/publication.rs:151–160`: the publication runtime opens its configured group key directory.
- Original `../manga-protocol-lab/server/src/node.rs:1000–1010`: the embedded native listener independently opens its group key directory.
- Original `server/src/groups.rs:58–64,90–111`: the key store canonicalizes/creates any configured root and stores a regular `<public-group-id>.key` file, mode 0600.
- `backend/src/router.rs:242–255`: the same process serves ordinary files from the UI tree with ServeDir.

Scenario: either recognized group-key root is accidentally placed inside the public release tree. Existing regular mode-0600 key files pass the UI artifact scan, and the server process can read them. An unauthenticated request for the predictable public group ID's `.key` file receives private signing material. If the UI is writable, group creation can introduce new files after the preflight scan.

Owner: bridge/configuration. Required acceptance: reject both configured group-key roots when they are inside or contain the public UI tree, including canonical/symlink-resolved paths and missing descendants that would later be created. Production group-key roots must be explicit absolute private paths. Demonstrate rejection before opening key stores, workers or listeners, and acceptance of disjoint provisioned paths. Do not relax key permissions or expose them in evidence.

## R2-SEC-04 — P2: media accesses PostgreSQL before admission

Evidence:

- `backend/src/domains/uploads/media.rs:32–37,68–73`: every chapter/cover media request first calls the protocol media selector; only a native fallback later obtains local media permits.
- `backend/src/protocol/media.rs:105–119,121–130`: chapter source selection checks out a DB connection and queries before request/read admission.
- The cover path does the same at lines 176–182,189–198.
- Unknown and ordinary native resource IDs also take those preadmission queries. Router API admission does not wrap the media routes (`backend/src/router.rs:60–82`).

Scenario: while request permits or the bounded live-read queue are saturated, concurrent `/data/...` or `/covers/...` requests still enter PostgreSQL checkout/SQL. There is no admission bound on that first pool-waiter population. The configured checkout deadline bounds its duration, not its size, and these operations are not counted by the platform request drain until later admission succeeds.

Owner: bridge/media. Required acceptance: obtain one request/read admission lease before all source-selection SQL, preserve it through original/native/unknown branches, and retain it through streaming. With permits exhausted, every media branch must reject before DB checkout. Conditional/HEAD/error/cancel paths must release it without double admission or leaks.

## R2-SEC-05 — P2: abandoned expired uploads are never reclaimed

Evidence:

- `backend/migrations/0020_upload_sessions.sql:9`: ordinary staged sessions expire after 24 hours.
- `backend/src/domains/uploads/sessions.rs:451,487,957–960`: expiration and reference deletion run only when that same owner starts another upload/edit.
- `backend/src/domains/uploads/sessions.rs:974–1004`: startup/maintenance cleanup reconciles pending markers and at most 100 already-queued object removals; it never finds expired abandoned sessions.
- `backend/src/runtime.rs:113–123`: the 30-second maintenance worker calls that cleanup plus `/temporary` cleanup, which does not remove staged `upload_files` references.

Scenario: an uploader stages a large chapter and never returns. At expiry the session becomes inaccessible, but its original/derivative rows remain referenced indefinitely. Maintenance cannot reclaim up to 500 MiB of originals plus derivatives per abandoned account. Continued abandoned uploads grow private disk use without the intended lifetime releasing it.

Owner: catalogue/uploads. Required acceptance: startup/background maintenance expires ordinary sessions in a bounded batch without an owner request, removes only their staged references and queues/retries unlink. Preserve active sessions, committed/recoverable chapters, shared objects, and all prepared/source-accepted exact-input commands/pins. Concurrent attach/cancel/publication must serialize safely with expiry. Record real DB/filesystem outcomes in the later system gate.

## R2-SEC-06 — P2: embedded native drain omits cancellation-surviving mutation tasks

Evidence:

- Original `../manga-protocol-lab/server/src/node.rs:716–725,728–758`: EmbeddedWorkerOwner tracks and joins/observes named background workers only.
- Native owned mutation spawn sites include staged publisher upload at 1596–1601, publication at 1844–1849 and signed library commit at 3040–3050. Those tasks deliberately retain admission/ownership after a request handler is cancelled, but are not registered with EmbeddedWorkerOwner.
- Additional owned tasks occur at node lines 2159,2232,2361,2404,2615,2671,3102/3111,3483,3551 and 3624. Audit each site's owned operation, including nested `spawn_blocking`, when adding the tracker; do not assume a background-worker-only registry covers them.
- Original `node.rs:1461–1486` drains the native HTTP listener and serving role. `serve_prepared_with_store:801–814` subsequently terminates only the background worker registry.
- Platform `backend/src/main.rs:137–145,177–182` waits its own `state.requests` and the native listener, then can close the pool and exit successfully. The original task permits/counters are separate and never inspected by that drain.

Scenario: a genuine original signed mutation has started its owned task, its HTTP handler is cancelled/disconnected, and shutdown begins. Network drain and background-worker shutdown may finish while the original owned mutation is still using storage/SQL. The platform's counter is empty, so the process can report a successful drain and then drop the native mutation with the runtime.

Owner: bridge/original lifecycle. Required acceptance: embedded startup owns an in-flight operation tracker covering every cancellation-surviving native operation, retains the operation lease through nested work and durable finalization, closes new admission on shutdown, and waits actual owned-operation termination before successful native listener completion. Wait for operations before aborting infrastructure they require. A deadline must propagate an unsuccessful/uncertain drain, with durable restart reconciliation intact. Preserve standalone-node defaults unless deliberately reviewed. Later system tests must disconnect and block a genuine signed native mutation after its spawn, request shutdown, and prove success waits for termination or the deadline fails truthfully. This report makes no runtime cancellation claim.

## Reviewed controls with no additional substantiated finding in this pass

Source inspection covered the narrow bearer permission/role implementation, disabled/recovery identity handling, expected-account comparison, private draft/cover/upload-preview guards, browser-origin validation and outer duplicate-Origin rejection, effective CSP origin policy, streaming body deadlines, file/decode budgets, retained original/native response permits, report/claim reviewer rules and fresh group eligibility, ordered checksummed migrations, embedded background-worker ownership, release/service/proxy templates, and checkpoint/source-package exclusion mechanisms. This scoped negative result is not proof against other defects.

Deployment documentation correctly keeps genuine mail, identities/admission, installed projection backend, DNS/TLS, private permissions, backup/restore drills and runtime/system verification as operator or testing prerequisites. No production operation or performance claim is approved here. There is also a current operational dependency to document and verify later: `main.rs:58` calls `ProtocolFacade::install_visibility_function`, which performs CREATE OR REPLACE FUNCTION at `protocol/mod.rs:36–47`; a deployment using separate migration/runtime DB roles must explicitly arrange the needed function ownership or eliminate runtime DDL. This was not added as a seventh source finding because the exact intended role provisioning has not been supplied.

## Grounded cleanup guidance, after correctness

Matt Pocock's official [codebase-design reference](https://github.com/mattpocock/skills/blob/main/docs/engineering/codebase-design.md) emphasizes a small interface that hides invariants, ordering and error modes; fixes and tests should cross the same seam. Apply that narrowly here: browser-only social/report access should be an authoritative guard; shared media admission should own source selection plus response lifetime; upload maintenance should hide bounded expiry and pin preservation; embedded native lifecycle should own task admission/drain. Avoid pass-through abstractions or a parallel storage owner. Test observable Router/storage/runtime results before broader refactoring. Performance work remains last.

