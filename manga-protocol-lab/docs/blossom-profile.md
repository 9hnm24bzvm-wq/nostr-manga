# Server-scoped Blossom transport profile

This is an explicitly restricted interoperability profile using BUD01 retrieval, BUD02 upload, BUD11 signed authorization and BUD12 deletion routes. It is not unrestricted/full Blossom conformance. The user chose mandatory server scope for every authenticated blob action on 7 October 2026 (choice A).

## Endpoints and authority

- PUT `/upload`: exact request bytes, optional `X-SHA-256`, 201 for newly stored bytes and 200 for existing available bytes. The result includes URL with extension, SHA256, size, detected MIME and generation creation timestamp. The new/existing classification is captured under the same physical mutation guard as admission, not from an unlocked pre-check.
- GET/HEAD `/<sha256>[.extension]`: suffix never determines MIME. Single byte ranges are supported; unsupported multipart/unknown range units and conditional If-Range requests fall back to the whole representation. HEAD preserves metadata and omits body. All reads are no-store, avoiding cached moderation/revocation promises.
- DELETE `/<sha256>[.extension]`: exact one hash, current publisher authority and staged-object interest. Shared/hosted/retired/held bytes cannot be removed through this route. The existing durable cleanup/refund path remains authoritative. Success follows confirmed cleanup; failures never imply deletion. Multiple x tags never cause bulk deletion.
- Optional HEAD `/upload`, `/list`, `/mirror`, media/transcoding, payments and other BUD extensions are not implemented.

New routes require Base64url without padding for kind 24242 tokens. Server tags must contain the configured lowercase domain, without scheme, port or path. Scope is domain-based, not service-key- or port-based: services at different ports on the same domain are not distinguished by BUD11. Upload/deletion require a matching x hash; GET/HEAD may omit x tags. Every token has a past creation time, a future expiration, nonempty human-readable content, and a locally restricted maximum 300-second lifetime. Restoration cutoffs and current admission epochs remain enforced.

An uploader needs publisher admission; account admission alone does not grant it. A public instance permits anonymous reads only for verified, projected hosted originals. Unpublished staging stays private to authorized upload interests. Authenticated/admitted instance read policies apply to the new routes too. No browser origin grants storage, publishing, account or operator rights.

## Deliberate compatibility restrictions

BUD11 normally accepts tokens without server tags. This node rejects them by explicit user choice. Legacy `/upload/{hash}` and `/blobs/{hash}` routes retain full-origin server tags and their prior token behavior, avoiding a silent breaking change to existing clients.

BUD02 describes publicly accessible descriptor URLs. This node's staging descriptor URL requires authorization until publication; the response explicitly marks staging/private access. Upload does not by itself publish bytes. Capabilities retain `standard_blossom:false` and identify the restricted profile and implemented routes instead of claiming unqualified interoperability. Clients that assume anonymous access immediately after upload need an adapter or must wait for approved publication.

## Cross-origin policy

Blossom root blob routes and NIP11 discovery always use `Access-Control-Allow-Origin: *`, as required by their browser contracts. General application routes use configurable `cors_origins`, default `["*"]`. An explicit list allows only those exact HTTPS origins (HTTP is permitted for loopback development). An empty list disables cross-origin application access. A supplied unapproved Origin receives 403 before its handler executes. Requests without Origin still require ordinary authorization.

OPTIONS performs no state mutation and exposes the allowed methods/headers. Authorization is explicitly allowed; browser cookie credentials are not enabled. Response proof, range, length and diagnostic headers are exposed. CORS headers cover success and error responses. This is HTTP contract testing, not a claim that a production reverse proxy or every browser/client implementation has been qualified.

Sources checked: https://github.com/hzrd149/blossom/blob/master/buds/01.md , /02.md , /11.md , /12.md .
