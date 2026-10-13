# Guarded Nostr relay transport

Current implementation notes reconciled on 2026-10-11; the schema-26 regression evidence below is historical and does not certify later refactors. See the [current architecture](rebuild/ARCHITECTURE.md) and [current acceptance/status](rebuild/STATUS.md).

Schema 26 originally introduced WebSocket upgrade at the same root URL as HTTP capabilities. The wrapper serves its PostgreSQL-authorized, projected view. Do not expose the raw strfry listener: it lacks the wrapper's quarantine, admission and protected-publication deletion rules.

## Wire behavior

The implemented relay NIPs are 01,11 and42 within the following bounded experimental profile. HTTP GET with Accept: application/nostr+json returns relay information and CORS permission. Normal root GET still returns wrapper capabilities. The relay URL is the configured base URL with ws/wss and a root slash.

- REQ accepts1–8 filters. Fields within a filter are AND; values and separate filters are OR. Exact64-character lowercase IDs/authors, kinds, inclusive since/until, single ASCII-letter tag filters and limit are supported. Prefix IDs and unknown extensions are rejected.
- Historical results are newest-first, lowest ID on ties. Each filter's limit is clamped to500; limit0 sends EOSE then listens for future matches. This NIP01 history order is separate from Q93D's explicit HTTP discovery ordering.
- EVENT deliveries can be duplicated around the history/live boundary; clients deduplicate by signed event ID. EOSE only marks completed history, not an immutable snapshot across later requests.
- CLOSE releases a subscription. Reusing its ID replaces the previous subscription.
- AUTH uses a fresh per-connection challenge and a signed ephemeral kind22242 event with exact relay/challenge tags and a timestamp within60seconds. The restore authorization barrier applies. Up to8 different authenticated keys remain active together. AUTH events are never stored or broadcast.
- EVENT requires that event's key to have authenticated on this connection. It then enters the same guarded account, library or publication workflow as HTTP. OK true means durable admission, not necessarily finished strfry projection.
- Kinds0/3/10002,1,7/1111,5,11999 and1001/1002 use their dedicated workflows. Library pointers need all referenced local objects; publications require valid manifests and verified local originals. Managed-group members continue to use the instance's group signing API rather than impersonating its custody key over this socket.
- Kind5 ordinary deletion evidence is visible to clients. Protected publications still require local moderation. No blanket NIP09 automatic-deletion compatibility is advertised.

## Live visibility and failure

An acknowledged import transaction notifies a dedicated PostgreSQL LISTEN connection. Notifications contain IDs only and confer no authority. The shared dispatcher performs one guarded visible-event lookup per processed notification, excluding pending projection, deleted/superseded events and quarantined content, then routes the result to matching client queues. Each socket separately rechecks current read authority before delivery. Deletion requests use a separate signed evidence view.

The dispatcher session is registered before historical queries, so matching commits during history queue for live evaluation. The feed has a bounded 1,024-entry notification channel; the dispatcher has up to 40 sessions and 64 queued events per session, covering the ordinary and peer transports. Lag, a full client queue, disconnected notification service or an unverifiable event lookup interrupts affected continuity instead of silently pretending the stream is complete. Clients must reconnect and issue REQ again. Historical queries are repeatable-read with500ms statement budgets,4MB work_mem and4MiB aggregate response budget. The feed uses one dedicated connection in addition to the configured pool. Schema26 backfills the local tag index during migration; schedule an appropriate migration window for large existing catalogs. The local MD5 tag fingerprint only accelerates candidate lookup: exact original JSON tags are always rechecked; it is not a signature or content hash.

Current source: [feed](../server/src/relay_feed.rs), [shared dispatcher](../server/src/relay_dispatch.rs), [socket lifecycle](../server/src/relay.rs), and [history queries](../server/src/relay_query.rs).

## Instance read policy

Set read_policy in node configuration:

- public (default): anonymous HTTP and WebSocket reads.
- authenticated: valid NIP98 HTTP requests or NIP42 socket authentication; reading does not require posting admission.
- admitted: the authenticated key must also be an allowed personal account. Socket subscriptions recheck before delivery and attempt a periodic check every 2 seconds while active. If read admission is busy, the periodic check is deferred; delivery still requires successful authorization within its bounded deadline. Revocation closes their access; it cannot retract bytes already delivered.

This policy covers public events, originals, discovery, social queries, libraries/blobs/batch reads, account metadata heads and ordinary deletion evidence. HTTP HEAD is also guarded. Supplied HTTP credentials are validated even in public mode. NIP98 binds the exact URL/method, read-POST bytes and replay ID. Private batch reads bind the exact request bytes, including whitespace. Root capabilities, identity proofs and NIP11 information remain public. Federation routes retain separate bilateral peer authority, so a public-reader setting does not grant peer replication permission.

The policy is startup configuration; changing it requires a restart. Account admission changes are live. TLS remains mandatory in advertised non-loopback base URLs. These are application policy guarantees, not a claim that a domain/key cannot be seized or leaked.

## Explicit limits

Default fixed transport limits:32 WebSocket connections,8 subscriptions each,8 filters per request,8 authenticated keys,64-byte subscription IDs,1MiB incoming messages/frames,2MiB maximum WebSocket write buffer,5seconds per send and10seconds for the entire history delivery. A per-connection token bucket allows100 incoming commands/second with200 initial burst tokens, then closes on excess. Event admission has 16 slots shared with HTTP event routes; the separate approved-peer socket endpoint has 8 connection slots. Protocol/connection limits are defined in [protocol limits](../server/src/protocol_limits.rs), [node configuration](../server/src/node.rs) and [relay transport](../server/src/relay.rs).

Reads now use configurable Catalogue, LibraryHead and Live classes under an overall limit, rather than one undifferentiated query semaphore. Defaults are overall min(4, database connections), per-class active min(2, overall), 8 waiters per class and a 200 ms admission wait. These caps provide bounded contention isolation, not strict reserved shares. HTTP exhaustion returns 429/Retry-After; new socket AUTH/REQ operations report a rate-limited result. Existing subscriptions do not close merely because a periodic policy check cannot obtain admission. Event lookup/delivery authorization retries admission within a 2-second deadline and interrupts continuity if it still cannot verify the result.

Admitted HTTP handlers and owned live-event/policy lookups retain read credits until their actual database work ends, even after response deadlines or caller disconnect. Socket sends do not retain database read credits. Download response count/bytes, upload/library processing and physical storage retain independent bounds. See [read admission](../server/src/read_admission.rs), [HTTP lifetime ownership](../server/src/node/read_admission.rs), [refactor evidence](../results/rebuild-admission/parity-admission-dispatch.md), and [disconnect/deadline evidence](../results/rebuild-admission/http-work-ownership.md).

These limits bound individual resource consumers; they are not a DDoS, Internet deployment or sustained-capacity assessment. Guarded image HTTP reads consult local metadata. Optional managed NGINX direct serving is implemented for retained verified public originals only, with explicit public-policy configuration, generation-bound export holds and verified reader-process drain before reclamation/refund. It does not expose manifests, staging, private submissions or library objects. Uncertain helper/namespace/reader health fails direct serving closed; recovery does not automatically reactivate it or imply reverse-proxy fallback. See [managed serving](managed-public-serving.md), [current serving-role source](../server/src/serving_role.rs), and the [current architecture](rebuild/ARCHITECTURE.md).

## Historical schema-26 evidence and limitations

The following logs describe the original schema-26 implementation. They are retained for provenance, not used as proof of the later classed-admission/shared-dispatch/managed-serving behavior. Current combined acceptance is linked from [rebuild status](rebuild/STATUS.md); focused refactor evidence is linked above.

- relay-query-regression.log: real PostgreSQL migration, projection visibility, exact tag recheck even with a forged index candidate, multi-filter deduplication, timestamp ordering, deletion evidence, zero-limit live matching and revoked writing.
- relay-websocket-regression.log / relay-websocket.json: real node and strfry, anonymous history/live, multi-key AUTH, rejected challenge, protected-publication validation and deletion replay.
- relay-policy-regression.log / relay-policy-*.json: all public HTTP routes gated, NIP98 replay and body binding, authenticated-versus-admitted readers and live subscription revocation.
- relay-feed-recovery-regression.log: terminated PostgreSQL notification connection emits an explicit continuity reset, reconnects and resumes notifications.

The historical complete schema-26 regression, Clippy and multi-process PostgreSQL restart checks passed in guarded-relay-full-regression.log. These passes do not imply target-scale capacity or slow-client chaos coverage. These tests use synthetic data and loopback, not100TB or8billion monthly image requests. Raw failed compile iterations are retained for transparency and are not passing test evidence.

## Sources

Official specifications consulted2026-10-07:

- https://github.com/nostr-protocol/nips/blob/master/01.md
- https://github.com/nostr-protocol/nips/blob/master/11.md
- https://github.com/nostr-protocol/nips/blob/master/42.md

## Approved-peer event transport

The separate [peer WebSocket interface](peer-websocket.md) uses NIP42 plus the current peer allowlist at `/federation/relay`. It is read-only and does not change the ordinary root relay's reader policy. Normal account admission and peer approval remain separate roles.
