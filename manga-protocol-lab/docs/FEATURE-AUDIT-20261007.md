> **Historical October 7 baseline audit.** The preserved report below describes its original revision,
> including any then-pending work, test counts and measurements. It does not
> establish the current reconstructed build's status or capacity. For current
> requirements and evidence use [rebuild status](rebuild/STATUS.md) and the
> [parity contract](rebuild/PARITY-REQUIREMENTS.md). Any “current” navigation
> inside the preserved body refers to that historical report's time.

> Historical baseline audit from7October2026. The reproduced gaps below drove subsequent fixes. For current status and remaining limits, use [the requirement matrix](requirement-evidence-matrix.md) and [checkpoint26 release evidence](RELEASE-20261008.md). Do not treat baseline missing-feature statements as current defects.

# Federated manga backend audit

Feature gaps bugs and refactor priorities

Prepared by dot • 7 October 2026 • Backend and protocol scope

## Conclusion

The backend is not feature complete. The earlier completion statement covered a narrower implementation ticket graph, not every accepted requirement in the original protocol plan. The original plan explicitly includes capabilities that are still absent, and new probes reproduced functional gaps that the existing tests did not cover.

The implemented foundation is substantial: PostgreSQL with deadpool-postgres/tokio-postgres and Moka; verified originals; immutable publications; moderation; approved-peer replication; public chunked libraries; social events; managed groups; and guarded original serving. Those working pieces should be retained. This audit recommends targeted completion and refactoring, not a rewrite.

Performance tuning is paused. The two already completed load failures are retained as evidence and inform refactor candidates only. Website design, anime streaming, private libraries and paid or public deployment remain outside this audit.

## Priority summary

- First: make admitted individual publishers work, complete abandoned-upload expiry and direct deletion, and isolate unsupported sync items instead of failing an entire batch.

- Next: finish Blossom and browser interoperability, NIP-11 capability discovery, signed endpoint continuity, multi-relay library delivery, and the promised MangaDex compatibility surface.

- Before another completeness claim: test those contracts through real client workflows, review restore activation defaults, and replace the narrow ticket checklist with a requirement-to-test matrix.

## How to read the findings

P1 means a blocker to the agreed feature-complete backend or its dependable operation. P2 means an important operational, security or maintainability improvement. “Reproduced” means a local diagnostic executed the behavior. “Source-confirmed” means the path is present in code but the complete failure scenario was not injected. “Open contract” means a decision or exact wire definition is still needed. No unauthorized-access exploit or data-corruption exploit was demonstrated by this audit; that is not a security certification.

## Feature coverage

| Area | Status | Coverage and limitation |

| --- | --- | --- |

| Original storage | Working core | Hash/size/format validation, local retention, durable receipts and reader holds. Abandoned staging lifecycle is incomplete. |

| Publishing | Partial | Operator and managed-group paths work. Admitted independent publishers remain blocked on ordinary upload/publication routes. |

| Protected deletion | Working core | Local chapter/series approval, persistent tombstones, shared retention and no series cascade. |

| Discovery | Partial | Chapter offset/time queries and bounded Boolean filters work. Series/title/category discovery is not complete. |

| Approved replication | Working core | Pull transfer, fallbacks, Negentropy snapshots, durable jobs and recurring schedules. Unsupported-item isolation is incomplete. |

| Service identity | Partial | Expected-key proofs and response binding work. Endpoint announcements and automatic first-key remembrance are missing. |

| Capabilities | Partial | Basic NIP-11 and a separate feature response exist. The agreed namespaced versions/endpoints/formats/limits contract is absent. |

| Public libraries | Working core | Selected C root/chunk storage, replacement/CAS, reads, retention and approved-peer replication work. |

| Client distribution | Missing | No complete NIP-65 multi-write-relay client delivery, per-recipient retry/status or fresh-device recovery workflow. |

| Social accounts | Working core | Profiles, plaintext comments/replies and reactions with ordinary author deletion. Ranking and deduplication policy remain instance-local. |

| Managed groups | Partial | Custodied key signing, scoped members, revocation and recovered-key registration work. Backup usability and action attribution need completion/review. |

| Blossom and browsers | Partial | Custom Blossom-style routes exist. Standard upload/delete behavior and cross-origin authenticated HTTP are incomplete. |

| MangaDex migration | Partial | Reviewed supplied-export import works. It is not a MangaDex-compatible API for existing clients. |

| Serving and recovery | Partial | Managed nginx startup/policy fencing and offline restore work. Restart-after-fault and host-move recovery remain restrictive. |

| Deployment capacity | Unproven | Local load gates failed. This is separate from the feature-completeness work now prioritized. |

## Confirmed functional gaps

### B1 Admitted individual publishers are still blocked

P1 • Reproduced

An identity explicitly admitted in both account_permissions and publisher_permissions received HTTP 403 for upload and publication. The upload handler requires the configured administrator; publication uses operator authentication and rejects other authors. The lower-level local publication path also equates publisher identity with the physical staging owner. Merely removing one HTTP check would therefore be an unsafe and incomplete fix. Add a version-fenced publisher permit and per-publisher upload grants independent of physical storage ownership.

Evidence: results/feature-audit-observations.json; server/src/node.rs blob_actor and publish_event; server/src/publication.rs prepare_inner.

### B2 Expired abandoned uploads are not collected

P1 • Reproduced

A successfully stored, never-published blob remained present after its staging expiry was moved into the past in the fixture. Recovery and enabled library/publication cleanup selected zero candidates; no GC item existed. Current expiry timestamps protect other cleanup paths but do not schedule these abandoned available blobs for collection. This can consume the small-VPS staging/physical budget indefinitely. Add a bounded expiry queue and receipt-backed cleanup, preserving active grants, hosted references and in-flight readers. Group staging grants also need an explicit lifetime/release policy.

Evidence: results/feature-audit-observations.json; server/src/blobs.rs finish_blob; server/src/upload.rs recover_page and reclaim_retired_page; server/migrations/0011_group_authority.sql.

### B3 Standard upload deletion and browser contracts are absent

P1 • Reproduced

PUT /upload returned 404; DELETE on the existing blob route returned 405; an authenticated-HTTP browser preflight to /libraries returned 405 with no allow-origin header. The code exposes PUT /upload/{hash} and custom retrieval routes, so Blossom-style authentication is not the promised interoperability layer. Accepted decision 74A also requires authorized deletion of unreferenced uploads. Implement an explicit BUD subset plus a configurable CORS policy, including access to response-proof headers. Keep chapter/reference protection on every delete path.

Evidence: audit observations; server/src/node.rs router; official BUD-01, BUD-02 and BUD-12. The BUD subset is still an open wire-contract detail.

### B4 Unsupported publications can poison a synchronization batch

P1 • Source-confirmed

Unsupported schema/required-feature and local format errors propagate out of synchronize_fenced. The worker retries the same failed item; after five failures retry_sync fails the batch, cancels its remaining partitions and deletes their pending item records. This conflicts with the agreed rule to skip unsupported publications while continuing others. Introduce typed outcomes for unsupported, policy-rejected, invalid, retryable and admitted items. Preserve per-item reasons and make partial/skipped completion explicit rather than pretending every advertised item was hosted.

Evidence: server/src/peers.rs synchronize_fenced; server/src/sync_jobs.rs quantum and retry_sync; accepted decision 46A. A mixed supported/unsupported peer fixture is still required.

## Agreed features omitted from the ticket graph

### F1 Signed endpoint discovery and remembered service identity

P1 • Missing implementation

Expected-key verification works, and manual peer endpoint changes are fenced. The accepted plan also requires a replaceable service-key-signed endpoint list, peer caching/exchange, latest-timestamp and lowest-ID ordering, and automatic remembrance of a first possession-verified key when no expected key is supplied. Those discovery and persistence paths are not implemented. A known endpoint going away still needs manual location knowledge. Define URL safety, replay/staleness rules and first-contact limitations before implementing them; a new key must never inherit old peer permissions.

Evidence: Original protocol draft Service continuity and discovery; decisions 67C,68A,83A; server/src/peers.rs set_peer and verify.

### F2 Namespaced capability and limit advertisement

P1 • Reproduced partial implementation

The NIP-11 response has basic relay limits and supported NIPs, but no manga namespace advertising endpoints, schema/API versions, original formats and application/query limits. A separate feature-boolean endpoint does not provide the accepted contract. Build both representations from one capability registry so advertisements cannot drift from actual handlers and configured budgets.

Evidence: decisions 65D,66A,85A; recorded NIP-11 response; server/src/node.rs root and capabilities.

### F3 Multi-relay client library delivery and recovery

P1 • Missing client workflow

The server stores and replicates kind 10002 metadata, and approved-peer library replication works. That does not implement the other required path: a user-controlled client discovers selected write relays, sends the same signed library plus required objects, completes the initial write after one acceptance, retries remaining recipients and reports each outcome. Fresh-device recovery from surviving write relays is also not supplied as a complete client workflow. The later C chunk format supersedes the old inline format; it does not cancel these distribution requirements.

Evidence: Original draft Public accounts and library synchronization; decisions 55C and 56A; server/src/accounts.rs; no matching shipping client delivery/recovery component.

### F4 MangaDex API compatibility

P1 • Missing launch feature

The supplied-export migration tool preserves reviewed records, originals and exact-ID mappings, with explicit consent for public history. The accepted launch scope also includes API compatibility for browsing/search/chapter/images, accounts/lists/progress and publishing/group management. Those endpoints and auth/ID translations are not implemented. Define a concrete supported-client/endpoint matrix and test it. Never reuse MangaDex credentials, infer group control from a name, or publish private legacy data implicitly.

Evidence: Original draft MangaDex migration and compatibility; docs/postgres-migration.md explicitly disclaims a drop-in API facade; current router.

### F5 Richer metadata and public series discovery

P1 • Partial feature

Series metadata bytes can preserve optional descriptions, categories and import sidecars, but current series validation is principally version/required-feature/titles. Discovery selects chapter events and only indexes series, publisher, language, chapter, page count, size and timestamp. The category query probe returned 400. Free-form categories, public series/title search and the metadata needed by the compatibility layer still need typed/indexed contracts. Cover references and remaining metadata validation were explicitly open; do not invent their final schema or claim unknown optional fields are automatically invalid.

Evidence: decisions 78A–80C; server/src/manifest.rs series; server/src/filter.rs field; server/src/discovery.rs discover; src/mangadex_migration.py series_packet.

### F6 Submission review and group administration workflows

P1 or P2 by workflow • Partial

The reference plan calls for validated approved publishers/peers and review of other submissions. The node currently rejects unadmitted personal submissions rather than supplying a bounded review inbox. Group member permissions and revocation are implemented, and importing a group only registers a public key whose protected secret file is already local; there is no HTTP private-key import. Designated administrator offline backup is presently an operator file-copy/runbook concern. Complete that usable workflow and decide action-attribution requirements. Portable membership export is not missing scope: decision 50D explicitly rejected requiring it.

Evidence: Original plan admission and group sections; server/src/groups.rs; node create_group/import_group and admission handlers.

## Operational risks and test gaps

### O1 Restore activation defaults need an explicit policy

P2 • Source-confirmed risk

The restore helper resets sync, reclamation and auditing, but carries a source public_serving.enabled=true into the restored configuration. Restore itself does not start a service, and later serving still reconciles before activation. Nevertheless, the new role is not included in the explicit re-enable-after-review convention. Prefer disabled-on-restore and add a managed-serving backup/restore fixture. Also test path/port changes and source-reader shutdown obligations.

Evidence: src/postgres_backup.py restore configuration rewrite; existing restore fixture predates an enabled managed-serving configuration.

### O2 Safe shutdown is not automatic service recovery

P2 • Documented limitation

The direct-serving role deliberately remains closed after a fault until restart. Any retried export currently stops the role, including potentially benign visibility races. This is safe but coarse. Foreign boot/PID-namespace/physical-root records require offline review. Add typed failure handling and a controlled recovery state machine after the core missing features; do not clear holds or reopen merely because a timer expires.

Evidence: server/src/serving_role.rs; server/src/export_queue.rs; docs/managed-public-serving.md.

### O3 Broader operational and security assurance remains open

P2 • Test and hardening gap

Individual modules have strong local race/crash tests, but no independent security audit, production reverse-proxy/TLS fixture or complete cold-device founder-loss exercise covers endpoint discovery and multi-relay recovery. Key files are protected local files, not a vault. Signed response proofs authenticate bytes but are not TLS-channel binding. Multi-process metadata coordination is tested; distributed API-to-storage routing and alternate object-store backends are not complete deployment products.

Evidence: existing HTTP/service-proof/backup/serving fixtures; docs/scale-foundation.md; node role startup and local FilesystemVolume.

## Refactors worth doing

### R1 Separate publishing authority from storage ownership

Do with B1. Introduce one reusable PublisherPermit/grant flow with final epoch checks. Share the domain operation across HTTP, WebSocket and group adapters. Keep physical mutation ownership single and explicit.

### R2 Unify blob lifecycle transitions

Do with B2/B3. Model writing, staged, retained, quarantined, retiring and reclaiming states through a common service. One protection calculation must cover personal/group grants, publications, libraries and external readers. Expiry alone must never authorize a refund.

### R3 Use typed errors and supervised background roles

Do with B4 and operational gaps. Replace generic string-driven failure propagation with typed dispositions and stable API errors. Give backend, sync, audit, cleanup and serving tasks explicit lifecycle/status/shutdown ownership instead of scattered detached spawns.

### R4 Split the HTTP module by domain

Do incrementally. node.rs is about 2,835 lines. Separate transport/authentication, publication, account/library, group, peer and operator routers; keep authorization/business rules in shared services, not copied endpoint checks. Add contract tests before moving behavior.

### R5 Create one capability and schema registry

Do with F2/F5. Derive namespaced NIP-11, HTTP capabilities, validation limits and client discovery from a common definition. Add an explicit experimental wire version and compatibility matrix; do not silently claim registered Nostr allocations.

### R6 Centralize queries and transaction boundaries

Do alongside features. Name fixed SQL operations, use bounded prepare_cached usage for fixed shapes, and share a typed filter AST. Keep dynamic filters parameterized. Review long lock scopes, but never remove the public-link/moderation fence simply to reduce latency.

### R7 Consolidate client protocol code

Do with F3/F4. Supply a signer-agnostic client layer for NIP-98/BUD tokens, expected-key proofs, library batching, relay delivery and recovery. Keep private signing keys user-controlled and preserve per-recipient failures. Avoid growing unrelated bespoke clients.

### R8 Profile before performance-specific changes

Defer implementation until feature gaps close. Candidates include fair foreground/cleanup admission, repeated validation/database work, immutable-data caching and the native backend adapter. The current failed trials identify symptoms, not a measured causal bottleneck. Preserve fsync, verification and authority checks.

### R9 Replace completeness claims with traceable evidence

Do now. Maintain accepted requirement → wire contract → implementation → positive/negative/failure tests. Archive the historical SQLite coverage as history. Reopen the overall completion gate and add the omitted feature tickets before calling the backend complete again.

## Existing performance evidence

These runs finished before the renewed feature-first instruction. No further baseline comparison or tuning is part of this audit. Both offered 43,200 saves at 120/s for 360 seconds across 128 synthetic readers, including two 500k histories, with cleanup enabled and client signature verification. Both failed the provisional gate.

| Setting | Accepted | HTTP 429 | Elapsed | Scheduled p95 |

| --- | --- | --- | --- | --- |

| Conservative limits | 42,529 | 671 | 532.5 s | 157.3 s |

| Wider limits | 43,079 | 121 | 434.1 s | 68.5 s |

All recorded errors were library_busy/work_queue_timeout. Projection eventually drained, and the final accounting checks found zero budget-conservation failures. However, cleanup still had 19,180 and 15,877 due objects respectively. This does not establish sustainable cleanup or the 300M/month target. The shared 9-visible-CPU laboratory, overlay filesystem, finite histories and client backlogs are material limitations. No cause is attributed solely to PostgreSQL, the selected chunk format or a recent commit.

## Recommended completion order

1. Publisher permits and per-publisher staging grants; prove a non-admin upload/publish/revoke workflow.

2. Abandoned-upload expiry and authorized direct deletion; prove reference protection, cancellation and exactly-once refund.

3. Typed per-item synchronization outcomes; prove supported items continue past unsupported schema/format items.

4. Blossom/CORS and namespaced capabilities, then endpoint announcement and first-contact persistence contracts.

5. Multi-relay library client delivery and fresh-device/founder-loss recovery with one relay unavailable.

6. Metadata/search and the explicit MangaDex compatibility matrix, including account/group authority and migration cases.

7. Group backup/review workflows, restore activation policy and cross-feature recovery tests; then reevaluate completeness.

8. Only afterward resume performance diagnosis and target-infrastructure validation.

Keep the settled constraints: immutable delete-and-upload publications, instance-local associations/moderation, original-byte retention, one public library per key, no private-library hierarchy, no mandatory replica count and no UI/transcoding scope expansion.

## Evidence and limits of this audit

Code baseline: managed-serving schema 35 source checkpoint, SHA256 1c560cb08905c492288a7623bf98f965aeb238ab67e6abe94695f545c0b1eb62. Subsequent local changes used here are diagnostic tests and benchmark-client verification, not fixes for these findings.

Existing functional evidence: results/serving-role-full-final-2.log (43 test-result groups),107 Python tests and a source-archive rebuild. New evidence: server/tests/audit_gaps.rs and results/feature-audit-observations.json. The diagnostic fixture reports gaps; its successful execution must not be counted as passing the missing feature contracts. Staging expiry was advanced by fixture SQL rather than a wall-clock wait.

The original planning documents contain obsolete inline-library language; the later explicit C decision supersedes that format. Other accepted requirements, such as multi-relay delivery, remain applicable. Exact wire choices still marked open require a short contract decision, not an invented claim of compatibility.

[Original protocol planning decisions](https://chatgpt.com/space/page_913e894327708191808e721acab6eb40)

[Original protocol draft and accepted scope](https://chatgpt.com/space/page_f36c5f5ff6508191bc28ce4ec482d406)

[Blossom BUD 01 retrieval](https://github.com/hzrd149/blossom/blob/master/buds/01.md)

[Blossom BUD 02 upload](https://github.com/hzrd149/blossom/blob/master/buds/02.md)

[Blossom BUD 12 ownership and deletion](https://github.com/hzrd149/blossom/blob/master/buds/12.md)

[Nostr NIP 11 relay information](https://github.com/nostr-protocol/nips/blob/master/11.md)

[Nostr NIP 65 write relay metadata](https://github.com/nostr-protocol/nips/blob/master/65.md)

External standards were checked on 7 October 2026. This is a first-party requirements/code audit with focused probes, not exhaustive formal verification, an independent security review or a production readiness certificate.
