# Request-bound federation metadata proofs

The service identity challenge proves possession of the pinned key for that challenge. By itself it does not authenticate subsequent HTTP response bytes: a proxy controlling an endpoint could forward the challenge and then alter unsigned metadata. The earlier prototype had that gap. This extension binds metadata responses to the request and service key.

NIP98 defines HTTP request authorization. The response envelope here is an experimental application extension, not a claimed NIP98 response standard. See the official request specification: https://github.com/nostr-protocol/nips/blob/master/98.md

## Envelope

The X-Manga-Response-Proof header contains a base64-encoded signed Nostr event with the Nostr token prefix. The experimental ephemeral kind is20006. Its tags bind:

- protocol = manga-federation-response-v1
- u = exact public request URL
- method = GET or POST
- request = the signed NIP98 request event ID
- status = HTTP status code
- payload = SHA256 of the exact response body bytes

The receiver verifies the signature against the independently approved service key, all bindings, and a60-second timestamp window before interpreting or admitting metadata. Each request has a fresh nonce, so a proof from an earlier request cannot authenticate a later response. The response event is never imported into the relay. The allocation remains private/experimental and needs protocol registration review before a public standard claim.

Signed routes include federation events, account/deletion evidence, library heads/routing hints and reconciliation frames. Authenticated client metadata and write acknowledgements also carry proofs; the migration client verifies them, including upload/staging acknowledgements and signed errors. It pins the service key before sending requests and uses fresh nonces for both NIP98 and Blossom upload authorization. Signed error statuses matter too: for example, an authenticated partition-required response can drive bounded splitting. Successful metadata from an older unsigned implementation is rejected; there is no silent downgrade. Upgrade cooperating nodes together.

Originals and root/chunk blob routes are content-addressed. Their expected SHA256 is already committed by the verified signed metadata/manifest, and the client checks bytes against the requested hash and size. They do not incur another service signature per blob, and ordinary public image reads are unchanged.

Federation metadata signing has4 pre-handler admission slots and a2MiB body limit. Authenticated client metadata uses a separate32-slot post-handler budget and4MiB limit, preserving existing save-admission concurrency. Both retain bounded output lifetimes. A write may commit before a response-budget/network failure; clients retry exact IDs/plans rather than assuming failure rolled it back. Content-addressed byte responses and requested library object batches are verified by their committed hashes instead; HEAD responses do not use this envelope. The peer client still has its original bounded reads, timeouts, no-proxy/no-redirect behavior and late approval fences. Intermediaries must preserve exact metadata body bytes; even a harmless JSON whitespace rewrite invalidates the proof. Do not enable transforming proxy filters on these routes.

Key continuity authenticates signed data, not domain ownership, availability or the absence of transparent forwarding. It does not protect a reader who runs a compromised website's client code or erase previously delivered public data. Non-loopback HTTPS remains required for transport confidentiality.

## Evidence status

service_proof unit tests check exact body/URL/method/request/status binding, pinned key and missing proofs. federation-proof-regression logs track two real PostgreSQL/strfry nodes behind a synthetic loopback adversarial proxy. The proxy forwards a valid identity challenge but rewrites JSON whitespace, removes proofs, or replays a proof from an earlier request. An honest pass-through transfer must still succeed with complete local originals. The first fixture incorrectly required200 rather than allowing202 durable acceptance before projection; that assertion is corrected in the rerun. federation-proof-regression-3.log passed the corrected peer fixture; authenticated-response-proof-regression.log also passed the migration-client acknowledgement tampering case. authenticated-metadata-full-regression.log passed the full suite, including import/restore, Clippy and PostgreSQL restart checks.
