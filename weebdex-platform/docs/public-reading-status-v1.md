# Reading status transport v1

`GET /api/v1/manga/status` and `GET /api/v1/manga/{id}/status` read the caller's
published original signed library. They require the same exact service-bound Nostr
HTTP capability as read markers, without a native-account binding. Public UUIDs
identify projected manga; original signed series IDs remain immutable hashes.

The only statuses are `reading`, `on_hold`, `plan_to_read`, `dropped`, `re_reading`
and `completed`. The collection accepts the optional exact `status` filter. Both
operations accept the explicit `head` extension for snapshot pinning. Unrecognized
parameters, repeated scalar parameters, malformed IDs and nonempty GET bodies are
rejected. An absent saved record or absent published head returns an actual null
single status or empty collection, not a fabricated default reading state.

The adapter reuses the one shared original `LibraryObjects` owner, its validated
manifest, exact-key lookup for one series, and bounded filtered scan for all
statuses. Collection views are bounded at10,000 matching records and64MiB decoded
source descriptors by the original helper. Exceeding a budget fails explicitly.
Only current locally projected, published, cached-visible and original-live-visible
manga receive canonical UUIDs. No alias or graph row is written by these reads.
After UUID/DTO work, the original module rechecks the exact signed head, original
admission and configured selected service in its own transaction.

The `nostr` response extension records the signed head/root (nullable when absent),
`scope: signed_public_library` and `followFlagInferred: false`. A saved series or
reading status does not imply a follow action. The existing graph has no distinct
follow flag; follow routes remain pending a compatible client-signed representation.

Fresh original-runtime/TCP/PostgreSQL verification at64973e5 covers all six values,
status/head filters, owner separation, missing-head nulls, no-GET-writes, strict
queries/bodies, cached withdrawal and original admission revocation/recovery. See
`test-evidence/public-api/reconstructed-64973e5/` for exact evidence and cleanup.
