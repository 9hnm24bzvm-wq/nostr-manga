# Storage credit lanes

The full pre-existing regression suite and tiny-budget sequential/concurrent rebalance/conservation fixtures passed. The actual-node release accepted all2,400 writes in a20-second120/s paced run. Longer-load and peak verification remain pending; this is not a month-long capacity claim.

## Measured reason

The actual-node120/s attempt still rejected writes after network admission and backend projection changes. Diagnostic PostgreSQL samples found transaction/tuple waits at shared capacity-grant reservation, settlement and staging-refund locks. Increasing concurrency alone lengthened the queue. The original owner selected one shared grant per budget and serialized all object admission under one local mutex.

## Refinement

Each storage owner now derives16 stable credit-holder identities from its existing holder ID and the first hash byte's lane. Each lane has a local selection/reservation mutex; unrelated lanes can reserve concurrently. Ordinary object operations still use the same durable reservation, verified filesystem receipt, settlement and confirmed-refund rules.

Credit refill batches are at most4MiB and at most1/64 of the configured budget unless an individual object needs more. Only rare refills serialize against the shared budget counter. Partial indexes keep hot selection/reclaim queries away from old sealed grants. Migration0017 adds these indexes without changing existing accounting rows.

A shared process-local gate protects grant selection until reservation commits. If a lane lacks credits because idle credits are held elsewhere, a rare exclusive rebalance waits for current selections, seals owned grants, and returns only unused credits. It includes the pre-lane legacy holder for upgrades. Pending reservations and used bytes remain allocated; neither a timeout nor a rebalance authorizes a physical refund. Settlement remains legal for previously admitted in-flight reservations on a sealed grant.

This avoids treating stranded idle credits as genuine quota exhaustion. It does not bypass capacity constraints or remove atomicity between paired physical/staging reservations. Filesystem I/O, later settlement and root publication do not hold the process-local selection gate.

## Required checks

- Full existing quota, generation, corruption, publication and restart suite.
- A new tiny-staging-budget fixture visits every lane twice, retaining4096 physical bytes while repeatedly recycling only128 staging bytes; ledger sums must remain conserved and an extra object must fail at genuine physical exhaustion. An already-owned duplicate must still succeed at full quota.
- Actual-node120/s rerun, then sustained/mixed-reader/peak measurements if successful.

See library-node-performance.md for failures and current measured capacity. This change does not implement physical garbage collection or metadata-history pruning.

Credit lanes are configurable as storage.credit_lanes, power-of-two1..256, default16. Rebalance recognizes every historical lane identity, so downsizing does not strand idle credits. The64→16 test passes. A64-lane peak trial barely improved240/s performance and is not a recommended universal fix. The successful125/s baseline used16 lanes.
