# Public upload integration01 diagnosis

The first genuine public upload integration run failed at its first expected
successful commit with HTTP503, `Resource projection is unavailable`. The test
and its owned PostgreSQL guard exited101; the guard verified all saved process
births absent and the owned port refused. Earlier sequential checks executed;
later committed-session, signed-event and original-byte assertions did not.

The unchanged failed source and sanitized run/guard/error-class evidence are in
`test-evidence/public-api/upload-product-recovery/backend-fixture-id-source/`.
The raw PostgreSQL log and runtime database are not copied into this checkpoint.

## Observed constraint failure

The retained PostgreSQL diagnostic identifies
`protocol_publication_commands_catalog_id_check`. The test seeded the native
manga identifier `UploadManga`, which is11 ASCII bytes. Migration0070 constrains
publication-command `catalog_id` to10 characters. The native catalogue table has
a wider identifier allowance, so inserting the fixture row did not detect the
later publication incompatibility.

For this fresh fixture, `publish_native_upload` prepares its manga command first.
`command_publish` inserts that unchanged native catalogue ID before committing
the command transaction, looking up the compatibility ID, or calling
`publish_for_native_user`. The observed failing INSERT therefore does not itself
accept an original publication. This source ordering is narrower than treating
an arbitrary commit503 as proof that no original write occurred: later failures
can follow independently durable command or original-publication acceptance.

The correction uses shared ten-byte manga/group fixture constants in parameterized
seed/binding statements. Before fixture setup, both are checked using the actual
native short-ID validator. A pure Rust case binds that check to the unchanged
publication schema and rejects the original11-byte value, a short value,
punctuation and non-ASCII alternatives. Generated session/file/chapter IDs retain
their existing native short-ID construction. Production schemas and mappings are
unchanged by this fixture correction. Arbitrary longer legacy catalogue IDs are
not qualified for original publication by this result.

The first compile of the new negative case stopped on an invalid Rust Unicode
escape before any test ran. Its exact source/log are retained. The corrected
literal UTF8 case compiled and the exact pure identifier test passed1/1.

## Separate account-lock counterexample

The public write path also holds `FOR UPDATE OF u` on the bound native user in
its outer transaction. Independent publication-command preparation inserts a
`user_id` foreign key referencing that same row. The predicted conflict is that
the FK's key-share lock cannot coexist with the outer update lock, while the
outer transaction is waiting for command preparation. This was initially a
source-derived next-blocker hypothesis, distinct from the observed ID constraint.

An explicitly ignored real-PostgreSQL seam reuses the existing disposable fixture
and extracts the exact single private production query literal. It holds that
query's transaction while another transaction inserts then rolls back a valid
publication-command FK. Separate transactions attempt user disable, permission
and role changes, binding reassignment/deletion and a second bound-account write;
each must hit the bounded lock timeout and roll back. The owning transaction is
also rolled back before evaluating FK success, and fresh final reads check that
no seam command or authority change remains.

The initial seam retained the original `FOR UPDATE` query. Its first outer
observer failed before the test started; that result is retained separately.
The next owned run reached the test and reproduced SQLSTATE55P03 while the FK
requested `FOR KEY SHARE` on `users`. All six authority-exclusion probes and
owner rollback preceded the failing FK-success assertion; the final fresh state
assertions were not reached. PostgreSQL and the fixture's ordinary Axum listener
were both verified closed, with saved process births absent.

The successor changes only the account lock to `FOR NO KEY UPDATE`, keeping the
capability recheck, binding share lock, policy checks and transaction lifetime.
The same real-schema seam must now prove FK success and unchanged authority
exclusion; it is not a mock or a successful publication result. Its source and
fresh compile are independently reviewed. Corrected seam03 passed1/1, including
the final fresh unchanged-state assertions. Its owned PostgreSQL/Axum teardown
verified all saved births absent and both exact listener ports refused.

## Pending receipt target association

A separate source-reachable mismatch was not exercised in integration01: an
accepted-but-pending native chapter receipt carries its native `manga_id` without
a readable `relationships.manga` projection. The public DTO previously omitted
the relation, while the signed upload adapter correctly requires its exact
public target.

For explicit pending statuses only, the successor reads the actual chapter and
nondeleted published parent under transaction-held share locks and resolves the
parent's persisted preferred public UUID. Missing rows/alias or disagreement with
saved native target fields yields an unavailable optional receipt. SQL errors
still propagate. The public relation contains only the real ID, with no profile
or claim that the chapter is readable; publication status and unavailable state
remain unchanged. The two exact pure target/mismatch/absence cases and the
fixture-ID case passed under the fresh compile05 binaries (three cases total).
Direct commit plus exact-session recovery target assertions remain pending the
full HTTP integration. No actual202 behavior or full browser journey is inferred.
