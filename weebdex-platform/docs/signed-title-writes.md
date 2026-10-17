# Signed title drafts and metadata writes

The public adapter supports these explicit operations:

- `POST /api/v1/manga`: create a private native draft; returns a canonical Manga entity, HTTP 200.
- `PUT /api/v1/manga/{uuid}`: partial metadata edit with the current positive integer `version`; returns the canonical entity, HTTP 200.
- `POST /api/v1/manga/draft/{uuid}/commit`: submit for review, HTTP 201. Requires an existing, visible original-language cover linked to actual original media bytes.
- `POST /api/v1/manga/draft/{uuid}/publish` and `/reject`: retained-platform review extensions, HTTP 200. These are not MangaDex operations. Submission does not publish or approve anything.

Every write requires an admitted NIP-98 proof for the exact method, externally configured origin/path, service key and raw request body. Cookies do not authorize public writes. The same transaction rechecks service/admission and locks the explicit publisher/account binding and current user policy before invoking the native catalogue core. Native optimistic versions, draft ownership, locks, per-field grants and immutable prepared-publication fences remain authoritative. The public adapter never constructs a session principal.

## Fields and limits

The wire contract uses the hash-pinned MangaDex 5.13.1 `MangaRequest` field names: `title`, `altTitles`, `description`, `authors`, `artists`, `tags`, `links`, `originalLanguage`, `lastVolume`, `lastChapter`, `publicationDemographic`, `status`, `year`, `contentRating`, `chapterNumbersResetOnNewVolume` and `version` for edits. Create requires title, original language, status and rating. Authors/artists default to empty lists. Create rejects a caller-assigned version.

Localized maps preserve all validated translations. They accept up to 64 supported native language keys. Titles are nonempty and bounded by both the native 500-character and original protocol 1024-byte limits. Description text has the native 50,000-character aggregate budget; the complete JSON body is limited to 64 KiB. Alternate titles retain their localized-map grouping in the original manifest and public DTO. Author/artist/tag inputs are distinct, typed UUID lists, at most 100 each, resolved in the write transaction. A UUID from another resource family is not a native identifier or authority token.

Year, demographic and last-volume/chapter accept null to clear the value. `officialLinks`, `primaryCover`, source identity/group ownership changes, cover upload, relation mutations, lock/unlock and deletion are not implemented by this adapter and fail closed. Their absence must not be presented as full editor parity. Public draft submission's original-cover prerequisite is stricter than the pre-existing native submit handler. The existing native cover interface supplies actual cover bytes separately.

## Original publication and local annotations

Publishing uses the same durable native publication command and original protocol publisher as native review. A staff role and review grant alone cannot borrow the configured operator signer. Current explicit signer binding and original publisher admission remain mandatory, including replay. No key is generated, no publisher is admitted, and no original group key or group role is changed by these routes.

Native title/description translations and alternate-title maps feed the canonical kind-1002 original manifest. Original approval, backend projection and visibility remain independent. A pending publication response preserves the authorized draft fields and includes `attributes.nostr.eventId`, `publisher`, `publicationScope` and `publicationStatus`. The public title remains unavailable until original visibility converges. Import then records visible original provenance/status without changing optimistic versions or replacing local annotations.

Editing an already source-backed title changes this instance's metadata overlay only. The response discloses `attributes.nostr.metadataScope: "instance-local"`; the original signed event, canonical metadata, media and source ownership remain unchanged. The original source fields that have no representation in the protocol manifest remain instance metadata, consistent with the native core.

## Verification

`backend/tests/public_manga_writes.rs` owns a real signed HTTP journey with ephemeral PostgreSQL, synthetic accounts/images and the pinned strfry backend. It exercises actual cover upload, draft review, original publication and readable UUID convergence, and compares original event/metadata before and after local edits. It also checks foreign drafts, missing binding/grants, typed UUIDs, stale versions, exact proof method/target/service/body binding, replay, original publisher admission, account revocation, and grant revocation while the HTTP request is demonstrably blocked on the current authority row. Runtime material and generated keys stay outside checkpoint evidence.

Pinned upstream schema: `contracts/mangadex-5.13.1-inventory.json`, source SHA-256 `004031ccdfc4b66727dd1f85d09b5300cf60235bbab2de6f6ea4816094bd6d33`, commit `60a3bb3c9f25675435a479c925d857015201739e` of the official API documentation. This partial adapter does not claim every optional upstream field or workflow.

## Captured account intent

Dedicated signed title clients include `expectedAccountId` in the exact JSON proof body for create, update and all review transitions. The adapter removes this compatibility extension before native metadata parsing and compares its typed User UUID to the actual native actor inside the final `bound_account_write` transaction. A different currently eligible account returns409 before any catalogue mutation, even when both accounts have colliding create grants. Legacy clients may omit the extension and retain existing current-binding semantics; omission does not assert captured-account intent. Client preflight reads remain useful UI checks but are not the transaction-level guarantee.
