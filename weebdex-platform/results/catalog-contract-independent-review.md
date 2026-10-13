# Independent catalog contract source review

Reviewed 2026-10-13 UTC, after the relation PUT/version repair and additive Cover DTO migration 0085.

## Result

The scoped **backend repair is source-clean**: no new actionable defect was found in relation authorization, owner-version handling, transaction boundaries, duplicate behavior, no-op behavior, related-title eligibility, response projection, or the Cover extension/visibility adaptation.

The retained caller trace identified **one concrete, pre-existing frontend permission blocker**. It prevents eligible non-staff users from reaching the relation controls. This is not a backend authorization leak or a regression introduced by migration 0085 or the relation handlers.

This is a finite source review, not production proof. I did not execute Cargo, application/runtime tests, SQL, migrations, HTTP requests, or browser E2E. I made no application-source changes. Newly added HTTP assertions were read as test source, not counted as executed evidence.

## Finding: P2 — live non-staff relation controls are always disabled

### Exact locations

- `frontend/src/lib/permission/manga.ts:92-96`: `canEditRelations` returns false for non-staff when `manga?.locked || !manga?.deleted_at` is true.
- `frontend/src/lib/components/draft/manga/MangaEdit.svelte:720`: the actual editor passes this helper's result to `RelationsSection.enabled`.
- `frontend/src/lib/components/draft/manga/MangaCreate.svelte:273`: creation calls the same helper with no manga argument.
- `frontend/src/lib/components/draft/manga/RelationsSection.svelte:26-29`: the false value disables the relation group and displays “You do not have permission to edit relations.”
- `backend/src/domains/catalog/repository.rs:513-527`: the actual backend authorizes an unlocked published title with `manga.relation.edit`; a live draft owned by the actor is authorized with `manga.create`.

The shared, reader, and local-library overlay modules have no patch for `canEditRelations` or `src/lib/permission/manga.ts`. The retained helper therefore still executes; the repaired relation request overlays do not remove the gate.

### Source-backed reproduction

1. Use a verified browser account without sysop/staff/moderator roles, with `manga.relation.edit` and the ordinary permissions needed to open the title editor.
2. Open an unlocked, live published title. Its `deleted_at` is absent/null, so `!manga?.deleted_at` evaluates to true.
3. `canEditRelations(manga)` returns false before examining `manga.relation.edit`; the Relations tab reports lack of permission and the relation controls cannot initiate the repaired POST/PUT/DELETE flow.
4. For creation, a non-staff actor calling `canEditRelations()` has `manga === undefined`, making `!manga?.deleted_at` true as well. The create form cannot accept the relations that the backend permits the eventual draft owner to add.

This conclusion is direct control-flow evaluation, not a browser-run claim. Staff fixture acceptance alone would not expose it.

### Acceptance criteria

- An eligible non-staff browser actor can edit relations on an unlocked live published title using the existing `manga.relation.edit` permission.
- An eligible non-staff draft owner can use the existing relation controls under the backend's current owned-draft `manga.create` semantics; another account cannot.
- Creation admits relation drafting consistently with the existing creation permission, without inventing a saved title or weakening backend authorization.
- Locked/deleted titles, missing permissions, and unauthorized draft ownership remain disabled/rejected; preserve the existing staff exception.
- Preserve the supplied markup/styles by applying any permitted behavioral repair through the existing integration mechanism. Verify both ordinary eligible and ordinary ineligible browser actors, not only staff.

## Backend contract checks completed

### Relation requests and acknowledgements

- `backend/src/domains/catalog/mod.rs:37-42` registers GET, POST, the new PUT, and DELETE on the same relation route.
- POST remains create-only and returns 201. `mutations.rs:763` uses conflict-do-nothing and maps an existing pair to 409 without updating its type.
- PUT requires a positive owner version; POST/DELETE allow an optional owner version and validate it whenever supplied (`mutations.rs:823-877`, `906-925`; `validation.rs:89-95`). Self relations and unknown/malformed fields are rejected.
- The POST/PUT acknowledgement is the actual current projected related Manga/Relation, plus `manga_version` for the owning title and `relation_scope: instance-local` (`mutations.rs:885-904`). The related title's own DTO `version` is not substituted for the owner version.
- DELETE returns 204 with the actual post-delete owner version in `x-weebdex-manga-version` (`mutations.rs:947-953`). The response boundary preserves this header and applies `no-store` to API responses (`router.rs:476-485`). The deployed default API path is same-origin `/api` (`scripts/build-frontend.mjs:16`), so the header is readable by the retained XHR caller without a cross-origin exposure assumption.

### Authorization, atomicity, and visibility

- All three handlers use the same mutation principal/CSRF validation and `require_manga_edit(..., manga.relation.edit)` before mutation (`mutations.rs:752`, `760`, `788`, `796`, `912`, `924`, `969-971`). API/recovery identities cannot acquire the protected write permission through `SessionPrincipal.has_perm` (`auth/principal.rs:26-36`).
- `require_manga_edit` locks the owner title row before checking current edit authority and immutable pending publication commands (`repository.rs:513-537`). Reading/checking its version afterward occurs while that lock is retained, so owner relation edits and ordinary title writes serialize on the owner.
- A changed relation type, owner-version increment, and audit are all inside one transaction. PUT with an unchanged type returns the unchanged owner version and does not update/audit (`mutations.rs:799-820`). Missing relation, stale owner version, duplicate POST, or response projection failure leaves the transaction uncommitted.
- New/updated related targets must be published, non-deleted, locally source-visible, and canonically source-visible (`mutations.rs:879-883`). The response projection independently rechecks the owner and target and omits unavailable targets (`0082_catalog_projection_visibility.sql:14-16`). No related-title DTO is fabricated.
- DELETE intentionally permits removing a stored pair whose related target is no longer public; it does not return that target's DTO. The owner still passes the current visibility/authorization/pending-command fence.
- The repair is a local editorial relation write. It does not change protocol signatures/events, source identity, or publication command state.

### Retained request versions

- The exact overlay for existing edits chooses PUT when an acknowledged relation exists, POST for a new one, and DELETE for a removal (`platform-integrations/frontend-overlays.mjs:2786`). It sends current `manga.version`, requires the matching 200/201/204 acknowledgement, adopts the returned `manga_version`/delete header, and never uses `Relation.version` or a guessed increment.
- The create overlay uses POST201 with the acknowledged draft's current owner version (`frontend-overlays.mjs:2761`). Both paths retain acknowledged changes and reconcile uncertain outcomes through read-only current-state reads instead of replaying an ambiguous mutation.
- The HTTP overlay exposes the version response header to those callers (`frontend-overlays.mjs:277`). Actor/target lifetime checks remain in `frontend-behavior/authenticated-mutation.ts:21-49`.

## Cover extension and visibility checks completed

- Migration 0085 changes only the public projection's extension expression from `c.ext` to `'.' || c.ext`; its cover tombstone, current parent, local source visibility, and canonical source visibility predicates match the final 0082 helper. No older migration was edited for this adaptation.
- `backend/src/domains/mod.rs:181-184` registers it as ordered migration 85. Its SHA-256 is `3668bf28a8032cd4b2d4f485cca06625502aaf173fc3d6d7546f512765cae692`, matching the supplied candidate identity.
- Bare file/storage extensions remain constrained and stored in `catalog/repository.rs:385-404`; upload publication passes the original bare extension (`uploads/sessions.rs:1295-1308`). Native media lookup still compares the parsed suffix with bare `media_objects.extension` (`uploads/media.rs:77-78`); protocol originals still compare with bare `catalog_covers.ext` (`protocol/media.rs:161-190`).
- Original cover URLs concatenate ID and DTO extension in the retained Manga lightbox (`frontend/src/routes/(app)/title/Manga.svelte:167`), cover gallery (`MangaCoversSection.svelte:26`), and saved editor cover adapter (`frontend-behavior/editor-cover-read.ts:71-72`). The dotted DTO now yields the required `id.png` path. Fixed thumbnail paths remain unchanged.
- Draft cover reads remain caller-authorized through the existing endpoint wrappers (`uploads/sessions.rs:1173-1192`, `catalog/read.rs:579-608`); the public projection itself continues enforcing current parent/source/tombstone eligibility. The dotted extension adds no fields or visibility bypass.
- New native/protocol HTTP assertions inspect `.png`, the retained ID+extension path, and unchanged bare storage (`tests/upload_http.rs:1546-1568`, `tests/protocol_bridge_http.rs:1569-1608`). Their execution remains outside this review.

## Preference-tag fixture check

The reported recent tag join is fixture-only, not a production preference/filter implementation change. `backend/examples/browser_fixture/ui_regressions.rs:309-315` first creates or verifies the marker/name/language/rating identity of `UiPfLang01`, then attaches the existing base-fixture `SeedGenre1` using a parameter-free fixed pair and conflict-do-nothing. The descriptor advertises that exact pair at line 376. Existing source/filter and preference authorization behavior is unchanged; no performance recommendation is made.

Fixture helper SHA-256: `98b2a1be4c85fcb5152ba0ad395b4e92fb5d09c0680e2333a11428f2551ee944`.

## Reviewed source identities

- Relation mutations: `53e764a999f7fe00b85c122145f1a727588b44be92a90ff09630eecf04ca53d9`
- Catalog repository: `0e6f11fc4b3133da602d566368fe7d0c91d25b369ffd193ee3f54d9eba624785`
- Shared frontend overlay module: `6c2324801226db03d8671303a64d96548d789c1fe37768e3777f83fb418ac017`
- Retained permission helper: `fdf62a1220d8e13f446f4fc070788c78aded93baeef360ae88cee2b67827e799`

No additional actionable source defect was found within the stated backend/retained-caller/cover/fixture scope. Actual E2E and final runtime acceptance are still required before claiming those behaviors pass in a running deployment.
