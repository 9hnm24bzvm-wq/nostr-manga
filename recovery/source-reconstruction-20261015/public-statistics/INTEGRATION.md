# Statistics source recovery

Status: RECONSTRUCTED-UNVERIFIED, 2026-10-15.

The complete implementation and dedicated real HTTP/PostgreSQL test are under `weebdex-platform/`, with documentation and an explicitly unverified evidence template. These are reconstructed files. Their bytes are not claimed to match lost commits. Fresh tests are required.

Required restored foundation interfaces:

- `public_api::{PostgresPublicIdResolver, PublicApiError, PublicApiResult, PublicIdResolver, PublicUuid, ResourceKind}`
- `PostgresPublicIdResolver::new(PgStore)`
- async `resolve_many(kind, &[PublicUuid]) -> ApiResult<BTreeMap<PublicUuid, ResolvedPublicId>>`
- `ResolvedPublicId::{native_id: String, preferred_id: PublicUuid}`
- `PublicUuid::{parse, as_str}`, Clone + Ord + Serialize + Display
- `ResourceKind::{Manga, Chapter, Group}`, PartialEq
- `PublicApiError::{bad_request, not_found}`, conversion from native ApiError, public `status`, `title`, `detail`
- `PublicIdResolver::{preferred, preferred_many}` and concrete `register_alias` for the test
- original native domain migrations and visibility functions through public UUID migration
- existing router admission, JSON error conversion and no-store response policy

Integration:

1. Copy the `weebdex-platform/` files into the target repository without replacing any unrelated foundation files.
2. Add `pub mod statistics;` to `backend/src/public_api/mod.rs`.
3. Add `.merge(super::statistics::router())` to the foundation `public_api::routes::router()` chain. Preserve all catalogue, auth and media merges. `integration.patch` captures the earlier minimal wiring shape for review; its line locations must be reconciled with the reconstructed foundation.
4. Merge only the six `coverage-rows.json` entries into the existing `docs/mangadex-operation-matrix-v1.json` operations by `(method,path)`. Preserve all other operations and inventory metadata. Verify operation IDs against the official pinned schema.
5. The coverage enum must support `Partial` serialized as `partial`. No migration or new rating table is required or included.
6. Run the fresh unit and real HTTP/PostgreSQL commands in `docs/public-statistics-v1.md`. Then update the unverified test evidence only with actual observed fresh results.

Behavior preserved: all six GET routes; actual follows and unavailable chapter counts; comments null pending external forum bridge; omitted unsupported ratings; kind-scoped canonical alias keys; duplicate alias collapse; strict UUID/encoding/query validation; batch budget 100; one resolver query plus one set-based visibility/count query; single404 and batch omissions for hidden/unknown/wrong-kind; public400/405/503 errors. No auth/protocol mutation or new features.
