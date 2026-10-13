# Service identity remembrance and outbound discovery boundary

The user approved no automatic endpoint-announcement expiry and public-HTTPS automatic connections with explicit private/development overrides. This implementation establishes safe live verification and durable first-key remembrance. The initial schema-42 boundary is extended by the subsequently approved schema-43 announcement exchange described below.

## Operator workflow

POST `/admin/identities/verify`, authenticated with the existing operator NIP98 scheme, accepts `endpoint` and an optional `expected_key`. Before connecting it uses any already-remembered key; a conflicting explicit key is rejected. The endpoint must answer a fresh nonce with a valid service-key signature binding that nonce and its exact origin.

The first successful observation is recorded independently of peer/account/publisher permissions. A matching key can be recognized at another origin. A previously recorded origin presenting a different key returns 409 and cannot silently replace the binding, including after process restart. No expiry removes remembered identities. The table has a 10,000-origin budget and transactionally serializes first-observation races. Clearing/reassigning a binding is deliberately not exposed as an automatic operation.

First contact without an expected key establishes possession, not independent proof that the endpoint belongs to the intended organization. It cannot detect a domain transfer that happened before this instance ever learned the old key. Response signatures and content hashes remain necessary for subsequent data; a nonce response is not TLS-channel binding.

## Outbound policy

By default the connector accepts HTTPS and conservative public-unicast destinations. It rejects private, loopback, link-local, multicast, documentation, benchmarking, mapped/transition and other excluded ranges. Every DNS answer must pass; a mixed public/private answer is rejected. Answers are capped at 16, DNS resolution has a 3-second deadline, and validated addresses are pinned into the HTTP client. Redirects and ambient proxies are disabled. Connections and response bodies remain bounded.

`discovery_private_overrides` contains exact configured origins, including their effective port. It is not a suffix or wildcard rule. Overrides permit explicitly chosen private/development destinations, including HTTP, but do not disable certificate verification for HTTPS. These private allowances are not advertised publicly in capabilities and are cleared in restored configurations so the destination host's network policy can be reviewed.

The address rule is conservative, not a promise that every routable special-use address is supported or that IP classification replaces network egress controls. Its special-use exclusions were checked against the IANA IPv4/IPv6 registries on 7 October 2026. The production TLS/reverse-proxy environment still needs deployment qualification.

## Evidence

`endpoint_policy` units cover special addresses, mixed DNS answers, explicit overrides, hostname pinning, redirects, wrong nonce/origin/key and oversized proof responses. `http_remembered_identity` uses actual HTTP services and a real node/PostgreSQL: wrong expected key, first-key persistence, same-key domain move, changed-key rejection after actual node restart, no implicit peer grants, operator authentication and override enforcement. Restore coverage checks that source overrides remain unchanged while restored overrides are empty.

Sources: https://www.iana.org/assignments/iana-ipv4-special-registry/ and https://www.iana.org/assignments/iana-ipv6-special-registry/ .

## Schema 43: signed endpoint announcements

The subsequently approved experimental kind 11998 uses the required `manga-service-endpoints-v1` protocol tag and a version-1 JSON endpoint array. It is not an official Nostr allocation. Events are limited to 16 KiB, eight distinct canonical origins and a 60-second future-clock allowance. Unknown optional JSON fields are tolerated; unsupported required features are rejected. Signatures are verified before caching. Highest creation timestamp wins, with lowest event ID breaking ties. There is no expiry. An empty array withdraws addresses without deleting the service key.

The cache stores at most 4,096 identities and serializes capacity/replacement decisions transactionally. Startup seeds the local identity only when it has no announcement. Changing `base_url` does not overwrite an existing announcement or withdrawal: the operator must explicitly publish the desired complete list. Multiple API processes sharing a service key must coordinate that list.

Operator routes publish (`POST /admin/service-announcement`), import (`POST /admin/service-announcements`), fetch from a seed (`POST /admin/service-announcements/fetch`) and resolve a key (`POST /admin/identities/discover`). Public metadata is available at `/service/announcement` and `/service/announcements/{key}`; cached public reads obey the instance read policy. Approved peers can use `/federation/service-announcements/{key}` with NIP98 authentication.

The resolver accepts a service key and up to eight optional seeds. It refreshes signed metadata, tries advertised endpoints under the pinned outbound policy, proves live possession of the target key, and preserves remembered-origin conflicts. Discovery has four concurrent slots and a 30-second total deadline. It rechecks the winning announcement before returning an endpoint. A concurrent replacement requires retry (409); withdrawal returns 410; exhausted candidates return 503; deadline exhaustion returns 504. Seed failures are reported by index. Knowledge is the latest supported announcement observed, not global consensus; an isolated node cannot detect an unseen newer withdrawal.

Neither imported metadata nor successful discovery grants peer permissions or changes configured peer endpoints. Automatic peer relocation, periodic discovery and WebSocket/strfry announcement projection remain unimplemented. These HTTP adapters exchange signed Nostr-format events, not a claim of full relay interoperability.

`http_service_announcements` exercises independent databases and real HTTP nodes: a service moves with its original key, its old process stops, another node supplies signed cached metadata, live resolution succeeds, withdrawals propagate while the source is offline, replay cannot restore withdrawn addresses, equal timestamps resolve deterministically, and tampered signatures fail. Peer-authenticated exchange requires approval in both directions.

## Approved-peer relocation (schema46, focused tests passed)

The background discovery worker refreshes signed endpoint records for already-approved peers. It checks the target and up to eight other approved seed identities, verifies every cached target signature and live destination key, then conditionally updates only the existing approved peer's endpoint. It never creates or re-enables approval. Final commit locks both the announcement decision and peer row; a newer withdrawal, revocation or operator endpoint change wins over an older in-flight proof. A move advances the existing approval epoch, fencing old operations.

Automatically learned endpoints are explicitly marked in PostgreSQL. Their subsequent replication HTTP requests use the conservative public-HTTPS/DNS-pinned transport, including a fresh expected-key proof; merely checking DNS at discovery time would not protect later transfers from rebinding. Exact development overrides remain supported. Explicit operator-configured endpoints retain their existing policy. A later explicit operator update clears the learned marker and advances the epoch when that changes its transport policy.

The worker handles four peer identities per keyset page, one refresh at a time, with a30-second per-refresh deadline and60-second pauses between pages. Seed fetches are limited to four concurrent requests and8 seconds each. It starts after the initial delay, and is disabled when sync_worker_enabled or peer_discovery_enabled is false. Offline restore's paused sync setting therefore also pauses relocation. The operator status exposes the worker's lifecycle, not proof of network freshness or a completion SLA. Empty endpoint announcements prevent new relocation, but do not silently revoke a local peer approval or delete retained content.

Focused evidence: audit-peer-relocation-http.log (founder address loss, independent cached announcement, live move, epoch advance, withdrawal and revocation), audit-peer-relocation-fences.log (revocation, newer withdrawal and operator changes while proof is blocked), and audit-peer-relocation-transport.log (actual publication/library transfer through the restricted learned-endpoint path). Full schema46 integrated regression now passes60 test-result groups, Clippy and independent-process/PostgreSQL restart checks (audit-discovery-integration-full.log). Announcement projection through strfry/WebSockets remains outside this HTTP discovery path.
