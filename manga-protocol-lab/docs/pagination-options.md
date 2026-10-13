# Accepted discovery query contract

On 6 October 2026, the user chose original question 90B with timestamp filtering, 91A and 92A. These mappings were verified against the original question, not the differently lettered supplementary comparison previously in this file.

## Selected semantics

- Ordinary discovery uses offset and limit against the live index. Concurrent additions and withdrawals can repeat or skip results. It is not a snapshot cursor.
- Timestamp filtering is supported. The implementation uses inclusive Unix-second bounds on signed Nostr created_at, consistent with Nostr since/until filtering. This is author event time, not local ingestion time.
- Non-language text matching is exact and case-sensitive. Language matching separately retains RFC 4647 basic filtering and case-insensitive language-tag comparison.
- Comparisons involving absent metadata yield unknown. Only a final true selects the record; NOT unknown remains unknown. exists(field) and missing(field) explicitly handle absence.
- Stable bounded Negentropy inventories remain unchanged. Ordinary live discovery pagination does not replace those reconciliation snapshots.

## Current experimental HTTP allocation

GET /query accepts filter, offset, limit, since, until, sort and order. All query parameters must be part of the exact URL bound by a supplied NIP-98 signature; clients must sign the URL encoding they actually send.

- offset: nonnegative integer, default 0.
- limit: integer 1–1000, default 1000. This is an implementation budget, not an accepted protocol-wide ceiling.
- since/until: inclusive nonnegative Unix seconds on created_at. Reversed bounds are rejected. Bounds intersect the expression filter; they do not replace it.
- sort: currently created_at only.
- order: asc or desc. Descending is the current provisional default; event ID ascending breaks equal-timestamp ties in either direction.
- Responses contain signed events, offset/limit, has_more, next_offset, ordering metadata and snapshot:false. No exact total is currently computed.
- Invalid, duplicate and unknown query parameters are rejected. SQL remains parameterized; a work budget and two-MiB response budget reject excessive work rather than returning an unexplained shorter page.

Questions 93 and 94 are pending: default ordering and whether exact totals should be opt-in. Current defaults/response shape are provisional, not answers to those questions.

## Evidence

The actual HTTP fixture covers stable-data traversal, timestamp ties, both time directions, inclusive time bounds combined with expressions, exact text matching, unknown under NOT, explicit absence, invalid/duplicate parameters and full-URL authentication. It also inserts a new event between page requests and observes the expected live-offset repetition. That last result documents the selected tradeoff; it is not a failed snapshot guarantee.

The fixture revealed a client URL-encoding issue: aiohttp normalized encoded parentheses after the helper signed the raw URL. The helper now preserves the exact encoded URL it signs; server authentication was not weakened.

Source baseline: [NIP-01 filters](https://github.com/nostr-protocol/nips/blob/master/01.md#from-client-to-relay-sending-events-and-creating-subscriptions). The custom offset HTTP API and its response envelope remain project extensions.

### Basic language range syntax

The filter accepts the basic language-range grammar in [RFC 4647 section 2.1](https://www.rfc-editor.org/rfc/rfc4647.html#section-2.1): a 1–8 ASCII-letter primary range followed by zero or more 1–8 ASCII-alphanumeric subtags, or a single `*`. Embedded wildcards, empty subtags and numeric primary ranges are rejected. Range validity does not require a complete registered language tag: `en-a` is a valid range even though it is not a complete tag. Matching is case-insensitive and respects subtag boundaries. Missing language values retain unknown under comparisons and negation.

### Deep offset cost

An actual nginx/Rust HTTP experiment used 500,000 offline-injected signed chapter-event rows, without historical original files or normal publication admission. It returned 100 events per request using three warm sequential measurements per case. Newest-first median latency grew from about 1.1 ms at offset zero to 45.6 ms near offset 500,000. Offsets still require skipping preceding matches; the API does not make that work constant-time.

With only the descending-time/ascending-ID index, oldest-first deep pages exceeded the 2-million-SQL-step budget. A matching ascending-time/ascending-ID index removed the intermediate sorting: offset 100,000 measured 7.7 ms instead of 86.7 ms; offset 400,000 completed in 25.1 ms instead of being rejected. The extra index added about 45.5 MB to this 648-MB metadata fixture and took 1.62 seconds to build. These are local measurements, with other low-rate test activity on the same machine, not production latency guarantees. Results retain each sample in discovery-offset-scale-before.json and discovery-offset-scale-after.json.

The reference schema now creates both order indexes. This does not choose the default order pending Q93. Expensive queries remain bounded and return an explicit work-budget error, without partial results. Narrow timestamp ranges can reduce the scan: the 101-second bounded case measured about 0.86 ms before the index addition. Exact totals remain pending Q94. Building a missing index at startup adds one-time time and disk requirements; back up before schema upgrades.

## PostgreSQL port checkpoint, 6 October 2026

The server/ implementation now exposes the same experimental query parameters through an operator-authenticated GET /query. Later approved-peer admission extends that authorization policy. Every signed URL includes its exact encoded query string. Both timestamp directions and ID-ascending ties are implemented; descending and no exact total remain provisional under unanswered Q93/Q94.

The PostgreSQL compiler binds typed parameters, compares binary IDs without losing exact case semantics, uses C-collated text and ASCII basic-language comparisons, and preserves SQL NULL under negation. Each request uses one repeatable-read snapshot internally while successive offset requests remain live. A first bounded query retrieves IDs and serialized sizes; only an in-budget page loads event bodies. This avoids buffering up to1000 maximum-size events before discovering an oversized result. Each SQL statement has a250-ms deadline, work_mem4MiB, with four concurrent discovery slots and a2-MiB response limit. These are local implementation budgets, not newly settled protocol constants. Failures return an explicit error, never an unexplained partial page.

The new test evidence is postgres-http-discovery.json and postgres-discovery-tests.log. Most pagination cases use explicitly injected signed metadata rather than original-image admission. A separate real-publication HTTP fixture queries the chapter it actually admitted. No old SQLite timing numbers above are PostgreSQL performance claims.
