# WeebDex local implementation tickets

Source: 51 UI paths, 107 observed HTTP operations. All operation implementation and tests remain pending.

## Immediate ownership and launch queue

Platform root: /workspace/scratch/3eafc9756ff3/weebdex-platform. Planning owns only /workspace/scratch/3eafc9756ff3/platform-planning.
Foundation owner is design_protocol_module_boundary. Frontend owner is preserve_weebdex_frontend.
Next available slots: implement_catalog_api, implement_platform_auth, implement_upload_reader. Later: implement_personal_library, implement_community_api, implement_moderation_api, implement_analytics_api.
Each domain owns its own directory, contract file and reserved migrations. Foundation alone edits Cargo, shared types, state, errors and router wiring.
Feature signature: router() -> axum::Router<AppState>; handlers Result<_, ApiError>.
Platform DB tracking must stay separate from protocol exact 52-migration history. Default reversible composition uses separate configured protocol origin to preserve signed URLs.

## Working-build critical path

Foundation -> catalog reads + real secure login/register -> chapter media -> preserved frontend build -> one real browse/search/title/read/account vertical slice -> parity/contract checks.
The one-hour target is a target, not a guarantee; the full 107-operation feature-complete gate additionally includes all editorial/upload/library/community/moderation/client/statistics work.

## Tickets

### WD-001 Freeze source and capabilities
Owner: inspect_weebdex_attachment. Status: inventory_complete. Priority: P0. Dependencies: none.
Owned files: Inspection reports only
- Archive baseline has exact bytes and hashes; unsafe paths rejected during extraction
- 51 distinct UI route paths and 107 observed HTTP operations recorded
- Archive has no backend, existing tests, LICENSE, or verified upstream identity; those remain unknown
Sources: reports/frontend_file_hash_baseline.json; reports/frontend_routes.json; reports/frontend_api_contract_inventory.json

### WD-002 Preserve and build frontend
Owner: preserve_weebdex_frontend. Status: source_preserved_build_pending. Priority: P0. Dependencies: WD-001.
Owned files: weebdex-platform/frontend/**; weebdex-platform/scripts/frontend-*
- All existing archive source, static, HTML, CSS/Sass/SCSS, Svelte, and TypeScript bytes remain identical
- Build-time PUBLIC_* endpoints supplied by additive config/environment override; original .env files unchanged
- No formatter run on source; build/check/lint evidence reported separately; toolchain/dependency failures disclosed
- Static fallback serves all 51 verified route paths and PWA/service-worker assets without redesign
Sources: package.json; svelte.config.js; vite.config.ts; src/routes/+layout.ts; src/service-worker.ts

### WD-003 Create platform runtime foundation
Owner: design_protocol_module_boundary. Status: pending. Priority: P0. Dependencies: WD-001.
Owned files: weebdex-platform/backend/Cargo.toml; weebdex-platform/backend/src/{lib,main,config,state,router,error,db}.rs; weebdex-platform/backend/src/domains/mod.rs; weebdex-platform/backend/src/contracts/mod.rs; weebdex-platform/backend/src/protocol/**
- Separate Rust crate depends on unchanged manga-node library by path; no protocol domain copy
- AppState Clone fields db: PgStore, config: Arc<Config>, protocol: Arc<ProtocolFacade>, reads: Arc<manga_node::read_admission::Admission>; same shared deadpool_postgres pool; feature router() -> Router<AppState> and Result<_,ApiError>
- API nested under /api; native Ory-compatible flows configured under /auth; static/asset fallback never masks API errors
- Platform schema and migration history separate from exact 52 protocol migration checksums
- One application runtime and graceful shutdown owns workers/relay/storage; no duplicated worker lifecycle
- Loopback default, explicit allowed origins, bounded body/time/concurrency limits, structured error bodies and observable health readiness
Sources: manga-protocol-lab/server/src/lib.rs; manga-protocol-lab/server/src/node.rs:676-1335; manga-protocol-lab/server/src/db.rs:292-355

### WD-004 Implement catalog read APIs
Owner: implement_catalog_api. Status: pending. Priority: P0. Dependencies: WD-001, WD-003.
Owned files: backend/src/domains/catalog/**; backend/src/contracts/catalog.rs; backend/migrations/0001_* through 0009_*
- Match flat Manga/Chapter/Author/Group/Tag DTOs plus relationships, version and RFC3339 timestamps; no MangaDex attributes wrappers
- Every catalog-read operation in capability matrix yields actual persisted results or accurate empty feed with total/limit/page; no constant successful placeholders
- Search implements actual title/year/language/status/content-rating/tag include-exclude/AND-OR/author/artist/chapter availability filters and stable paging/sorts
- Title detail includes covers, relations, recommendations, languages/groups, thread references; chapter aggregate and feeds have exact mapped envelopes
- Seeded synthetic fixture proves homepage -> search -> title -> chapter navigation; missing id returns source-compatible failure
Sources: src/lib/API/{Manga,Chapter,Author,Group,Tag}.ts; src/@types/{Manga,Chapter,ChapterAggregate,Response}.d.ts

### WD-005 Implement catalog editorial state
Owner: implement_catalog_api. Status: pending. Priority: P1. Dependencies: WD-004, WD-006.
Owned files: Same catalog-owned module/migrations as WD-004
- Create/edit author, group, title and relations persist with authenticated permission/ownership checks
- Draft get/search/commit/publish/reject follow state enums and optimistic version checks; locked-title permissions enforced
- Invalid edits cannot silently overwrite newer version; transaction rollback leaves no partial relationship update
- CatalogRepository is sole platform catalog writer; uploads publishes chapters through an explicit service seam
Sources: src/lib/API/{Manga,Author,Group}.ts; src/lib/constants/Manga.ts; src/lib/permission/{manga,author,group}.ts; src/lib/components/draft/**

### WD-006 Implement secure native auth flows
Owner: implement_platform_auth. Status: pending. Priority: P0. Dependencies: WD-001, WD-003.
Owned files: backend/src/domains/auth/**; backend/src/contracts/auth.rs; backend/migrations/0010_* through 0019_*
- Rust flow facade satisfies actual @ory/client browser SDK methods without external Kratos dependency or source rewrites
- Login/register flow create/get/update returns exact consumed id/return_to/ui.nodes/ui.messages and active-session result fields; contract in auth-flow-contract.json
- Password hashes use vetted adaptive hashing; session cookies HttpOnly, secure in production, SameSite, expiry and revocation; never fake successful authentication
- CSRF is flow/session bound and validated; flow expiry emits 410 with return_to; invalid payload produces UI error nodes; safe redirect allowlist
- /api/auth/check returns identity, preferences, follows, groups, roles and permissions only from authenticated persisted state
- SessionPrincipal + permission helper published for feature modules; anonymous, unverified, owner, moderator and staff tests deny unauthorized writes
Sources: src/lib/Kratos.ts; src/lib/API/Auth.ts; src/@types/Response.d.ts; src/routes/(app)/auth/{login,register,logout}/+page.svelte; src/lib/stores/Session.ts; src/lib/permission/**

### WD-007 Implement account recovery verification
Owner: implement_platform_auth. Status: pending. Priority: P1. Dependencies: WD-006.
Owned files: Same auth-owned module/migrations as WD-006
- Recovery/verification link flows and settings profile/password flows satisfy SDK and UI contracts
- Recovery tokens one-use, expiry-bounded, hash-stored; email enumeration resisted; privilege refresh for sensitive settings
- No account marked verified without proved link; no recovery marked successful without actual delivery path
- Configured delivery may use local development outbox with explicitly labeled non-production behavior; production SMTP/domain/provider unresolved until verified
- OIDC provider/link/unlink nodes only advertised for configured working providers; none invent successful external login
Sources: src/routes/(app)/auth/{recovery,verification,error}/+page.svelte; src/routes/(app)/settings/account/+page.svelte

### WD-008 Implement chapter media reader
Owner: implement_upload_reader. Status: pending. Priority: P0. Dependencies: WD-003, WD-004.
Owned files: backend/src/domains/uploads/**; backend/src/contracts/uploads.rs; backend/migrations/0020_* through 0029_*
- Chapter node and data/data_optimized filenames resolve exact GET ${node}/data/${chapter.id}/${page.name}
- Images have genuine validated dimensions/media type; optimized variants use same media URL route
- Reader first/last page, next chapter, group/language aggregate, single/double/webtoon, data-saver and repeated navigation work with unchanged UI
- Public page delivery preserves bounded protocol admission if protocol-backed; no unrestricted raw Repository pool access
- Media paths cannot traverse roots; cache keys/ETags and conditional reads correct; interrupted reads release owned permits
Sources: src/@types/Chapter.d.ts; src/lib/components/reader/{reader-provider,reader-queue,reader-pages,reader-pagination}.svelte; manga-protocol-lab/server/src/node/read_admission.rs

### WD-009 Implement chapter upload sessions
Owner: implement_upload_reader. Status: pending. Priority: P1. Dependencies: WD-004, WD-006, WD-008.
Owned files: Same uploads-owned module/migrations as WD-008
- Begin/begin-edit/get/delete/commit/file-delete/batch-delete/approval-required operations persist owner-scoped session state
- Multipart accepts repeated file fields and returns ordered UploadSessionFile[] with per-file errors and exact field types
- Chapter metadata, page_order and groups are validated; atomic commit creates actual chapter + ordered files through CatalogRepository
- Cancellation/retry/edit/reorder/delete cannot leak another user files or publish a partial upload; duplicate commit idempotency tested
- Extension, content sniffing, dimensions, file count/bytes/decompression/pixel budgets bounded; quarantine/process lifecycle visible
Sources: src/lib/API/UploadSession.ts; src/lib/components/upload/**; src/@types/Upload.d.ts; src/lib/API/Chapter.ts; src/lib/permission/{upload,chapter}.ts

### WD-010 Implement covers and guarded image proxy
Owner: implement_upload_reader. Status: pending. Priority: P1. Dependencies: WD-004, WD-006.
Owned files: Same uploads-owned module/migrations as WD-008
- Cover upload/list/get/update/delete serve original and .256.webp/.512.webp variants at ${PUBLIC_COVER_URL}/${mangaId}/${coverId} suffixes
- Cover DTO ext/dimensions/volume/language/version matches source; ownership/version checks enforced
- PUBLIC_UPLOAD_URL preview URLs resolve actual session media with authorized lifetime
- PUBLIC_PROXY_URL?image=encoded is bounded; only vetted public http(s) targets, redirect revalidation, private/link-local/metadata address denial, MIME/byte limits
Sources: src/lib/API/Cover.ts; src/@types/Cover.d.ts; src/routes/(app)/title/MangaCoversSection.svelte; src/routes/+layout.ts; src/lib/components/upload/UploadChapterEdit.svelte

### WD-011 Implement personal library and history
Owner: implement_personal_library. Status: pending. Priority: P1. Dependencies: WD-004, WD-006.
Owned files: backend/src/domains/library/**; backend/src/contracts/library.rs; backend/migrations/0030_* through 0039_*
- Manga reading status/private flag follow update/unfollow/import/export matches exact source payloads and real persisted rows
- Group/user follow list/status/follow/unfollow and followed chapter feed/updates honor session and private preferences
- History search/list/feed/read-unread batch supports exact nested records and mapped feed DTOs; repeated requests safe
- Settings fields content filters/languages/translated languages/tags/uploaders/groups/items_per_page/data_saver and optimistic version persist
- Library import/export roundtrip survives duplicates and invalid entries; export returns real download and never leaks other accounts
Sources: src/lib/API/{MangaFollow,ChapterHistory,Settings}.ts; src/lib/API/{User,Group}.ts follows functions; src/@types/{MangaFollow,ChapterHistory,UserPreference,Response}.d.ts; src/routes/(app)/{library,history,settings/library}/**

### WD-012 Implement profiles and discussions
Owner: implement_community_api. Status: pending. Priority: P1. Dependencies: WD-004, WD-006.
Owned files: backend/src/domains/community/**; backend/src/contracts/community.rs; backend/migrations/0040_* through 0049_*
- Public user search/get, authenticated user profile edit and actual relationship stats match source User DTO
- Thread get/replies/get-reply/create/edit/soft-remove/hard-delete/vote/unvote implemented with exact integer reply id versus string route inputs
- Owner/version/moderator permissions enforced; self-cross-thread reply id misuse denied; vote uniqueness and unvote idempotency
- Deleted/reported content behavior and pagination anchor links match forum/comment flows; no unsanitized server-rendered markup
Sources: src/lib/API/{User,Thread}.ts; src/@types/{User,Thread,Reply}.d.ts; src/lib/components/comments/**; src/lib/permission/{reply,user}.ts

### WD-013 Implement moderation and report actions
Owner: implement_moderation_api. Status: pending. Priority: P1. Dependencies: WD-005, WD-006, WD-009, WD-012.
Owned files: backend/src/domains/moderation/**; backend/src/contracts/moderation.rs; backend/migrations/0050_* through 0059_*
- Report reasons/create/search/delete/accept/reject match Report DTOs and entity reason enums
- Ordinary user can see own permitted reports; staff/moderator permissions required for review/actions; UI guards mirrored on server
- Accept/reject notes and actions audited with identity and timestamps, version/state conflicts blocked
- Group claims/title publish/reject/locks/chapter claims use shared role/permission decisions with feature owner; no unconditional approval
Sources: src/lib/API/Report.ts; src/@types/Report.d.ts; src/lib/constants/Report.ts; src/lib/permission/report.ts; src/routes/(app)/my/reports/+page.svelte

### WD-014 Implement API client lifecycle
Owner: implement_platform_auth. Status: pending. Priority: P1. Dependencies: WD-006.
Owned files: Same auth-owned module/migrations as WD-006
- Client list/create/rename/delete/regenerate-secret exact routes including POST rename; proper APIClient DTO
- Secret generated securely, disclosed only once at issuance/rotation; persist hash and bounded scopes; revoked/rotated credentials stop working
- No real user credential generated during automated demonstration; tests use isolated synthetic credentials
- Client feature is security-critical and cannot be declared production-ready until auth/rate-limit/revocation tests pass
Sources: src/lib/API/Client.ts; src/@types/Client.d.ts; src/routes/(app)/settings/client/**; src/lib/permission/client.ts

### WD-015 Implement statistics and rankings
Owner: implement_analytics_api. Status: pending. Priority: P1. Dependencies: WD-004, WD-006, WD-011, WD-012.
Owned files: backend/src/domains/analytics/**; backend/src/contracts/analytics.rs; backend/migrations/0060_* through 0069_*
- Manga/chapter view POSTs and user/manga/chapter/group/thread statistics GETs persist and return actual counters
- Trending/most-read daily/weekly/monthly/all-time sorts use documented source-driven time windows and tie-breaking
- Public analytics bounded and abuse-controlled; duplicate/retried UI loads do not inflate counts without policy
- No fabricated popularity/history used to claim functionality
Sources: src/lib/API/{Analytics,Statistics}.ts; src/routes/(app)/popular/**; src/lib/components/{MostReadCarousel,RecentlyAddedCarousel}.svelte

### WD-016 Verify locale mobile and PWA parity
Owner: verify_frontend_seams. Status: pending. Priority: P1. Dependencies: WD-002, WD-008, WD-011, WD-012.
Owned files: weebdex-platform/tests/ui/**; weebdex-platform/test-evidence/ui/**
- Exact existing language codes incl zh-hk/pt-br/es-la used in filters/uploads/metadata; UTF-8 alternate titles/descriptions roundtrip
- Source has language metadata and flags, but no proven translated application-string catalogs; full UI i18n explicitly remains unspecified
- Responsive sidebar/header, search, library, upload reorder and reader checked at desktop and narrow widths without HTML/CSS edits
- PWA/service-worker navigation/cache/update behavior checked offline and after repeated route changes; no claim universal mobile coverage
Sources: src/lib/Locale.ts; src/lib/hooks/is-mobile.svelte.ts; src/service-worker.ts; src/lib/components/{AppHeader,AppSidebar}.svelte

### WD-017 Verify security performance and isolation
Owner: verify_platform_security. Status: pending. Priority: P1. Dependencies: WD-005, WD-007, WD-009, WD-010, WD-011, WD-013, WD-014, WD-015.
Owned files: weebdex-platform/tests/security/**; weebdex-platform/tests/performance/**; weebdex-platform/test-evidence/security/**; weebdex-platform/test-evidence/performance/**
- Authorization matrix and CSRF/redirect/session expiry/upload SSRF/path traversal/admission cancellation regressions pass
- Platform migrations upgrade/fresh install/rollback boundaries do not change protocol schema checksum or authority/storage semantics
- Measure representative persisted query and media load under bounded concurrency; record hardware/dataset/p50/p95/error/memory with no invented speedup
- Index actual filter/sort/follow/history/upload ownership queries; avoid N+1 relationship queries and per-request blocking image transforms
- NIP98 exact URL/method/body binding, per-instance policy, guarded relay and cancellation permit tests remain passed
Sources: manga-protocol-lab/server/src/{node.rs,db.rs}; manga-protocol-lab/server/src/node/read_admission.rs; src/lib/HTTP.ts

### WD-018 Verify complete API UI seams
Owner: verify_frontend_seams. Status: pending. Priority: P0. Dependencies: WD-002, WD-005, WD-007, WD-008, WD-009, WD-010, WD-011, WD-012, WD-013, WD-014, WD-015, WD-016.
Owned files: weebdex-platform/tests/contracts/**; weebdex-platform/test-evidence/contracts/**; weebdex-platform/test-evidence/end-to-end/**
- Every one of 107 observed operations has method/path/status/payload/envelope/auth contract result and associated UI seam evidence
- SDK auth flows validated against installed pinned @ory/client request paths; interrupted/back/forward/expired/repeated flows covered
- Synthetic account/title/chapter/pages/covers/thread/report/library fixture makes real end-to-end roundtrip; mocked constant 200 endpoint responses do not qualify
- All 51 paths reachable, including error/empty/forbidden/not-found states; not-run/failed checks explicitly retained
- Reader source load handler currently accepts any HTTP status as a blob: record this unchanged-frontend inherited limitation rather than silently claiming robust errors
Sources: reports/frontend_api_contract_inventory.json; src/routes/(app)/auth/**; src/lib/components/reader/reader-queue.svelte

### WD-019 Gate immutable frontend parity
Owner: verify_frontend_seams. Status: pending. Priority: P0. Dependencies: WD-002, WD-018.
Owned files: weebdex-platform/test-evidence/frontend-parity/**
- Recompute every baseline source/static file hash at final integration; additions separated and no baseline file mismatch
- HTML/CSS/Svelte byte parity required even after frontend build, metadata post-build and integration tests
- Generated output tracked separately from source; screenshot comparison proves broad appearance but cannot substitute for source hashes
- Any required source change is a disclosed blocker against unchanged frontend requirement, never silently accepted
Sources: reports/frontend_file_hash_baseline.json; scripts/post_build.js

### WD-020 Package verified project checkpoint
Owner: coordinate_release_checkpoint. Status: pending. Priority: P1. Dependencies: WD-017, WD-018, WD-019.
Owned files: weebdex-platform/docs/**; weebdex-platform/recovery/**
- Working-build target is best effort within one hour, not a completion guarantee; release reports which checks passed/failed/not run
- Local docs include startup/env/migration/runtime architecture and complete capability status table
- GitHub ticket/commit publication remains blocked until private repository exists and authorized authenticated route verified; no local ticket called remote
- Checkpoint includes source/build/docs/test evidence only, excludes credentials/private keys/personal data/runtime databases; encrypted archive download/recovery verification handled by authorized checkpoint owner
- Source licensing/upstream provenance remains unknown until established; no license invented
Sources: reports/archive_inventory.json; reports/frontend_api_contract_inventory.json

### WD-021 Recover missing announcement contract
Owner: preserve_weebdex_frontend. Status: pending. Priority: P1. Dependencies: WD-001, WD-002.
Owned files: weebdex-platform/frontend-integration/**
- Original homepage imports missing announcement data/Entry components and sidebar links /announcements absent from supplied archive
- Parent authorized additive reconstruction fallbacks only; keep original frontend files hash-identical
- Empty announcement data and minimal reconstructed Entry are explicitly unverified fallbacks, not completed announcement functionality
- Recover real original announcement DTO/content/rendering and missing route contract before claiming full UI parity; do not invent content
Sources: src/routes/(app)/+page.svelte:8-21; src/lib/components/AppSidebar.svelte:94

## Explicit pending unknowns

- U-01: Frontend license and exact upstream commit (unknown). Do not invent reuse/publication licensing terms.
- U-02: Installed SDK-generated request URL paths (pending dependency materialization). Native auth facade must pass actual @ory/client requests; method names alone do not prove route compatibility.
- U-03: Production verification/recovery email transport (unconfigured). Cannot call those flows production-complete without delivery and one-use link proof.
- U-04: External OIDC providers (unconfigured). No fake provider nodes or successful login. Source conditionally renders nodes.
- U-05: Same-origin original protocol/SPA dispatch (pending foundation choice). Protocol GET / and /{blob} conflict with SPA root. Preserve exact signed URLs; separate protocol origin is the reversible safe default.
- U-06: Authenticated GitHub publication (pending). No repository exists yet; source candidate fixture-sharing consent is separate.
- U-07: Production infrastructure/config/security defaults (unverified). Local working build does not imply production deployment readiness.
- U-08: Full translated application interface (not present in verified inventory). Language metadata/flags are present; multilingual UI-string translation is not established.
- U-09: Build/check/test execution (not run by planner; owner must report). No compile, frontend build or speed claim follows from planning.
- U-10: Protocol interoperability mapping (pending service facade implementation). WeebDex ids/catalog/public reader and protocol canonical event/storage identities must be mapped, not guessed.
- U-11: Missing announcement components and case-sensitive imports (verified attachment defects reported by preservation/inspection workers). Source remains unchanged; missing announcement source blocks truthful exact complete UI until resolved within user constraints.

## Full contracts
capability_matrix.json assigns every observed operation to a ticket with exact method/template/source line. auth-flow-contract.json captures unchanged UI consumption. source_hash_baseline_reference.json fingerprints the inspection baseline.
Graph integrity verified by local planning script only. No application build/test executed by the planner.
