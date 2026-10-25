# Signed cover-editor pagination

The signed original-cover editor reads explicit bounded pages of the existing private `/api/v1/nostr/manga/{mangaUuid}/covers` collection. This fixes the formerly hardcoded newest-100 window without changing cover mutation authority or the original image pipeline.

## Contract and scope

- Pinned `contracts/mangadex-5.13.1-inventory.json`, GET `/cover`, declares `limit` 0..100 and `offset` >=0 with no maximum. Its public default is10. The existing private signed route is a distinct extension, not a claim of full public filter/order compatibility; it retains its historical default100. The editor always sends explicit `limit=100`, `offset`, and captured `expectedAccountId`.
- The public catalogue parser's existing10,000 offset ceiling is not an upstream rule and is not copied here. The private route supports offsets through9,007,199,254,740,991, the exact JavaScript integer boundary. It returns exact totals or the client refuses an unrepresentable response. It never truncates a collection total, installs a storage/title cap, or silently clamps a requested page. No public `/cover` API compatibility change is included.
- Each page is a live repeatable-read snapshot: exact data and count share the same account/binding, parent/source-visibility predicates and transaction. Rows are ordered by created-at descending then native ID ascending, matching the prior route and its index. The title path is the sole parent selector; unknown, duplicate and cross-parent query controls fail. The full query URL remains covered by NIP-98 service-bound authentication. Existing no-query restrictions on cover mutations remain intact.
- `expectedAccountId` is optional for prior private clients but always sent by this editor. When present, it must match the current bound native account's canonical public UUID within the transaction. It does not grant permissions or replace live admission.

There is no upstream ambiguity about the pinned maximum page size or non-negative offset. The distinct private route's fixed ordering/default and numeric representation limits are explicitly described as local behavior, not accepted upstream exceptions.

## Reader and UI behavior

The actual controller reads exactly one page per request, validates the echoed offset/limit, exact page cardinality, total, UUIDs, provenance, uniqueness and one exact parent, then replaces its editable set. A failed replacement leaves no editable old rows. Overlapping reads/mutations, disposal, changed title identity/version, signer, account and service are fenced. Cover CAS and original descriptors/source-event immutability checks are unchanged; a title version change requires an explicit fresh cover read. Unknown mutations keep the existing no-replay pending marker across any number of reads.

Previous/Next controls expose all available pages and current exact range/total, including an older main cover. There is no background page loop. Changing page clears selection, form and verified preview; reloading a page preserves only a still-present selected UUID with its newly read version. Unsaved file/metadata changes require discard confirmation before selection, paging or reload. Cancelling preserves the exact fields and makes no request.

Uploading returns to the first/newest page. Updating retains the current page. Removal clears selection and refreshes the same page. If deletion shrinks a last page away, a single additional bounded request loads the last nonempty page. If the collection shrinks again before that correction, the editor shows no fabricated range and asks for another explicit read. Failed reads disable stale editing but keep the explicit reload button available. Preview object URLs are revoked on replacement, selection change and disposal.

As with any live-offset collection, concurrent insertions/removals can shift rows between independent page snapshots; a page is not a cross-request immutable snapshot. Every actual edit still uses its selected current cover version, account/service intent and server authority. This is not an automatic all-history export.

## Regression evidence

`tests/ci/test_public_cover_pagination.mjs` executes the real controller and signed transport with finite HTTP/signature stand-ins. `test_public_cover_pagination_dom.mjs` mounts the actual Svelte component with that controller in JSDOM. They cover205 covers with the main on page3,101-cover removal, counts, ordering as returned, explicit next/previous reads, stale/off-page and foreign rows, canceled edits, reload, failed/overlapping/late reads, original previews and pending no-replay fences. The shared existing command fixture was extracted without removing its59 prior regression cases.

`backend/tests/support/cover_pagination.rs` runs through real signed HTTP/PostgreSQL on the existing strong-owned disposable fixture, independently of other ignored tests. It creates205 permitted covers with an older main actual original, plus hidden/deleted/foreign records. It checks tie ordering, counts, wide offsets, query signature/account binding, real grants, edits/removal/version conflicts, private headers, exact original bytes, and visibility of genuine protocol-source covers without changing signed event bytes.

Execution status and limits are recorded separately in `test-evidence/signed-cover-pagination-20261022/qualification.json`. JSDOM is not real-browser rendering or accessibility acceptance. Semantic/build/browser qualification belongs to the integrated lane. No deployment, public publication, real account credentials or production database is used by this change. The supplied676 frontend files and3 existing additions remain unchanged; changes are additive integration modules and tests.
