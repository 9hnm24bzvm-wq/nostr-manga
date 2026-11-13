# Protocol and architecture

MangaPub core is the existing experimental protocol implementation packaged independently. The complete HTTP routing contract is declared in `server/src/node.rs` and its `node/` handlers; it is not an officially standardized Nostr namespace.

## Signed events and immutable objects

`auth` provides Nostr event IDs, Schnorr signatures and verification. HTTP authentication binds the exact external URL, method and body hash. A valid signature establishes authorship only; node-level admission and replay checks remain separate. `blossom` implements bounded blob authorization scope, path and range handling.

`manifest` and `series_metadata` validate deterministic CBOR chapter/series metadata, BCP 47 language tags, image descriptors and feature requirements. `library_manifest` validates chunked public-library roots and records; `library_wire` bounds HTTP envelopes and compression. `protocol_limits` centralizes shared advertised/enforced budgets.

The current application uses chapter kind 1001, series kind 1002, public-library head kind 11999 and provisional service-announcement kind 11998. These are the implementation's experimental choices, not a claim of external standards allocation. Consult the corresponding parser and capabilities response for schema versions and required features.

## Publication, authority and storage

`publication`, `upload`, `publisher_authority`, `groups`, `submission_review` and `moderation` implement the protected publication workflow. A publication is not admitted as complete merely because a signed event exists. Required immutable manifests and originals must be validated, retained and covered by current authority and storage grants.

`db::Repository` and PostgreSQL own current permissions, generations, heads, visibility, queues and accounting. Moka caches derived immutable data, not live authority. Revocation and generation checks remain part of commit. `storage`, `blobs`, `library_storage` and reclamation modules preserve reference and reader lifetimes before physical cleanup and refunds.

`accounts`, `library`, `library_view`, `social` and `ordinary_deletion` retain the existing account/public-library/social lifecycle. Ordinary author deletion is separate from protected publication moderation. Reader enrollment, publisher permission, group role and approved-peer permission are different authorities.

## Federation and relay

`service_identity`, `service_announcement`, `service_proof`, `peer_discovery`, `endpoint_policy` and `peers` bind discovery and approved endpoints to service identity. Discovery itself never grants permission. Transfer destinations and redirects are constrained by approved-peer and endpoint policy.

`inventory`, `transfer`, `sync_jobs` and `sync_schedules` implement bounded reconciliation and durable peer synchronization with explicit item outcomes and retry/cancellation state. Peer failures do not authorize accepting unverified or unretained content.

`backend` projects accepted signed events to native PostgreSQL through durable state; legacy strfry is explicitly selectable on Linux. `relay`, `relay_query`, `relay_feed` and `relay_dispatch` provide the public bounded relay surface, read-policy checks and live delivery. The raw strfry endpoint is not an equivalent policy authority.

## Embedding

The public `manga_node` modules can be used by another Rust program. Most complete workflows require `db::Repository` and related storage/configuration objects. Pure parsing or event-verification calls do not automatically execute the full node's authority, retention or publication workflow.

`platform_bridge` offers a read-only visibility projection to downstream applications while retaining protocol visibility rules. It has no dependency on the removed WeebDex platform. `mangadex_compat` and `compatibility_query` retain the protocol node's mapped API subset; they do not make arbitrary MangaDex clients compatible without adaptation.

## Boundaries

This branch has no browser UI, full website/platform backend, browser signer integration, general import/backup CLI suite, benchmark/demo programs or test material. It retains the protocol node's native operator key utility and optional production image-serving supervisor. The complete production module graph remains in `server/src/lib.rs`.

