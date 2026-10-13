# Historical API notes

For the current Rust/PostgreSQL wrapper, start with [API-INDEX.md](API-INDEX.md). The chronological notes below describe earlier milestones and may say later-implemented features are pending. They are retained as historical evidence, not a current endpoint inventory.

# PostgreSQL reference API checkpoint

All allocations here are experimental project interfaces, not registered Nostr extensions. Website design is excluded. This version completes local tickets 01–10; social/library state, maintenance, backup/restore and nginx/static acceleration are still in progress.

## Authentication and service identity

The service key comes from an operator-managed private file with restricted access; PostgreSQL stores public identity bindings, not that secret. GET /identity?nonce=<64 lowercase hex> returns a signed kind 20005 nonce/endpoint proof. It is not TLS-channel-bound and says nothing about a service's reputation. Configured public origins require HTTPS except explicit loopback development; PostgreSQL requires verified TLS unless explicit loopback plaintext is selected.

Operator requests use NIP-98 with exact public URL, method and body hash, fresh timestamps and shared PostgreSQL replay protection. There is no website login/UI yet. All exposed administrative examples are intended for loopback fixtures, not public deployment.

## Implemented routes

- GET / and GET /admin/status: capabilities and authenticated process/pool status.
- PUT /upload/{sha256}: bounded, operator-authorized Blossom-style staged upload. Its token binds action, server origin and hash. This experimental route is not full Blossom compatibility.
- GET /blobs/{sha256}: operator's authorized staged read.
- POST /publications: operator-signed kind 1001 chapter or1002 series event, containing only manifest/version. All referenced originals must be verified and locally retained before completion.
- POST /events: ordinary signed kinds 1, 7, 1111. Replaceable/expiring events require their own lifecycle and are not accepted here yet.
- GET /events/{id}: available signed event;202 indicates pending projection rather than success. Removed/candidate publications are unavailable.
- GET /originals/{sha256}: exact verified bytes for a retained, projected publication. This is the bounded HTTP fallback; direct nginx delivery is not implemented in this port yet.
- GET /query: operator-authenticated live offset/limit discovery with filter, since, until, sort and order. Q93D requires explicit order=asc or order=desc; Q94A enables optional include_total=true for an exact same-request-snapshot total. Missing order is rejected. Expensive/oversized results fail rather than silently truncate.
- POST /deletion-requests: record a signed protected request without automatic deletion by default.
- POST /admin/deletions/{request}/approve: local approval, with optional targets array. Known targets must belong to the request author. Raw kind 5 never reaches strfry import.
- POST /admin/publications/{id}/restore with an empty object: reopen a completed removal as a candidate requiring full validation. Old approved requests do not automatically undo explicit restore.
- POST /admin/peers: key, endpoint and allowed policy. Updates/revocation advance an approval epoch.
- GET /federation/events/{id} and /federation/blobs/{sha256}: approved-peer exports, scoped by fresh NIP-98 authorization. Private staging is not exported.
- POST /admin/sync: peer key, event ID, filter and separately approved fallback keys; synchronize one chapter. Retention and projection status are distinct in the result.
- POST /reconcile: approved-peer bounded chapter-inventory snapshot negotiation using Negentropy. This is separate from live discovery pagination.
- POST /admin/sync-jobs: peer/filter/fallbacks; enqueue or coalesce a durable active batch.
- GET /admin/sync-jobs/{id}: progress, state and last error.
- POST /admin/sync-jobs/{id}/cancel with an empty object: fence uncommitted work, preserving already admitted content.

## Important limits

Single-node storage ownership and process-safe PostgreSQL coordination are implemented. The public-original HTTP fallback still reads/hashes bounded files; it is not the final high-throughput static path. Chapter sync does not yet synchronize every series/account/deletion inventory. Uploaded generic bytes are not accepted images until publication validation succeeds. Custodial groups are implemented; ordinary Nostr replaceable/deletion lifecycles remain pending.

A source's local deletion policy does not force another instance to delete a retained copy. Approved federation controls the supported transfer API; it cannot prevent copying content made publicly readable. No immunity from every takedown, permanent availability or target-scale throughput is promised.

Source/fixture commands and dependency prerequisites are in ../server/README.md. In addition to PostgreSQL and Rust, projection tests require an installed strfry executable, GNU timeout and util-linux flock. Test configuration/keys are synthetic and must never be deployed.

## Custodial groups (Ticket10)

With `group_keys_dir` configured, instances hold ordinary group signing keys in local0600files; PostgreSQL stores membership and versioned authority. Keys alone do not restore roles. This is a local software keystore, not an HSM or encrypted vault.

- `POST /admin/groups` creates a group for an explicitly supplied administrator.
- `POST /admin/groups/import` registers an existing local key file by public key; no private key is submitted through HTTP. It starts disabled and without members.
- `POST /admin/groups/{key}/bootstrap` explicitly assigns the first administrator to a disabled, memberless recovered group.
- `POST /admin/groups/{key}/enabled` controls local group admission.
- `GET/POST /groups/{key}/members` reads or changes roles. Revocation requires an explicit JSONnull role; omitting the field is an error.
- `POST /groups/{key}/upload/{hash}` stages bytes with a group-specific grant. Another group cannot publish private staged content just by knowing its hash.
- `POST /groups/{key}/publish` accepts only the approved publication purpose, not arbitrary event signing.
- `POST /groups/{key}/deletion-requests` records the protected request; local moderation remains separate.

Both preflight and final publication commit check membership/authority epochs. Revocation during expensive validation prevents publication. Last-administrator removal is blocked. Evidence: `results/postgres-http-groups.json`, `results/postgres-group-fences.json`, and `results/postgres-groups-tests.log`.

## Public-library performance experiment

`server/examples/library_ac.rs` is an isolated fixture with synthetic data and test-only inspection/crash controls. It must not be deployed as the public node. See `docs/library-ac-performance.md` and `bench/library-ac/LIMITATIONS.md`. Production social/library lifecycle integration remains Ticket11.
