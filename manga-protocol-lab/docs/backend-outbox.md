# Bounded backend outbox batches

The prototype now batches consecutive pending immutable-event imports, primarily improving recovery from an accumulated outbox. It does not introduce delayed/asynchronous publication acknowledgements or change the fact that ordinary isolated submissions immediately drain their work.

## Preserved boundaries

Each batch contains at most 64 events and 1 MiB of serialized input. Only the prototype's regular kinds 7, 1001, 1002 and 1111 batch together. Replaceable records and explicit backend deletions are individual ordering barriers. Signed kind-5 requests are rejected before any backend import command; the wrapper-owned moderation gate is unchanged.

The native importer can exit successfully while rejecting a record. After each batch, the wrapper scans the exact expected IDs and acknowledges only records whose required presence/absence was observed. Verified records are acknowledged transactionally; rejected/unverified records remain queued and produce an error. A corrupt pending record can therefore block normal startup until investigated/repaired; it is not silently dropped or treated as published. The fixture uses an injected invalid signature to exercise that failure path, not a permitted public admission route.

## Tests against actual strfry

The instrumented fixture in prototype/tests/backend_outbox.py passed:

1. 128 queued regular events replay in two 64-event batches, each with exact-ID readback. Four native commands replace the previous 256 import/readback commands for that backlog.
2. Replaceable-event imports remain separate; only the later library snapshot remains in strfry. Explicit deletion and later restoration retain their order.
3. A forced failure after native import commit leaves all jobs unacknowledged; restart replays idempotently and clears them only after verification.
4. A mixed valid/invalid batch acknowledges only its verified records; the rejected item remains queued until its synthetic corruption is repaired.
5. An injected raw kind-5 job fails before invoking the backend, leaving its target intact.

The full regression and deep suite also passed after this change, including 40 process-kill/restart cycles, the large-chapter admission case, 87-round HTTP reconciliation and the 120,000-record partition fixture. The native unit count is now 62. The prior recorded hour-long soak used an earlier binary and is not evidence for this later batch adapter.

## Remaining work

This is not a persistent streaming backend adapter, normal HTTP bulk-admission API, independent writer connection pool or fair multi-worker scheduler. The SQLite mutex still spans backend work, and an isolated request still generally needs its own import/readback pair. A backlog microbatch speedup is not a claim that all live publication throughput now scales. Production work still needs bounded backend execution time, observability, rejected-job recovery tooling and independently tested crash/power-loss behavior.

## Command deadlines

Backend imports, explicit deletes and verification scans now run under GNU coreutils timeout. backend_timeout_seconds defaults to 30 and accepts 1–300; zero cannot disable it. TERM is followed by KILL after one additional second. Foreground mode is deliberately not used so descendants share the command deadline. This adds a GNU coreutils runtime prerequisite to the Linux research build.

The fixture now has seven checks, including an import that does not read its large stdin and a verification scan that stalls. Both synthetic commands and their descendants ignored TERM. With a two-second configured deadline, escalation stopped them, the unverified work remained queued, and a healthy restart succeeded. The import path always waits for the child even after a stdin write error. These checks bound the tested backend-command failure; they do not establish a bound on an entire multi-batch recovery or image validation.
