# ADR 0002: chunked public libraries

Status: format approach accepted by the user on6 October2026; production encoding/API details below are an implementation draft pending conformance tests.

## Decision

One public library belongs to one ordinary Nostr key. A replaceable signed event points to an immutable, content-addressed root. The instance verifies and retains all required local objects before accepting a hosted root. Changed state is transferred in bounded batches; a client can retrieve one needed chunk or a complete-library bundle.

There is no private-library/master-key subsystem, automatic cross-device merge or per-chapter addressable-event model. Nostr timestamp/event-ID ordering remains the conflict rule. A local expected-head guard helps one-instance clients detect stale writes, and exact retries must remain idempotent even after a lost acknowledgement.

## Production implementation draft

- Reuse the publication pointer envelope: experimental kind11999, content containing manifest hash and schema version. Do not put the full read history in the Nostr event.
- Use deterministic CBOR objects with the same untrusted-input preflight discipline as publication manifests. The comparative benchmark intentionally used JSON for both options; its timings are not a claim about the final CBOR implementation.
- Partition typed state so series/list metadata cannot become another unlimited monolithic snapshot. Proposed families: chapter state, series state, list descriptors, list membership. Each family has a bounded power-of-two bucket vector in the root. Different families may use different bucket counts.
- Chapter state references the exact chapter event ID. Series state references the exact series event ID. List descriptors use stable client-generated IDs; list membership identifies both list and series. Referenced manga need not be hosted by the same instance merely to appear in someone's public history.
- Canonical records are strictly ordered and unique within each chunk. A deterministic hash of the record key chooses its bucket. Root descriptors include object hash, encoded size and record count, with checked totals. Empty bucket slots use CBORnull and require no blob/reference-counter row, avoiding a globally shared hot empty-object lock.
- Treat size/count limits as explicit local admission budgets. Test at least the accepted30k/500k scenarios. Large single requests must not bypass an aggregate in-flight byte budget by queuing outside an operation semaphore.

## Avoid whole-library validation on every change

Validate newly uploaded chunks once, under bounded CPU/memory/I/O limits. Persist trusted summaries only after canonical decoding and verified durable storage. A summary can record its type, size, count and the common low bits of all record-key bucket hashes. This allows a later root to prove that an unchanged chunk fits a particular power-of-two bucket without decoding every entry again. Empty chunks are a separate case.

The root commit still checks current object generation/state, all descriptors, counts, local grants/authority, expected head and replacement ordering in a short transaction. A client-supplied summary is never authoritative. Cross-chunk uniqueness follows from deterministic bucket assignment plus per-chunk uniqueness, rather than rescanning500k records on every page save.

## Replacement, retention and recovery

A current root, an in-flight read and pending strfry projection are different references. Do not reclaim an old root's chunks merely because a newer PostgreSQL head exists: the backend may still be serving the old pointer. Projection must respect replaceable-address barriers and verify the new event/old absence before retiring backend-dependent references. Keep bounded read grace afterward.

Object intent, byte charges and generation fences are durable. A process crash after writing objects but before root commit leaves accounted staging work, not an accepted incomplete root. A collector needs reference/pin checks and confirmed generation-specific cleanup before refunds. Expiry alone does not prove I/O stopped. Audit/repair and safe reclamation are shared work with Ticket12.

## Serving and portability

Expose a small signed head, immutable object reads, a bounded/streamable full bundle, and batched commit. A full500k restore still transfers substantial data; selective access is a first-class path. Avoid a request waterfall or materializing the entire library for a one-chapter operation. Peer import remains limited to approved sources and must not grant imported authors a local publishing login.

## Evidence and validation gates

See the A/C report and raw fixtures. Early RAM-backed tests were superseded by workspace-filesystem large-reader runs. Initial concurrent results were superseded after removing heavyweight monitoring, bounding client caches and replacing historical-root visibility scans with current-reference accounting. The corrected five-minute fixture completed 30,000 writes at 100 offered writes/second with no errors or final backlog; this is not yet an integrated production-path result. No physical power-loss or target-deployment capacity claim follows from these fixtures.

Before marking production support complete: canonical codecs and malformed-input tests; replacement/tie/deletion tests; stale-authority and object-generation fences; missing-object rejection; crash/retry behavior; bounded batches and byte admission; shared-reference/read-grace tests; approved-peer portability; corrected sustained-load tests against the integrated path.
