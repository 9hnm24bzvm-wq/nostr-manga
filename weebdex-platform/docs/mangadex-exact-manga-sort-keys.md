# Exact public Manga comparison keys

Published Manga resources expose `attributes.nostr.sortKeySemantics` equal to
`postgresql-exact-v1` and `attributes.nostr.sortKeys`. This application extension
allows a device-local library to hydrate multiple UUID batches and then sort the
whole result by the same keys as a public `/manga` collection. It does not turn a
device save into an account follow or claim that native MangaDex exposes this
extension.

The keys are selected directly from trusted catalogue columns and eligible
chapter rows, in bounded batches. They are never copied from publisher metadata.
Collection selection, aliases and comparison keys share the caller's database
transaction. Draft/private editorial resources may omit the extension.

| Key | Exact representation and source |
| --- | --- |
| `title` | PostgreSQL `lower(m.title)` encoded as UTF-8 bytes and then lowercase hexadecimal. Nonempty even-length ASCII hex, at most 8000 characters. |
| `year` | Actual `NULLIF(m.year,0)` as decimal text, or null. |
| `createdAt`, `updatedAt` | Actual row timestamp, UTC `YYYY-MM-DDTHH:MM:SS.ffffffZ`, exactly six fractional digits. |
| `latestUploadedChapter` | Maximum eligible, nondeleted chapter publication timestamp, in the same UTC format, or null. Revoked canonical chapter sources do not contribute. |
| `followedCount` | Actual `COALESCE(stats.follows,0)` SQL sort expression as integer text. |
| `rating` | Actual `COALESCE(stats.rating,0)` SQL sort expression as exact decimal text. This is not a per-account rating. |
| `tieBreak` | The resource's actual persisted preferred public Manga UUID, identical to the returned resource ID. |

`followedCount`, `rating` and nonnull `year` match
`^-?\d+(?:\.\d+)?$`, with at most 256 ASCII characters. Exponents, nonfinite
numeric values and oversized persisted keys fail503 instead of being rounded or
silently substituted. A missing statistics row uses the actual existing SQL
comparison expression's zero, not an invented observation count.

Compare hexadecimal title keys lexically by ASCII bytes, without localeCompare
or recalculating lowercase from the display title. The database's locale defines
its lowercase conversion; Manga title ordering explicitly uses C-byte collation.
Compare UTC timestamps lexically without passing through millisecond-only Date
objects. Compare decimal strings using exact sign, integer and fractional digits,
without JavaScript Number coercion. All supported nullable sort keys use nulls
last for both directions. Equal keys always compare `tieBreak` ascending, even
for descending primary order. Server Manga ordering now uses that persisted
public UUID as its final tie-break, so no native identifier is exposed or needed.

These keys describe the selected database snapshot. They cannot freeze separate
network requests into one snapshot when the catalogue changes between batches.
`relevance` is query-dependent and intentionally has no general comparison key.

The real HTTP/PostgreSQL regression checks values above JavaScript's safe integer
range, high-precision NUMERIC values, microsecond timestamps, source revocation,
nullable keys, Unicode title byte ordering, canonical UUID ties across pages,
metadata spoofing and invalid stored-key failures. Existing catalogue and signed
account/follow/history/settings HTTP suites cover shared DTO seams as well.
