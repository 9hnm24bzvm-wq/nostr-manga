# Genuine instance popularity and view recording

Qualified backend successor, 2026-10-18, exact source
`a23ac87a945ecff3d5e7daa07d428bb41efc4820`, based on 0395fbc. Compile, 140 library
tests (2 ignored) and four PostgreSQL regression suites passed; exact receipts are
in `test-evidence/public-uuid-popularity-20261018/report.json`. The earlier
nineteen-case backend qualification remains evidence for its frozen baseline,
not a new full-matrix pass for this successor. These are app-owned extensions,
not additional claimed MangaDex inventory operations or imported global rankings.

## Bounded observed-window reads

GET `/api/v1/manga/top` and `/api/v1/chapter/top` return ordinary typed UUID
collection envelopes with actual visible resource metadata and relationship
includes. They accept the corresponding existing public catalogue filters and
includes, plus `rank=read` and `time=24h|7d|30d` (defaults read/30d). Limit is 0..10,
default 10; offset must be 0. Client order/sort and unsupported metrics/windows fail 400.
Current reader preferences are not silently dropped.

Scores reuse the native catalogue ranking core's persisted `catalog_view_events`
window count. One fixed database statement timestamp and repeatable-read
transaction bind filters, score order, total and resource metadata together.
Only matching resources with positive observed scores enter the collection;
no observations means an honest empty collection. Cumulative counters, follows,
ratings, recommendations and imported peer popularity are never substitutes.
Order is score descending then actual native ID ascending; no canonical UUID
sort is invented by the client. Total counts all eligible positive-score matches,
even when limit 0 or the top 10 truncates the returned data deliberately.

The top `nostr` extension is:

```
{scope:'instance-catalogue', source:'persisted-native-view-events',
 metric:'deduplicated-views', rank:'read', time:'30d',
 windowSeconds:2592000, observedAt:'2026-10-18T00:00:00.000000Z',
 order:'window-views-desc-native-id-asc'}
```

`observedAt` is UTC with exactly six fractional digits. Each returned resource
adds `attributes.nostr.popularity={views,rank:'read',time}` without overwriting
its actual source/publication identity or exact sort metadata. `views` is a
positive canonical decimal string, at most 19 digits and 9223372036854775807;
JavaScript Number rounding is not part of this contract. No per-visitor identity
or private membership is exposed by the public aggregate.

## Actual view commands

POST `/api/v1/analytics/manga/{UUID}/views` and
`/api/v1/analytics/chapter/{UUID}/views` accept no body/query and return 204 only
through the existing native analytics transaction. The typed UUID is resolved
to a real native target, then current canonical/local visibility is locked and
rechecked. Chapters also retain actual native unavailable-target refusal.
No account login, binding, signer or private-library authority is inferred.

The same core checks cross-site/origin policy, obtains rate identity from the
central verified transport/proxy extension, enforces 120 requests/minute and
retains Retry-After: 1 on 429. It records at most one event per resource and visitor in each UTC-hour bucket,
and advances the corresponding cumulative counter only for an inserted event;
failed writes roll back the event, counter and rate mutation together. Fake XFF
or a new visitor cookie cannot select a new peer-rate identity.

The non-authority visitor cookie remains HttpOnly/SameSite=Lax, Secure onHTTPS.
Native routes keep Path=/api/analytics; this transport uses
Path=/api/v1/analytics so ordinary same-origin credentialed fetch can actually
store/send it. Catalogue reads are anonymous; cookies never grant ranking or
account authority. Clients must require
confirmed 204 before marking an analytics submission acknowledged; no successful
view is fabricated after refusal or transport failure. The cookie is an abuse/
deduplication identifier, never an account or publication capability.

## Qualification boundary

`backend/tests/public_popularity.rs` contains a real loopback/PostgreSQL fixture
for empty data, distinct 24h/7d/30d observed ordering, ignored cached counters,
ratings/visibility/limit filters, exact decimal metadata, both resource kinds,
alias-safe dedup, visitor separation, cross-site/body/query/target refusals,
actual-peer rate exhaustion and transaction rollback. This actual TCP/PostgreSQL
suite passed 1/1; the native community/analytics Router+DB, public catalogue reads
and public statistics regressions also passed 1/1 each. All 914 frozen compile
inputs stayed unchanged; all four guards physically stopped with closed ports,
no forced cleanup, survivors, unknown births or cleanup errors.

The later test-only `f505bcc` hook qualifies the exact production TypeScript
client/ranking adapter against the actual owned backend on the composed `d633690`
source. All four executed TS module hashes and the current-checkout script match
their compiled pins. Its 22 real requests and 18 window observations establish
Manga 0→1→1 and Chapter 3→4→4 across all three windows, including real first204
and same-visitor deduplication. It used 16,978 response bytes in 252.578ms; these
are fixture measurements, not a production performance result. Separate probe
self-tests passed 3/3, 140 units passed again, and the complete popularity fixture
with mandatory client bridge passed again with physical cleanup verified.
Exact raw receipts are in
`test-evidence/public-popularity-client-bridge-20261018/report.json`.

The probe deliberately supplies a Node in-memory cookie jar and same-origin
Origin stand-in, without retaining cookie values. It does not qualify a native
browser's cookie policy or mounted UI. Production dataset/load and actual
browser/PWA/proxy acceptance remain independent gates.
