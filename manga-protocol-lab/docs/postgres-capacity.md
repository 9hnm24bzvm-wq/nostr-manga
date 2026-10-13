# PostgreSQL byte credit accounting

The new capacity module allocates durable byte credits to workers in batches. Upload reservations and settlements update their grant rows; they do not update one instance-wide allocation counter for every object. A single idempotent reservation can debit separate volume and owner-staging budgets atomically.

Each grant tracks capacity, available credits, reserved in-flight bytes and used bytes. Database constraints require nonnegative balances and exact conservation. Allocation/resize triggers keep the budget's allocated total equal to grant capacity. Worker identities and budget assignments cannot be retargeted silently.

A sealed grant accepts no new reservations. Only unused credits are immediately returned; uncertain in-flight reservations remain charged and can settle later. There is no automatic expiry-based release of possibly written data. Confirmed cleanup can release a volume charge; confirmed retained hosting can release the appropriate staging charge. These caller obligations still need to be wired to real blob generations, durable receipts, publication transactions and fenced cleanup.

Real PostgreSQL tests cover paired-reservation rollback and idempotency, settlement and per-budget refunds, sealing with pending work, 32 competing allocations and reservations, and reconciliation against the source reservation/charge records. MVCC row versions verify that ordinary object reservation/settlement/refund operations do not rewrite the shared budget allocation rows.

This is an accounting component, not yet end-to-end upload quota enforcement. Fair grant sizing/rebalancing, ledger retention/compaction, actual backend receipts, filesystem headroom, orphan reconciliation and realistic throughput remain implementation and validation work. Payload-byte accounting must not be represented as a guarantee about filesystem blocks or object-store billing overhead.
