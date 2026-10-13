# Historical API notes

For the current Rust/PostgreSQL wrapper, start with [API-INDEX.md](API-INDEX.md). The chronological notes below describe earlier milestones and may say later-implemented features are pending. They are retained as historical evidence, not a current endpoint inventory.

# Experimental loopback API

This describes the implementation, not an accepted wire-standard allocation. All routes are rooted at the configured base_url. Actual request construction and signature examples live in prototype/tests/e2e.py.

## Authentication

Writes and administrative control requests use Authorization: Nostr followed by a base64 JSON signed NIP-98 event. Bind the exact external URL, method, body SHA-256 for every write including empty bodies, and a fresh nonce. Events expire within 60 seconds and cannot be reused.

Public metadata reads are anonymous by default. The optional require_metadata_auth setting restricts metadata endpoints; this development profile still serves admitted images publicly and is not a complete private-instance mode. If a read supplies an authorization token, that token must validate.

Blob requests use a base64url signed kind-24242 event, action, domain, hash and expiration scopes. Maximum token lifetime is 300 seconds; at most 100 hash tags. Public admitted blobs need no token; staging reads require uploader authorization.

## Publication and reading

- PUT /upload: X-SHA-256 and body bytes, authenticated upload. Returns blob hash/size/URL.
- POST /events: JSON signed event. Acceptance is not a claim all images are hosted.
- GET /events/{id}: exact signed event or retained deletion request.
- GET /query?filter=...: parameterized expression over verified hosted chapter metadata. Live offset/limit pagination, with inclusive since/until timestamp bounds. See pagination-options.md for parameters and current provisional ordering/response details.
- GET or HEAD /{sha256}.png: direct nginx original bytes for admitted PNG. The .jpg and .webp paths serve admitted JPEG/WebP originals with matching MIME and nosniff headers.
- GET /{sha256}: guarded request with internal nginx redirect.
- DELETE /{sha256}: owner-only unreferenced staging deletion; shared claims and publication references prevent deletion.
- GET /accounts and /deletions: bounded inspection interfaces, 10,000 events each.
- WebSocket /relay: bounded NIP-01 EVENT/REQ/CLOSE adapter with NIP-42 AUTH. Initial queries use parameterized SQL over events plus retained deletion requests; exact IDs still work beyond 10,000 total records. Eight filters/subscriptions, 1,000 results per filter and a 2-MiB initial-response budget apply. Tag scans have a SQL-work budget. Not a full relay replacement.

Blob filesystem objects and public links are split by the first two and next two hash characters, while public URLs remain hash-based. Admission and deletion preserve local reference ownership. This layout is new experimental storage, with no upgrade migration from the earlier flat research layout.

## Group operations

- POST /groups/{key}/members: member and role (admin, publisher, remove); authorized group admin or instance admin.
- POST /groups/{key}/publish: manifest hash and optional kind (1001 chapter default, 1002 series). Group publisher/admin can publish.
- POST /groups/{key}/delete: targets array. Group admin requests kind-5 deletion; the instance moderator still decides whether to apply it.
- Adding a member grants that group scope and staging-upload access without adding global individual-publisher privileges. Revocation removes that scope. Configured bootstrap administrators are seeded only when a group is first created, so restart does not silently restore a revoked role.
- The group role matrix remains an experimental conservative default, not a final accepted role policy. The caller never receives the group secret.

## Federation and moderation

- POST /sync: synchronous instance-admin request with approved peer key, filter and inventory (chapters/accounts/deletions).
- POST /jobs/sync: experimental admin-only background equivalent; returns 202 and a persistent job ID. GET /jobs/{id} inspects it; POST /jobs/{id}/cancel and /retry control it. See jobs.md.
- POST /reconcile: approved-peer-only Negentropy session messages. Fixed snapshot, 100,000-entry cap, work budget, 64 KiB message cap, eight concurrent sessions, 30-second expiry, 256 rounds and 16 MiB cumulative binary protocol bytes, explicit close. An optional session-bound lowercase hexadecimal prefix selects an event-ID range; the async worker can split overfull ranges. High-latency exchanges can still expire and need smaller scopes.
- GET /identity?nonce=...: signed fresh service-key proof with nonce and endpoint. This is not channel-bound TLS.
- POST /moderation/approve: request and target IDs; independently applies matching-author request locally.
- POST /moderation/restore: id; explicit restoration of exact protected publication.
- POST /maintenance/retry: id; revalidate a candidate after its missing originals arrive.
- POST /maintenance/scrub: check next bounded batch of 128 hosted publications using a persistent rotating cursor. A returned count is withdrawals in that batch, not proof the entire library was scanned.
- POST /maintenance/gc: expire unprotected staging data.

## Experimental resource limits

1 MB events/manifests; 16 MiB individual blobs; 32 MiB default unhosted staging per identity (configurable through staging_bytes_per_identity); 40 million decoded pixels; 10,000 manifest pages; 32 concurrent HTTP control handlers. A single large chapter can exceed the default staging allowance. Operators can raise that explicit allowance within disk capacity. A production transfer-reservation scheduler is still needed for efficient concurrent imports. event_store_map_bytes sets the LMDB virtual map ceiling independently of blob storage capacity (1 GiB default); this is not preallocated physical image storage.

Unsupported expiration tags and oversized tag values are rejected before strfry ingestion. Backend imports are verified by exact-ID readback before the durable outbox job is acknowledged.

The formats configuration advertises the supported instance profile. Default implemented types are image/png, image/jpeg and image/webp. Other implementations can select other formats; there is no protocol-wide PNG baseline.

backend_timeout_seconds configures the native backend command deadline (default 30 seconds; range 1–300), with a further one-second KILL grace. This is a local implementation control, not a federation wire guarantee. GNU coreutils timeout is required.
