# Signed upload to reader acceptance

## Current qualification status

The first repair failed an independent exact-source/executable repeat: completed
`GET /api/upload` returned HTTP 200 instead of 204 after readable projection.
The [sanitized failure](../test-evidence/final/publication-reader-20261017/independent-repeat-failed.json)
remains preserved. The corrected page-end repair below passed its deterministic
child-before-parent regression independently, with normal verified cleanup.

The complete integrated HTTP, chapter and source-lifecycle suites are being
rerun on commit `eca83392ae039487b2e3376c2fdee8f0961f2d8a`. Their final-source
qualification is pending its fresh receipts. Native browser and production
acceptance remain separate unverified scopes.

## Historical isolated result

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

The first repair updated the receipt inside chapter materialization. An
independent rerun exposed a dependency-order race: signed content hashes can put
a child before its parent. The child saw its platform parent hidden; a later
parent in the same page made the chapter readable while its saved receipt stayed
pending. The returned owner DTO hydrates current catalogue state, so that DTO's
readable status did not prove its stored snapshot had advanced. The earlier pass
was insufficient; the corrected evidence retains the independent failure digest.

Receipt reconciliation now runs after all projections in a source page. It
selects at most 16 currently ready pending receipts, including earlier-page
children, and checks the exact command/session/owner/chapter/commit hash/source
event. Current canonical child and parent visibility must both hold. Busy
sessions are skipped for a later page; invisible candidates do not consume the
ready batch. Already-readable snapshots are not rewritten.

The real-source regression performs a pending sweep before starting the original
worker. It selects actual synthetic publications until a child-before-parent
hash order is exercised (bounded at 64 attempts; a pass requires that order).
It directly checks stored committed_result before requesting resume dismissal.
No additional wait after reader visibility hides a stale receipt. Later sweeps
must preserve the reconciled session version.

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
- An independent rerun of the corrected ordering case passed (one case, 1.84s)
  against the pinned unchanged source and executable. The earlier independent
  failing run remains associated with the superseded repair in the evidence.
- Both final disposable PostgreSQL fixtures and the independent rerun report
  verified shutdown, closed loopback ports, no remaining owned processes and no
  unconfirmed births.
- Reconciliation also runs on empty source pages, and the importer repeats after
  completed sweeps without new events. More than 16 ready receipts therefore
  drain through subsequent bounded pages; earlier-page children stay eligible.

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
