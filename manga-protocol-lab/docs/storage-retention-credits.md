# One-time staging release without shared-object write contention

Migration0015 moves the one-time staging-release flag out of the blob receipt row into a generation-bound accounting relation. Existing values are copied transactionally; the old flag column is then removed. New writing intents create their matching accounting row in the same transaction as the reservation.

A publication holds compatible shared locks on verified blob generations and checks quarantine. It locks accounting rows only where staging has not already been released. After waiting for a competing publisher, PostgreSQL rechecks that condition. Reservation rows and charge-grant rows are then locked in a common sorted order, the staging charges are refunded once, and the selected accounting flags are marked released in the same publication transaction.

Already-retained shared originals receive neither an exclusive receipt lock nor an accounting update on every new publication. Physical capacity remains charged. A failed publication transaction rolls back its reference changes, credit release and backend intent together.

The full regression passed after this change. A targeted publication test holds both a shared health-generation lock and an exclusive lock on an already-released accounting row; another publication using that original still completes. Two distinct publication transactions also race to retain a newly staged shared original and leave the charge ledger conserved. This establishes the lock/atomicity behavior, not a target-throughput number.

Evidence: results/retention-credits-regression-2.log. The initial compile-error log is preserved separately; the helper uses the project's deadpool transaction wrapper consistently.
