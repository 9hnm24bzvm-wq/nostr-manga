> **Historical October 7 audit follow-through.** The preserved report below describes its original revision,
> including any then-pending work, test counts and measurements. It does not
> establish the current reconstructed build's status or capacity. For current
> requirements and evidence use [rebuild status](rebuild/STATUS.md) and the
> [parity contract](rebuild/PARITY-REQUIREMENTS.md). Any “current” navigation
> inside the preserved body refers to that historical report's time.

# Audit follow-through, 2026-10-07

The delivered audit describes the pre-fix managed-serving/schema35 checkpoint. It remains a historical finding, not a live claim that every listed defect persists. The backend remains incomplete.

## Verified changes

- B1/R1: independent publisher upload, staged reads and publication now use epoch/generation-scoped authority independent of physical ownership. Both signed HTTP and NIP42 WebSocket paths pass real PostgreSQL/strfry tests. Isolation, revocation/re-admission, grant expiry and deduplicated physical ownership are covered. Schema36 is additive. See publisher-authority.md.
- O1: restored configurations explicitly disable managed public image serving and retain recovery settings. Source configuration is unchanged. Eight Python safety tests and the actual node restore fixture pass. Broader ticket29 recovery-state-machine work is still pending.
- R9: added remediation tickets19-30 and an acyclic dependency graph. Historical tickets1-17 cover a subset, not all accepted features. Performance qualification ticket18 is explicitly deferred.

## Evidence

- results/audit-publisher-full-regression.log: combined functional suite, Clippy and PostgreSQL process/restart validation pass.
- results/audit-publisher-regression-2.log and publisher-authority.json: dedicated admitted-publisher workflow.
- results/audit-restore-policy-unit.log and audit-restore-policy-regression.log: restore activation policy.
- results/feature-audit-current.json: current diagnostic observations, not an acceptance suite.
- results/feature-audit-observations.json: diagnostic reproduction against the pre-fix schema35 source checkpoint; provenance in audit-baseline-reproduction.log. Kept separate so later fixtures do not overwrite historical findings.

The baseline reproduction is not a performance comparison. No new benchmark or capacity claim is made.

## Remaining first-priority work

B2 staged expiry and receipt-backed direct deletion; B4 unsupported sync item isolation; B3 standard Blossom/browser interoperability; then full capabilities/service discovery, client distribution, richer discovery, review workflows and MangaDex compatibility. Exact unresolved wire contracts remain explicit.

## Subsequent verified expiry work (schemas37–39)

Abandoned unretained uploads now enter a bounded receipt-backed cleanup queue. Current publisher/account/group leases, held transfers, retained publications/libraries and external readers remain protected. Group staging grants now bind generation and expiry with a fresh upgrade grace. Publication and library peer transfers have600second deadlines and at most660second durable holds, preventing an individual staging clock from expiring mid-transfer. Restore clears copied operation holds and disables new staging cleanup pending review.

The combined functional regression and109 Python tests pass; see results/audit-staging-full-regression.log and audit-staging-python.log. Authenticated direct deletion is still missing, and candidate query cost at production scale has not been qualified. No new load test was run.

## Verified direct deletion (schema40)

The reference publisher and managed-group staging deletion routes now use the same durable reclaim/receipt/refund path. Every ordinary uploader, including the administrator, has an explicit temporary interest. Another active interest, hosted or retired content, reader or transfer hold causes409 without releasing anyone's data. Epoch/action/hash/server checks are enforced. Failed cleanup reports an unconfirmed outcome and retains accounting; cancelling the requesting future does not cancel owned cleanup. Existing source40 full regression and109 Python tests pass (audit-direct-delete-full-regression.log, audit-direct-delete-python.log). Standard Blossom route/profile and browser CORS remain open ticket22.

## Verified per-item synchronization (schema41)

Typed unsupported/policy/invalid rejections now settle a queued item without cancelling unrelated work. Local removal remains binding. Batches explicitly end completed_with_skips; complete aggregate counts and up to256 labelled diagnostic samples are persisted atomically, with truncation disclosed. Operational/transport/storage failures still retry rather than being hidden. Signed library routing no longer needs the root codec just to identify its author. The mixed-peer fixture,300-item accounting fixture and combined functional regression pass. See synchronization-item-outcomes.md and results/audit-sync-full-regression.log.

## Verified unified capabilities (schema41, no migration)

HTTP and namespaced NIP11 share the same whitelisted registry with codec/API versions, endpoint templates, configured formats and role flags, read/admission policies and enforced discovery/library budgets. NIP11 now separates the service key (`self`) from administrator contact (`pubkey`). Key relay/filter/query constants feed both enforcement and advertisements. Real HTTP tests cover metadata-only and storage-enabled nodes, custom limits, root CORS preflight, private-config exclusion and over-budget rejection. The combined functional regression passes; see results/audit-capabilities-full-regression.log. Standard Blossom remains explicitly false in this checkpoint. User chose mandatory server scope for all authenticated blob actions at16:18UTC; implementation follows separately.

## Verified scoped Blossom/browser adapters (schema41, no migration)

Standard-shaped upload, hash lookup/HEAD/range and deletion routes now share the existing staging/retention services. Domain-scoped Base64url authorization is mandatory for authenticated actions, per the user's explicit A choice. New/existing upload status is determined under the mutation guard. Private staging, shared interests and hosted deletion protection remain intact. General application CORS origins are configurable, with successful authenticated library response proofs exposed; mandatory wildcard Blossom/NIP11 discovery is separate. Tests include wrong/missing domain scopes, concurrent deduplication, hash mismatch, shared/hosted delete refusal, disallowed origins, private staging and admitted-reader restart behavior. Full regression and Python tests pass.

This is a restricted interoperability profile: staging URLs are not anonymously public before publication, and unscoped tokens are intentionally rejected. Capabilities disclose these restrictions and do not assert full Blossom conformance. Optional upload HEAD/list/mirror/media extensions are absent. See blossom-profile.md and results/audit-blossom-full-regression.log.

## Offline group-key recovery workflow

The new manga-key-tool verifies expected public identity and creates exclusive owner-only offline backups with sync/readback checks. It never prints secret bytes, contacts the network, accesses PostgreSQL or recreates memberships. The actual two-instance group fixture now uses this tool before recovered-public-key registration and explicit administrator bootstrap. Three focused unit tests, the real group HTTP fixture and the combined functional regression pass (results/audit-offline-keys-full-regression.log). The helper does not encrypt backups or revoke other key holders. Submission review and action-attribution decisions are still open, so ticket27 remains in progress.

## Verified worker lifecycle visibility

Named long-lived workers now report running/returned/cancelled/panicked states through the operator-authenticated status endpoint. Disabled roles are absent. Relay notification drop guards clear readiness, notify subscribers and abort the owned database driver on early exit or cancellation. Lifecycle units deliberately exercise each terminal state; actual HTTP role status and the combined regression pass (audit-worker-lifecycle-full-regression.log:52 passing test-result groups). This is observation and safer cleanup, not automatic restart, progress proof or full controlled service recovery. R3 remains partial. See background-worker-status.md.

## Verified permission-store failure classification

A focused failure injection reproduced403 from both upload profiles when publisher-permission lookup failed in PostgreSQL. Typed publisher denials now distinguish actual authority rejection from operational failure, returning503 for database outages. Staged-read database errors no longer masquerade as404; genuinely absent/ungranted bytes remain hidden. The complete regression passed; before/after observations remain separate in publisher-db-error-before.json and publisher-db-error-after.json.

## Remembered service identity and outbound boundary schema42

The operator verification API proves possession of an expected or first-seen service key before remembering it, never grants peer permissions, and rejects a changed key at a known origin. Actual-node restart and same-key/new-origin tests pass. Automatic destinations use public HTTPS, bounded/pinned DNS answers, no redirects/proxies, and exact configured private overrides. Restores clear those host-specific overrides while preserving source configuration. Full regression and110 Python tests pass. The experimental announcement format and kind11998 were approved at18:28UTC; announcement publication/exchange is the next implementation step, not completed by this identity foundation.

## Signed service announcement exchange and resolution, schema43

Approved experimental kind11998 now carries a bounded, signed endpoint list with deterministic timestamp/lowest-ID replacement and persistent withdrawal. Independent HTTP nodes exchange cached announcements, including through mutually approved peer routes. The operator resolver refreshes from seeds and proves live possession of the target service key at a moved address without granting permissions. Public-HTTPS/pinned-DNS policy and exact development overrides remain enforced. The real multi-node fixture covers original process loss, cold-node resolution, offline withdrawal propagation, rollback, tampering and ties. Full regression passes (54 test-result groups), Clippy with warnings denied, separate-process/database-restart checks,110 Python tests, release build and an extracted-archive library/binary rebuild. Evidence: audit-service-resolution-*.log and http-service-announcements.json.

Automatic changes to approved peer endpoints, periodic discovery and WebSocket/strfry projection are not implemented. Latest-observed signed metadata is not global consensus and cannot reveal an unseen newer withdrawal. No target-scale performance claim is added.

## Local group acting-member audit, schema44

Successful first local publication/deletion-request commits now record the authenticated member and checked permission version in an operator-only audit table. No public event tags or peer inventories expose this attribution. Record failure rolls the action back with HTTP503. Duplicate publication preserves the first actor; failed/revoked operations leave no success record. The audit API has bounded keyset pagination and survives restart. Real HTTP permission, rollback and pagination tests plus group fencing/duplicate tests pass. Final full regression (54 passing test-result groups), Clippy, process/restart checks,110 Python tests, release and extracted-archive rebuild pass. See audit-group-attribution-full-final.log and group-action-audit.md. Submission review remains separate unfinished work; audit storage is not tamper-proof and has no automatic retention purge.

## Signer-agnostic public-library client SDK (schema44 unchanged)

A validated immutable bundle and bounded batch planner now feed independent recipient delivery. Signed NIP65 helpers publish/fetch/select write routes with explicit expected-service bindings. First receipt is separate from remaining outcomes; signed metadata, signer output, full object graphs and replacement ordering are verified. The local public-data outbox persists attempts/outcomes with atomic replacement and fsync, retains unfinished work across bounded retry rounds, excludes concurrent owners, fences late attempts and retires only old fully accepted jobs. An actual killed subprocess verifies pending-state/lock recovery.

Real two-node tests cover first acceptance while a recipient stalls, duplicate delivery, disk reopen/resume without resending accepted destinations, NIP65 rediscovery, old-head supersession, wrong service key, signer substitution, revoked account and cold recovery from a surviving node. HTTP negative fixtures reject tampered signed receipts and corrupt content; older-complete recovery exposes newer-unavailable candidates. Final combined regression passes (56 test-result groups;81 library units passed and1 environment-gated unit skipped), Clippy, separate-process/PG restart checks,110 Python tests, release build and extracted archive rebuild. See audit-library-client-full-regression.log and library-client-delivery-contract.md. Initial fixture bootstrap omissions and the reproduced lock-contention test failure are retained with corrected passing logs.

This is a Rust SDK/host integration layer, not an end-user website or browser/mobile storage adapter. Automatic NIP65 route-to-service binding and interrupted large multi-batch delivery remain unqualified in this checkpoint. Relay-list propagation is a separate helper, not yet a durable library-job dependency. No new load/capacity certification is claimed.

## Interrupted multi-batch reader recovery qualification

The actual HTTP client fixture now covers30,000 chapters across65 objects (1,267,344 canonical bytes) and500,000 chapters across257 objects (21,042,147 bytes). It stages only the first batch (64/50 objects), verifies that the public head did not advance, reopens the persistent queue, resumes the same signed save, then cold-recovers and decodes every chapter record. Both pass. The final combined regression, Clippy and independent process/database-restart checks pass (audit-library-client-large-full-final-exit.txt:0). The earlier interrupted combined-run log is retained without claiming completion. The source archive rebuild runs library/binary tests and compiles the large HTTP fixture; application source is byte-identical to the already verified SDK release build. These are single functional scenarios, not load/throughput/latency certification.

## One-off admitted-account review, schema45

Private bounded drafts, exact declared-original grants, operator approval/rejection, owner withdrawal and expiry are implemented. Approval atomically commits the decision and hosting/outbox without granting publisher permission. Pending/shared interests block cleanup; rejection releases only its own grants, and rejected IDs cannot be resurrected by another publication path. Eight approve/reject races each commit one decision. Expanded real-HTTP tests include submitter moderation refusal, undeclared hashes, revoked uploads, injected database failure and restart. The complete regression passes58 test-result groups, Clippy, independent-process/database restart checks and110 Python tests. See submission-review-proposal.md and audit-submission-review-full.log. Records have a configured intake cap, no automatic audit purge; capacity remains unqualified.

## Automatic client binding and approved-peer relocation, schema46

The client now discovers compatible library APIs for root NIP65 routes with live service-key proof and remembered-key fencing. Unsupported non-root routes remain explicit; application persistence of first observations and arbitrary third-party adapters are outside the helper. The local-only executable demo is now integrated into the full source.

The instance worker refreshes signed announcements for existing approved peers and conditionally relocates their endpoint. Revocation, newer withdrawal or operator changes during live proof fence the update. Learned destinations use the stricter DNS-pinned outbound policy on subsequent requests, avoiding reliance on a one-time destination check. Actual replication, late-proof races and founder-address loss pass. The complete schema46 regression passes60 result groups, Clippy and process/database restart checks. See service-identity-remembrance.md and results/audit-discovery-integration-full.log. WebSocket projection of announcements remains unimplemented; configured HTTP discovery and replication do not require it.

## Recovery diagnostics and TLS boundary, schema46 unchanged

Serving state now includes a bounded typed fault category, preserving the cause even when shutdown is unconfirmed. Live helper loss and failed namespace withdrawal are distinguishable without weakening reader holds or automatic reactivation rules. Restore explicitly pauses peer discovery as well as sync. The process supervisor still refuses foreign boot/PID-namespace state; no timer clears ownership or holds.

A real nginx HTTPS reverse-proxy fixture uses a temporary CA trusted only by its test client. Default TLS trust rejects that certificate. External-origin NIP98 writes and signed identity proof work; replay, body substitution, forged forwarding headers and internal-origin signatures are rejected. Initial fixture readiness incorrectly accepted a502 response; the corrected fixture waits for a successful upstream response and passes, with the failed log retained. This is local functional TLS evidence, not production certificate lifecycle qualification.

The combined regression passes61 result groups (83 library units passed,1 optional stress unit skipped), Clippy and independent-process/PostgreSQL restart checks.110 Python tests, the optimized build and integrated two-node demo pass. See audit-recovery-tls-full.log and related evidence. Catalogue/cover/compatibility choices still gate feature completion.

## Rich catalogue and original covers,8October2026/schema47

The owner approved richer optional metadata, one original-backed series cover and staged MangaDex compatibility. Typed localized/alternative titles, descriptions, free-form categories, credits, language/status/year and optional rating/demographic fields are indexed without changing immutable event IDs. Required cover feature markers make older decoders skip unsupported cover-bearing series. Local/group/review publication and peer transfer require verified local cover bytes; shared chapter/cover images survive independent series deletion. New public catalogue routes obey read policy; operator-only legacy-index diagnostics do not grant publishing authority. Optional textual volume/title predicates now work in chapter queries and synchronization filters.

The62-group combined regression, Clippy, independent-process/PostgreSQL restart and112 Python tests pass. Additional focused tests cover derived-index rebuild on restart, authenticated catalogue access, private operator errors and one-off reviewed covers. Reviewed offline import accepts verified local covers and refuses hotlinks/unmatched source IDs. Compatibility facade implementation follows; no UI or target-capacity claim.

## Documented compatibility profile, schema48

MangaDex-shaped content reads now use explicit local UUID aliases while preserving signed Nostr identities and immutable originals. Queries, entities, feeds, covers, authors, categories, aggregation and mapped group/individual publishing have actual-node coverage. The operator-reviewed alias helper uses pinned service identity and signed receipts; it does not publish history or rewrite content. The account/list/progress workflow remains the native signed API/SDK, explicitly documented in the supported-client/endpoint matrix. This is not every MangaDex endpoint or unchanged-client certification.

The final combined regression passes62 result groups (86 library units and one optional stress skip), Clippy and independent-process/PG restart checks.115 Python tests, optimized build and demo pass. A broader first run caught an unreadable-original response changing from404 to503; typed verification failure restores that contract while database failures remain503. Image response leases stay held while streaming, and selected-page metadata has SQL-side byte-budget preflight. See compatibility-full-final.log and RELEASE-20261008.md.
