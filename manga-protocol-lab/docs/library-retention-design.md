# Library object retention: integration design

Draft implementation plan. No storage/GC claims in this document are implemented merely by the codec or the account-head migration.

## Required boundaries

1. Uploaded bytes have a verified hash and a generation-specific durable receipt before the metadata store advertises availability.
2. A server-computed chunk summary belongs to that exact hash and storage generation. It proves canonical structure, family, count and bucket membership, not continued durability by itself.
3. A root transaction rechecks authority epoch, expected head, all object generations/health, summary matches and admission budgets. Exact retries are recognized before expected-head mismatch.
4. New root hosting and projection intent commit together. Old pointer disappearance in PostgreSQL alone never releases backend-dependent pins.
5. Physical removal and quota refund require durable deletion intent, a current generation fence and confirmed cleanup. A timer or expired lease alone is insufficient.

## Avoid work proportional to all historical libraries

Persist per-author/object retention for the current head and predecessors waiting on backend acknowledgement. Update only changed object memberships; unchanged chunks do not require repeated counter writes. Empty bucket slots reference no physical object.

One possible representation is an author/object row containing object generation, whether it belongs to the current head, the predecessor event whose backend retirement it awaits, and a read-grace deadline. Reusing a retired object restores current membership without double-counting it. An aggregate per-object index tracks current memberships, backend-dependent memberships and the maximum unexpired grace deadline. Admission and retirement updates must be atomic and idempotent.

The aggregate is a derived correctness-critical index: repair/audit must independently recompute it. Public object reads use a bounded primary-key health/retention lookup, not a search through historical root arrays. Moka may cache immutable bodies but cannot bypass current visibility or health.

## Lock ordering and contention

Use the same lock order in every writer: author authority/head, sorted object-health rows, then sorted changed-reference counters. Unchanged shared objects require compatible health locks; only reference deltas need exclusive counter updates. The garbage collector acquires health before counters and must not invert that order.

Do not hold a database transaction across filesystem work, decoding or strfry. Persist intent, do bounded work, then verify the generation/authority fence when committing the receipt. No single global per-progress counter or empty-chunk row should serialize all readers.

## Reads, corruption and reclamation

A reader may retain an open descriptor beyond a metadata lookup. Local read handles and deletion must coordinate so an unlink is not incorrectly reported as reclaimed physical capacity while old descriptors remain open. External static-serving descriptor caches need an explicit deployment policy or a conservative accounting boundary.

Known-bad object health must immediately prevent new claims of healthy complete content. Repair requires verified replacement bytes and staging allowance; it cannot silently refund the old generation or reinterpret corruption as an abandoned upload. A rotating, bounded scrub makes eventual progress without promising instant detection or an unmeasured 100TB audit interval.

## Tests before enabling kind11999

- Missing or wrong-generation object rejects the root atomically.
- Revocation during preparation rejects final admission.
- Concurrent same-author updates select the documented winner; expected-head conflict preserves prior state.
- Crash before/after root transaction leaves either accounted staging or a recoverable accepted head.
- Crash or timeout during strfry import/deletion retains every object the backend can still reference.
- Shared chunks survive one author's replacement/deletion.
- Returning to a formerly retired chunk does not double-count or under-count.
- Current, backend-pending, read-grace and active-reader holds each independently prevent premature reclamation.
- Repeated cleanup acknowledgement refunds once, only for the intended generation.
- Bounded scan cursors survive restart and eventually cover all eligible objects.

## Compressing pending predecessor holds

An author/object row can use states current, pending-backend and grace, with one pending predecessor ID. On removal from the current root it waits for that predecessor's explicit deletion acknowledgement. On reuse by a later root it becomes current again; a later removal replaces the dependency with the newer predecessor.

This compression depends on the existing single-owner, ordered projection workflow: imports/deletions for one author address cannot jump past earlier deletion barriers. A later predecessor's confirmed retirement then also follows retirement of older predecessors. Exact outbox version checks and subprocess locking remain necessary; a stale import side effect after process death must be resolved before later retirement is acknowledged. Do not apply this algorithm to an unordered/multi-writer backend.

The separate seeded experiment in bench/library-ac/retention_model.py exercises publication churn, delayed projection, import side effects followed by crashes, replay and grace expiry. It checks that every current database root and every modeled backend-visible root still has object holds. Twenty seeded runs totaling200,000 scheduled actions passed, including final draining and grace expiry (results/library-retention-model.json). It is a design model, not a real filesystem/strfry crash test or a proof of the future counter implementation.

## Existing accounting integration constraint

Before migration0015, the blob row contained both storage health and a retained flag used to release staging charges at first publication. Simply taking shared health locks and later upgrading selected rows to update this flag can deadlock when different transactions retain overlapping objects. Avoid that lock-upgrade pattern.

Migration0015 now separates the one-time staging-release state into a generation-bound accounting relation, and the existing publication path uses shared blob locks plus conditional accounting locks. Real publication contention/conservation tests passed; library root admission must reuse this helper and add its own tests. See storage-retention-credits.md. Refunding staging before the root transaction is committed would be an accounting bug. Physical charges remain until confirmed cleanup.

## Indexed holds instead of a hot global counter

The next root-admission implementation will use per-author/object holds with separate partial indexes for current/backend-pending rows and time-ordered grace rows. Public visibility can use two EXISTS probes; it need not increment one shared-object counter for every account. The grace predicate uses a stable statement timestamp so an expired historical tail is excluded by an index range.

A separate PostgreSQL query-plan fixture seeded500,000 expired holds for one popular hash. The absent/current/pending/live-grace/expired-grace cases all returned the correct visibility without a sequential scan, touching3–6 execution buffers in that warm metadata fixture. See results/library-reference-index.json and server/tests/library_ref_index.rs. This is RAM-backed query-plan evidence, not end-to-end library throughput, a500k-chapter benchmark, cold storage behavior or concurrent-write proof. Root admission, backend retirement, cleanup and correctness under concurrency still need integrated tests.
