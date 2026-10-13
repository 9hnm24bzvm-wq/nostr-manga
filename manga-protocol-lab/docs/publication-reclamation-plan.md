Implemented by schema24; see publication-reclamation.md for the verified behavior and remaining limits. The original design checklist follows.

# Remaining protected-original reclamation work

This is a plan, not implemented behavior. Library cleanup already has generation-specific claims, read pins, durable filesystem receipts and crash recovery. Protected publication originals need their own eligibility records and opt-in policy before reusing that machinery.

Required sequence:

1. An author request alone changes no protected retention. Only local approval and acknowledged backend removal retire publication references.
2. Before removing the acknowledged event's publication_blobs rows, record generation-specific grace for each local object. Shared hosted publications, current/pending libraries and group staging remain strong references.
3. Freshly restaged originals need a durable staging lease. Re-uploading a formerly retained hash must prevent a collector from racing a new publication preparation.
4. A separate opt-in publication collection flag must not broaden the meaning of the existing library-only flag without notice. Existing committed cleanup claims still recover after disabling new collection.
5. Claim only an available, locally owned matching generation after all grace/lease/strong-reference checks. Keep charges until exact file cleanup and directory synchronization succeed.
6. Finalize through sealed cleanup receipts. Preserve signed publication events and moderation tombstones; physical cleanup must not permit deleted-event replay or imply that restoration can skip re-upload/validation.
7. Verify shared originals, moderator approval boundary, restaging, partial cleanup, stale receipt replay, exact budget recovery and restart in real fixtures.

External direct-serving readers need a separate lifetime/retention integration before collection may refund their bytes. Existing internal node read pins do not establish that an external nginx worker has closed its file descriptors. Never describe this plan or the historical SQLite behavior as implemented PostgreSQL acceptance.
