# Pinned MangaRequest.primaryCover

The inspected MangaDex 5.13.1 schema defines `MangaRequest.primaryCover` as a nullable UUID (lines 5214–5217; source SHA-256 `004031ccdfc4b66727dd1f85d09b5300cf60235bbab2de6f6ea4816094bd6d33`). This is a supported field, not an optional local feature. The schema does not specify its clearing/fallback behavior or make every property required.

## Persisted behavior

- PUT `/api/v1/manga/{uuid}` accepts an omitted field, `null`, or a public cover UUID. Omission preserves existing explicit selection. Null clears all live explicit main flags. A UUID selects a visible, non-deleted cover already owned by that exact title and demotes its previous explicit main.
- Any explicit `primaryCover` requires signed `expectedAccountId`, including null. Ordinary title version/CAS and mutation authority remain mandatory. Published titles require both `manga.update` and `manga.cover.edit`; locks and pending original-publication intents remain enforced. Native draft-owner behavior remains `manga.create`, as in existing title and cover editing. It does not grant access to someone else's draft.
- The adapter only resolves typed UUIDs and normalizes presence. The shared native title core invokes the guarded cover repository operation inside the same transaction. The repository independently rechecks title and cover authority, current title version, same-parent cover identity and live source visibility.
- The title is locked before any cover row, matching native cover upload/edit/delete writers. The title increments once per accepted PUT. Each changed cover increments once with `main-select` or `main-clear` audit evidence; unchanged selected/cleared state does not invent a cover change. A stale cover editor therefore cannot overwrite a title-driven change using its previous cover version.
- POST accepts omission or null because a fresh title has no cover to select. POST rejects a UUID: a pre-existing cover cannot belong to the new server-assigned title. Upload a cover after creation; this API never reparents a foreign cover.

## Explicit-main versus fallback projection

Null clears explicit preference, not every visible cover or its stored image. Existing title projection selects explicit main first, then newest visible cover (`created_at DESC`, stable ID tie-break). Therefore `cover_art` may still contain the newest visible cover after null. This preserves the retained catalogue's established fallback and is documented as a schema ambiguity, not asserted as a verified upstream MangaDex behavior. No new UI field or duplicate cover-management surface was added; existing cover-main controls remain available.

## Original authority and nullable dependency

Selection is an instance-local metadata operation. It never reparents a cover, changes source-event identity, rewrites signed source events/manifests, replaces original bytes, or issues a source publication/deletion. Reimport does not reassert the cleared imported cover flag. Pending immutable title publication intents are refused before mutation.

The implementation was based on integration `5cb8111` plus unchanged nullable-field source `cf77890effda108192dbbd786f6284b609c65bdc` (local cherry-pick `c67566a`). That dependency's chapter and cover presence-aware null edits are preserved. `primaryCover` reuses the existing tri-state validator; no parallel nullable-field repair was introduced.

## Verification scope

`tests/support/primary_cover_contract.rs`, included by `public_manga_writes.rs`, exercises real transaction rollback, signed HTTP selection/clear/omission, GET projection, empty creation, account intent and binding changes, title/cover-specific permission refusals, stale title/cover versions, concurrent title CAS, foreign/deleted/hidden covers and parents, immutable publication intent, and byte-for-byte original retention after clear/reselect/reimport. Separately selectable HTTP selection and null probes fail against unchanged integration source `5cb8111` with HTTP 400 `Unknown field: primaryCover`, while both pass on the repaired source.

Native browser acceptance is not claimed. No production data, publication, new credentials, or browser launch is involved in this verification. All test identities, keys and databases are owned synthetic fixtures.

## Frozen verification

Source commit: `bd54b5f0c2c25f293727f19693b4bcd318aaf54e`. The sanitized [verification receipt](../test-evidence/primary-cover-contract-20261022/verification.json) records exact source/binary hashes, compiler settings and outcomes. All 152 library tests pass (two existing ignored cases). All six selected real-HTTP suites pass: comprehensive primary-cover contract, isolated selection/null probes, neighboring signed-title flow, nullable catalogue edits, and signed-chapter flow. All eight final positive/control fixture lifecycles shut down cleanly without forced termination or remaining owned processes.

These targeted results are not a full application/browser qualification or an independent review approval. Raw logs, runtime databases and synthetic key files are not included in the receipt.
