# Public editorial and private draft adapter

## Scope

The restored source implements the six author/group create, edit and delete
operations and the two private draft read operations from the hash-pinned
MangaDex 5.13.1 inventory. This is a bounded adapter checkpoint, not a claim that
the complete MangaDex API or all upstream behavior is implemented.

Public transport uses UUID resource identifiers and MangaDex entity/collection
response envelopes. Author links retain their camelCase wire names and nullable
values. Group create and edit keep their different pinned publishDelay contracts.
Group leader/member UUIDs must resolve to current native accounts; a protocol
publisher alias is not a native account. Public group creation makes the actual
bound actor its local editorial leader. Staff/leader permissions, locks, versions,
reference guards and audit records are enforced by the shared native mutation
core. Native controllers keep their original contracts.

These are instance-local editorial overlays. They do not sign protocol events,
modify protocol group keys, or claim federated publication authority. Original
source visibility remains a prerequisite for projected resources. Original Nostr
publication and group-key behavior is unchanged by this exception.

## Authority and transactions

Every protected operation verifies the actual HTTP method, original URI, raw
body digest, configured service key and replay nonce through the original
service-bound Nostr verifier. It then rechecks that original capability inside
the transaction and requires a current explicit protocol_identity_bindings row.
The account must be verified, enabled and neither banned nor restricted.
Session cookies and public UUIDs alone grant no authority. No fake native
SessionPrincipal, private signing key or read-capability promotion is used.

Writes lock the current bound account for update and its binding for share,
invoke a sealed native-core actor, and render UUID aliases/relationships on the
same transaction before commit. A core, audit or response-construction failure
rolls back the write. Group membership targets are native-only and their current
enabled rows are share-locked before mutation.

Draft reads use a repeatable-read transaction, the current read binding and
account policy, and transaction-local UUID expansion. Owners need manga.create;
moderators need manga.state.update to see other accounts' drafts. Queries retain
limit (1–100, default 10), offset, state (draft/submitted/rejected), deep-object
ordering by title/year/createdAt/updatedAt, and inherited includes for manga,
cover_art, author, artist, tag and creator. Detail accepts only includes[]. Drafts
never become visible through public published-manga reads.

## Fresh verification entrypoints

Run the new unit and real signed-HTTP regressions, plus the unchanged native CRUD
regression, with the checked-in isolated PostgreSQL runner. Tests are marked
ignored unless the owned fixture is explicitly supplied. Runtime keys and
accounts are synthetic; no production credentials or databases are used.

- backend/tests/public_editorial_drafts.rs
- backend/tests/catalog_entity_mutations_http.rs
- public_api::editorial::tests
- public_api::drafts::tests

Fresh execution results belong in the checkpoint evidence. Recovered historical
passes are not evidence that this restored source passed.
