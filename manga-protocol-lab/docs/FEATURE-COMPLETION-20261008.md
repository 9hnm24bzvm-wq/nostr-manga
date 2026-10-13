> **Historical October 8 feature follow-through.** The preserved report below describes its original revision,
> including any then-pending work, test counts and measurements. It does not
> establish the current reconstructed build's status or capacity. For current
> requirements and evidence use [rebuild status](rebuild/STATUS.md) and the
> [parity contract](rebuild/PARITY-REQUIREMENTS.md). Any “current” navigation
> inside the preserved body refers to that historical report's time.

# Feature completion follow-through, 8 October 2026

The owner reiterated features first at04:35UTC. Further performance experiments are paused. Keep the already verified checkpoint22 implementation; do not undo completed correctness-preserving work or start another throughput experiment. The prior narrow reference-profile completion statement did not establish every accepted reader workflow.

Original accepted scope includes API compatibility for accounts, lists and progress as well as content and group publishing. The checkpoint22 facade redirected these reader workflows to the native SDK, leaving the promised convenience interfaces incomplete. Schema50 now supplies these interfaces; the combined final regression is pending. Complete verification while retaining user-held Nostr signatures, public single-key libraries, exact publication IDs, local read policy and explicit query budgets. Do not add MangaDex credentials, private libraries, unsigned server-authored reading state or website design.

## Active feature slice

- Bounded JSON views of existing public library families, with explicit signed-head identity and stale-snapshot rejection.
- Public account/profile reads and authenticated current-user identity through existing NIP98 verification.
- Public list enumeration/detail/member access and series/chapter reading-state reads.
- Explicitly mapped signed account/library writes through the same native authority and storage pipeline.
- Actual HTTP tests for empty/populated accounts, list creation/rename/removal, completion/unread changes, stale snapshots, access policy, replay rejection, shared identities and unavailable data.

A derived convenience view is not a new authoritative library representation. Preserve raw signed roots/chunks and expose their provenance; clients needing independent end-to-end verification continue to verify those exact bytes. Query limits must fail explicitly rather than silently discard records.

## Following checks

Reconcile group management/profile operations and a combined founder-loss/recovery workflow against the original accepted plan. Open semantics must be named rather than hidden behind a narrowed completeness claim. UI remains a later phase. The feature matrix and deliverable status will be revised only after the corresponding workflow tests pass.

## Progress at05:50UTC

Reader-state HTTP workflows pass, including public profiles/lists/progress, signed mapped writes, stale/deleted-head conflicts, denied replay, wrong signer and admitted-read-policy checks. Managed group public profiles pass administrator/publisher distinction, compare-and-set, atomic private audit failure, revocation, disabled instance group and metadata clearing checks. Capability advertisements and endpoint documentation now describe these additions.115 Python tests pass.

The combined PostgreSQL/strfry regression is running. Its existing cold-recovery fixtures are extended to read first/tail/empty pages of30000/500000-record libraries from a surviving node, and publish a profile using a recovered group key with explicitly bootstrapped local authority. These are correctness checks, not load experiments.

Reader enrollment remains a pending owner choice (configurable closed-default recommended); no self-registration default was changed. Group-profile/record deletion semantics remain separate from clearing metadata. Neither pending choice is silently marked done.

## Verified milestone at05:57UTC

The combined regression passes66 result groups, including88 library unit tests with one optional stress test skipped. Clippy,115 Python tests and separate-process/PostgreSQL restart pass. The survivor follow-up verifies both exact originals and library chunks after source shutdown. The optimized application and enhanced two-instance demo pass reader progress/identity and group profile workflows. Extracted-source library/binary tests pass. New source/runtime packaging is next; pending enrollment and group-deletion choices remain explicit.

The checkpoint23 runtime archive was split, reassembled, extracted into a fresh directory, verified against its checksum inventory and ran the enhanced two-node demo successfully. The source archive inventory and application-byte match are verified. Saving these deliverables is the remaining packaging step.

## Subsequent feature audit

The current native API guide was still a historical Ticket01–10 document. A checked index now covers all120 declared paths and132 explicit method/path pairs, links current feature contracts, and labels legacy notes as historical. Four added checks detect missing/dynamic/duplicate route declarations, stale generated inventory and broken local contract links;119 Python tests pass.

An uncertain managed-group publication retry could previously pick a new timestamp and create another event. A backward-compatible `created_at` field now preserves the caller's pre-saved Nostr command identity. Focused tests pass delayed retries after grant expiry, fresh HTTP auth, unchanged first-actor attribution, permission revocation and no resurrection after deletion. Omitting the field retains legacy behavior; this is an opt-in retry-safe contract, not a global exactly-once promise. Combined regression is running.

The retry correction was moved into the shared publication lifecycle rather than kept as an adapter-specific bypass. Both native and group commands now use the same authority/generation/local-removal checks for durable duplicate acknowledgment. New events still require unexpired grants. The group timestamp field remains optional and backward-compatible. Final regression is running against this refactor; checkpoint23 remains the latest delivered working artifact.

Final shared-lifecycle checks pass at06:27UTC:67 functional result groups,88 library unit tests plus one optional stress skip,119 Python tests,Clippy,process/database restart,release build,enhanced demo,release-binary retry fixture and extracted-source library/binary rebuild. Native publisher retries after invalidated grants are exercised separately from fresh publications, which remain denied without new grants. Checkpoint24 packaging is underway.

Checkpoint24 runtime reassembly, fresh extraction, checksum inventory and extracted enhanced demo pass. The final archive contains current source and a self-contained checked API index. Source and runtime Library replacements are the next delivery step.

## Schema52 enrollment and group decisions

The owner selected configurable self-registration disabled by default at11:48UTC, then selected profile clearing/operator disabling with no group-deletion endpoint at12:06UTC. Both choices are implemented. New admission grants only personal-account permission; replay/body checks, concurrent duplicate epochs, creation caps, legacy/rolling-upgrade denials, revocation races and budget rollback are tested.69-group regression,89 units+1optional skip,120 Python tests,Clippy,process/PostgreSQL restart and release build pass.

An audit against the original accepted planning pages identified a separate37B transport gap: approved peers have NIP98 HTTP replication but no peer-allowlisted NIP42 WebSocket interface. Add that explicit path without weakening the ordinary reader-policy gate. This keeps the completion gate tied to original requirements rather than another narrowed reference milestone.

Checkpoint25 source-library/binary rebuild and runtime split/reassembly, fresh extraction, checksum inventory and enhanced signup demo pass. Enrollment remains disabled in the production example. Both approved policy choices are settled; ticket35 peer WebSocket transport remains the next original-requirement task.


## Checkpoint26 final code gate,13:38UTC

Checkpoint26 candidate (schema52): the accepted peer-allowlisted NIP42 WebSocket interface is implemented and transient admission-store failures retain retryable503 responses. The combined suite passes71 result groups,89 library unit tests with one optional stress skip,120 Python tests,Clippy and separate-process/PostgreSQL restart checks. Optimized build, local demo and release-binary admission/peer fixtures pass. Enrollment remains closed by default; group lifecycle remains clearing/disabling only. UI, independent security review and target-scale production qualification remain outside this verified backend milestone.

Original37B is covered separately from59A HTTP federation: peer AUTH binds to the exact endpoint and current allowlist, EVENT writes are denied, idle unauthenticated connections expire and revocation closes the stream. Real nginx WSS verifies the external endpoint, certificate validation and rejection of forged forwarding scope. Founder-loss recovery now stops nodeA before the recovered group publishes a new original-backed chapter on nodeB. Transient account/group/peer permission and metadata database errors return503; actual revoked/missing permission stays403. Focused real-database outage/recovery tests and final release-binary probes pass.

Evidence: backend-mvp-final-full.log, backend-mvp-python.log, backend-mvp-release.log, backend-mvp-runtime-demo.log, backend-mvp-release-admission.log, backend-mvp-release-peer.log and their zero exit records; peer-websocket-tls.json, admission-errors.json and postgres-http-groups.json. Packaging/recovery verification follows this code gate.


## Checkpoint26 package recovery verified

Fresh extracted-source library/binary tests pass. The runtime was split into five parts, reassembled in a fresh directory, checked against its complete checksum inventory and ran the two-instance demo successfully. See backend-mvp-archive-rebuild.log and backend-mvp-runtime-recovery.log with zero exit records. Source application bytes match the final tested checkout. Library save/delivery receipts are tracked separately; downloading saved Library bytes remains unverified because the prior materialization request was scope-denied.


## Owner-added local-first browser client,14:00UTC

The default client library now lives locally in IndexedDB without account/signing/network. Publication is separately opted in and remains a public snapshot under one key. Browser modules/reference UI implement import/export,multi-tab revision fences,external signer integration,pinned service proofs,durable explicit retries/cancellation and verified recovery.12 JS/model tests and an actual PostgreSQL/strfry client-module fixture pass; full backend regression now passes72 groups,89 library units+1optional skip,Clippy/restart and120 Python tests. Rendered-browser tests have NOT passed: local browser launch and supported localhost preview were blocked. Private static preview authorization was requested and is still pending. No deployment performed. See browser-client/README.md and browser-client-plan.md. The30-minute capacity run was interrupted for this request; itspartialmetrics are not a pass.
