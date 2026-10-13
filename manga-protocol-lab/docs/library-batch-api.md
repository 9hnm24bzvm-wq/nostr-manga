# Batched library transport checkpoint

The batch endpoints and bounded transport passed the full HTTP/PG/strfry regression. Actual-node serial and initial concurrent measurements are available in library-node-performance.md; previous A/C timings belong to the separate comparative fixture. Throughput refinements are being verified.

## Writes

POST `/libraries` still accepts `event` and optional `expected_head`. It now optionally accepts `objects`, an array of `{ "type": "chunk" | "root", "data": "standard-base64" }`. Changed chunks and the new root can travel with the signed pointer in one request. Every object is verified and durably staged before root admission. A failed final commit can leave accounted private staging bytes, not a partially hosted root.

POST `/library/batches` accepts an `objects` array without committing a head, for initial large uploads split across bounded batches. Replies explicitly identify these objects as staged and not public. The original single-object endpoints remain supported.

Limits: at most64 objects,8MiB wire JSON/decompressed JSON,6MiB aggregate decoded object bytes,1,000,000 bytes per chunk and256KiB per root. Separate count/byte-bounded receipt and eight owned processing operations now bound receipt/parsing/work; see library-throughput-controls.md for the queue/deadline limits and verification status. The event is borrowed as raw JSON and bounded to16KiB before deserializing it; typed bounded object parsing avoids first materializing an unlimited generic JSON array.

Signed write routes accept identity or one gzip member. NIP-98's payload hash covers the exact transmitted body bytes, including gzip encoding when used. Decompression has its own output limit and rejects trailing members/data. Compression does not alter the canonical manifest bytes or their content hashes.

## Reads

POST `/library/read` anonymously accepts `{ "hashes": [ ... ] }`, at most64 hashes in an8KiB request. Every requested object must be publicly retained; a mixed public/private batch fails without returning any objects. The result is `{ "objects": [{ "hash": "...", "data": "standard-base64" }, ...] }`. An estimated8MiB encoded-response bound is checked before building the body. Optional gzip responses honor an explicit acceptable gzip token; clients verify the decoded object hashes.

Public and authenticated staging reads use the same bounded response producer. Payload/encoding leases share a64MiB budget; encoding reserves its bounded transient workspace as well. The producer copies8KiB frames through a one-frame channel, retains its lease while blocked, releases on disconnect and has a120-second deadline. A stalled receiver therefore cannot retain the entire payload indefinitely. This does not claim to bound every HTTP connection, TCP/kernel buffer or process allocation; connection/IP limits and deployment serving roles remain work.

## Evidence and limits

- results/library-batch-unit.log:51 normal unit tests passed; the large codec test remains separately invoked. New cases cover malformed/oversized lists, gzip expansion/multiple members, null-versus-absent expected heads, ordinary streaming, disconnect and a stalled receiver's deadline.
- results/library-batch-regression.log: full regression, including combined gzip upload/commit, exact wire-body authentication, gzip public bundles, private-object exclusion, count limits and decompression rejection.
- These tests use synthetic local data. They are not a WAN, slow-socket system-memory,100TB or target-traffic certificate. Physical reclamation, ordinary deletion and integrated load validation remain open.
