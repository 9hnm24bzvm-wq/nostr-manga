# Library object staging

Implementation checkpoint: repository/storage integration and authenticated HTTP staging passed the complete PostgreSQL/HTTP/strfry regression. No public library-root commit is enabled by this step.

`LibraryObjects` accepts a bounded canonical chunk or root. Chunks are decoded before storage and receive a server-computed structural summary; empty chunks are rejected because empty root slots are null. Roots are structurally checked but may still reference missing chunks while privately staged. Neither operation creates a hosted event.

An admitted account uploads the complete bytes. The storage role verifies hash and length, writes and syncs the generation, and commits its durable receipt. A short metadata transaction then checks the current account epoch, the exact available object generation, size, owner and volume before storing a private account grant. A late revocation can leave accounted orphan staging bytes; it cannot create an unauthorized grant or justify a fabricated refund.

Two accounts may upload identical bytes and share one physical generation while retaining separate grants. Knowing a hash alone does not grant access to somebody else's staged bytes. Private reads check current authority and the generation-specific grant, verify bytes on disk, and recheck authority/grant before returning. These are staging grants, not a private-library feature: the eventual accepted library is public.

The server persists chunk family/count/bucket summaries from verified codec output. Client-supplied summaries are never accepted. Summary rows reference the exact blob generation, so an old summary cannot silently validate replacement storage. Final root admission must still check actual availability/health and all relevant generations.

Limits currently follow the codec defaults: 1,000,000 bytes per chunk, 256KiB per root, 4,096 records per chunk. Upload operations share the storage-role semaphore. Owned staging tasks retain their permit across decode/write/grant completion even if a caller disconnects. HTTP must additionally acquire a bounded body-receipt slot before allocating input.

Corrupt available objects fail reads/reuploads; this step does not implement health withdrawal or repair. Such failure does not delete an intent or refund capacity. Shared public retention, read grace, backend-dependent pins and generation-fenced garbage collection remain separate gates.


## HTTP checkpoint

- POST `/library/objects/chunk` or `/library/objects/root` with canonical CBOR bytes and personal NIP-98 body authentication. Reply explicitly reports `staged:true` and `public:false`.
- GET `/library/staging/{hash}` with current personal NIP-98 authorization. A matching account grant is required; responses use `private, no-store`.

The HTTP receipt stage is bounded by the shared16 body slots before the storage operation acquires its own bounded slot. Requests cannot publish a root via the metadata endpoint. Test evidence: results/library-staging-regression.log and results/library-staging-http-regression.log, including the real filesystem/PG grant test and HTTP account fixture. The grant test deliberately revokes authority while final admission is blocked after durable writing: bytes remain charged, no grant is created. Shared-object reads, malformed/oversized rejection and corrupt-file rejection are exercised.
