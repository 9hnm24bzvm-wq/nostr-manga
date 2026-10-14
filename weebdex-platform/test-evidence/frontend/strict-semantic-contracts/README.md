# Reviewed strict-semantic generation

Latest independently approved application source:
`47c4ec7a65e2a5f298939fae118c76a7373b9ccc`.
Its exact-source checker, 132 seams, bootstrap, parity, 110/591 artifact
acceptance, all ten negative controls, cache proof, and bounded ownership receipts
are in [`generation-47c4ec7`](generation-47c4ec7/). The records below remain the
historical source-bound generation at `0a18be23`; they are not overwritten.

The final follow-on validates the complete outgoing Apply query after sentinel
and default-key mutations. A shared effective-read helper also validates the
preference/default-expanded query before both API reads and cache admission,
while retaining early invalid-link rejection. Independent source review passed
132 seams plus 12 additional checks, including exact final 16,384-byte admission
and 16,385-byte rejection, owned-field corrections in both modes, and retention
and blocking of invalid unknown passthrough. Valid boundary counterparts retain
limit zero, nondefault sort/order and the selected tag modes. The source/map
cardinalities and acceptance tests are unchanged by this final follow-on.

Application source: `0a18be23b5e43979f7f5b6336faa7d91a038a412`.
Checker baseline: `899e0be3cb2d22874faae765dac6db291c6a1986`.
These structured results were collected locally against that exact application
commit. Absolute checkout, dependency-cache, and executable locations are replaced
with labeled placeholders; the original owner receipts and full generated
artifacts remain in ignored local build output. No runtime database, credentials,
cache contents, or raw process logs are included here.

## Results and boundary

- Actual production-composed strict/checkJs checking: **0 errors, 15 warnings**.
  The warnings are retained; no unrelated warning repairs are included.
- **119** helper and actual composed-script seam assertions passed. The tests
  exercise cache admission, initial reads, preference application, Apply and
  correction paths, signed-receipt consumers, transferable views, and guarded
  browser-extension behavior using explicit platform stand-ins.
- The five bootstrap entrypoints passed with 87 composed modules evaluated each.
- All **676** attachment files and the complete **679** canonical frontend hash
  inventory remain byte-identical. This establishes source preservation, not
  screenshot or visual parity.
- Artifact acceptance verifies **110** transformed original sources, **591**
  production-bound maps, all canonical identities/hashes, map cardinalities,
  actual source graph, strict flags, worker checking, and both cache roots.
- All **10** fail-closed controls rejected their intentionally malformed evidence
  or guard configuration. The missing-map control still fails specifically at
  the mapped-source cardinality assertion. The one intentional denied write
  targets only a synthetic checkout-local probe cache.
- All **42,805** dependency-cache byte/stat records are unchanged; actual-cache
  Node guard audits record zero write attempts. The guard is in-process
  verification plus complete snapshots, not OS syscall enforcement.
- Every final bounded command terminated successfully before its deadline with
  no cleanup signals or surviving owned descendants. No unrelated process was
  touched. No browser, service, database, network test, or dependency install was
  used. This is not a new end-to-end browser or production-bundle claim.

## Source-bound acceptance changes

Only the generation expectations in the two artifact tests change. The checker
itself has the same SHA-256 as the reviewed baseline:
`ad526c8bc80d8792c0bc81e61ee4533717e27e95c5fe0a02a0e7d1e0db064bc6`.
Historical baseline receipts are retained unchanged.

The integration-source manifest contains 110 actual transformed sources, up
from 107. The three additional transformed original paths are:

- `src/lib/components/AppHeader.svelte`
- `src/lib/components/Lightbox.svelte`
- `src/routes/(app)/search/+page.svelte`

The actual projection manifest contains 591 maps, up from 586. The five new
source identities are:

- `platform-integrations/frontend-behavior/install-prompt.ts`
- `platform-integrations/frontend-behavior/manga-search-query.ts`
- `platform-integrations/local-library/graph-transfer.ts`
- `platform-integrations/local-library/protocol-receipt.ts`
- `platform-integrations/local-library/protocol/types.d.ts`

`generation-counts.json` records those set differences from the baseline source
manifest. The full canonical and projection hash inventories, map-validation
records, and current integration manifest substantiate the counts. Acceptance
still requires the exact counts and source identities; it additionally requires
the current summary to pass with zero errors. Error-category and graph-group
expectations move from the historical 232 errors to zero. The malformed-map
control's expected message is updated to 591 without changing its removal or
rejection checks.

## Necessary behavior changes

Search URLs are parsed by known field contracts before cached-result admission
or an initial API read. Bracketed/repeated arrays and accepted `0`/`1` modes are
canonicalized before preference and default-limit application. Page is validated
as a singleton 1–1,000,000, retained on initial reads, and excluded on Apply.
Recognized shared transport keys are also validated, including library username
singleton handling; truly unowned parameters remain passthrough and cannot
override validated filters or reintroduce page on Apply.

Malformed filters now use the existing visible error/toast state. Correcting the
relevant UI field clears its historical field-owned error, while the entire
outgoing query is revalidated. Remaining excess budgets or malformed unknown
passthrough stay blocked. Reset deliberately clears that URL state. False, zero,
list filters, author/artist distinctions, accepted legacy `mx`, and the actual
included/excluded AND/OR radio mapping are preserved. Explicit library
`contentRating=__none__` survives Apply instead of widening to default ratings.

Browser-install extension members are checked at runtime before calls. The
integration consumers use the real protocol record/count/job shapes and validate
unknown public receipts. Worker transfer accepts genuine nonempty Uint8Array
ArrayBuffer views, including offsets and shared backing buffers, and rejects
unsupported representations before transfer. Canonical frontend source remains
unchanged; all adjustments are external integration modules or exact anchored
overlays. Protocol typing and malformed-input guards are supplied by the separately
reviewed protocol commits.

## Local commit mapping

- Protocol `8e7425f2a87a262f7d5cbe214b25dd5f514453d4` → local `3abfe47`.
- Frontend/integration contracts: `bc971fd`.
- Protocol `47cce54cbed95609371c0f349862e7df62b3a27e` → local `6ad3bfd`.
- Protocol `7b4ee1b7569ee9fcbe181e3acb96801c3c9ecf77` → local `00e4a2d`.
- Query admission/normalization and library sentinel: `64f0494`.
- Field-owned query-budget correction: `0a18be2`.

Independent scoped source review approved `64f0494` and independently exercised
the production read/cache/Apply seams, extra valid and invalid query combinations,
and actual structuredClone transfer. The final budget delta is reviewed separately;
the review outcome is provided with the final publication handoff.
