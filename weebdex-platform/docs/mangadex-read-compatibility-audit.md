# MangaDex read compatibility and public UUID audit

Audit dates: 2026-10-17/18. Qualified 113-operation baseline backend:
`53130b47424b107d3b9d1b5ceb00959b62addaa5`, with test-only native fixture
prerequisite correction `c96ead07654d38ead63bab9e6172328b79811ec9`.
The [113-operation snapshot](mangadex-operation-compatibility-snapshot.json)
now covers every pinned upstream operation exactly once. It identifies 91
source-routed scoped operations, 12 explicit protocol/authentication 501
exceptions, one operator-only template administration boundary, and 9
unsupported plain-REST operations. **This is not full MangaDex compatibility.**
The actual unauthenticated routing probe passed all113 operations on that source,
independently of functional seam tests. A refusal establishes
handler/auth/validation routing only, never a working authorized command.

The retained `test-evidence/final-combined-api-20261017/report.json` establishes
138 lib tests (2 ignored), seven functional HTTP seams and 113 routing checks
for **ec1b216 only**. Current
`test-evidence/native-status-combined-api-20261018/report.json` establishes139
library tests (2ignored), all seven baseline HTTP groups, the new native-status
HTTP journey and the113 routing checks against the current backend0102 source.
The operation JSON remains the frozen pre-run source inventory at717d889; the
current report records its exact hash and the actual compiled/run source. Fresh
chapter/publication/relation and other feature-lane runtime receipts remain
separate. Source review and older branch checks cannot replace that acceptance.
The read checklist below remains a bounded catalogue audit. Native account,
original signed graph, source publication and device-local features have separate
contracts and dedicated tests.

The newer [genuine popularity/view extensions](public-uuid-popularity.md) are
separately qualified at `a23ac87a945ecff3d5e7daa07d428bb41efc4820`, based on
the final `0395fbc` baseline. They add GET `/manga/top`, GET `/chapter/top`,
POST `/analytics/manga/{UUID}/views` and POST `/analytics/chapter/{UUID}/views`.
These four app-owned extensions are outside the pinned MangaDex inventory;
the 113 keys and 91/12/9/1 classification counts do not change. Their dedicated
report, `test-evidence/public-uuid-popularity-20261018/report.json`, records
140 library tests (2 ignored) and four current-source database suites, including
the actual TCP popularity/view journey. The earlier nineteen-case full backend
matrix remains associated with its baseline, not a fresh full-matrix pass for
this successor. Frontend gates and actual browser/live-client qualification have
their own source and acceptance boundaries.

## Reference and compatibility boundary

The primary reference is the official [MangaDex OpenAPI specification, version
5.13.1](https://gitlab.com/mangadex-pub/mangadex-api-docs/-/raw/main/static/api.yaml),
consulted on the audit date. The official API documentation host was unavailable
to the research tool, so its linked official source repository was used. The
checked-in parameter inventory is pinned to [upstream commit
60a3bb3c9f25675435a479c925d857015201739e](https://gitlab.com/mangadex-pub/mangadex-api-docs/-/raw/60a3bb3c9f25675435a479c925d857015201739e/static/api.yaml)
with SHA-256 `004031ccdfc4b66727dd1f85d09b5300cf60235bbab2de6f6ea4816094bd6d33`.
The selected read parameter vocabulary agrees with the current official source.
The source contract is reference material; this application does not proxy the
MangaDex service or claim its catalogue, accounts, ranking algorithm or authority.

All paths here are relative to `/api/v1`. The retained `/api` short-ID surface
and original protocol endpoints are separate compatibility boundaries. Known
unimplemented operations stay JSON 501, unknown paths JSON 404 and unsupported
methods JSON 405. Read-only aliases grant no account, group or signer authority.
The final public-router default method fallback is now explicitly JSON for
read-only paths that have no other inventory methods, including ping and
recommendation; per-operation custom method handling is preserved.

## Endpoint checklist

| Operation | Result and checked contract |
| --- | --- |
| `GET /ping` | Plain text `pong`, with the existing admission and response boundary. Newly implemented. |
| `GET /manga` | Collection; title, creator, year, tags/modes, status, language/excluded language, translated availability, demographic, UUID IDs, rating, dates, availability and group filters. Fixed full reference-expansion vocabulary. |
| `GET /manga/{id}` | Canonical UUID entity, visible published title only; author/artist/cover/manga/tag/creator expansion. |
| `GET /manga/random` | Same visible catalogue/rating/tag policy and includes; no invented resource when none matches. |
| `GET /manga/tag` | Complete bounded local taxonomy; no query parameters, UUID tag entities. Explicit 503 above 4096 rows. |
| `GET /manga/{id}/feed` | Visible parent plus chapter filters; default limit 100, maximum 500, nonzero limit. Includes, language/rating/exclusions/dates and publish/external/empty/unavailable controls. |
| `GET /manga/{id}/aggregate` | Language/group/unavailable filters; canonical chapter IDs, alternate IDs and real availability. Fixed missing `isUnavailable`; available duplicates take primary position. Explicit 503 above 10,000 rows. |
| `GET /manga/{id}/relation` | Newly implemented real persisted outgoing relations, canonical directional edge IDs and target IDs; optional target manga attributes. Missing/invisible root 404, invisible targets excluded. No pagination controls upstream; optional bounded application paging is documented separately. |
| `GET /manga/{id}/recommendation` | Newly implemented persisted recommendations, score asc/desc, default ratings and explicit adult rating selection, optional manga attributes. At most 20 rows; truthful stored scores, no fabricated algorithm or empty fallback. |
| `GET /chapter` | Collection with documented ID/title/group/uploader/parent/volume/chapter/language/rating/date/exclusion/toggle filters. Both the documented `includes` and common `includes[]` spelling work. |
| `GET /chapter/{id}` | Canonical UUID entity, current parent/source visibility; chapter metadata and optional parent/group/uploader attributes. |
| `GET /author` | UUID/name filters and name ordering, optional visible manga attributes. Additional created/updated sort vocabulary is an application extension. |
| `GET /author/{id}` | Canonical UUID author and current visible related manga. |
| `GET /group` | UUID/name/focused-language filters and all documented sorts. Fixed 501 on followedCount/relevance: count actual non-disabled local followers; name relevance uses exact/prefix/substring order. |
| `GET /group/{id}` | Canonical UUID group and optional actual leader/member account attributes. A registration is not ownership. |
| `GET /cover` | Manga/cover/uploader UUID and locale filters, created/updated/volume sorts, manga/user expansion. |
| `GET /cover/{mangaOrCoverId}` | GET uses a cover UUID. The official shared path explicitly uses a manga UUID on POST, not GET; no guessed manga-to-cover fallback was added. |
| `GET /at-home/server/{chapterId}` | Existing original/data-saver page-name response and current visibility/media checks. `forcePort443=true` works only when the configured public origin is actually HTTPS/443; otherwise 501 rather than inventing a host. Original protocol hashes and opaque filenames are not resource UUIDs. |

## Filters, ordering and pagination

- General lists default to 10 rows and accept limit 0 through 100. Limit zero
  returns an empty data array and a real total. Feed defaults and limits differ
  as specified above. Offset counts rows, not pages.
- General catalogue and feed offsets are locally bounded at 10,000. This is an
  application resource budget; the upstream OpenAPI offset has no numeric
  maximum. The adapter does not claim arbitrary deep pagination.
- Public filters are bound SQL parameters. Order identifiers come only from
  operation-specific fixed vocabularies; the native resource ID breaks ties.
  Chapter/volume numbers sort numerically when numeric, with null/nonnumeric
  keys last. Native IDs used internally for ties are not emitted as resource IDs.
- Public discovery defaults to safe/suggestive/erotica. A known adult title
  detail is still readable. Recommendation rating filters apply to targets.
- Native retained empty-rating sentinels are not silently added to the upstream
  query vocabulary. Genuine time-window popularity uses only the explicit local
  `/manga/top` and `/chapter/top` extensions; ordinary `/manga` and `/chapter`
  searches do not silently reinterpret rank/time controls or substitute rankings.
- UTC dates are calendar-validated `YYYY-MM-DDTHH:MM:SS`. Malformed UUIDs,
  duplicate scalar controls, unknown parameters/enums/order fields and unsafe
  query sizes are rejected. Repeated bounded arrays are supported.
- Unpaged relation success shape remains available with explicit 4096-edge and
  response-size budgets; refusals never silently omit edges. The separately
  documented [bounded relation extension](mangadex-bounded-relation-pages.md)
  opts in through limit/offset/ownerVersion/snapshot, returns at most 32 whole
  full-metadata resources per page and uses visible-count snapshot continuity.
  Complete envelopes stay within 1 MiB with truthful nextOffset; no public
  persisted hidden-edge count is exposed. Manga aliases allow exactly 4097
  identities (owner plus 4096 targets); other kinds remain bounded at 4096.
- Recommendations report limit 20, offset 0 and the eligible persisted total.
  The upstream operation does not accept limit/offset; these are rejected.

## UUID consistency checklist

- Public UUID parsing requires the hyphenated hexadecimal form, normalizes
  uppercase and accepts the project's custom version-8 UUIDs. It does not
  substitute native short IDs, 64-hex events or list internal keys.
- Alias lookup is typed by resource kind and preserves immutable persisted
  preferred IDs and imported aliases. Unknown include-filter UUIDs match no
  resource; a UUID from another kind does not acquire authority.
- Canonical IDs are returned for resources, nested relationships, tag DTOs,
  aggregate chapter/alternate IDs and newly implemented relation/recommendation
  edges, even when the request used an imported alias.
- Relation and recommendation edge IDs use the existing directional,
  length-delimited version-2 key and persisted kind/namespace UUID mapping.
  They are not guessed from either endpoint's public UUID. Reads do not call
  alias creation/backfill helpers.
- Opaque media filenames and original hashes are transport/storage locators,
  intentionally distinct from resource IDs. Original Nostr keys/events remain
  64-hex under their own contract.
- The real HTTP regression compares alias-table counts before and after reads,
  tests uppercase imported aliases, wrong-kind/native IDs, nested UUIDs, paging,
  reference expansion and visibility revocation while persisted edges remain.

## Intentional protocol/authentication exceptions and open coverage

There is no MangaDex OAuth account, password session, API-client identity or
centralized user authority. Namespaced Nostr challenge, signed library and
protocol-publisher identity contracts remain explicit. Local account sessions,
upload permissions and native forum thread IDs remain separate capabilities.
This audit neither changes auth nor introduces a native-write fallback.

Relations are local editorial annotations. Their version is the actual owner
manga version, documented in `attributes.nostr.versionScope`; no signed-source
revision or nonexistent independent edge version is invented. Recommendations
are actual persisted local edge scores. A missing record is a real empty result,
not a claim of a remote recommendation engine.

## Remaining plain-REST coverage and supported journeys

The nine unsupported plain-REST operations are explicit gaps in this API surface:

- `POST /list`, `PUT /list/{id}`, `DELETE /list/{id}` and both membership commands
  at `/manga/{id}/list/{listId}`: the application has persisted device-local list
  creation/rename/removal/membership and explicit client-signed original graph
  publication (`library.ts`, `LibraryManagementTools.svelte`, `publication.ts`).
  These journeys do not authorize a server to invent a Nostr event or convert a public
  protocol list into a native private list. These REST commands remain 501.
- `DELETE /user/{id}` and `POST /user/delete/{code}`: no public MangaDex-style
  account deletion/verification-code command exists. Directory/profile/follow
  reads do not grant deletion, moderation or signer ownership. A supported
  self-account deletion journey is not claimed by this audit. The retained
  `User.ts` has no User.Delete command; original kind5 metadata/social
  withdrawal requests are not native account deletion.
- `POST /legacy/mapping`: MangaDex numeric database identity has no manufactured
  equivalent. The separate typed persisted `/legacy/resolve` extension resolves
  real app mappings only; it does not implement numeric MangaDex mapping.
- `POST /captcha/solve`: no remote MangaDex CAPTCHA authority is simulated.

Template installation (`POST /settings/template`) is explicitly operator-only,
not a missing user preference save. The app-owned schema and actual settings
GET/POST are implemented, versioned and guarded by native-account binding.

The 12 Nostr/auth exceptions remain documented separately: password login,
logout, refresh, seven OAuth client/secret operations, plain read-marker mutation
and MangaDex forum-thread creation. Native forum companion IDs and original
signed read-marker commits are their own types and capabilities.

[Native-account status editing](mangadex-native-reading-status.md) now has a
source-implemented private preview and scoped POST `/manga/{id}/status`. It uses
captured account/canonical target plus a genuine independent status revision
observing native-session/import updates and removal/readd. It edits existing
membership only, preserves flags/followtime/counters, and refuses null clear
rather than silently unfollowing. Original GET status routes still project the
signed library; device Save stays local by default. The secondary AccountFollow
status editor and device list rename are composed source. The native-status
backend has the fresh qualification above; frontend gates and actual browser
acceptance have separate receipts/boundaries. Native account deletion remains
unsupported.

Implemented operations are still scoped adapters. Title creation/edit accepts
only the documented localized/UUID fields. Title request-review returns 202,
including after a later grant gain; an explicit fresh moderator approve-hide may
return 204. Neither mode requests original-source deletion.
Chapter/cover/editorial/relation writes use real native permission/version cores;
ratings persist actual user observations and aggregates; follows persist native
membership revisions; settings preserve private unknown/unavailable values.
Original source metadata is retained alongside trusted exact sort keys. None of
these facts implies upstream OAuth, complete schema-field parity, MangaDex data,
recommendation algorithms, or an unbounded projection budget.

## Verification

The regression sources are `backend/tests/public_catalogue_compatibility.rs`,
the existing `backend/tests/public_catalogue_reads.rs`, catalogue schema parity
unit tests and read-only edge-query unit tests. Run the integration binaries
through `scripts/run-auth-tests.sh` against its disposable loopback PostgreSQL,
never a persistent/user database. Exact run results accompany the commit's
test-evidence report. Pure source/schema tests are not browser or production
performance evidence.
