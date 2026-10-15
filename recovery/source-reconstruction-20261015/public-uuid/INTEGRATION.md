# Reconstructed, unverified recovery source

These files are actual source reconstructed from retained implementation context,
not commit objects. They are not claimed byte-identical to lost commits
4344f0e, 3f9ae59 or 0fcc70b. Compile and rerun all tests on the recovered base.

Integration:
1. Copy migration0089 and postgres_identity.rs into the recovered project.
2. Register migration89 after88 in backend/src/domains/mod.rs:
   Migration { version:89, name:"0089_public_resource_ids", sql:include_str!("../../migrations/0089_public_resource_ids.sql") },
3. In public_api/mod.rs declare pub mod postgres_identity and re-export
   postgres_identity::PostgresPublicIdResolver.
4. Reconcile the included identity_core.rs with foundation's latest identity.rs.
   Do not erase later ProtocolList principal support from another recovery owner.
5. Replace only the resource_id function in domains/catalog/protocol_bridge.rs
   with the separate reconstructed function file in this bundle.
6. Change the existing feed_projection test migration-tail assertion from the
   final4 migrations to .iter().filter(|m| (85..=88).contains(&m.version)); keep the
   exact pinned86 checksum assertion and global ordered/unique assertion.
7. Run real disposable Postgres tests; no previous test success is reproduced.

Source-first backup should include this bundle immediately. No credentials,
private keys, runtime databases or external publication are included here.
