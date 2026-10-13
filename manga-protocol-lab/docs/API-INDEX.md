# Current backend API index

This index describes the schema52 Rust wrapper. It is the starting point for clients and operators; older chronological documents do not override current route implementations. The source-derived table is checked by `python3 scripts/api_index.py --check`. It lists every declared route, including deliberately unsupported mutation handlers. It is an inventory, not an OpenAPI request/response schema or a claim that every deployment enables every feature.

Axum also supplies HEAD handling for GET routes. CORS preflights are governed by configured origin policy. The root can serve NIP11/capabilities or upgrade to the guarded Nostr WebSocket protocol. A configured route can return503 when its optional backend is unavailable, or an authorization/policy error; presence is not permission or health.

## Authentication and identity

- [Service identity and remembered keys](service-identity-remembrance.md): possession and continuity, not reputation or a central authority.
- [Peer WebSocket interface](peer-websocket.md): approved-peer NIP42 read-only events, separate from HTTP manifest reconciliation.
- [Guarded relay](guarded-relay.md): NIP42 authentication, subscriptions, multiple authenticated keys, per-instance admission/read policy and ordinary signed events.
- [Publisher authority](publisher-authority.md): independent account/publisher permissions, NIP98 HTTP authentication, scoped staged grants and commit-time revocation fences.
- [Scoped blob adapter](blossom-profile.md): the exact implemented Blossom-shaped subset, BUD authorization and browser behavior. Staging remains private.
- [Capabilities](capability-registry.md): configured roles, limits and experimental event/schema namespace. No throughput guarantee.

NIP98 signatures bind the exact externally configured URL, HTTP method and body hash. Generate a fresh authorization event for each attempt. Do not reuse MangaDex passwords/tokens. Reader enrollment is operator-controlled by default, with explicitly enabled optional self-registration; see [reader enrollment](reader-enrollment.md).

## Feature contracts

| Workflow | Current contract |
|---|---|
| Series/chapter publication, rich metadata, one original cover | [Catalogue and covers](catalogue-and-covers.md), [publisher authority](publisher-authority.md) |
| Public catalogue/client compatibility | [MangaDex-shaped profile](mangadex-compatibility.md) |
| Reader profiles, public lists and exact reading progress | [Compatibility reader interfaces](mangadex-compatibility.md#public-reader-interfaces-schema50), [public library API](public-library-api.md), [signed batch wire](library-batch-api.md) |
| Multi-relay delivery, persistent retry and cold recovery | [Library client contract](library-client-delivery-contract.md) |
| Comments/replies/reactions and ordinary author deletion | [Social API](public-social-api.md), [ordinary deletion](ordinary-deletion-api.md) |
| Managed groups, private member roles and actor audit | [Group audit](group-action-audit.md), [group recovery](group-key-backup-recovery.md), [public profiles](mangadex-compatibility.md#managed-group-public-profiles-schema51) |
| Private one-off submissions for moderator review | [Submission review](submission-review-proposal.md) |
| Approved peer transfer, inventories and recurrent jobs | [Series federation](series-federation.md), [account/library inventory](account-library-inventories.md), [recurring sync](recurring-sync.md), [per-item outcomes](synchronization-item-outcomes.md) |
| Original storage, repair and safe reclamation | [Audit/repair](storage-audit-repair.md), [staged deletion](staged-deletion.md), [retention credits](storage-retention-credits.md) |
| Optional managed original serving | [Managed serving](managed-public-serving.md), [reader lifetime](direct-serving-lifetime.md) |
| Offline migration and reviewed aliases | [PostgreSQL migration](postgres-migration.md), [compatibility alias tool](mangadex-compatibility.md#reviewed-legacy-alias-tool) |
| Backup/restore and activation defaults | [PostgreSQL backup/restore](postgres-backup-restore.md), [group key recovery](group-key-backup-recovery.md) |

## Group profile request

POST `/groups/{key}/profile` (or `/compat/mangadex/group/{alias}/profile`) takes `{ "metadata": { "name": "Example", "about": "Description" }, "expected_head": "optional-current-event-id" }`. NIP98 must identify a current local administrator. The group signs a kind0 event; the service returns the event without its private key. Profile content is a JSON object limited to16KiB. CAS conflicts return409, clock-ahead guard429 and unavailable audit/storage503. Empty metadata clears descriptive fields, not membership, originals or publication history.

## Declared HTTP routes

<!-- generated-routes:start -->

123 declared paths; 136 explicit path/method pairs.

| Method | Path | Implementation entry point |
|---|---|---|
| GET | `/` | `root` |
| OPTIONS | `/` | `root_options` |
| POST | `/account-deletions` | `delete_account_events` |
| GET | `/account-deletions/{hash}` | `read_account_deletion` |
| POST | `/account-metadata` | `submit_account_metadata` |
| GET | `/accounts/{key}/metadata/{kind}` | `account_head` |
| POST | `/admin/accounts/{key}` | `configure_account` |
| GET | `/admin/catalogue/errors` | `catalogue_http::errors` |
| POST | `/admin/catalogue/reindex` | `catalogue_http::reindex` |
| POST | `/admin/check` | `operator_check` |
| POST | `/admin/compatibility/aliases` | `compatibility_http::aliases` |
| POST | `/admin/deletions/{hash}/approve` | `approve_deletion` |
| POST | `/admin/groups` | `create_group` |
| POST | `/admin/groups/import` | `import_group` |
| GET | `/admin/groups/{key}/audit` | `group_action_audit` |
| POST | `/admin/groups/{key}/bootstrap` | `bootstrap_group` |
| POST | `/admin/groups/{key}/enabled` | `enable_group` |
| POST | `/admin/identities/discover` | `service_discovery::discover` |
| POST | `/admin/identities/verify` | `verify_service_identity` |
| POST | `/admin/peers` | `configure_peer` |
| POST | `/admin/publications/{hash}/restore` | `restore_publication` |
| POST | `/admin/publishers/{key}` | `configure_publisher` |
| POST | `/admin/service-announcement` | `service_discovery::publish` |
| POST | `/admin/service-announcements` | `service_discovery::cache` |
| POST | `/admin/service-announcements/fetch` | `service_discovery::fetch` |
| GET | `/admin/status` | `operator_status` |
| POST | `/admin/storage/audit` | `audit_storage` |
| POST | `/admin/storage/repair` | `repair_storage` |
| GET | `/admin/submissions` | `review_http::list_admin` |
| GET | `/admin/submissions/{submission}` | `review_http::admin` |
| POST | `/admin/submissions/{submission}/decision` | `review_http::decide` |
| POST | `/admin/sync` | `sync_peer_chapter` |
| POST | `/admin/sync-account` | `sync_peer_account` |
| POST | `/admin/sync-jobs` | `enqueue_sync` |
| GET | `/admin/sync-jobs/{hash}` | `sync_status` |
| POST | `/admin/sync-jobs/{hash}/cancel` | `cancel_sync` |
| POST | `/admin/sync-library` | `sync_peer_library` |
| GET | `/admin/sync-schedules` | `list_sync_schedules` |
| POST | `/admin/sync-schedules` | `save_sync_schedule` |
| POST | `/admin/sync-schedules/{hash}/enabled` | `enable_sync_schedule` |
| DELETE | `/blobs/{hash}` | `delete_blob` |
| GET | `/blobs/{hash}` | `read_blob` |
| GET | `/catalogue/{key}` | `catalogue_http::entry` |
| GET | `/compat/mangadex` | `compatibility_http::capability` |
| GET | `/compat/mangadex/at-home/server/{key}` | `compatibility_http::at_home` |
| GET | `/compat/mangadex/author` | `compatibility_http::authors` |
| GET | `/compat/mangadex/author/{key}` | `compatibility_http::author_one` |
| GET | `/compat/mangadex/chapter` | `compatibility_http::chapters` |
| GET | `/compat/mangadex/chapter/{key}` | `compatibility_http::chapter_one` |
| PUT | `/compat/mangadex/chapter/{key}` | `compatibility_http::unsupported` |
| GET | `/compat/mangadex/cover` | `compatibility_http::covers` |
| GET | `/compat/mangadex/cover/{key}` | `compatibility_http::cover` |
| GET | `/compat/mangadex/covers/{key}/{file}` | `compatibility_http::cover_image` |
| GET | `/compat/mangadex/data-saver/{event}/{file}` | `compatibility_http::image` |
| GET | `/compat/mangadex/data/{event}/{file}` | `compatibility_http::image` |
| GET | `/compat/mangadex/group` | `compatibility_http::groups` |
| POST | `/compat/mangadex/group` | `compatibility_http::group_create` |
| GET | `/compat/mangadex/group/{alias}` | `compatibility_http::group_one` |
| POST | `/compat/mangadex/group/{alias}/deletion-requests` | `compatibility_http::group_deletion` |
| GET | `/compat/mangadex/group/{alias}/members` | `compatibility_http::group_members_get` |
| POST | `/compat/mangadex/group/{alias}/members` | `compatibility_http::group_members_set` |
| POST | `/compat/mangadex/group/{alias}/profile` | `compatibility_http::group_profile` |
| POST | `/compat/mangadex/group/{alias}/publish` | `compatibility_http::group_publication` |
| POST | `/compat/mangadex/group/{alias}/upload/{hash}` | `compatibility_http::group_blob` |
| GET | `/compat/mangadex/list/{key}` | `reader_http::list` |
| GET | `/compat/mangadex/list/{key}/members` | `reader_http::members` |
| GET | `/compat/mangadex/manga` | `compatibility_http::manga` |
| GET | `/compat/mangadex/manga/read` | `reader_http::read_markers_many` |
| GET | `/compat/mangadex/manga/status` | `reader_http::statuses` |
| GET | `/compat/mangadex/manga/tag` | `compatibility_http::tags` |
| GET | `/compat/mangadex/manga/{key}` | `compatibility_http::manga_one` |
| PUT | `/compat/mangadex/manga/{key}` | `compatibility_http::unsupported` |
| GET | `/compat/mangadex/manga/{key}/aggregate` | `compatibility_http::aggregate` |
| GET | `/compat/mangadex/manga/{key}/feed` | `compatibility_http::feed` |
| GET | `/compat/mangadex/manga/{key}/read` | `reader_http::read_markers` |
| GET | `/compat/mangadex/manga/{key}/status` | `reader_http::status` |
| POST | `/compat/mangadex/nostr/account-deletions` | `delete_account_events` |
| POST | `/compat/mangadex/nostr/account-metadata` | `submit_account_metadata` |
| POST | `/compat/mangadex/nostr/deletion-requests` | `deletion_request` |
| POST | `/compat/mangadex/nostr/enrollment` | `enrollment_http::enroll` |
| POST | `/compat/mangadex/nostr/libraries` | `commit_library` |
| POST | `/compat/mangadex/nostr/library/batches` | `upload_library_batch` |
| POST | `/compat/mangadex/nostr/publications` | `compatibility_http::publication` |
| GET | `/compat/mangadex/user/list` | `reader_http::my_lists` |
| GET | `/compat/mangadex/user/me` | `reader_http::me` |
| GET | `/compat/mangadex/user/{key}` | `reader_http::user` |
| GET | `/compat/mangadex/user/{key}/library` | `reader_http::entries` |
| GET | `/compat/mangadex/user/{key}/list` | `reader_http::user_lists` |
| POST | `/deletion-requests` | `deletion_request` |
| GET | `/enrollment` | `enrollment_http::policy` |
| POST | `/enrollment` | `enrollment_http::enroll` |
| POST | `/events` | `submit_event` |
| GET | `/events/{hash}` | `read_event` |
| GET | `/federation/account-events/{hash}` | `peer_account_event` |
| GET | `/federation/blobs/{hash}` | `peer_blob` |
| GET | `/federation/events/{hash}` | `peer_event` |
| GET | `/federation/libraries/{key}` | `peer_library_head` |
| GET | `/federation/library-routing/{hash}` | `peer_library_routing` |
| GET | `/federation/library/blobs/{hash}` | `peer_library_blob` |
| GET | `/federation/relay` | `peer_websocket` |
| GET | `/federation/service-announcements/{key}` | `service_discovery::peer_cached` |
| POST | `/groups/{key}/delete-upload/{hash}` | `group_delete_upload` |
| POST | `/groups/{key}/deletion-requests` | `group_deletion_request` |
| GET | `/groups/{key}/members` | `list_group_members` |
| POST | `/groups/{key}/members` | `set_group_member` |
| POST | `/groups/{key}/profile` | `set_group_profile` |
| POST | `/groups/{key}/publish` | `group_publish` |
| POST | `/groups/{key}/upload/{hash}` | `group_upload` |
| GET | `/identity` | `identity` |
| POST | `/libraries` | `commit_library` |
| GET | `/libraries/{key}` | `read_library_head` |
| GET | `/libraries/{key}/entries` | `read_library_entries` |
| POST | `/library/batches` | `upload_library_batch` |
| GET | `/library/blobs/{hash}` | `read_library_object` |
| POST | `/library/objects/{kind}` | `stage_library_object` |
| POST | `/library/read` | `read_library_batch` |
| GET | `/library/staging/{hash}` | `read_library_staging` |
| GET | `/originals/{hash}` | `public_original` |
| POST | `/publications` | `publish_event` |
| GET | `/query` | `query_chapters` |
| POST | `/reconcile` | `reconcile_inventory` |
| GET | `/series` | `catalogue_http::series` |
| GET | `/service/announcement` | `service_discovery::own` |
| GET | `/service/announcements/{key}` | `service_discovery::cached` |
| GET | `/social` | `query_social` |
| POST | `/social` | `submit_social` |
| GET | `/submissions` | `review_http::list_own` |
| POST | `/submissions` | `review_http::declare` |
| GET | `/submissions/{submission}` | `review_http::own` |
| GET | `/submissions/{submission}/blobs/{hash}` | `review_http::read_blob` |
| POST | `/submissions/{submission}/blobs/{hash}` | `review_http::upload` |
| POST | `/submissions/{submission}/withdraw` | `review_http::withdraw` |
| PUT | `/upload` | `blossom_http::upload` |
| PUT | `/upload/{hash}` | `upload_blob` |
| DELETE | `/{blob}` | `blossom_http::delete` |
| GET | `/{blob}` | `blossom_http::read` |

<!-- generated-routes:end -->

## Boundaries and pending contracts

All libraries and social state are public under each instance's access policy. Immutable publication corrections use delete-request plus new upload; no in-place revision links or automatic read-progress inheritance are supplied. Public-readable bytes can be copied regardless of approved federation membership.

The owner selected group-lifecycleA: profile clearing and reversible operator disabling, with no group-deletion endpoint for the MVP. Keys, roles and published content are preserved. Reader enrollmentA is implemented with a disabled default. Group publication commands now accept a caller-persisted `created_at` timestamp. Reuse it with the same kind/manifest/version to retain exact Nostr event identity across retries. Omitting it preserves the legacy server-time behavior and is not retry-safe. See [group publication retry contract](group-publication-retries.md). This does not promise global exactly-once processing.

The wrapper is a backend, not the deferred reader website. Target scale, named external-client compatibility, public production deployment and independent security certification remain unproven. See [feature follow-through](FEATURE-COMPLETION-20261008.md) and [requirement/evidence matrix](requirement-evidence-matrix.md).
