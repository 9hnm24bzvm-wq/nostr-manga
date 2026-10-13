# Reviewed export migration into the PostgreSQL node

`src.postgres_migration` converts supplied MangaDex-shaped resources and local originals into a deterministic, reviewable import plan. It reuses the offline field adapter described in migration.md, but applies the selected chunked library schema and actual PostgreSQL HTTP APIs. No MangaDex login, catalogue query, scraping or image download is performed.

## Plan

```sh
PYTHONPATH=vendor/pydeps:. python3 -m src.postgres_migration plan bundle.json plan.json \
  --originals /absolute/originals --publisher YOUR_PUBLIC_KEY
```

The bundle has series resources, chapter entries containing a chapter resource plus ordered_originals, and an optional library export (read_markers, statuses, lists). An optional mapping file supplies exact previously imported series/chapter IDs. Planning needs a public key, not a private key. It reads and validates original bytes and preserves page order, string/null chapter labels and source metadata sidecars.

Unsupported fields in the supplied library envelope produce an explicit library
review error, not a partially imported library. In particular, follow state has no
defined migration field and must not be silently converted to a reading status.
Known nested response envelopes remain supported. A plan containing errors cannot
be applied; retain the supplied source export while reviewing unsupported state.

The plan records its import timestamp and deterministic unsigned event IDs. Series IDs are then used to form chapter manifests; chapter IDs map exact read markers. Ambiguous/duplicate source identities, missing relationships and unsupported inputs are reported before applying anything. There is no title/number guessing or protocol revision/equivalence linking. Batches are bounded; large migrations use multiple reviewed batches and explicit mappings.

## Apply

```sh
PYTHONPATH=vendor/pydeps:. python3 -m src.postgres_migration apply plan.json \
  --originals /absolute/originals --origin https://your-instance.example \
  --service-key EXPECTED_SERVICE_PUBLIC_KEY --key-file /private/importer.key \
  --journal /private/import-journal.json --accept-review
```

The existing signing key must match the reviewed publisher and already have the required node authority. The helper does not create credentials, enable accounts, guess group authority or configure peers. It pins the expected service identity using a fresh nonce proof. HTTP outside literal loopback requires HTTPS; proxies and redirects are disabled. Connection credentials/private keys are not included in the review plan or journal.

All selected bytes and unsigned events are checked before the first upload. Each immutable publication is uploaded, submitted with its fixed ID and awaited through backend projection. The private atomic journal records completions. Reapplying the same plan rechecks the node using the same IDs, so a stale journal cannot bypass a moderation tombstone. Changed originals invalidate the plan.

## Public library consent

Add `--publish-public-library` only after reviewing that reading history becomes public. Without it, only publications are imported. Private legacy lists are excluded rather than silently made public. Unmapped read/list references block library publication. The importer refuses to replace a different existing library; explicit merge/review is required. Exact retry of the same library is supported. Record update timestamps indicate import time, not invented historical reading dates.

## Evidence and boundaries

`results/postgres-migration-plan-tests.log`:32 adapter/plan tests. `results/postgres-migration-http-regression-2.log` and `results/postgres-migration-http.json`: actual two-node PostgreSQL/HTTP/strfry import, original ordering, labels, consent, retries, exact progress mapping and independent receiver reads after source shutdown.

This is a migration tool, not a drop-in MangaDex API facade or a determination of publishing rights. Existing MangaDex clients cannot simply switch base URLs. Keep plans/journals and original supplied exports in your own backup scope; they are external operator artifacts, not automatically part of node storage.

## Legacy compatibility aliases

The new aliases subcommand registers a reviewed series/chapters source-UUID-to-Nostr-ID mapping through the operator-authenticated compatibility endpoint. It pins service identity and verifies response receipts, does not republish content or history, and is idempotent for the same mapping. A conflict cannot reassign an existing alias. Registration is a separate explicit operator action because an importing publisher does not automatically control the instance's legacy UUID namespace. See mangadex-compatibility.md for commands and partial-batch retry semantics.
