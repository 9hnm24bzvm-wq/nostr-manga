# Federated Manga PostgreSQL Foundation Specification

Status: implementation specification for the agreed reference software; unresolved wire defaults remain explicitly open. Updated 6 October 2026.

## Problem statement

An instance operator needs a federated manga service that starts on a small VPS but can grow without replacing its core architecture. The reference implementation must own the originals it offers, replicate only through approved peers, preserve author signatures and local moderation independence, and support a public account/library model. The target is at least 100 TB of originals,8 billion monthly image requests and350 million monthly page views. Existing SQLite experiments are useful behavioral evidence, but are not the selected foundation.

## Solution

Build the reference node with PostgreSQL, deadpool-postgres/tokio-postgres and Moka. Keep API handling, blob mutation, validation, replication and strfry projection as separable roles that can initially run together. PostgreSQL is authoritative for metadata and decisions; caches are derived. Original serving uses operator-controlled storage and nginx/CDN delivery, without a database lookup on the ordinary public image path. The protocol remains Nostr-based, with external hash-addressed manifests and approved-peer transfer.

## User stories

1. As an operator, I want the same architecture on my 200 GB pilot and larger deployments, so that growth does not require replacing the foundation.
2. As an operator, I want multiple API/worker processes to coordinate correctly, so that one process's memory or mutex is not the authority.
3. As a publisher, I want an authenticated original upload verified by hash and size, so that stored bytes match my publication.
4. As an operator, I want durable space reservations and backpressure, so that concurrent uploads cannot oversubscribe configured budgets.
5. As a client, I want the service's signing key to identify it across domain changes, so that a new domain owner does not inherit its identity or permissions.
6. As a publisher, I want retries to be idempotent and interrupted writes recoverable, so that I do not create duplicate charges or falsely complete content.
7. As a reader, I want offered chapters to have their complete verified originals locally retained, so that reading does not depend on fetching from another relay.
8. As a publisher, I want immutable series/chapter publications carrying a manifest hash and version, so that signatures bind exact content.
9. As a publisher, I want replacement to mean a deletion request and a new upload, so that the protocol does not introduce editable chapters or revision/equivalence links.
10. As an operator, I want to choose accepted image formats and hosting policy, so that another instance cannot impose its policy on mine.
11. As a reader, I want original bytes preserved, so that recompression is not silently substituted into the schema.
12. As a client, I want HTTP discovery over verified manifest metadata, so that strfry need not manage external manifests.
13. As a client, I want live offset/limit pages with timestamp filtering, so that I can query the index using the accepted pagination model.
14. As a client, I want exact case-sensitive non-language comparisons, RFC 4647 basic language matching, and true/false/unknown absence semantics, so that filters agree across implementations.
15. As a client, I want expensive or invalid queries rejected explicitly, so that a partial response is not mistaken for a complete result.
16. As a moderator, I want local approval required by default for protected-publication deletion, so that receiving an author request does not automatically remove content.
17. As an operator, I want shared originals retained while still referenced and persistent tombstones for approved removals, so that cleanup cannot break retained chapters or replay removed ones.
18. As an operator, I want no automatic series-deletion cascade, so that independently retained chapters remain under local policy.
19. As a peer operator, I want explicit peer approval and selected synchronization, so that my node does not distribute its retained collection to every relay by default.
20. As an ecosystem participant, I want anyone to run an instance or publish under their key, so that there is no central membership authority.
21. As an operator, I want approved fallbacks and signature/hash checks, so that origin loss can be repaired without trusting arbitrary endpoints.
22. As an operator, I want bounded inventories, partitions and durable resumable jobs, so that large syncs survive interruption without unlimited memory or work.
23. As an operator, I want stale, expired or cancelled workers fenced from completion, so that retries cannot overwrite newer job state.
24. As an operator, I want corruption and missing originals detected and withdrawn, so that known-incomplete content is not advertised as healthy.
25. As a group member, I want to authenticate with my personal key to a purpose-specific group API, so that an instance-held ordinary group key can publish for an authorized group.
26. As a group administrator, I want role changes and revocations to persist across restart, so that bootstrap settings cannot restore revoked access.
27. As a group owner, I want key backup and migration to preserve identity, so that I can run my own instance; a key alone does not recreate lost local roles.
28. As a reader, I want one public library per key, including progress and lists, so that my public reading state is portable without a private-library/master-key subsystem.
29. As a participant, I want public comments, reactions and voting under ordinary Nostr identities, so that social actions can accompany publications.
30. As a reader, I want anonymous reading by default, while operators can impose their own access policy.
31. As a MangaDex user, I want a close and understandable migration of supplied exports and originals, so that identifiers, labels and progress are not guessed or silently lost.
32. As an operator, I want explicit versioned schema migrations, durable projection work and recoverable backups, so that updates and restarts preserve the instance's identity and decisions.
33. As an operator, I want caches to fail closed for authoritative decisions and recover after missed invalidations or database outages, so that cached data cannot bypass removal or authorization.
34. As an operator, I want bounded pools, work queues and resource use with observability, so that overload produces backpressure instead of uncontrolled collapse.
35. As an operator, I want measured cold/warm, concurrent-write and failure results, so that architectural expectations are not confused with proven capacity.

## Settled implementation decisions

- PostgreSQL, deadpool-postgres/tokio-postgres and Moka are the selected foundation. Do not use SQLx. SQLite remains historical reference material during the port.
- Every offered chapter retains its own verified originals and manifest. Reader-time relay leeching is not the reference behavior.
- Instance policy is independent. Approved replication partners are not a central trust authority.
- Author-signed events are immutable. Protected deletion requests are stored outside strfry's automatic kind 5 mutation path and require local approval by default.
- Reference event kinds and extension field allocations currently used in experiments remain an experimental profile, not claimed registered standards.
- One public library per key, represented by a signed replaceable pointer to immutable external chunks (C, explicitly accepted). No private-library/master-key system. Normal social/library Nostr deletion and replaceable-event rules remain separate from protected manga moderation.
- Full original validation, complete-before-hosted state, shared-reference retention and exact removal tombstones remain mandatory across the database port.
- Byte accounting uses durable reservations and batched worker credits. Lease expiry alone does not prove physical cleanup.
- Immutable cached bodies never establish current visibility, authorization or quota availability. Mutable caches require explicit version/invalidation recovery rules.
- A small deployment uses smaller configured limits and co-located roles, not a different database or correctness model.

## Testing decisions

The primary acceptance seam is a real two-node HTTP/WebSocket fixture using PostgreSQL, original files, actual strfry and nginx. Extend the earlier behavioral tests at that seam as the port becomes runnable. Use separate processes for multi-writer behavior. Add focused component tests only where they expose races, accounting conservation or malformed inputs that cannot be controlled reliably through the public seam.

Every ticket needs executable acceptance evidence. Tests must distinguish direct delivery, guarded delivery, metadata lookup, media admission and peer synchronization. Exercise cold caches, duplicate requests, deletion races, pool exhaustion, lease loss, process/database restart and corruption. Keep durability enabled. Legacy SQLite results cannot count as PostgreSQL validation.

## Open decisions and assumptions

- Q93 resolved: explicit ascending/descending ordering is mandatory.
- Q94 resolved: optional exact totals; explicit errors on count-budget failure.
- Final extension/kind/schema allocations and deployment-specific budgets remain explicit profile/configuration work. Preserve settled semantics while keeping reversible implementation details visible.
- Real 100 TB and target Internet-load validation requires suitable infrastructure and authorization. The local laboratory cannot certify those targets.

## Out of scope

Website presentation/UI, anime streaming/transcoding, recompressed-image schema, private-library cryptography, unsolicited external publishing, paid infrastructure, and production exposure without separate authorization. No design can guarantee immunity from every takedown or force another operator to retain or delete content.

## Workflow

Follow the published to-spec and to-tickets structure using local files in this authorized computer workspace. Keep one ticket per file and explicit blocking edges. Continue ready work while genuine user/infrastructure dependencies remain flagged. The user has authorized implementation; ticket planning does not reopen settled protocol decisions.

## Accepted Q95 decision - 6 October 2026

The user explicitly selected C after the A/C experiment: one public library per key represented by a signed, replaceable Nostr pointer to an immutable root manifest and content-addressed chunks. Whole-library inline snapshots are not the selected scalable format. The reference implementation must provide batched commits, batched complete-library transfer and selective chunk reads. Chunk count is bounded/configurable, not a universal hard-coded performance claim. All advertised root/chunk bytes must be verified and locally retained; incomplete roots cannot become hosted.

This does not introduce private libraries, a master-key hierarchy, per-chapter addressable library events, automatic revision links, or automatic multi-device merge. Ordinary Nostr replacement/deletion semantics remain separate from protected manga-publication moderation. A local compare-and-swap guard may prevent an accidental stale overwrite at one instance, but cannot be represented as global consensus. Old root/chunk retention must cover in-flight readers and pending backend projection before safe collection.

Performance evidence and qualifications live in docs/library-ac-performance.md. Initial concurrent-load figures are being corrected for an instrumentation scan; selection of C is explicit, while sustained-load acceptance remains an engineering validation gate.

## 2026-10-07 clarified library throughput

The user explicitly requires300 million library/progress saves/month (messages Sentinel_833cfb1e00148191bf4d1c81e84f2702 and Sentinel_af8d877a4e1c8191b38687db810e69b5). This is separate from image/page-view traffic. Over30days it averages115.74 successful saves/s. Use120/s as a rounded baseline test, disclose higher peak assumptions, count HTTP rejections and growing projection backlog as failure to sustain offered work. Do not certify a month from a short test.

## 2026-10-07 feature-first priority

At03:34UTC the owner explicitly asked to focus on all features before worrying too much about performance (Sentinel_0e5a4985c5f481918950c5e416f38b86). Retain the PostgreSQL/deadpool/Moka scale-oriented architecture and correctness/resource bounds, but defer deeper load tuning. Finish lifecycle work already in progress, then ordinary social/library deletion, approved library replication, repair/serving, migration and backup/restore. The interrupted schema19 GC soak is not a capacity result. Existing300M monthly saves target remains a later validation requirement.


## Q93/Q94 resolved -2026-10-07

The owner's DA response (Sentinel_750546eb47f8819185ca223ed265595c) selected Q93D: HTTP discovery clients must explicitly supply ordering, and Q94A: exact totals are optional, with an explicit error if counting exceeds its budget. The current request parameter is `order=asc|desc`; `include_total=true` requests the count. No estimated count is silently substituted. Per-page consistency does not turn live offset pagination into a multi-request snapshot.

## Scope correction after the feature audit

The original implementation graph omitted accepted endpoint announcements/first-key remembrance, namespaced NIP-11 capabilities, client multi-relay library distribution/recovery, the MangaDex compatibility layer and complete publisher/staging workflows. Its completed status is not overall feature completion. The audit at docs/FEATURE-AUDIT-20261007.md is the remediation inventory. Performance remains deferred under the owner's reiterated feature-first priority. Diagnostic gap probes are not conformance passes.
