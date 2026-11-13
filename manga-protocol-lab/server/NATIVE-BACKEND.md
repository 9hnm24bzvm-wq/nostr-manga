# Portable native projection

The default backend is a Rust/PostgreSQL verified event projection. MangaPub's
existing Rust relay reads guarded PostgreSQL views, not the private projection;
no strfry or nostrfy subprocess is required in native mode.

Example configuration fragment for a **new installation**:

```json
{"backend":{"engine":"native","data_dir":"./projection","timeout_seconds":10}}
```

PostgreSQL remains required. The native adapter uses the exact detached database
session holding the projection advisory lock. It preserves signature checks,
newest-timestamp/lowest-ID replacement, explicit guarded deletion, bounded
readback, and generation-fenced outbox acknowledgement. Raw kind-5 events never
enter this projection.

## Existing installations

There is no automatic in-place strfry-to-native migration. Native mode has a
separate `.native-projection-id`; the database remains bound to its existing
projection identity. Mismatches stop startup. Do not delete identity files,
change database identity rows or mark jobs complete to bypass this protection.
Completed legacy jobs do not automatically replay into the new projection.
A migration needs a reviewed export/rebuild and lifecycle validation, or start a
separate fresh installation without reusing the legacy database.

Existing Linux strfry deployments can retain their adapter explicitly:

```json
{"backend":{"engine":"strfry","binary":"/absolute/path/to/strfry","data_dir":"/existing/projection"}}
```

The old process-group, subprocess-lock and timeout safeguards remain Linux-only.
Supplying a legacy binary without an engine now selects the native default, so
review existing configurations before upgrading.

## Validation status

This is reconstructed source after loss of an unpublished workspace. No compiler
or PostgreSQL is available in the reconstruction environment; these reconstructed
changes are not yet compiled or runtime-validated. Earlier test results for lost
code do not validate this reconstruction. The ignored unit integration test
`backend::native_tests::native_projection_semantics` needs a fresh isolated
`manga_test` database through `MANGA_TEST_DATABASE_URL`. It checks identity
rejection, import/replay, replacement ordering, kind-5 rejection, deletion and
session-loss fencing. Full outbox crash/concurrent-withdrawal tests remain needed.
Test sources must be excluded from the user's test-free protocol-only release.
