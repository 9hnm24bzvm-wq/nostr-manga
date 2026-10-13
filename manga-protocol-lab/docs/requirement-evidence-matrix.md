> **Historical October 8 requirement/evidence matrix.** The preserved report below describes its original revision,
> including any then-pending work, test counts and measurements. It does not
> establish the current reconstructed build's status or capacity. For current
> requirements and evidence use [rebuild status](rebuild/STATUS.md) and the
> [parity contract](rebuild/PARITY-REQUIREMENTS.md). Any “current” navigation
> inside the preserved body refers to that historical report's time.

# Backend requirement and evidence matrix

Status as of 8 October 2026. The initial audit is historical; this matrix tracks remediation without erasing reproduced baseline failures. A passing fixture proves its described contract, not production capacity or all possible failures. Original accepted plans: https://chatgpt.com/space/page_913e894327708191808e721acab6eb40 and https://chatgpt.com/space/page_f36c5f5ff6508191bc28ce4ec482d406 .

| Requirement | Implementation / evidence | Remaining scope |
|---|---|---|
| PostgreSQL authority, deadpool/tokio-postgres, Moka; no SQLx | db.rs; postgres_foundation, account_heads, retention_index fixtures | Target deployment throughput unqualified |
| Immutable original-backed chapter/series publication; delete then upload, no protocol revision links | publication.rs, manifest.rs; publication_fences, http_publication, http_publishers | Rich metadata and retained original covers verified; see catalogue-and-covers.md |
| Optional closed-default reader enrollment | enrollment.rs, enrollment_http.rs; http_enrollment, enrollment_fences; migration52 and restore-policy tests | Owner selectedA; personal-account grants only, bounded signup and no revocation bypass; no production setting enabled |
| Independent admitted publishers, account admission separate | publisher_authority.rs; http_publishers | Private bounded one-off review now verified; see submission_review fixtures |
| Original self-hosting and allowed formats | upload.rs, storage.rs; http_upload, blob_receipts, upload_recovery | No mandatory replication count; alternate renditions outside schema |
| Staged grants, abandoned expiry, safe direct deletion | reclamation.rs, staged_deletion.rs; staging_reclamation, http_staged_delete | Combined regression passes; bounded cleanup controls and all-state FK index verified; sustained GC/write capacity remains unqualified |
| Instance-local moderated protected deletion, no resurrection/cascade | moderation.rs; http_moderation, publication_reclamation | Per-instance policies remain independent |
| Approved-peer synchronization; other peers confer no trust | peers.rs, sync_jobs.rs; http_peers, http_sync_jobs, sync_queue, sync_schedules | Signed endpoint HTTP exchange and relocation pass. Original37B now has a separate read-only NIP42 peer WebSocket interface with current allowlist, revocation and WSS/TLS tests; normal reader policy is unchanged |
| Skip unsupported/policy-rejected items, continue supported work | transfer_rejection.rs; sync_skips, http_sync_skips | Operational failures remain retryable; 256 diagnostic samples per batch |
| One public library per key, selected root/chunk representation | library_manifest.rs, library_storage.rs; library_roots, library_staging, http_accounts, library_ref_index | Explicit-bound-service client delivery, durable outbox and cold recovery now have focused tests; 30k/500k interrupted delivery and full recovery pass; same-origin root relay binding is verified; arbitrary external route adapters remain explicit |
| Public profile, comments/replies, reactions, ordinary author deletion | social.rs, ordinary_deletion.rs; http_accounts, ordinary_deletion | Instance-specific ranking remains outside protocol |
| Groups as instance-custodied ordinary keys | groups.rs; http_groups, group_fences | Offline backup CLI/runbook now verified; local action attribution full regression passes; submission review full regression passes; no required membership export; owner selected clearing/disabling only, no MVP group-deletion endpoint |
| HTTP chapter query with offset/limit, timestamp filters, explicit order, exact optional totals | discovery.rs, filter.rs; http_discovery, capability unit boundaries | Series/title/category discovery verified; query performance at target scale remains unqualified |
| Shared NIP11/HTTP capability schema, formats and budgets | capabilities.rs, protocol_limits.rs; http_capabilities; audit-capabilities-full-regression.log | Registry describes support, not health or capacity; not every internal resource budget is public |
| Service key continuity across domains, remembered first possession proof | service_proof.rs, service_identity.rs, service_announcement.rs, endpoint_policy.rs; http_remembered_identity, http_service_announcements | HTTP resolver verified; approved-peer relocation/periodic refresh verified; optional WebSocket announcement projection is not implemented; required signed HTTP exchange is implemented |
| Standard-shaped blob routes and browser access with scoped auth | blossom.rs, node/blossom_http.rs; http_blossom focused pass | Full combined regression passes; explicitly restricted profile keeps staging private |
| MangaDex migration and client compatibility | Offline import/migration tools; http_migration, http_reader_state, http_group_profile | Content-read, public account/list/progress and mapped signed reader/group workflows pass; see mangadex-compatibility.md and API-INDEX.md. No unchanged-client claim |
| Backup/restore without accidental serving activation | postgres_backup.py; test_postgres_backup.py, http_migration | Foreign-host/unknown-process state intentionally requires offline operator review; typed recovery status and refusal tested |
| Guarded direct serving and exact-generation reader drain | serving_role.rs, serving.rs; http_public_serving, public_export_*, serving_process | Typed fault/restart behavior verified; actual local HTTPS proxy auth checks pass; no production proxy/CDN security qualification |
| 8 billion monthly images, 350 million page views, 100 TB library, 300 million progress saves | Capacity targets recorded; prior failed local runs retained | Not certified. Earlier sustained save tests failed; prepared-query code passes one local ten-minute120/s gate. See query-cache-results.md; production capacity remains unqualified |

## Release gate

The owner settled the catalogue/cover/compatibility choices on8October2026. Implemented reference workflows and deliberate compatibility boundaries are summarized in RELEASE-20261008.md. The extracted archive rebuild passes; release evidence and limitations remain explicit. A source checkpoint is recoverable development state, not a production-ready release. The report's refactor candidates remain valid: smaller HTTP modules, typed errors, supervised roles, fixed-query reuse, signer-agnostic clients and requirement-linked negative tests. Performance-specific optimization follows measured bottlenecks after functional completion.

## Schema51 feature-first follow-through

The reader and managed-group profile workflows pass combined regression and runnable demo checks. Recovery verifies30000/500000-chapter public views from a survivor and original image delivery after source shutdown. Source23/runtime4 were saved and delivered8October2026. Reader enrollmentA and group-lifecycleA were subsequently resolved on8October2026. A further audit identified uncertain group-publication retries as a command-level idempotency gap. The persisted-created_at path and shared native/group duplicate lifecycle pass the full checkpoint24 regression.

## Schema52 decisions and evidence

EnrollmentA (11:48UTC) and group-lifecycleA (12:06UTC) are resolved. Enrollment is optional and disabled by default; group keys/roles/content are retained under clearing/disabling only.69 result groups,89 library units plus one optional stress skip,120 Python tests,Clippy,process/database restart and optimized build pass. The remaining identified original-requirement gap is the approved-peer NIP42 WebSocket interface; existing HTTP peer replication and reader Nostr transport must not be mislabeled as that separate allowlisted path. Full backend completion is not yet claimed.

## Additional original-contract coverage

| Requirement | Implementation and verification | Boundary |
|---|---|---|
| Exact-byte deterministic CBOR, page properties, optional/required extensions | manifest.rs, series_metadata.rs and codec units; http_publication, http_sync_skips | Experimental versioned project schema; no silent transcoding or re-encoding before hash verification |
| Exact series/chapter event identity and no replacement links | publication.rs, library_manifest.rs; http_publication, http_reader_state | Local presentation associations remain outside protocol; no inherited read completion |
| RFC4647 language matching, exact non-language text, absent/unknown Boolean semantics | filter.rs, discovery.rs; http_discovery and filter units | Explicit bounded query grammar; not raw SQL |
| Live offset/timestamp discovery versus stable manifest-aware reconciliation | inventory.rs, sync_jobs.rs; http_inventory, http_sync_jobs | No resumable change-feed or standard NIP77 claim |
| Protected and ordinary deletion, retained evidence, explicit exact-ID restore | moderation.rs, ordinary_deletion.rs; http_moderation, ordinary_deletion, publication_reclamation | Chapter/series deletion defaults to local approval; no cascade or global erase guarantee |
| Separate HTTP and WebSocket peer authentication | peer_authorized/reconcile handlers and peer_only Relay; http_peers, http_peer_websocket, http_tls_proxy | NIP98 HTTP transfer and NIP42 read-only event subscriptions are distinct |
| Founder-loss continuation | http_sync_jobs, http_library_client, http_groups | Source-offline originals/library recovery and new group-signed chapter publication on a surviving logical instance; local fixtures, not full infrastructure-disaster qualification |
| Retryable operational failure versus denied account/group/peer | typed AccountDenied and database failure mapping; http_admission_errors | 503 remains fail-closed but retryable; actual missing/revoked authority remains403 |

The policy choices are closed. Peer transport focused/release/TLS checks and founder-loss publishing pass. Final combined regression after the operational-error correction is running. No untested code should be described as the finished checkpoint until this gate, source integrity and runtime extraction checks pass.


## Checkpoint26 final gate

Checkpoint26 candidate (schema52): the accepted peer-allowlisted NIP42 WebSocket interface is implemented and transient admission-store failures retain retryable503 responses. The combined suite passes71 result groups,89 library unit tests with one optional stress skip,120 Python tests,Clippy and separate-process/PostgreSQL restart checks. Optimized build, local demo and release-binary admission/peer fixtures pass. Enrollment remains closed by default; group lifecycle remains clearing/disabling only. UI, independent security review and target-scale production qualification remain outside this verified backend milestone.

The earlier peer-transport gap is closed by relay.rs/node.rs with http_peer_websocket and http_tls_proxy evidence. The combined gate includes strengthened http_groups founder-loss publication and http_admission_errors database outage/recovery. Database/pool failure remains retryable503 rather than403/400. Account admission and peer permission remain independent. These local tests do not establish independent-machine disaster recovery or target-scale capacity.
