# Approved-peer NIP42 event interface

This closes accepted decision37B independently of the NIP98 HTTP replication interface selected in59A. The ordinary reader relay remains at the root URL under its existing public/authenticated/admitted policy.

## Wire and authorization

The namespaced capability registry advertises `peer_websocket` with its URL and `/federation/relay` path. Use a WebSocket upgrade at that endpoint. A non-upgrade GET returns426 with an Upgrade header and no content inventory. An unavailable event backend returns503.

The server issues a fresh NIP42 challenge. The connecting client signs an ephemeral kind22242 event with its own peer/service key, exact endpoint URL, challenge and fresh timestamp. The key must currently be on this instance's approved-peer allowlist. Personal-account admission alone grants no access to this interface. Conversely, approving a peer does not admit its key as an ordinary reader or publisher.

The interface accepts bounded NIP01 REQ/CLOSE subscriptions for the guarded, projected event view. History/live behavior, exact event identities, filter limits, health/deletion gates and continuity-reset handling are shared with the normal relay. AUTH is ephemeral and is never stored as catalogue data. Multiple authenticated keys are allowed within the existing eight-key bound.

This endpoint is read-only. EVENT writes are refused even when the authenticated peer separately has a personal account or publishing permission. Publication/group APIs and receiver-driven approved HTTP transfer remain the write/admission paths. Stable manifest-aware Negentropy snapshots, original-byte transfer and selection verification still use the HTTP contracts; this is not a NIP77 claim.

## Revocation and resource limits

Peer approval is checked during AUTH, before history/live delivery and periodically while an authenticated peer connection exists. Revocation closes active subscriptions and the connection; a separate personal-account permission cannot keep it open. Depending on concurrent notifications, the client may receive CLOSED or an ordinary socket closure and must not assume continuity.

Eight peer connections have their own semaphore, separate from the ordinary reader connection budget. Unauthenticated connections close after15seconds. Authenticated idle connections remain subject to the approval check. Existing message, command, query, subscription, output and send-time budgets still apply. These bounds are admission controls, not target-capacity certification or comprehensive Internet DoS protection.

## Service identity

NIP42 authenticates the client to the service. It does not independently prove the server's identity. Clients must retain the existing expected-service-key/verified endpoint and TLS procedures before using a discovered endpoint. The peer endpoint is advertised under the configured service origin; it does not authorize following arbitrary URLs or inheriting another service's peer permissions.

## Evidence

The real PostgreSQL/strfry HTTP/WebSocket fixture `http_peer_websocket` exercises distinct peer and reader roles, wrong endpoint/challenge, projected historical and live events, denied EVENT writes, revocation, ephemeral AUTH and idle unauthenticated connection expiry. Focused checks,Clippy and process/database restart pass in `results/peer-websocket-focused.log` and `peer-websocket.json`. The final combined71-group regression passes, including the later operational-error correction, strengthened founder-loss publishing and validated WSS through nginx. See results/backend-mvp-final-full.log and its zero exit record.

The implementation extends the existing NIP01/NIP42 framing; peer-role routing is a project extension. The standard references remain [NIP01](https://github.com/nostr-protocol/nips/blob/master/01.md) and [NIP42](https://github.com/nostr-protocol/nips/blob/master/42.md). No raw strfry listener is exposed.

## TLS reverse proxy

The reverse proxy must forward WebSocket upgrades using HTTP1.1 with the Upgrade header and an appropriate Connection mapping. Configure the wrapper base_url as its public HTTPS origin; NIP42 then binds the public WSS peer URL, not an internal HTTP listener or supplied forwarding headers. The local TLS fixture exercises this with a temporary trusted CA, rejects that CA in a default client, and never disables hostname/certificate validation. This is not a production certificate-management service.
