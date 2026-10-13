# Catalogue metadata and original covers

Approved by the owner with A/A/A on8October2026. Schema47 adds a derived local catalogue index; signed event IDs and exact deterministic-CBOR bytes remain authoritative. Existing immutable delete/upload semantics and instance-local policy remain unchanged.

## Series manifest version1

Required titles is a localized BCP47-keyed text map. Optional fields are altTitles (array of localized maps), description (localized map), categories (free-form text array), credits (role/name pairs), originalLanguage, status, year, contentRating and publicationDemographic. Status/rating/demographic/credit roles are descriptive text, not a new globally enforced taxonomy. No case folding, trimming, synonym inference, author-key inference or series-equivalence links are introduced. Unknown optional fields remain in the retained original bytes.

One optional cover uses the same SHA256/size/MIME/width/height descriptor as a chapter page. A cover requires required:["series-cover-v1"], and the marker requires a cover. Older decoders therefore skip this required feature rather than falsely claiming complete hosting without the cover. Event kinds and pointer version are unchanged. The instance verifies and retains exact cover bytes before hosting, including through one-off review and approved-peer transfer. Shared cover/chapter originals remain protected when either publication is removed independently.

Implementation bounds:64 localized entries,1024 bytes per title,128 alternative-title maps,65536 bytes per description,256 categories of512 bytes,128 credit pairs (role64/name1024 bytes), optional year0..9999. Existing CBOR byte/depth/node limits and configured image-format/physical budgets also apply. These are advertised reference limits, not proof of deployment capacity.

## Queries and indexing

GET /series uses the existing explicit order=asc/desc, live offset/limit, signed created_at since/until, common filter and optional exact total. Additional title, search, category, status and year parameters are bounded and parameterized. title matches an exact primary/alternative localized value. search is a literal case-sensitive substring; percent/underscore are not wildcard syntax. category is exact free-form text. Only hosted, projected, non-quarantined publications are eligible. No implicit synonyms are applied; an instance may resolve its own aliases before submitting exact predicates.

GET /catalogue/{event_id} returns the signed event and typed derived metadata. The original retained blob preserves optional unknown fields. Chapter volume and title are also indexed and supported by the existing filter grammar; labels remain text and do not establish equivalence. For series, chapter-only predicates remain absent/SQL NULL, preserving three-valued semantics.

Exact title lookup uses a GIN title-value array; category containment uses JSONB GIN; status/year have indexed expressions. Substring search and exact totals have a bounded statement timeout and may require further measured optimization. Queries return no successful partial result on budget failure. These indexes are not a target-scale benchmark.

Legacy supported hosted manifests backfill in64-row quanta. Unsupported/malformed legacy descriptive metadata is recorded for operator review without rewriting or deleting the publication. Unavailable manifests do not block later records. GET /admin/catalogue/errors exposes bounded operator-only diagnostics; POST /admin/catalogue/reindex with an event ID clears that derived error for a later retry. Neither operation changes publication authority or signed content. All public catalogue routes honor the instance read policy.

## Import and evidence

The offline series converter accepts --cover LOCAL_FILE with --originals DIRECTORY. Reviewed multi-record import bundles accept covers mapping source series UUIDs to local original filenames. The converter verifies the original and stages it before the series manifest/publication; external links and covers without a corresponding series resource are errors. Source UUIDs remain local mappings.

The62-group combined functional regression, Clippy, independent-process/PostgreSQL restart and112 Python tests pass. The actual two-instance catalogue fixture checks exact-case/substring queries, required original possession, preservation of unknown raw bytes, replicated covers after source loss, textual volume/title filters and shared-original retention after series-only deletion. The additional restart/backfill, authenticated-route and operator-only diagnostic checks pass, as does one-off review of series with shared original covers. No UI or target-capacity claim is made.
