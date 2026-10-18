# Real synthetic private-checkpoint mechanics, 2026-10-18

**PASS:** one fresh PostgreSQL 17.11 cluster, three named synthetic databases,
two small application-shaped tables (three rows), and three disjoint storage
roots containing 87,040 original bytes. The checked-in checkpoint script was
unchanged. This establishes backup/restore mechanics, not complete platform
schema, signing-authority, application-readiness or production recovery.

The default backup plan created no bundle; the default restore plan created no
storage roots. Real `pg_dump --format=custom --no-owner --no-acl` produced a
verified custom archive. An actual nonempty database was refused before storage
creation, leaving its sentinel unchanged. Real `pg_restore --dbname NAME
--single-transaction --exit-on-error --no-owner --no-acl` into a separate empty
database reproduced exact table values. Source tables and all three original
storage roots remained unchanged; restored membership/bytes/hashes matched.
Private checkpoint/root/file modes and release-manifest continuity were checked.

Run shape (physical fixture paths and private environment values are omitted):

```
python3 private-checkpoint.py backup --config SOURCE_CONFIG --bundle NEW_BUNDLE \
  --release-manifest SYNTHETIC_RELEASE --database-env DRILL_DATABASE_URL
python3 private-checkpoint.py backup [same bounded arguments] --execute --quiesced
python3 private-checkpoint.py verify --bundle NEW_BUNDLE [same finite budgets]
python3 private-checkpoint.py restore --config NEW_TARGET_CONFIG --bundle NEW_BUNDLE \
  --release-manifest SYNTHETIC_RELEASE --database-env DRILL_DATABASE_URL
python3 private-checkpoint.py restore [same bounded arguments] --execute --quiesced \
  --confirm-empty-database checkpoint_nonempty   # required refusal
python3 private-checkpoint.py restore [fresh empty target arguments] --execute --quiesced \
  --confirm-empty-database checkpoint_empty      # actual restore
```

Each operation used 45 seconds, 100 entries, 4 MiB checkpoint payload and a
4 GiB +128 MiB live free-space reserve. The outer driver additionally enforced
180 seconds, 128 MiB fresh allocated filesystem bytes, 512 MiB owned RSS and
1 GiB host available memory; startup required 4 GiB +256 MiB and projected
headroom for the full fixture cap. PostgreSQL used loopback-only trust for this
synthetic cluster, fsync/full_page_writes enabled, 16 MiB shared buffers and eight
connections. No application/original listener was started.

The owned driver used Linux subreaping, captured process-birth identities and
pidfd signalling via the checked-in auth fixture guard helper. Normal fast
shutdown was physically verified: closed port, no survivors, no unknown adopted
births, no escalation/errors. All local synthetic data, private configuration,
custom dump and bundle were retained outside Git. Published material contains
only commands, counts, hashes and cleanup proof; no archive/database/config/key
contents or environment values.

An initial 0.511-second attempt hit an allocation-sampler race while initdb
created/removed a transient statistics file. Its clean shutdown and failure are
retained separately. The successor sampler tolerates vanished filesystem entries;
the product checkpoint script was unchanged. The actual retry passed in 2.641s.
