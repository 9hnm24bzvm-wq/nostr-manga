# Nullable and Unicode chapter receipt review

This test-only supplement keeps production source `9145d8e654c551a66a1773022cf02bbc53d974a3`
unchanged. A possible empty-string-to-null receipt mismatch was investigated, not
assumed. Current native and public source distinguish these representations:

- `backend/src/domains/catalog/validation.rs`, `string` and `nullable_string`:
  Rust Unicode whitespace trimming preserves a supplied empty string, omission
  is absent, and explicit null is an explicit nullable value. The existing unit
  test specifically asserts empty-string preservation.
- `backend/src/domains/catalog/repository.rs`, `update_chapter`: presence bits
  select the supplied value, including empty string or explicit null. The update
  does not use COALESCE to erase a nullable edit's presence.
- `backend/migrations/0088_catalog_correlated_visibility_lookups.sql` preserves
  title/volume/chapter columns in the native projection. JSON nulls may be
  stripped, not empty strings.
- `backend/src/public_api/catalogue_dto.rs`, Chapter projection: `nullable`
  preserves supplied strings; missing/native-null values become JSON null. This
  is not the separate `nullable_nonempty` helper used elsewhere.
- `platform-integrations/public-api/validation.ts` and `normalize.ts`:
  `nullable` and the actual `normalizeChapter` preserve empty strings. The UI
  presents native null as an empty input without converting it to literal text.

The signed adapter's existing input contract accepts strings for these controls
and rejects explicit null before dispatch. That is distinct from the direct
native/public API's nullable input support. This review neither broadens that
adapter contract nor describes it as full nullable-input support. Omitted
adapter fields remain omitted and retain their native representation.

The exact expected-receipt comparison already used the explicit Unicode
`White_Space` property, not JavaScript `String.trim`. U+0085 NEL is trimmed;
U+FEFF BOM is not. Mixed outer/inner boundaries preserve every non-whitespace
code point. Accepting null for an empty-string intent, or substituting JavaScript
trim's different boundary set, would weaken or break exact receipt validation.
No production normalization change was warranted.

## Executed controls

The actual pinned mounted component, normalizeChapter and signed controller pass
27 focused cases, including empty/ASCII/Unicode clears for all three nullable
fields, NEL/BOM boundaries, exact omitted fields, native-null presentation,
explicit-null adapter rejection, and contradictory null/undefined/string replies.
The finite HTTP seam models the source-confirmed server response; it is not a
live database or native Rust execution claim.

All 63 earlier uncertainty tests also pass, including partial202 source
uncertainty, wrong target/version/parent/account, malformed/5xx, interruption,
account/service changes and storage failures. The broader suite reports
505 passes, zero failures and two optional older-source controls skipped.

The exact c392b9d component/controller passes 20 unaffected focused cases and
fails all seven contradictory-receipt fencing assertions. Separately, an
explicitly artificial control replaces only the current comparator's Unicode
normalization with `String.trim`; all six selected NEL/BOM assertions fail.
That differential control is not presented as historical source. All failures
are behavioral assertions rather than harness/import failures.

The source files and sanitized result hashes are recorded separately. Raw logs
remain private. No new Cargo, service, browser, publication or integrated build
was run for this supplement; earlier DTO qualification remains unchanged.
