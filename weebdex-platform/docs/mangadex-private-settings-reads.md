# Native-account settings and versioned schema reads

The authenticated `/api/v1/settings`, `/settings/template` and
`/settings/template/{version}` GETs now use the same original request-bound
Nostr capability and explicit active native-account binding as private follows.
A browser cookie or signed public-library principal alone is insufficient.
The account/admission/binding locks, stored preference selection, visibility and
UUID alias projection share one repeatable-read transaction. Responses remain
private/no-store. These are read-only adapters, not a native settings mutation
or signed-library migration.

## Real application-owned template

`contracts/native-settings-template-v1.json` is the actual JSON Schema used by
this read projection, with immutable application template UUID
`513e0d48-ecc4-5301-b5eb-5bb369efaf22`. Its UUID identifies the versioned schema
artifact, not an account, Nostr key, server-side secret or MangaDex-owned object.
The latest-template GET returns the JSON Schema directly. The versioned GET
returns the identical schema for that UUID, 404 for an unknown UUID and 400 for
malformed UUID input. All three reads reject query parameters and bodies.

The schema uses JSON Schema draft2020-12 and describes actual native preference
fields: positive persisted version, decimal items-per-page1–100, bounded known
language arrays, rating/data-saver booleans and bounded canonical UUID arrays
for tags, groups and uploaders. Unknown private source fields are not exported.
A schema change requires a new template UUID/version; it must not silently
rewrite the meaning of this persisted version identifier.

## Settings response and preservation

The settings response is `{result, settings, template, updatedAt, nostr}`.
`settings` contains only known, actually stored native fields. Missing optional
fields are omitted instead of inventing defaults. Malformed persisted scalar
values or a missing/invalid native preference version fail503. `updatedAt` is
the real native account row timestamp; `nostr.updatedAtSource` explicitly names
that clock because no independent native preference-only timestamp exists.

Reference filters are resolved from their persisted native keys to their real
preferred public UUIDs, with current tag/group/user visibility. Deleted, hidden,
unknown or disabled reference targets are omitted from this response's filter
arrays and counted in `nostr.unavailableFilterCounts`. Their native source
values remain unchanged, alongside unrecognized private preferences. No raw
unresolved native identifier or private unknown value is returned. A visible
resource without a valid persisted public alias fails503 rather than inventing
an alias.

The extension explicitly marks `scope:instance-account`,
`projection:known-native-preferences-only`, `readOnlyProjection:true` and
`nativeStorageUnchanged:true`. This read projection must not be copied back as a
replacement of the full native record. The subsequent POST `/settings` adapter
now uses a known-field merge that preserves unavailable and unrecognized source
settings; see [the write contract](mangadex-native-settings-writes.md).
Template-management POST remains explicitly501.

## Verification

The real signed account HTTP/PostgreSQL regression checks latest/versioned
schema equality, private authorization, malformed/unknown versions, absent
preference version failure, actual source version and timestamp, canonical
reference UUIDs, hidden/disabled filter counts, unrecognized-value exclusion,
other-account separation and byte-equivalent stored preferences after reads.
It saves only the synthetic fixture's projected settings in its disposable test
directory for independent JSON Schema validation. Unit tests pin the template
UUID and native language vocabulary and reject malformed persisted scalars.

This is application-specific settings-schema compatibility. It does not claim
MangaDex's hosted settings contents, template administration, OAuth identity or
full remaining API completion.
