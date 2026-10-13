For the current reviewed PostgreSQL import/apply workflow and selected chunked libraries, see postgres-migration.md. This page documents the earlier offline adapter.

# MangaDex-shaped migration experiment

This is an **offline, unsigned preview tool**, not a live account connector or drop-in MangaDex API replacement. Tests use invented metadata and generated image fixtures. No MangaDex account, catalogue or image API was queried.

The official [OpenAPI document](https://api.mangadex.org/docs/static/api.yaml) was inspected on 6 October 2026; it identified itself as API 5.13.1. Its checksum is recorded in results/migration-source.json. That document defines localized series metadata, string-or-null chapter/volume labels, chapter UUID read markers, reading statuses and public/private lists. Actual service policies and API behavior must be reviewed before building a live connector.

## Implemented

- Preserve available author, artist and scanlation-group credit names as descriptive attribution, without inferring Nostr keys. Missing names are explicit review items; source UUIDs remain local.
- Convert a series resource into a deterministic-CBOR metadata proposal. Keep source UUIDs, relationships and source versions in a local review sidecar.
- After an actual series publication exists, use an explicit source-UUID-to-publication-ID mapping to create chapter manifests from an ordered set of local originals.
- Inspect each original's real bytes, SHA-256, size, format and dimensions. Preserve order and bytes; do not use filenames or MangaDex server hashes as trusted content digests.
- Preserve labels such as 001.5 and 03 as strings. Null labels are omitted, never fabricated.
- Reject link-only/external chapters from automatic conversion; no hotlinking or data-saver substitution.
- Preview read history using exact chapter UUID mappings only. Never infer a read publication from a matching chapter number, title, language or group.
- Preserve reading-status information separately; a completed series does not fabricate read markers for every chapter.
- Exclude private source lists from the proposed public fields pending explicit review. All read-history publication requires review because the destination library is public.
- Report unresolved IDs and illustrative full-event byte size; never silently truncate a large library.

The series optional metadata keys and library status/list fields are experimental migration proposals, not finalized protocol schema. No source account/group UUID is treated as a Nostr key, and no source author signature is forged.

## Commands

From the prepared project directory:

```sh
PYTHONPATH=vendor/pydeps:. python3 -m src.mangadex_migration series series.json new-series-packet
PYTHONPATH=vendor/pydeps:. python3 -m src.mangadex_migration chapter chapter-input.json new-chapter-packet --mapping mapping.json --originals ./originals
PYTHONPATH=vendor/pydeps:. python3 -m src.mangadex_migration library library-input.json new-library-preview --mapping mapping.json
```

The output directory must not already exist. No command signs or publishes anything. Keep review sidecars private; source library information may be private.

- Series input: a manga resource or an entity response containing it.
- Chapter input: an object with chapter (the source resource) and ordered_originals (relative local filenames in page order).
- Mapping input: series and chapters objects mapping explicit source UUIDs to exact 64-character target event IDs.
- Library input: read_markers (flat/grouped source response), statuses, and lists.

The supplied library envelope accepts only those three optional fields. Unsupported
fields (for example, `follows`) are reported explicitly before any importable
backup or preview packet is written; separate follow state is not implemented.
Existing response envelopes inside read_markers/statuses and wrapped list resources
remain supported. The PostgreSQL planner records a library review error instead
of producing a partial library; applying a plan with errors is refused before
contacting the destination. Keep the original export for review rather than
removing unsupported data merely to make an import succeed.

The script bounds JSON input, rejects path traversal and symlink escapes, validates actual local image data and produces a review packet. It does not guarantee the operator's mapping is factually correct or that they have permission to distribute supplied content.

## Evidence and remaining work

Dedicated migration tests passed, covering ordering, exact mappings, nullable labels, path safety, format policy, privacy separation and unresolved data. A live two-node synthetic integration accepted the converted manifests, replicated exact originals and mapped one source read marker to the actual new chapter event ID.

Still needed: an approved final status/list schema, complete field-level migration policy, attribution review, authorized real exports, account-specific consent, importer idempotency/recovery, a richer catalogue index and any desired MangaDex-compatible API facade. The target currently uses new Nostr identities and publication IDs; existing MangaDex clients cannot simply switch their base URL.
