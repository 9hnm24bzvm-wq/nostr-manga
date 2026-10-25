# Source-backed link dictionaries

## Requirement audit

The supplied native frontend and the pinned MangaDex API are different sources, not interchangeable evidence.

- `frontend/src/@types/ExternalLink.d.ts` defines only `{ code, link, valid }`; `frontend/src/@types/Manga.d.ts` and `frontend/src/lib/API/Manga.ts` use one `links: Record<string, string>` map.
- `frontend/src/lib/ExternalLink.ts` supplies 12 codes (`al`, `ap`, `mu`, `nu`, `kt`, `mal`, `bw`, `amz`, `ebj`, `cdj`, `raw`, `engtl`). Matching provider URLs for the first seven become provider identifiers. The latter five retain full URLs. The retained editor adds/removes local rows and sends that dictionary; it has no provider identity UUID, language/region record, or separate provider registry operation.
- `frontend/src/lib/components/draft/manga/MangaEdit.svelte` passes `Permission.Manga.canEditLinks(manga)` into its links section. `frontend/src/lib/permission/manga.ts` makes that permission distinct from ordinary title updates.
- The official pinned [MangaDex API schema](https://gitlab.com/mangadex-pub/mangadex-api-docs/-/raw/60a3bb3c9f25675435a479c925d857015201739e/static/api.yaml), commit `60a3bb3c9f25675435a479c925d857015201739e`, SHA-256 `004031ccdfc4b66727dd1f85d09b5300cf60235bbab2de6f6ea4816094bd6d33`, 195189 bytes, defines **both** `links` and `officialLinks` independently as object dictionaries with string additional properties. `MangaRequest` lines 5161–5168 and `MangaAttributes` lines 5413–5420 contain those definitions; `MangaCreate`/`MangaEdit` lines 5480–5492 inherit them. The inventory remains the pinned 113-operation contract.

There is no schema support for inventing an official provider UUID/language/region model. Conversely, implementing legacy `links` alone is not equivalent to supporting the separate `officialLinks` field. Earlier rich-editor evidence asserted the former unsupported premise; its historical test outcomes are retained and explicitly corrected, not reused as evidence for the new field.

## Narrow compatible implementation

`POST /api/v1/manga` and versioned `PUT /api/v1/manga/{uuid}` accept independent `officialLinks` dictionaries. GET entity/draft/collection projections return `attributes.officialLinks` when stored. The native storage key is `catalog_manga.metadata.official_links`, separate from `metadata.links`. Omitted fields are preserved; `{}` explicitly clears just the selected dictionary. Empty keys/values, opaque identifiers, whitespace and ordinary URL-looking strings retain their exact value. No URL expansion, trimming, provider guessing, automatic navigation or nested identity object is performed.

Local resource/storage exceptions are explicit: at most 100 entries, key 100 UTF-8 bytes, value 2048 UTF-8 bytes, the existing 64 KiB raw JSON request budget (the signed browser client retains its stricter 32 KiB preflight budget), and no NUL in PostgreSQL text keys/values. These limits are not falsely attributed to the upstream schema. Invalid stored values fail public projection closed rather than silently dropping entries. The original `links` rules remain unchanged; the rich UI preserves untouched/unknown legacy codes and continues their established provider identifier-versus-URL handling.

Every officialLinks mutation includes the actual captured `expectedAccountId` UUID in the signed body. The existing final bound-account transaction rechecks NIP-98 proof, service/admission, explicit current binding, verified/active account, actual target UUID and its current version. Published edits require both `manga.update` and `manga.link.edit`; own unlocked drafts retain the native `manga.create` rule. Foreign drafts and unavailable/locked sources remain constrained by the existing native core. Both dictionaries use the same link permission, not source publisher or group authority.

The actual Svelte editor exposes separate key/value rows, explicit add/remove, no clickable interpretation of official strings, and disables both dictionaries without link authority. Programmatically forced disabled-control changes are rejected before a mutation. Unrelated allowed edits omit and preserve both maps. A success receipt must acknowledge the exact officialLinks map in addition to target/account/version checks; missing, malformed or contradictory acknowledgement leaves the existing unknown-outcome fence in place. A GET does not clear it and no mutation is automatically replayed.

Official link annotations are instance-local. They do not modify an original signed event, original manifest, media bytes, source publisher, group identity or consent. This adds no registry, provider route, migration, account/key, remote fetch or publication mechanism. Original 676 attachment frontend files remain untouched; the additions are composed integration controls and existing backend adapter/core extensions.

## Verification scope

See `test-evidence/frontend/official-links-20261022/checks.json` for exact candidate hashes, commands, outcomes and external raw-log digests. Tests include real helpers/controllers, compiled actual Svelte routes in JSDOM with explicit finite service/signing stand-ins, an old-editor-source negative control, full composed semantic checking, backend contract units, and the owned synthetic signed HTTP/PostgreSQL fixture. DOM evidence is not a real browser acceptance claim; synthetic HTTP is not production/account acceptance. No hosted workflow, production service, real credential, local browser or independent publication was used in this lane.
