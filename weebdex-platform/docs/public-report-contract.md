# Public report contract

Recovery status: RECONSTRUCTED-UNVERIFIED after the executor replacement on 2026-10-15. The original implementation was lost before upload. This document describes the reconstructed intended behavior; fresh verification is required before treating it as confirmed.

Source: MangaDex OpenAPI 5.13.1, the pinned source and hash in `mangadex-operation-matrix-v1.json`.

The reconstructed instance implementation provides these report operations under `/api/v1`:

- `GET /report/reasons/{category}` returns UUID report reasons with localized `reason`, `detailsRequired`, `category` and a persisted positive `version`, inside a collection envelope.
- `GET /report` returns a collection of reports. Supported filters are `category`, `reasonId`, `objectId` and `status`; ordering is `order[createdAt]=asc|desc`; `includes[]=user|reason` expands only those relationship attributes. The default limit is 10, the range is 1–100, and offset is bounded at 10,000.
- `POST /report` accepts an application/json object containing `category`, `reason` (UUID), `objectId` (UUID), and optional `details`, with an 8 KiB body limit. Success is HTTP 201 with `{"result":"ok"}`.

The public categories are `manga`, `chapter`, `scanlation_group`, `user` and `author`. The native-only `thread` and `reply` categories remain available through the existing native interface. Statuses are `waiting`, `accepted`, `refused` and `autoresolved`. Native short IDs are never accepted or emitted by the public report transport. Existing aliases are accepted, and output uses their durable preferred UUIDs. Reason identities use the separate `report_reason` namespace, so a manga, user or report UUID cannot substitute for a reason.

## Authority and privacy

These three operations inherit authentication in the upstream spec. Here, each requires a fresh original-module Nostr request capability bound to the exact configured service, URL, method and body, plus an active, verified native account with an explicit operator-managed Nostr binding. Original admission, binding and account state are rechecked and locked in the same transaction as the protected operation. A public UUID, cookie, native bearer token, or admitted but unbound Nostr key cannot create or read a private report.

Creation also requires the native `report.create` permission. The shared native mutation core retains reason/category matching, required details, current source visibility, target existence, group-claim safeguards, per-user serialization, a limit of 20 reports per hour, waiting-duplicate rejection, and an atomic create audit record. Duplicate and rate limits retain the native 409 and 429 errors. A POST cannot choose another submitter, status, version or review outcome.

Ordinary users see only their own reports. The existing instance staff queue remains available only to an account that has both a native moderator role and `report.update`; either alone is insufficient. Soft-deleted reports are excluded. The public response contains report details, canonical object UUID, status and creation timestamp. It does not expose internal IDs, moderator notes, audit records, account contact details or target content. A report's private history can retain a target UUID after withdrawal, but cannot expand or reveal the withdrawn target. The `user` expansion is the submitter's public username, roles and version, and `reason` expands the reason attributes.

Reports, reason administration and review decisions are local instance moderation data. They are not signed relay events, do not claim to submit to MangaDex, and do not grant protocol or global moderation authority. No automatic announcement is sent.

## Migration and verification required

Migration 92 adds durable reason aliases, backfills existing reasons set-wise, creates aliases for future native inserts, and increments a reason's version on label, details requirement or enabled-state changes. Identity/category changes are rejected. Existing migrations and native keys remain unchanged.

`backend/tests/public_reports.rs` reconstructs the actual TCP HTTP fixture using an owned disposable PostgreSQL instance, with synthetic keys and accounts only. It covers genuine signed requests, missing/denied admission and bindings, wrong service/URL, replay, payload tampering, native-cookie non-authority, permissions and ownership, aliases and wrong-kind rejection, strict fields and body/query bounds, pagination/ordering/expansions, versioning, hidden/deleted targets, native audit, concurrent duplicate serialization, rate guards, additive migration and immutable alias behavior. The unchanged native `community_http` suite must also be run to recheck existing report/review/group-claim behavior.
