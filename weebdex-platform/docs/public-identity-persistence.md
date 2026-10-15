# Public UUID persistence: reconstructed, unverified

Recovered actual implementation source after executor loss. This document does
not claim prior test results have been reproduced or old commit identity restored.

Migration89 is additive. It changes no native PK/FK, media path, signed event,
Nostr key or original-module migration. Aliases are keyed by kind+UUID, append-only,
and first preferred IDs are immutable. Protocol compatibility aliases are seeded
first and all matching original aliases copied by read-only joins. Native UUIDv8
aliases derive SHA256 over platform-public-id-v1:{catalogue_namespace}:{kind}:{id}.
Native UUIDv4 user/client IDs are reused. Namespace0073 must survive backups.

Platform insert triggers cover all12 native creation families and platform protocol
mapping rows. Repeating backfills retain every alias and canonical choice. Source
aliases never confer visibility or permission. Every route must authorize native
keys through its existing domain repository; alias enumeration is not an API.

Protocol publisher presentation principals are separately typed. Their internal
key is protocol:<64hexpubkey> and principal_type=protocol_publisher. Existing node
user aliases and deterministic original manga-compat-v1:user:{pubkey} aliases are
preserved. Only publishers referenced by actual projected catalogue rows are
backfilled. No local user, account admission, publisher admission, signer binding,
session, role or permission is created.

PostgresPublicIdResolver uses async deadpool/tokio-postgres, one statement per batch,
max1024 IDs, no database work for empty batches and no aliases created by reads.
Preferred IDs and typed targets are returned by describe_many in one batch.

The bridge source-alias preference fix verifies the exact original(kind,UUID,target)
association under a source-row read lock and checks occupied native assignments.
It adds only a verified new alias, retaining the platform compatibility mapping and
first canonical UUID. Repeating sweeps use point reads for already known aliases.
Wrong-target or occupied-UUID collisions defer only the affected source.

Tests to rerun on the recovered base:
- cargo test --manifest-path backend/Cargo.toml --lib
- scripts/run-auth-tests.sh cargo test --manifest-path backend/Cargo.toml --test public_identity -- --ignored --test-threads=1
- scripts/run-auth-tests.sh cargo test --manifest-path backend/Cargo.toml --test public_identity_original_module -- --ignored --test-threads=1

The first suite covers deterministic/scoped aliases, native creation, immutable
canonical and legacy preservation, concurrent collision rejection, all12 native
kinds, typed protocol publisher provenance, no authority escalation and unchanged
native media/FK records. The second uses all52 actual original migrations and real
compatibility APIs; signed stored-source rows are explicit fixtures, not an
end-to-end publication/media acceptance claim.

Later migration90 relation/recommendation kinds and migration91 protocol lists
belong to separate recovery owners and must be reconciled before final build.
