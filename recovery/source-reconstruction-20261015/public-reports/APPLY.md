# Reconstructed report overlay

Status: RECONSTRUCTED-UNVERIFIED. Do not claim this is commit 6e5f0dd, byte-identical to that commit, or a fresh test pass. No historical logs were recreated.

## Dependencies

First restore the public API foundation, UUID resolver, DTOs, query parser, migrations 89–91, and genuine Nostr HTTP authorization. `public_api::auth` must supply `admitted_request`, `bound_account_read`, and a privately constructible `BoundAccountWrite` returned by `bound_account_write`.

The write capability must recheck original-module admission, explicit identity binding, active/verified account state and prohibited roles under the exact same transaction used by the write. It needs crate-private `user_id()` and `require_permission(&str) -> crate::error::ApiResult<()>`. Acquire `FOR UPDATE OF u FOR SHARE OF b` directly for writes to avoid concurrent SHARE-to-UPDATE upgrade deadlocks. Never manufacture a SessionPrincipal or infer a binding from a public UUID, native cookie, publisher alias or submitted account ID. The read capability needs `user_id()`, `is_moderator()` and `require_permission()` as reconstructed by the foundation owner.

## Applying

Run `python3 apply_recovery.py /absolute/path/to/the/repository` from this folder after the foundation is restored. The script stages all changes in memory before writing. It adds the small enum/module/router/migration wiring without replacing unrelated shared files, and extracts the creation block from the destination's existing native moderation controller while preserving its other handlers. The full reconstructed native module is also supplied for independent comparison.

Do not overwrite historical migrations 89–91. Before applying migration 92, compare its `public_backfill_resource_ids` wrapper to the restored migration 91/foundation implementation. The pre-loss migration 91 shown in the retained implementation context did not replace that wrapper; it defined signed-list aliases and cursor/projection tables and used a bounded background importer. If the recovered foundation adds a wrapper operation, preserve it when appending the report reason backfill. No list readiness, alias or authority behavior may be dropped.

After fresh tests, update only the three report operation matrix rows (`get-report-reasons-by-category`, `get-reports`, `post-report`) with truthful implementation/evidence status. This recovery does not mark coverage verified or create success logs.

## Required fresh validation

- Format the changed files and check the diff for unrelated changes.
- Run `cargo check --tests` in the integrated backend.
- Run `cargo test --lib public_api:: -- --test-threads=1`.
- Run the isolated `weebdex-platform/scripts/run-auth-tests.sh cargo test --manifest-path weebdex-platform/backend/Cargo.toml --test public_reports -- --ignored --test-threads=1`.
- Run the same isolated runner separately for `--test community_http` to recheck native moderation/review/group-claim behavior.
- Recheck migration ordering/checksums, reason UUID backfill/insert trigger/version updates, same-account concurrent duplicate behavior, and exact private owner/staff scope.
- Preserve actual new logs and PostgreSQL cleanup evidence after the runs. No runtime database, credentials or private keys belong in the checkpoint.

The fixture uses synthetic keys and accounts, loopback TCP and the designated disposable database only. Its embedded synthetic secret derivation is test source, not a production credential.
