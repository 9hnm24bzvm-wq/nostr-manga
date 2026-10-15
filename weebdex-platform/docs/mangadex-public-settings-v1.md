# Instance settings through the MangaDex-shaped API

The five Settings operations use the pinned MangaDex 5.13.1 paths and JSON
shapes under `/api/v1`. Authentication intentionally uses a fresh, service-bound
Nostr HTTP proof over the exact URL, method and body. A public UUID, browser
cookie, bearer session, unbound admitted public key or caller-supplied owner
never authorizes these instance-local settings.

An explicit operator-controlled Nostr/native-account binding and an active,
verified, unrestricted native account are rechecked and locked in the same
PostgreSQL transaction as the protected read or write. GET operations are
read-only. All responses, including errors, use `Cache-Control: no-store`.

## Operations

- GET `/settings/template`: raw JSON Schema for the current immutable template
- POST `/settings/template`: publish a raw JSON Schema and return it unchanged
- GET `/settings/template/{version}`: raw JSON Schema for that immutable UUID
- GET `/settings`: `{result, settings, updatedAt, template}`
- POST `/settings`: `{settings, updatedAt}`; returns the same envelope as GET

Template responses additionally include an ETag with the template UUID and a
Location pointing to its versioned API path. Version history is retained. A
missing UUID is 404; a malformed UUID is 400. None of these operations accepts
query parameters or a GET body. POST requires `application/json`.

Template publication requires both the existing `sysop` native role and the
explicit `settings.template.create` permission. Migration 95 grants that new
permission only to existing sysops. Future grants remain an operator action;
ordinary signup never acquires the role or permission. Stored templates cannot
be updated or deleted by the application. Publishing a template does not alter
any user's settings or grant access to them.

## One preference value for the API and site

`platform.users.preferences` remains the single native preference value source.
There is no independent public-API copy. Public writes require the existing
`self.preferences.update` permission. A public whole-object write retains all
native preference fields and applies the same native type, language, identifier,
reference and count constraints as the existing native settings route. It
replaces the entire document; unknown template-valid JSON fields are stored and
returned without being dropped or interpreted as account permissions.

The native fields are `items_per_page`, `languages`, `show_safe`,
`show_suggestive`, `show_erotic`, `show_hentai`, `tags`,
`translated_languages`, `uploaders`, `groups`, `data_saver`, and the server-owned
`version` revision. The first eleven fields must remain present and non-null.
Native normalization, such as canonical decimal counts and deduplicated IDs,
still applies. These native preference keys use native-module reference IDs, including tag,
uploader and group IDs; the adapter does not reinterpret arbitrary opaque
settings as public UUID references. Public GET returns those raw IDs; native session projections
may expand uploader/group IDs for their existing UI consumers.

The caller may echo the current `settings.version`, which is checked for
staleness, or omit it. The server chooses and increments the next stored native
revision. A caller cannot override it. The latest template validates the final
stored object, including that authoritative next revision.

The public write requires the exact `updatedAt` token returned by GET:
UTC with six fractional-second digits, such as
`2026-10-15T05:30:00.123456Z`. This is an intentional instance concurrency policy
in addition to the upstream object shape; alternate timezone spellings and
lossy timestamps are not interchangeable tokens. Missing/malformed fields are
400 and stale tokens/revisions are 409. Native and public writes lock the same
user row. Preference timestamps advance monotonically by at least one
microsecond, even when writes are rapid. Profile-only changes do not change the
preference token.

Existing native PUT `/api/settings` retains its patch plus integer-version
semantics and preserves unknown preference keys. A genuine native preference
change resets the validation link to the immutable seeded native baseline
schema. This keeps GET truthful if a native write would violate a tighter
custom public template. The next public write must validate against the latest
published template again. No-op native patches retain their original behavior.

## Offline schema and resource policy

Validation uses pinned `jsonschema` 0.33.0 (MIT, MSRV 1.71.1), with default features
disabled. HTTP and file retrieval are disabled and a rejecting retriever is also
installed. Template validation never fetches a URL, reads a referenced file,
executes code, or treats a schema as an account authority. Official dependency
references: [crate metadata](https://docs.rs/crate/jsonschema/0.33.0) and
[validator API](https://docs.rs/jsonschema/0.33.0/jsonschema/).

Recognized built-in dialect identifiers cover drafts 4, 6, 7, 2019-09 and 2020-12;
absent `$schema` means draft 7. Unknown dialects/formats are rejected. The vetted
validator enforces the selected dialect; this adapter adds conservative resource
constraints rather than a replacement partial schema evaluator:

- Template request limit 65,536 bytes; settings request limit 131,072 bytes
- Five-second request-body timeout
- Opaque settings depth 16, at most 4,096 JSON nodes, key length 256 bytes
- Template/ref expansion depth 20, at most 2,048 visited nodes, schema arrays at
  most 128 elements, schema strings at most 4,096 bytes
- Only acyclic in-document JSON Pointer `$ref` values; recursive/dynamic refs
  and external references are rejected
- Schema `$id` identifiers must be offline URNs; conservative scanning also
  applies to nested schema content
- At most 16 regexes, each at most 256 bytes, with the linear-time regex engine
  and 65,536-byte compiled-size and DFA-cache limits
- At most two blocking validator workers; at most 1,024 immutable templates

Requests outside these budgets are rejected explicitly. These are documented
instance limits, not a claim that all arbitrary JSON Schemas can execute safely.
Validation errors never echo private settings values or schema source.

## Verification

`backend/tests/public_settings.rs` exercises actual TCP requests, signed
NIP-98-style proofs and an owned disposable PostgreSQL database. It covers all
five operations, explicit account bindings, role and permission conjunction,
versioned schemas, opaque fields, native site/session visibility, native edits,
concurrency, admission revocation, wrong-service and replayed proofs, body
tampering, malformed/oversized requests, invalid references, offline schema
validation, immutable storage and owner isolation. The existing native
`library_http` test remains a separate regression gate.
