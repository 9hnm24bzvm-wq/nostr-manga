# Remaining source-test inventory

Read-only audit, 10 October 2026, after the non-library reconstruction checkpoint.
This inventory excludes the independently owned library/backend/admission/download
scopes; it is not a global seam-only certification.

## Confirmed caller-used seams

- `inventory::prefix_bounds`: called by the inventory operation and `sync_jobs`;
  typed prefix validation and literal bound outputs are legitimate module outcomes.
- `discovery::QueryOptions::parse`: actual HTTP request options and rejected input.
- `background_workers::BackgroundWorkers`: actual spawn/snapshot/exit lifecycle;
  snapshots are exposed in operator status.
- `blossom::{path_hash,range,scope}`: called by actual Blossom HTTP handlers.
- `offline_keys`: operator key-backup/verification file artifacts, no-overwrite,
  permissions, symlink and invalid-input rejection.
- `transfer::read_bounded`: actual reqwest response interface against a real local
  TCP peer, with observable length and transfer failure results.
- `transfer_rejection`: actual publication/library manifest typed codec rejection
  interfaces (`pointer_version`, `manifest_support`, support-feature validation).

No additional nested/bin `#[cfg(test)]` blocks were found in `server/src`.

## Endpoint policy migration

`canonical_origin`, `EndpointPolicy::new/connect` and `VerifiedEndpoint` methods are
actual application-used interfaces. The existing explicit-override/fresh-identity
fixture already uses these correctly.

The `public_address` export only has internal policy callers; direct classification
assertions are helper-level checks. The private `origin`, `check`, and
`connect_resolved` tests also cross implementation details. Replacement first passes:
`results/rebuild-seams/endpoint-public-first.log` and
`endpoint-dns-public-first.log`. Real public connection attempts now cover literal
SSRF destinations, origin rules and live identity-peer rejection behavior. The
Linux integration fixture injects mixed/empty/oversized/rebound DNS answers outside
the application process interface, preserving strict DNS validation and connection
pinning. A port-443 socket fault confines public-address transport classification
to the child process without public network traffic. The private test blocks were
archived only after those passes. `public_address` is now private, matching its
actual internal-only callers. No production injection interface was introduced.


## Test-only key-path export

`groups::recovered_key_path` had only three integration-fixture callers and
duplicated the private key-store path. Fixture-only path construction now lives in
`server/tests/support/mod.rs`, and the unused production export was removed. Group
fence and HTTP recovery checks passed after the move. The staging reclamation
fixture compiled but failed before reaching that call at its pre-existing exact
reclamation-page count (7 rather than 64); its owner is migrating that internal
batching oracle. The failed fixture and log are preserved rather than counted as
a pass. No key-store runtime behavior changed.


The staging reclamation owner subsequently replaced the exact batch/SQL oracles
and passed the real fixture (4.19s) plus targeted Clippy. Evidence:
`results/rebuild-admission/staging-reclamation-green.log` and
`staging-reclamation-clippy.log`. The earlier failure remains historical evidence,
not an unresolved test-only key-path migration blocker.

The later whole-HTTP audit found additional private outcome reads in backend,
group profile/retry, inventory, enrollment, moderation, submission review, remembered
identity, mixed-version sync and peer fixtures. Those results were reported before
edits and assigned to their owners. This source-module inventory must not be read
as an all-fixture seam-only certification. The owned peer slice is now migrated;
its exact evidence and fault-setup qualifications are in `peers-final.json`.
