# Public library root contract

Approach C is implemented through root admission, public reads, strfry retirement, batched transport, ordinary author deletion and receipt-backed physical reclamation. The reference client supports multi-relay delivery, durable retry and cold recovery. Schema50 adds bounded convenience views for accounts, lists and progress; see [reader compatibility](mangadex-compatibility.md) and [API index](API-INDEX.md). The comparative A/C benchmark is historical evidence, not the current authoritative API or a production capacity certification.

## Flow

1. The instance admits the account through its authenticated operator API.
2. The account uploads canonical chunks and a canonical root using the authenticated library-object staging endpoints. These bytes remain private staging until a root references them.
3. POST `/libraries` with a signed experimental kind11999 event under `event`. Its content is the root hash/version envelope described in ADR0002. Authenticate the exact request body with the same key using NIP-98.
4. Optional `expected_head` is the current event ID. Explicit null means no current head is expected; omission uses ordinary Nostr replacement ordering. Exact retries of the current event are idempotent even if their expected-head value is now outdated.
5. GET `/libraries/{pubkey}` returns the signed current event under the instance read policy (anonymous on public instances) and its projection status. GET `/library/blobs/{hash}` returns a publicly retained canonical root/chunk. These initial routes use no-store response policy because local visibility/health can change.

An accepted write returns200 if projection is already complete, otherwise202 with projected:false. A losing event or failed expected-head condition returns409. Current event retrieval remains guarded by primary metadata and storage health. Posting revocation does not itself delete public state.

## Admission transaction

Root bytes are hash-verified and decoded before the transaction. The short transaction rechecks account epoch, expected head and timestamp/ID order; locks exact available blob generations compatibly; checks local grants, sizes, quarantine and trusted per-chunk summaries; and validates family/bucket placement and totals. It releases staging credits once, installs the new event/head and writes projection intent atomically. No full read-history decode is required for unchanged chunks. A missing or unauthorized object rejects admission.

## Delta holds and backend retirement

Per-author/object rows represent current, pending-backend and grace holds. Unchanged objects are not rewritten on every progress save. Changed-out objects remain pinned to their predecessor's confirmed strfry retirement. Reusing an older object makes its hold current again, so a delayed acknowledgement cannot retire it accidentally.

The backend locks the same author before retiring a library predecessor. Confirmed deletion changes only matching pending holds into a five-minute read grace, in the same transaction as outbox acknowledgement. Separate partial indexes serve active holds and nonexpired grace; no per-object counter is incremented for every shared account. Expired grace changes public eligibility but does not delete physical bytes or refund physical storage.

The five-minute grace is a provisional local implementation policy, not a cryptographic lifetime or a promise that other relays/clients erase data. Directly exposing raw strfry would bypass the wrapper's health gates.

## Evidence

- results/library-root-tests.log: completeness, exact retry, stale ordering, expected-head races, foreign account grant rejection, reused predecessors, delayed actual-strfry projection, grace and quarantine.
- results/library-root-http-regression.log: full current regression, authenticated publish, anonymous head/object reads, duplicate retry, conflict response and author mismatch, plus prior migration/accounting/peer/restart tests.
- The500,000-record codec test, retention state model and500,000-expired-hold index probe are separate evidence with their own stated limits. None establishes deployment throughput for this integrated path.
