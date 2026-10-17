# Bounded relation page extension

The pinned MangaDex5.13.1 relation GET has no pagination controls. Its existing
unpaged request and successful response shape remain available. Unpaged results
are never silently truncated; a statement, alias or1MiB response-budget failure
returns503. Full4096-edge metadata is not claimed to fit one response.

`GET /api/v1/manga/{uuid}/relation?includes[]=manga&limit=32&offset=0` explicitly
opts into this application's bounded page extension. Supplying any of `limit`,
`offset`, `ownerVersion` or `snapshot` selects the extension. Recommendations
retain their existing distinct contract and reject these controls.

- `limit`:1..32, default32. `offset`:0..4096, default0.
- `ownerVersion`: positive32-bit owner Manga revision. Optional on the first
  public page; mandatory on continuation pages (`offset>0`).
- `snapshot`: lowercase64-hex token. Optional on the first public page;
  mandatory on continuation pages. Both supplied guards are checked, including
  on page0. A mismatch returns409; restart requires the caller's explicit choice.
- `includes[]`: only `manga`, with the complete existing target attributes.
  Unknown controls, duplicate scalar fields and invalid values return400.

The standard collection envelope retains truthful `limit`, `offset` and visible
eligible `total`. Extension fields are `ownerMangaId` (canonical UUID),
`ownerVersion`, `snapshot`, and `nextOffset` (an exact next offset or null).
The response may contain fewer than `limit` whole resources to fit1MiB. Follow
`nextOffset`, never infer completion from a short page. No fields or strings are
trimmed. One complete resource that cannot fit fails503.

Public pages also carry:

```json
{"nostr":{"scope":"instance-local","versionScope":"owner_manga","pagination":"bounded-relations-v1","identity":{"kind":"manga","requestedId":"normalized-request-uuid","canonicalId":"actual-owner-uuid","source":"persisted-public-resource-ids"}}}
```

The identity record proves presentation alias mapping only. It grants no account,
publisher, editing or original-source authority. Actual hidden+visible persisted
edge counts are not exposed publicly. Authorized editor pages may separately
return `persistedTotal`, preserving the distinction from visible `total`.

Each page resolves the typed requested owner, current source/target visibility,
complete ordered lightweight edge set, full selected native target metadata and
all aliases within one read-only repeatable-read transaction. At most32 targets
execute `catalog_manga_json` per page, keeping the existing statement budget.
The snapshot is SHA256 over canonical serde_json serialization of:

```text
["weebdex.relation-snapshot.v1",ownerNative,ownerVersion,routeScopedCount,
 [[edgeKey,relationKind,targetNative,targetVersion],...in returned edge order]]
```

Public `routeScopedCount` is the visible eligible count. Authorized editor
routes may bind their actual persisted count instead. Public tokens never hash
a small private hidden count that could be guessed offline; tokens are scoped
to their route and are not interchangeable between public and editor reads.

The digest binds owner edits, ordered eligible relation membership and actual
target catalogue revisions. Visibility changes can invalidate it without an
owner version change. It is not an authentication token or an immutable snapshot
of independently changing counters. No native key is returned by this encoding.
Relation `attributes.version` is the owner revision; expanded target
`attributes.version` is its own actual catalogue revision.

Consumers must keep the same requested owner, canonical proof, owner revision,
snapshot and total across pages, require contiguous progressing `nextOffset`,
reject duplicate edge/target UUIDs, and stop at4096 resources. No automatic
restart after409 or silent later hydration with a different target revision.
Successful recovery of4096 resources is an all-pages result, never a claim that
one4096-edge response fits the transport budget. Actual qualification evidence
is recorded separately; source shape alone is not performance acceptance.

The owner title's normal detail response must also remain readable at capacity.
Its alias projection allows exactly4097 Manga identities: one owner and4096
relation targets.4098 Manga identities still fail503, and every other resource
kind retains its4096-identity budget. Both public and transactional alias loaders
use this same narrowly scoped bound; no metadata or relation is omitted.
