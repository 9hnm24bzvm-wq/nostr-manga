RECONSTRUCTED-UNVERIFIED

Recovery of the transaction-owned native author/group mutation extraction,
public normalized input constructors, and public-creator leadership change.
Reconstructed from retained source/tool-patch context after executor replacement.
This is actual complete source, not a commit-hash-only recovery note.

Baseline: recovered-onboarding-reader-repository at
82290c732cc19b393b128fec0879885937f41054.

Historical lost source references: 3a18dcb, b32b922, 14eb2e1.
No claim is made that these reconstructed bytes match those lost Git objects.
The prior runtime/test results do not validate this reconstructed source.
Fresh compilation and tests are required before any acceptance claim.

Copy the weebdex-platform/ tree onto the matching recovered repository.
Only native author/group controller sections and catalog module registration
change in the pre-existing source files. Manga mutation behavior is retained
from the anchored baseline. Core methods borrow the caller's Transaction and
never start, nest or commit a transaction. Only genuine SessionPrincipal and
an explicitly sealed, privately constructed/rechecked account-write capability
should implement CatalogMutationActor; do not implement it for a read capability.

Public URL host and create publishDelay regex rules belong in the MDex adapter.
The native name limit remains 200 characters and existing database constraints
are not weakened. No source authority or signing key is fabricated.

Included tests: four validation/permission unit cases and one isolated database
case authenticating a real stored session, assigning actual account leadership,
verifying pre-commit invisibility and complete caller-owned rollback. The complete native
real-loopback HTTP regression is included at backend/tests/catalog_entity_mutations_http.rs.
It covers native create whitelists, CRUD, metadata, staff/leader authority,
concurrent optimistic locking, hidden source visibility, membership rollback,
referenced-author deletion guard, soft deletion and exact audit counts.

Fresh verification required after dependency/toolchain restoration:
- cargo test --locked --offline --manifest-path weebdex-platform/backend/Cargo.toml -j2 --lib entity_mutations -- --nocapture
- bash weebdex-platform/scripts/run-auth-tests.sh cargo test --locked --offline --manifest-path weebdex-platform/backend/Cargo.toml -j2 --lib public_group_creator_is_real_account_and_core_never_commits -- --ignored --nocapture --test-threads=1
- bash weebdex-platform/scripts/run-auth-tests.sh cargo test --locked --offline --manifest-path weebdex-platform/backend/Cargo.toml -j2 --test catalog_entity_mutations_http -- --ignored --nocapture --test-threads=1
- bash weebdex-platform/scripts/run-auth-tests.sh cargo test --locked --offline --manifest-path weebdex-platform/backend/Cargo.toml -j2 --test catalog_http -- --ignored --nocapture --test-threads=1
- cargo test --locked --offline --manifest-path weebdex-platform/backend/Cargo.toml -j2 --lib -- --test-threads=1

Use the coordinated shared Cargo target and CARGO_INCREMENTAL=0; retain sole
fixture ownership through verified PostgreSQL cleanup. No tests were run during
this source-only recovery. Existing old success reports are historical only.
