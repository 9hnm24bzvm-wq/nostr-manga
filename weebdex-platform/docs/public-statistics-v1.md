# Public MangaDex statistics boundary, v1

Recovery status: **RECONSTRUCTED-UNVERIFIED**. This document and the accompanying source were reconstructed from retained tool context after the execution environment was replaced on 2026-10-15. Fresh compilation and tests are required; earlier verification does not verify these reconstructed files.

Contract: MangaDex API 5.13.1, instance API base `/api/v1`.

## Routes and available data

- `GET /statistics/manga/{uuid}` and `GET /statistics/manga?manga[]=...`
- `GET /statistics/chapter/{uuid}` and `GET /statistics/chapter?chapter[]=...`
- `GET /statistics/group/{uuid}` and `GET /statistics/group?group[]=...`

Responses use the upstream `{ "result": "ok", "statistics": { "<canonical UUID>": ... } }` envelope. Manga statistics provide `follows` from actual persisted `library_manga_follows` rows and `unavailableChapterCount` from non-deleted, source-visible native chapter rows whose `is_unavailable` flag is true. Zero means an actual count of zero rows. Stored historical aggregate ratings or follows are not treated as per-rater or current-follow evidence.

All six operations are **partial**, with the following explicit gaps:

- `comments` is `null`: this instance has no MangaDex Forums bridge. Existing native discussions and replies keep their original native text IDs and original APIs. A native discussion or reply ID is never cast, hashed, or fabricated into an external numeric `threadId`. Native replies are not misrepresented as a MangaDex Forums `repliesCount`.
- Manga `rating` is absent: the native database currently has no authenticated per-rater records or population necessary to derive average, distribution, and Bayesian ratings. The legacy numeric counter is insufficient evidence. No zero histogram, arbitrary prior, upstream score, or invented rating is returned. Authenticated rating ingestion and its authority model remain pending.

The pinned upstream response schema does not require these properties. Its `StatisticsDetailsComments` object is nullable. `rating.average` is nullable, but `rating.bayesian` is explicitly non-nullable; the latter is therefore omitted together with the unsupported rating object, rather than emitted as an invalid null value. This is wire-compatible optional-field handling, not a claim of complete feature parity.

Source: [pinned official MangaDex API 5.13.1 schema](https://gitlab.com/mangadex-pub/mangadex-api-docs/-/raw/60a3bb3c9f25675435a479c925d857015201739e/static/api.yaml), paths `/statistics/*`, schema `StatisticsDetailsComments`.

## Identity, visibility, and budgets

Every path/query identifier must be a hyphenated UUID. Alias lookup is kind-scoped, does not manufacture aliases, and emits only the durable preferred UUID. Multiple aliases for one target collapse to one canonical result. Wrong-kind, unknown, deleted, draft, cached-withdrawn, and live-source-hidden resources do not reveal native identifiers or counts.

A single inaccessible resource returns the same public 404 error as an unknown resource. Batch results omit inaccessible/unknown/wrong-kind IDs. Invalid UUIDs reject the whole request with a public 400 error. Database failures produce a public 503 rather than a successful empty result. Error IDs are UUIDs; errors never contain SQL, credentials, internal errors, or native keys.

Batches require 1–100 repeated values under the exact endpoint-specific bracket-array key. Encoded brackets work. Duplicate input values count toward the transport budget and are deduplicated before lookup. Other query parameters, invalid percent encoding, and queries over 32 KiB are rejected. Single-resource routes have no query parameters. Invalid UTF-8 path extraction is translated into the public JSON 400 envelope. Unsupported methods have a public JSON 405 response.

The implementation performs at most one bounded alias query and one set-based visibility/count query per request, independent of batch size. Aggregations are restricted to requested visible resources. The HTTP router's existing admission and no-store policies apply. No resource, protocol event, alias, user setting, follow, or discussion record is mutated by these routes.

## Fresh verification required

`backend/tests/public_statistics.rs` contains a real ephemeral loopback HTTP listener and the owned PostgreSQL runner. Its assertions cover all six routes, live exact counts despite stale aggregate counters, source-hidden/deleted unavailable-count exclusions, canonical legacy aliases, wrong-kind behavior, no alias creation on reads, no fabricated rating/forum data, strict malformed/over-budget queries, a 100-distinct-resource batch, public/draft/deleted/cached/live visibility, group source withdrawal, and safe pool-failure errors.

After restoring the foundation/resolver and integrating the source, run:

```sh
cargo test --offline --manifest-path weebdex-platform/backend/Cargo.toml \
  --lib public_api::statistics::tests
weebdex-platform/scripts/run-auth-tests.sh cargo test --offline \
  --manifest-path weebdex-platform/backend/Cargo.toml \
  --test public_statistics -- --ignored --test-threads=1
```

The runner must only use the disposable `weebdex_platform_test` database on a private nonstandard loopback port, record fixture ownership, and verify PostgreSQL shutdown. Its runtime logs are not source or production data. Do not claim fresh success until the reconstructed sources pass these commands and cleanup is verified.
