# Rebuilt catalogue and saved-library migration review

Reviewed 15 October 2026. Only evidence generated after reconstruction is used.

## Verified checkpoints

- Catalogue, media and native-forum implementation: source reviewed through `07770e061ed33f0589f3dfafd9af714c215ea19c`.
- Browser saved-library migration and query composition: independently tested and source reviewed at `004354bb33de61a74647c8e909f77a372cf6a064`, including the UUID-proof change in parent `155f75455108b05954b1034f73a3b3a5b3d80dec`.
- Contract reference: official MangaDex OpenAPI 5.13.1, SHA-256 `004031ccdfc4b66727dd1f85d09b5300cf60235bbab2de6f6ea4816094bd6d33`.

Independent execution at the final frontend checkpoint passed 29 migration/IndexedDB tests and 36 typed-client/query tests. The two issue-bookkeeping regressions were also independently reproduced before repair and passed a separate 23-test review run at the earlier `6bf52943` checkpoint. The final tests additionally cover typed UUID-alias proof, immutable alias rebinding, UUID tombstone expansion, and mixed numeric/decimal/nonnumeric/empty chapter ordering. Reviewed source files match the final commit.

Fresh recorded HTTP execution at `07770e0` passed the comprehensive catalogue/media fixture in 1.26 seconds, including filters, paging, relationship expansion, nullable numbering, native original/optimized/cover bytes, range/HEAD requests, cached withdrawal, and canonical/nonpreferred/wrong-kind/hidden/unknown UUID recovery inputs. Isolated PostgreSQL cleanup was verified. A fresh signed-list/feed fixture passed in 3.42 seconds, including an empty projected scope remaining empty and final signed-head checks. The genuine original-runtime regression requires its own result.

## Review findings repaired

1. Native demographic `none` is normalized to null; its filter also includes native NULL values.
2. Native author descriptions and group MangaUpdates links are preserved in the public DTOs.
3. Replaying a migration after newer legacy state wins a canonical-ID collision no longer discards archival issue history or grows the revision.
4. Restoring an older hidden backup no longer marks a newer resolved local winner unresolved.
5. Chapter migration proofs explicitly include all content ratings and unavailable, empty and future chapter classes.
6. Named group resources preserve the standard `locked` attribute.
7. Native and original media source checks now include cached parent withdrawal while retaining the existing byte authority, filenames, and range/HEAD behavior.
8. Additive migration 94 aligns derived-group and community-forum visibility and group chapter counts with cached and live parent visibility.
9. Every saved native or UUID reference now requires an exact typed, namespace-scoped resolver proof. Only the proved preferred UUID reaches the unchanged visibility and parent checks. Immutable UUID-alias bindings and both original/preferred deletion fences are retained.
10. Composed author/parent-filter chapter views now use numeric-or-null volume/chapter sort keys and place nonnumeric, empty and null values last in both directions.

## Remaining checks at this review boundary

- The genuine original-runtime withdrawal fixture is pending. Its `7e240fb` run reached event validation and failed because the fixture cover lacked the required `series-cover-v1` marker. This is a fixture-repair blocker, not a production visibility verdict; no original-runtime pass is claimed here.
- No browser, product activation or compatible UUID-write acceptance is included in this bounded review. Later implementation changes require fresh evidence.

## Release boundary

The reviewed migration core captures the exact catalogue origin/service/namespace, a dedicated random device owner, revision and complete state fingerprint. A registered, explicitly confirmed review commits records, counters, markers and sidecars in one IndexedDB transaction. Original-row archives and immutable alias conflicts are validated; tombstones survive restores; unresolved records remain retained and exportable. Version 1 backups remain readable and version 2 backups round-trip with their sidecars and source-owner provenance.

Saved-library migration readiness remains disabled and no UI invokes the core. These finite checks do not establish compatible UUID writes, product browser activation, or complete MangaDex parity.

## Subsequent implementation-owner evidence

At frozen source `64973e5194f16068d8de32d43a8bdcef62f13fd2`, the genuine
original-runtime regression passed (1/1,0.98s) after correcting fixture metadata,
source dependency sweeps and indexing fixture-only managed group identities through
the original compatibility backfill. It uses actual signed source publications,
the original runtime and byte authority, PostgreSQL18.6 and the pinned strfry.
The signed status regression passed (1/1,1.65s), and the catalogue/media/UUID
resolver regression passed again (1/1,1.31s), now also checking independent actual
optimized dimensions. All three owned PostgreSQL cleanups were verified stopped,
exit0. Logs and source/binary pins are under
`test-evidence/public-api/reconstructed-64973e5/`.

This addendum records implementation-owner execution, not an additional independent
review or a browser/product activation claim. The earlier failed fixture logs are
retained alongside the passing evidence.
