# Bounded live reader selector, v1 source contract

Status: authored source and fixtures, **not compiled or executed**. This freezes a
backend seam for review. It does not enable frontend navigation, qualify large
catalogue performance, or complete exact automatic Continue parity.

## Routes and authority

- Native: GET `/api/manga/{nativeId}/reader`.
- Public: GET `/api/v1/manga/{publicUuid}/reader`.
- Existing aggregate routes, their 10,000-chapter caps, chapter detail/image
  routes, and device-local completion history are unchanged.
- A visible published parent is required. Every selected current chapter,
  boundary anchor, group representative and edition must belong to that parent
  and satisfy the exact effective filters in the same request transaction.
- Public aliases are resolved in that transaction. They do not grant visibility.
  Returned IDs, group IDs and cursor references use preferred public UUIDs.
  Native tie keys stay inside SQL. Base64 cursor encoding is not confidentiality.
- Existing router Catalogue admission wraps both routes; the handler must not
  acquire a second permit. One 8-second handler deadline includes pool/alias
  work; each SQL statement has a 2-second deadline, lock wait 250 ms and idle
  transaction limit 3 seconds. Each transaction is read-only RepeatableRead.

## Controls

Controls are scalars and may occur once:

| Control | Meaning |
| --- | --- |
| `operation` | `groups` (default), `editions`, `first`, `last`, `around`, `next`, `previous`, `resolve-group` |
| `limit` | 1..100, default 50; group/edition page ceiling, never an offset |
| `direction` | `newest` (default) or `oldest`, only for group pages |
| `current` | The actual currently read chapter; required for around/next/previous; optional scoring context elsewhere |
| `groupChapter` | An actual chapter representing a chosen exact nullable-label group; only for editions/resolve-group; defaults to current |
| `after` | Canonical encoded cursor, only for group/edition pages |

`groupChapter` intentionally does not occupy the existing native `group`
filter. There is no client-supplied label key, target winner, history list,
completion bitmap, offset, arbitrary ordering, global revision or snapshot ID.

Selector queries are at most 16 KiB. Malformed percent encoding, invalid UTF-8, NUL,
empty query fields, duplicate controls and oversized cursors are rejected.
Unknown filter fields are rejected by the route-specific filter vocabulary.

### Effective filters remain profile-specific

Native accepts the existing aggregate's chapter filters: `tlang`, `tlangx`,
`uploader`, `uploaderx`, `group`, `groupx`, `id`, `manga`, including their native
array forms. It reuses `chapter_filters`; native language validation is not
replaced with public language validation. It does not introduce public
availability, empty-page, content-rating, external-URL or future-date defaults.

Public uses the existing `/chapter` schema, `catalogue::filters`, public UUID
resolver and public language validation. Pagination, includes, caller ordering
and a second `manga` scope are disallowed. Reader defaults match the retained
site-client selector, not ordinary collection defaults:

- All four content ratings if none was supplied.
- Empty pages, future publication/updates and external URLs included by default.
- Unavailable chapters excluded by default, overridable by `includeUnavailable=1`.
- Existing positive/excluded groups, uploader filters, language filters, labels,
  dates and scoped external-host filters retain their existing meaning.

## Selection and order

Groups are exact nullable `(volume, chapter)` pairs. `null`, `""`, `"02"`, `"2"`
and `"2.0"` remain separate identities. Volume precedes chapter.

Native profile `native-aggregate-v1` preserves numeric-prefix DESC NULLS LAST,
then original raw label DESC, whose implicit null order is FIRST. It preserves
the actual database label collation; no lowercase or public comparator is used.
Within a group the default serde map's key order is followed by JavaScript
Object.entries array-index promotion: canonical integer keys 0..4294967294
come first numerically, followed by the serialized string-key order. This is
not native publication order.

Public profile `public-retained-v1` preserves complete ASCII decimal numeric
comparison exactly, then descending JavaScript UTF-16 raw order, with null last.
The SQL key expands supplementary Unicode scalars into high/low surrogates.
It does not substitute UTF-8, database locale, Number or parseFloat order.
Within a group, the retained public `/chapter` array determines the tie order:
`published_at DESC, native id ASC`, not public alias order and not compatibility
aggregate `id`/`others`.

Edition winner scoring counts visible shared stable group identities from the
actual persisted current chapter. The complete eligible edition set is ranked
by descending score and the profile's tie order, with only the final top-one
limited. A winner beyond edition 100 or 10,000 is not lost to a preliminary
page cap. The persisted source DTO always has group IDs; the retained overlay's
legacy missing-ID name fallback does not apply to this API's ID-only records.

- `first`: oldest eligible group, complete-set edition winner. This is Start.
- `last`: newest eligible group, complete-set edition winner.
- `next`: freshly computed nearest **newer** group relative to actual current.
- `previous`: freshly computed nearest **older** group relative to actual current.
- `around`: current group's winner in `data`, and freshly computed `next` and
  `previous` winners (or null boundaries).
- `groups`: bounded groups in requested direction, represented by each group's
  complete-set winner under current scoring context.
- `editions`: bounded editions in source tie order, without score reordering;
  independent `winner` always resolves the entire eligible group.
- `resolve-group`: freshly resolves a selected group's complete-set winner;
  does not reuse a cached first page or accept a caller-proposed winner.

Start/last and editions/resolve-group expose `hasMultipleEditions` from a
separate two-row lookahead, not from the displayed page length. Group-list
entries do not yet carry counts or complete edition metadata; selecting one
uses editions or resolve-group.

## Response shape

Every successful response includes:

- `version: 1`, `mangaId`, `operation`, `orderingProfile`, `scope`.
- `catalogIdentity: { catalogNamespace, serviceKey }` on every successful state.
- `consistency: "live-keyset-v1"`, `crossRequestSnapshot: false`.
- `state`, `restartRequired`, `data`, `nextCursor`, `limit`.
- `history: "device-local-only"`, `continueProof: "observed-only"`.
- `current` when an actual eligible current chapter was supplied.
- `groupAnchor`, `winner`, `hasMultipleEditions` for editions/resolve-group.
- `hasMultipleEditions` for nonempty first/last.
- `next` and `previous` for around.

Minimal chapter entries have `id`, nullable `volume`/`chapter`, `language`,
`publishedAt` (exact PostgreSQL UTC timestamp text including microseconds),
`version`, `groupIds`, `sharedGroupScore`, and `witness`. Group names, members,
images, page arrays, full manga DTOs and arbitrary metadata are not expanded.
A client needing labels can fetch detail for its selected chapter/group.

Catalogue identity is exact and bounded: `catalogNamespace` is the canonical
lowercase, hyphenated 36-byte UUID from PostgreSQL `namespace::text`; `serviceKey`
is a validated lowercase 64-hex public service key or explicit `null` for the
same unconfigured/native-only cases as `/api/protocol/capabilities`. The selector
reuses `protocol::adapters::configured_service` directly, including its existing
native-node prerequisite. It does not substitute `node_identity`, infer a key,
or copy configuration eligibility rules. The service config is immutable for
this AppState/request. Namespace is read as the first SELECT in the same short
repeatable-read transaction as aliases, visibility and rows, not on a second
pool connection or from cached capabilities.

The controller must compare both fields exactly with its independently verified
same-source capability identity before accepting any rows, boundary, stale or
excluded state. Missing/mismatched identity requires invalidation and reload;
matching manga/chapter IDs do not authorize rebinding local history to a new
catalogue. Errors carry no admissible reader result and cannot bypass this gate.
This is an instance-identity fence, not a catalogue revision: deliberately
cloning/restoring the same namespace and service key preserves that identity and
is not distinguishable as a new instance by this contract.

States:

- `ready`: the selected rows were valid under this request's live view.
- `boundary`: no eligible row exists in the requested direction/page in this
  request. This does not prove all-read or empty across multiple requests.
- `excluded-current`: the actual current is missing, wrong-parent, hidden or
  excluded by the effective query. Aggregate-dependent navigation is unavailable;
  no fallback target is invented. Direct chapter/image routes are unchanged.
- `stale-anchor`: a cursor/group anchor disappeared or visibly changed, or its
  request scope/current/group witness changed. `restartRequired` is true.

Stale/excluded states are explicit 200 result states with empty data, not false
404s for an otherwise directly readable current chapter. Invalid inputs use
400, missing parent 404, inherited capacity refusal 429, and database/deadline
or complete-resource budget failure 503. Never interpret an error as boundary.
All successful and error responses are no-store.

## Bounded transport and cursor witnesses

At most 100 requested group/edition rows and one internal lookahead are selected
per page query. SQL may examine a larger eligible set under the deadline; this
is **not** a claim of bounded index visits or qualified million-row latency.
No aggregate count or full chapter list is returned. Native/public labels and
IDs retain their existing schema bounds. Each complete chapter projection allows
at most 100 visible group identities, with one lookahead detecting excess and
failing closed rather than silently truncating identities.

Response JSON is capped at 256 KiB. If complete page rows exceed it, whole final
rows are removed and a cursor resumes after the last retained row. A single
resource that cannot fit fails explicitly. Returned pages are checked for
within-page duplicate references/groups; no unbounded cross-page seen-ID set
is stored server-side.

The cursor contains only a version, scope hash, actual chapter reference,
anchor witness, optional current witness and optional selected-group witness.
Scope binds catalogue namespace and service key (including null), parent,
profile, canonical effective filters, operation, direction,
current and selected group. Limit may change without invalidating scope.
Public scope/witness data does not contain raw native IDs.

On continuation the server resolves the anchor anew and checks its parent,
eligibility, exact labels, precise publication timestamp, version, language and
visible group identities. Current/group contexts are rechecked too. Anchors
are not authority, and no guessed successor is returned after a mismatch.

## Deliberate live-consistency limitation

A valid witness does not detect every insertion, movement, deletion or new
edition behind an earlier cursor. Multiple pages do not form one catalogue
snapshot. Completion history must never be uploaded. Traversal may support
observed unread candidates and observed exhaustion, but neither proves the
exact oldest unread group nor global all-read under concurrent drift.

Semantic operations do not accept an expected UI witness: they recompute the
current target. Before navigating, the future controller must compare returned
current/groupAnchor/target witnesses and exact nullable labels with its captured
UI selection intent. A moved representative or changed selected position requires
an explicit reset/reload; an eligible freshly returned target alone does not prove
that an older click still means the same group. This consumer integration and its
mounted stale-selection tests remain unfinished.

The frontend must retain service/account/library/query/navigation-generation
and local-history revision fences, bounded recent-key duplicate observation,
and explicit restart/refresh states. It must not automatically apply the old
newest-group all-read fallback based on live multi-page exhaustion. Exact
Continue parity and frontend integration are intentionally unfinished.

## Authored qualification, not execution evidence

Ordering unit goldens cover prefix versus whole-decimal behavior, null/special
labels, `02`/`2`/`2.0`, exact values above 2^53, supplementary/BMP UTF-16 order,
reversible keysets, numeric-string ID promotion and public publication ties.
Parser/witness unit fixtures cover bounds, profile/scope fencing, public-safe
cursors, timestamp precision and source-private ID exclusion.

HTTP/database fixtures are authored separately for future execution. Runtime
qualification must establish actual SQL ordering against the serialized source
fixtures, 10,001 groups/editions, winners beyond 10,000, cursor drift behavior,
semantic neighbor recomputation, unchanged aggregate caps, public alias safety,
visibility/withdrawal and actual response/work bounds. No test passage, compile
success or frontend adoption is claimed by this source checkpoint.

### Catalogue identity source controls

The identity successor is based on frozen backend/tests `483e9b9` and unchanged
capability semantics from main `f7c598c`: `protocol/adapters.rs::configured_service`
and `capabilities`, plus migration `0073_catalogue_namespace.sql`. No migration,
new revision counter, persistent snapshot or history transmission is introduced.
Authored unit controls distinguish namespace, service-key and null-key changes
with identical resource IDs and check identity on all four selector states.
An additional ignored real-HTTP fixture simulates namespace replacement with
matching existing aliases and exercises ready, boundary, stale and excluded
responses. It requires `PLATFORM_READER_IDENTITY_ISOLATED=1` and an exact test
filter in a disposable database, never parallel fixture execution. These new
controls are authored only; actual SQL/HTTP qualification, service-config
replacement and concurrent-snapshot timing acceptance remain pending.

## Native account completion lookup, v1 source successor

This separate, cookie-only read seam restores the retained native account
history input without reusing the unbounded `ChapterHistory.List` map. It does
not alter either selector's input vocabulary or complete automatic Continue.

GET `/api/user/history/manga/{nativeMangaId}/completion` requires:

- Exactly one `X-WeebDex-Expected-Account` header matching the authenticated
  native account. This is a constraint, never an authentication credential.
- A normal native browser session cookie. Any `Authorization` header is
  rejected, including a valid bearer credential alongside a valid cookie.
  Recovery, revoked, expired and disabled-account sessions cannot read here.
- Repeated `id` query fields: 1–50 distinct exact native chapter IDs. They are
  server-selected resource identifiers, not a local-history/read-ID upload.
- Exactly one `accountEpoch`: canonical unsigned decimal, 0 through
  9,007,199,254,740,991. This is echoed client-local correlation only. There is
  no server-stored authentication epoch and it grants no authority.
- Exactly one `catalogNamespace`: canonical lowercase hyphenated UUID.
- Exactly one `serviceKey`: lowercase 64-hex public service key, or the explicit
  query text `null` for JSON null. An omitted key is not equivalent to null.
- An already-ended empty request body. The handler checks the actual body's
  end-stream state without polling it, before database work. A not-yet-ended
  body, including a no-frame stream with `Content-Length: 0` or an exact-zero
  size hint, is immediately refused with413. It never awaits a Bytes extractor;
  a zero-byte cap alone would not bound that wait. Ordinary empty GET is admitted.
  Unknown controls, `id[]`, duplicate scalar controls,
  duplicate IDs, offsets, flags and history payloads are rejected.

The response shape is:

```json
{
  "version": 1,
  "mangaId": "native-manga-id",
  "catalogIdentity": {
    "catalogNamespace": "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
    "serviceKey": null
  },
  "account": {"id": "native-account-id", "epoch": 7},
  "consistency": "live-request-v1",
  "continueProof": "observed-only",
  "data": [{"id": "native-chapter-id", "completed": true}]
}
```

`data` covers every requested key exactly once and in request order. `completed`
means membership in retained `platform.library_chapter_history`, with the exact
account, manga and chapter bound in the query. It does **not** mean the device
completion flag or `library_chapter_progress.completed`; those can legitimately
disagree with retained history membership. No timestamps, progress, account
email, session ID or token are returned. No history or account writes occur.

A missing/invisible parent returns 404. Any missing, foreign-title, deleted or
source-ineligible requested chapter refuses the whole batch with 409, with no
partial completion data. Missing data must never be interpreted as unread.
Account or catalogue mismatch also returns 409. Native unavailable, empty-page,
external-URL, future-date and content-rating behavior is unchanged. Existing
normal browser owner-library permission semantics, including unverified or
restricted accounts, are retained; API-client permission rules are not borrowed.

The 24 KiB raw query cap accommodates fifty maximum 128-byte native IDs even if
every byte is percent-encoded. The native route's actual ID domain is ASCII
alphanumeric, underscore, hyphen and colon, not arbitrary PostgreSQL 128-character
strings. The entire success JSON is capped at 16 KiB and never trimmed. Maximum
legal IDs fit the cap; oversize input/output is refused, never silently partial.
Clients may use smaller batches for proxy URL limits, counting each lookup,
returned row and response byte inside the same finite reader action budget.

One existing LibraryHead admission covers each GET/HEAD request. Authority is
resolved from the cookie inside the same RepeatableRead transaction as identity,
parent/chapter eligibility and membership. The auth-owned helper locks active
user then exact normal session `FOR SHARE NOWAIT`, binding session ID, user ID
and token hash. It does not use a cached or separately pooled principal. Those
locks fence account authority changes and session revocation until commit;
conflicts/serialization failures refuse without retry. They do not prevent time
expiry, promise authority at browser receipt, or freeze source visibility later.
The transaction performs no durable writes, but cannot use PostgreSQL READ ONLY
because row-locking SELECTs require a writable transaction. Limits remain 2s per
statement, 250ms lock wait, 3s idle and 8s overall including pool acquisition/commit.

This native route has `Cache-Control: private, no-store` on success and refusal,
including outer admission/boundary failures. Existing origin/credential policy
is unchanged. Only GET preflight for this exact native path adds the required
expected-account header; HEAD preflight with it and unrelated/public route
preflight remain refused. The pre-existing expected-account preflight gap on
other native routes is not silently repaired here. There is no public API
completion route or public/device mode fallback to native account history.

Before consuming booleans, clients must compare exact requested coverage/order,
account ID and epoch, catalogue identity, captured actor/authority epoch, library
selection epoch/generation and account revision, navigation/filter context and
action generation. In particular, a cookie switch A→B can precede Session-store
notification; the mandatory server expected-account constraint closes that gap.
Pending library selection advances its epoch before committed mode changes.
Any mismatch requires an explicit restart/reload and must not fall back to device
history or another account. Repeated batches remain live observations only;
oldest-unread or all-read across drifting pages is still unproved.

Source provenance: this successor builds on `4ef7261`; native auth, history,
validation, routing/CORS and schema semantics were inspected at unchanged main
`f7c598c`. No migration, dependency, aggregate behavior or old native route was
replaced. New unit controls and isolated ignored HTTP/SQL fixtures are authored
only. Compilation, actual lookup SQL/HTTP, race timing, source withdrawal and
browser integration acceptance remain pending separate owned qualification.
