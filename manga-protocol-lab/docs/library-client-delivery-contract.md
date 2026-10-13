# Public library client delivery contract

Accepted scope: a signer-agnostic client selects write relays from the user's signed NIP65 list, sends the same signed library head and required immutable objects, completes the initial save after one acceptance, keeps other recipients' outcomes/retries separate, and recovers the newest valid available library on another device.

## Explicit-endpoint foundation

An explicit-endpoint transport can require already-known expected service keys. It must prove each service identity before sending authenticated requests and verify signed metadata responses. Unknown relay URLs from the NIP65 list remain unresolved rather than being contacted automatically. This is a deliberate partial implementation boundary, not completion of automatic service discovery.

The signer interface accepts structured signing requests and verifies that the returned event matches the requested owner, timestamp, kind, tags and content. Private keys never belong in delivery queues. Each retry creates fresh NIP98 authorization; the public signed library event remains identical across recipients and attempts.

## Delivery invariants

- Validate the signed pointer, exact root hash, chunk hashes, codec budgets, family/bucket placement and complete referenced-object set locally. Reject extra unreferenced objects.
- Share immutable source bytes across recipient attempts. Build bounded object batches within the server's existing wire/object limits.
- Keep recipient concurrency, attempt count, request size, response size and deadlines bounded. Distinguish authorization/protocol rejection from retryable transport or capacity failures.
- A verified200/202 receipt for the exact event counts as acceptance; backend projection may still be pending. A409 superseded outcome must remain visible and must not be silently treated as acceptance.
- First acceptance and completion of all recipient attempts are separate observable results. A slower/offline recipient cannot cancel an already confirmed acceptance elsewhere.
- The durable outbox stores public content and delivery state only; it never stores short-lived authorization tokens or private signing keys. Restart must resume only unfinished recipients, with idempotent event/object delivery.

## Recovery invariants

Verify service identity, signed head metadata and the user's event signature. Compare candidates by newest timestamp and lowest event ID on ties, then verify the complete content-addressed root/chunk graph. Never accept corrupted or partially fetched history as a complete library. Report unreachable peers and newer-but-unavailable candidates instead of asserting global freshness. Explicitly supplied known endpoints can be tested now; automatic first-contact and endpoint-announcement integration into this client remains pending.

## Evidence required

Real multi-node HTTP tests must cover one offline recipient, first-acceptance timing independent of that recipient, duplicate retry, superseded heads, revoked account access, wrong service keys, corrupted replies/objects, interrupted object delivery and recovery from a surviving copy. This document alone is planning, not implementation evidence. No new performance load qualification is implied.

## Implemented SDK slice (7 October 2026, after schema44)

The Rust SDK now separates complete-graph validation/batch preparation, signer-controlled HTTP delivery, signed NIP65 publishing/fetching/planning, complete-library recovery, and a durable local retry outbox. There is no private-key field in the SDK queue. The host supplies a Signer implementation; every returned authorization event must exactly match the requested author, timestamp, kind, tags and content. Metadata receipts bind the expected service key, request event ID, exact URL/method/status and response bytes. Original immutable library objects are verified by hash and codec.

`SaveBundle` is part of that host-facing SDK contract, rather than a node-internal
test hook. Its validated `event`, exact `objects`, total `object_count`, bounded
`batch(cursor)` and `commit_body` let a host display complete-graph progress and
resume delivery without inspecting private fields. `object_count` includes the
root and distinct chunks; batch cursors count objects in that immutable order.
These methods are exercised by the native delivery/recovery integration host.
They do not expose a database counter or certify remote acceptance.

NIP65 write routes are selected from a valid owner-signed kind10002 event. Read-only routes are excluded; unmarked routes are writable. An explicit route-to-HTTP-origin-and-service-key binding is required. Unknown bindings are returned as unresolved, not silently contacted. Helpers publish the signed relay list and fetch the newest valid observed list from bootstrap services. Relay-list propagation is a separate helper, not part of the durable library job. Automatic binding/discovery integration and an end-user UI remain absent.

`deliver` makes one bounded attempt per recipient. It exposes first acceptance, individual outcomes and all-recipient completion separately. Dropping an observer does not cancel delivery; explicit cancellation is available through the completion task handle. Default concurrency is4, with a120-second per-recipient deadline; supported bounds are1..8 and1..600seconds. At most32 recipients are selected. Initial success means a verified receipt for the exact head, not that every replica accepted or that projection completed. Superseded, rejected and retryable outcomes remain distinct.

`run_job` uses the local outbox to persist each attempt before network work and each outcome before emitting success. It retries unfinished destinations with bounded exponential backoff (up to30seconds), default8 rounds per invocation, configurable1..32. Exhausted retryable work remains queued for a later invocation; attempt numbers remain monotonic, so late results cannot overwrite newer attempts. Accepted, superseded and rejected destinations are not resent automatically. A single-process filesystem lock and an in-process active-job guard exclude duplicate owners. Cancellation aborts remaining client network tasks; a server may already have accepted a request, so recovery retries are deliberately idempotent.

The queue uses public source objects plus atomic, fsynced state files in an owner-controlled local directory. It never stores private signing keys or short-lived authorization tokens. Default limits are16 retained jobs and512MiB, configurable up to64 jobs/16GiB. Incomplete or rejected jobs are never automatically evicted. Enqueue retires older fully accepted jobs while retaining the newest completed job; retirement markers allow interrupted cleanup to resume. Current state plus a replacement file must fit the byte budget, so large jobs can require a larger explicit queue budget. No claim is made for hostile concurrent filesystem replacement, network-filesystem lock behavior, physical power-loss survival, or browser/mobile storage adapters. File APIs are blocking; the asynchronous driver uses blocking-task isolation.

Recovery fetches signed heads from known expected service keys, orders newest timestamp then lowest event ID, and verifies every referenced root/chunk before returning a complete library. It can recover an older complete candidate while explicitly listing newer observed unavailable candidates. Unavailable peers remain reported. This is newest complete observed state, not global consensus. Recovery has a300-second aggregate deadline and at most4 parallel metadata/object operations.

Evidence currently includes two real PostgreSQL/HTTP nodes with real strfry projection; stalled-recipient first-acceptance separation; duplicate retry; NIP65 publish/fetch/selection; supersession; wrong-service-key and signer-substitution rejection; account revocation; cold recovery from a survivor; persisted unfinished-recipient recovery; malicious signed-response/body mismatch; corrupt objects; newer-unavailable fallback; and an actual killed subprocess releasing its outbox lock while retaining an unfinished attempt. Check the final regression log for the exact checkpoint outcome. Automatic binding discovery remains unimplemented. Subsequent real-node functional cases now cover interrupted multi-batch staging, durable resume and complete cold recovery at30,000 chapters (65 objects/1,267,344 canonical bytes) and500,000 chapters (257 objects/21,042,147 canonical bytes). The public head remains unchanged after the interrupted partial batch. These single functional cases are not throughput or production-capacity benchmarks; see http-library-client-large.json and audit-library-client-large-http.log.

Primary protocol reference, checked7October2026: https://github.com/nostr-protocol/nips/blob/master/65.md . This project's HTTP adapters and queue budgets are additional application contracts, not requirements imposed by NIP65.

## Automatic same-origin client binding (after checkpoint16)

The Rust client discovery helper can turn an owner's signed NIP65 write list into verified HTTP recipients for the reference root-WebSocket profile. It treats ws/wss-to-http/https conversion only as a candidate location, then applies the outbound destination policy and fresh service-key possession proof. It requires the same origin to advertise compatible library support, namespace/API version and the identical service key. Advertised cross-origin URLs are not followed. Unknown non-root WebSocket routes remain explicitly unresolved; callers may supply an explicit expected binding for them.

Discovery accepts previously remembered bindings and never silently replaces their expected keys. It returns first verified bindings for the host application to persist, together with per-route unresolved reasons. This does not yet implement a browser's durable identity store. First contact establishes continuity from that observation, not earlier domain ownership. The helper verifies only configured support; actual account admission and library acceptance remain separate.

At most32 selected routes are processed, four concurrently, with a15-second per-route deadline. Known bindings and failed routes preserve list order. HTTP fixtures cover two cold bindings, remembered reuse, changed expected key refusal and an unsupported path. This helper is not universal NIP65-to-arbitrary-server interoperability.

The full schema46 integrated regression includes automatic binding and passes60 result groups, Clippy and independent-process/database restart checks. The30k/500k interrupted recovery fixtures remain included. See results/audit-discovery-integration-full.log.

A single cold-device HTTP fixture now chains surviving-seed NIP65 retrieval, automatic root-route binding and exact library graph recovery. It succeeds without saved per-recipient bindings, reports the offline route and unsupported path, and returns first observations for the host to persist. A surviving seed location/identity is still required; the private key alone cannot locate lost data. See audit-cold-bootstrap-pipeline-2.log. The initial added fixture used an older list timestamp and was correctly superseded; its corrected newer-list flow passes.
