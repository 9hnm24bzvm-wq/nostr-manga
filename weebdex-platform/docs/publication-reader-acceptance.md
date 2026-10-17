# Signed upload to reader acceptance

The 2026-10-17 local gate verifies the actual PostgreSQL/strfry publication
pipeline. A signed chapter is initially accepted with an owner-only pending
receipt. Starting the original native listener and ordinary supervised catalogue
importer makes it publicly readable without replaying the commit. This is a
backend HTTP seam test, not a rendered-browser or production acceptance claim.

## Corrected recovery transition

The source importer previously made the chapter readable but left the persisted
upload receipt marked pending. Consequently, `GET /api/upload` kept returning the
already completed upload indefinitely. The regression reproduces the old HTTP
200 where the completed resume selector should return 204.

Materialization now advances only the exact matching committed intent, checking
session, owner, chapter and commit hash. The new snapshot is taken from the
current canonical, source-visible chapter DTO. Pending source or parent visibility
cannot become a readable receipt. Already-readable snapshots are not rewritten
by later source sweeps; the regression checks stable session versioning.

## Executed coverage

- Seven generated-image upload/cover/storage regression cases passed.
- The original real-source rollback/recovery lifecycle case passed, including
  cover derivatives, adaptive catalogue pages, distinct-peer original transfer,
  explicit deletion review, revocation and original listener ownership.
- The added committed-pending journey verifies private pending state, anonymous
  denial before projection, automatic delivery, ordered repeated original hashes,
  byte-exact original GETs, HEAD and ranges, valid optimized JPEGs, current
  idempotent commit replay, completed-session resume dismissal, and stable later
  source sweeps.
- Both final disposable PostgreSQL fixtures report verified shutdown, closed
  loopback ports, no remaining owned processes and no unconfirmed births.

Machine-readable counts, source hashes, final runtime-log digests and cleanup
receipt digests are in
[`publication-reader-20261017/result.json`](../test-evidence/final/publication-reader-20261017/result.json).
Raw runtime logs, fixture databases and temporary signing keys are not committed.

## Reproduce

Use the project's pinned Rust toolchain/dependency cache, an installed PostgreSQL
binary directory, and the real pinned strfry binary. No stub projection backend
or manual outbox acknowledgement is supported by these tests. Compile before
starting the bounded fixture runner:

```sh
cargo test --locked --offline --manifest-path backend/Cargo.toml \
  --test upload_http --test protocol_bridge_http --no-run -j2

PLATFORM_TEST_PG_BIN=/absolute/postgresql/bin STRFRY_BIN=/absolute/strfry \
  scripts/run-auth-tests.sh /absolute/upload_http-test-executable \
  --ignored --test-threads=1 \
  --skip licensed_ubunchu_originals_upload_cover_reader_hashes_and_attribution

PLATFORM_TEST_PG_BIN=/absolute/postgresql/bin STRFRY_BIN=/absolute/strfry \
  scripts/run-auth-tests.sh /absolute/protocol_bridge_http-test-executable \
  --ignored --test-threads=1 --exact \
  accepted_upload_rollback_recovers_source_media_covers_revocation_and_embedded_owner_lifecycle
```

The two signed upload journeys use distinct generated originals, series metadata
and volume names. The original source store intentionally owns content hashes
globally in one database; unrelated synthetic authorities must not accidentally
share its immutable staging objects.

## Remaining limits

The licensed Ubunchu case requires its verified local cache, which is absent in
this worktree. The final run explicitly filters that case; an earlier broad run
failed its cache preflight, rather than silently skipping it. No image download
or licensed-data acceptance is claimed here. Browser execution remains separately
blocked/unverified. These bounded correctness checks do not establish load,
performance, production readiness or external publication.
