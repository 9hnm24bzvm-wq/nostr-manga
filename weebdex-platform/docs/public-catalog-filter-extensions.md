# Application-owned catalogue filter extensions

Corrected source candidate after review of `68a5e0649b9e75ecac0550b3992eeb4acec79702`, based on `5cac90bbf7be5e81ed816655de25347138d7e903`, October 22, 2026.
These additive application contracts are not MangaDex 5.13.1 schema parity. The
pinned schema inventory is unchanged. Native handlers and the original 679
frontend files are unchanged. Public-gate overlays supply the UI changes.

## Independent publication-year bounds

`GET /api/v1/manga` accepts `yearFrom` and `yearTo`, each optional, decimal integer
0–9999, once per request. Both are inclusive. When both are present, the lower
bound cannot exceed the upper bound. Equal bounds stay range parameters rather
than being rewritten as exact-year wire syntax. Duplicate, empty, fractional,
negative, out-of-range and exact-year/range combinations are refused. The existing
`year=1..9999` and `year=none` contracts remain separate and unchanged.

The adapter passes the admitted bounds into the existing native `manga_filters`
predicates. Native unknown year is stored as0: upper-only bounds include it;
positive lower bounds exclude it; lower0 includes it; 0–0 selects it. Public DTOs
still represent unknown year as null. Count-only (`limit=0`) and every page use
the same admitted SQL predicate. No response-page filtering or estimated totals
are introduced. Chapter searches with parent filters first apply the same bounds
to the server-side parent query; bounds never leak into the chapter endpoint.

The public overlay changes the retained numeric inputs' min/max from1900/2100 to
0/9999. Apply, reset, URL reads and Back use the existing typed parser/serializer.
The separately uncovered public-language control binding fault is fixed by not
providing a default to a binding whose retained parent legitimately supplies
undefined; display and parsing already handle undefined.

## Optional title-wide group choices

Public title resources advertise
`attributes.nostr.availableGroupSemantics = "visible-title-groups-v1"`.
Normalization supplies an empty `relationships.available_groups` capability slot
for the retained filter; this is not a claim that the title has no groups. The
public overlay uses its independent paged chooser. Absent/unknown/malformed
optional markers do not fabricate choices.

`GET /api/v1/manga/{mangaUUID}/available-groups` accepts:

- `limit`: decimal1–100, default100.
- `after`: the previous page's canonical public group UUID, absent initially.
- `ids[]`: up to100 selected public group UUIDs, for title-scoped selected-name
  hydration. It cannot be combined with `after`.

No offset, language, chapter-page, search, include or order controls are accepted.
The response is `{result:"ok", semantics:"visible-title-groups-v1", data, total,
position, limit, next}`. `position` is the exact number of matching groups at or
before the supplied cursor in this page's snapshot (zero on a first page). `data` contains scanlation_group Resources with canonical persisted
public UUIDs, actual stored name only, and empty relationships. It does not expose
native IDs, member rosters, authority claims, chapter-derived placeholder names
or synthetic group identities. `next` is the last returned canonical UUID when
another page exists, otherwise null. Page length must equal
`min(limit,total-position)` and next must exist exactly when
`position + data.length < total`, on every page, not just the first. Resources are ordered by canonical UUID,
not display name; the client validates strict ordering and continuation identity.
Title UUID aliases are resolved, while a continuation must be the canonical
UUID actually returned. Selected-name hydration is intended for the canonical
IDs emitted by the picker; an unrecognized/noncanonical selection remains
explicitly unresolved rather than being silently rewritten.

Membership uses the actual title-wide group set from native migration0082:
there must be at least one nondeleted, available, source-visible chapter of this
published, nondeleted, source-visible title, with the group nondeleted and
`protocol_group_is_visible`. Distinct groups are selected once through EXISTS,
regardless of chapter page, language, or duplicate memberships. Inactive groups
are not hidden solely for inactivity, matching the native projection. Chapter
preferences and ordinary page limits never shrink the available choices.

One repeatable-read transaction establishes parent visibility, exact matched
count, cursor membership/rank and at most101 minimal native rows. Only the first100
are resolved through persisted aliases. The101st row is a continuation sentinel,
not a truncated success. Missing canonical aliases fail this optional endpoint,
never the otherwise-readable title. The count scans the matched set in SQL but
there is no unbounded JSON aggregation, native group projection or application
materialization. Names are already bounded by the catalogue schema; the client
also validates a4096-byte name budget.

Migration0104 adds separate public manga/chapter projection functions. The manga
projection preserves0082 fields/visibility except that it never constructs the
unbounded `available_groups` array. The public chapter projection delegates all
retained fields, recursive null stripping and visibility to the latest native
three-argument function (0088), supplying only its public parent without whole-title groups and
unchanged chapter-local groups. Thus a chapter-count/feed request
cannot accidentally reintroduce the old title-wide aggregation. Public catalogue,
related-title and authorized draft projections use the new helpers; canonical
native functions are not replaced or changed.

The chooser fetches one page on mount; Next groups replaces its current page,
and First groups restarts. It holds at most100 page choices plus names for up
to100 selected IDs, rather than accumulating the entire title history. A later
page selected in a deep link or Back navigation is hydrated through bounded
`ids[]`, not by scanning earlier pages. More than100 draft selections are called
out explicitly. The composed form refuses Apply before clearing any feed or
navigating, retains the draft, and displays an inline error; the transport budget
also rejects oversized requests. Reset and applied URL navigation have a separate
token from mutable draft selections: both abort pending paging/hydration and
restart at the first page, including Reset while already at the default URL.

Failures show retry without disabling title/chapter reads. Page actions are
serialized, old requests are aborted, and generation/epoch fences reject late
responses after a title change, Back, Reset or unmount. A changed total/rank, removed cursor,
duplicate/reordered page, malformed response or missing continuation requires a
retry/restart instead of claiming completion. Every page has snapshot-consistent
counts, but different requests are live snapshots: equal-count concurrent edits
are not an immutable traversal guarantee. There is no cross-request snapshot
certificate or sustained-load qualification claim.

## Evidence and still-required acceptance

Finite executed tests include 24 actual-composed Svelte/JSDOM controls and 5
source/counterfactual checks. The mounted boundary uses actual retained forms,
actual numeric/language controls, actual public group component, parser,
normalizer and retained catalogue client. Router/store, transport, decorative
primitives and select/popover shells are finite stand-ins. It is not browser,
bits-ui popup, backend, PostgreSQL or HTTP acceptance.

The old public-query module from5cac90b is retained byte-exact with SHA256
`5c847c1b3cd3303cf871d1c4b1de6e0fcf89759b16040fb1e69374474e12b960`.
It rejects the same valid open/unequal bounds that the current module accepts;
exact-year controls still agree. The old language binding fault is also reproduced
in an actual mounted counterfactual. Exact68a5e06 component/composed/validator snapshots additionally
reproduce premature page completion, destructive101-selection Apply and stale
paging across Back/reset; current controls reject all three. Exact reverse-markup guards reject missing
filter exceptions and unrelated markup, with the preexisting guard unchanged.

`backend/tests/public_catalog_filter_controls.rs` contains two ignored,
prospective actual TCP/isolated-PostgreSQL tests, not executed in this source lane:

1. Independent/equal/unknown bounds; exact-year controls; count/empty/page
   symmetry; mixed tag/language/status/rating/demographic filters; hidden/draft/
   deleted titles; invalid requests at both count and page limits.
2.205 legitimate groups across chapter pages/languages; duplicate membership;
   deleted groups, unavailable/deleted/source-withdrawn chapters; actual names,
   canonical aliases, title alias, three keyset pages, selected-name hydration,
   UUID chapter filtering, invalid controls, removed cursor conflict, title
   survival and parent withdrawal; no read-created aliases. It also compares
   complete current native manga/chapter JSON, the installed0088 three-argument
   chapter function, and the public projections, excluding only the declared
   group extension. Synthetic protocol/source metadata, recursively stripped
   object nulls, retained array nulls, missing/withdrawn/deleted chapters, canonical
   DTO equivalence and actual HTTP JSON equality are asserted. These are authored
   controls, not executed migration or HTTP evidence.

The owned native runner must compile these fixtures, execute them in its guarded
isolated database, retain receipts and verify cleanup. Backend old-source controls
must replay the same positive tests against5cac90b production sources with the
new test fixture retained: the year test should fail at the first new-bound200
assertion after its exact-year controls pass; the group test should fail at the unavailable public SQL helper/capability or
endpoint; first establish baseline title readability separately. Neither backend
counterfactual has run. Final composed strict/build/CSS and actual native browser
journeys remain required. This candidate is implementation plus finite evidence,
not a release or parity certificate.
