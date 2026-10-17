# Lossless native-account settings save

Official POST `/api/v1/settings` now saves known fields from the actual
application-owned settings template returned by GET. It shares native scalar,
language, reference and version validation with the existing legacy native
PUT `/api/settings`, while keeping the two authentication boundaries explicit.
No new schema, dependency, template version or protocol publication is added.

## Request and authority

The exact signed JSON body is:

`{"settings":{"version":N,"items_per_page":"40",...},"updatedAt":"actual preview time","expectedAccountId":"canonical native User UUID","template":"optional current template UUID"}`

`settings.version` is mandatory and must match the actual persisted native
preference revision. Other known setting fields are optional patches, not a
replacement of the full private native record. The immutable current template
UUID is `513e0d48-ecc4-5301-b5eb-5bb369efaf22`; an optional different template is
rejected. The upstream `updatedAt` field, when provided, is a precondition on
the actual native account-row preview timestamp, never a client-supplied clock
to store. UTC bare seconds, Z and +00:00 spellings are normalized; calendar
errors and non-UTC offsets are rejected. The native preference version remains
the authoritative settings CAS even when account-row timestamps share a
millisecond.

GET `/settings` now exposes `nostr.accountId`. A UI should capture and submit it
as `expectedAccountId`; a pubkey rebind between preview and submit returns409
before any settings change. The account identity and timestamp preconditions
are optional application guards for compatibility callers, but the preference
version itself is required.

Every public request needs a fresh original Nostr proof bound to the exact
external URI, method, service, nonce and raw body. The current explicit binding,
active verified native account and `self.preferences.update` permission are
rechecked and locked inside the mutation transaction. Site cookies, API-client
secrets, signed-library identity and a UUID alone do not authorize this public
write. The JSON body is limited to32768bytes, with no query parameters. Unknown
or duplicate fields, explicit nulls, duplicate/oversized arrays, wrong scalar
types and noncanonical page-size strings are rejected against the real schema
vocabulary. Filter references must be real typed native resource UUIDs; no
native IDs, protocol publisher substitutes or invented aliases are accepted.

## Preservation and atomic projection

Supported scalar patches use native validated values. Incoming tags, groups
and uploaders map through persisted kind-scoped UUID aliases to actual native
keys, then receive current visibility checks. For each supplied reference array,
the submitted visible selection replaces only its visible source selection.
Existing deleted/disabled/unresolved source references are preserved internally
and remain counted, not exported, by the read projection. Their raw private
identifiers never leave the account boundary. A combined field over100 entries
fails instead of truncating it. Omitted reference fields are unchanged.

Unrecognized private native preference fields survive the JSONB known-field
merge unchanged. An empty visible array cannot silently erase unavailable
source filters. Their later removal requires a separate explicit native action
or a later supported visible selection, not a lossy copy-back of GET output.
This intentionally preserves the partial-projection contract of the read API.

An actual change increments the native preference version once and uses the
real database clock. A current-version identical patch is a no-op that keeps
revision and clocks. Stale revisions or timestamp/account preconditions409.
Concurrent writers of the same version cannot lose an update: one commits and
the other conflicts. Exact proof replay401s even for a state no-op.

The response uses the existing `{result,settings,template,updatedAt,nostr}`
shape, built from actual stored rows before commit. Schema/UUID projection
failure rolls back the write instead of committing and then returning an error.
The extension identifies `mutation:known-field-merge`, `changed`,
`nativeStorageUnchanged`, `preservedUnavailableFilterValues` and
`preservedUnknownNativeFields`. `readOnlyProjection:true` still means this
response is a known-field projection, not the complete private native record.
No hidden or unknown source field is echoed.

## Native compatibility and remaining boundary

The existing native PUT continues using a genuine site principal, current
native permission and same-origin mutation policy. Its legacy validation,
unverified-account preference allowance and revision semantics are retained;
read-only scoped clients remain unable to mutate preferences. Both adapters
use the same sealed genuine-actor transaction core. The canonical account-ID
presentation helper is shared with ratings and list follows; it grants no
authority by itself.

Template-administration POST remains a separate unavailable operation.
This save path neither invents central MangaDex template ownership nor mutates
the immutable version1 application schema.

Real HTTP/PostgreSQL tests cover schema-valid save/read roundtrips, native
session parity, private unknown/unavailable preservation, rejected lossy/null
inputs, account rebind, CAS concurrency, replay and permission boundaries.
