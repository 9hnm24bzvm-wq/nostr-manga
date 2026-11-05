# Offline PostgreSQL node backup and restore

The operator helper `src.postgres_backup` supports the current co-located storage/projection node. It never starts a server. Use trusted backups: checksums detect corruption, not malicious provenance, and restoring a PostgreSQL dump executes its SQL. Backups contain private service/group keys and must remain private; filesystem permissions are not encryption.

## Backup prerequisites

Stop the source node and all cooperating storage/projection writers. The helper must acquire the volume owner, projection owner and subprocess command locks. It refuses a live owner before creating output. Configured paths must be explicit absolute non-symlink paths. One local storage owner/volume is supported; distributed-role backup is not claimed.

Provide an explicit host/database/user connection string in MANGA_DATABASE_URL and the directory containing installed PostgreSQL tools. Connection options are parsed by installed libpq and passed to child tools via private environment variables, not credential-bearing process arguments or backup files. Service indirection is outside this profile.

```sh
PYTHONPATH=vendor/pydeps:. python3 -m src.postgres_backup --pg-bin /installed/postgresql/bin \
  backup /absolute/node.json /private/new-backup-directory
```

The helper holds a migration advisory lock and shared locks on application tables, exports a repeatable-read snapshot and runs pg_dump against that snapshot while filesystem owners remain stopped. It copies independent storage/backend/key files, validates healthy receipt hashes and sizes, preserves quarantined/uncertain states, checks ledger/queue conservation and writes a checksummed completion marker. File copying and manifests are streamed; this is a full offline copy, not incremental/online backup or a100TB timing claim.

## Restore

Create a new empty destination database and point MANGA_DATABASE_URL at it. Keep the original identity offline. A new filesystem destination is required; existing files/databases are never overwritten.

```sh
PYTHONPATH=vendor/pydeps:. python3 -m src.postgres_backup --pg-bin /installed/postgresql/bin \
  restore /private/backup /private/new-node --source-offline
```

Every listed file is copied independently and verified. pg_restore runs transactionally; nonempty targets and altered backups are rejected. The installed strfry binary must match the backup's binary hash. Optional bind/base-url arguments permit an explicit endpoint change while preserving the service key. The restored config rewrites local paths and disables new collection, periodic audit, sync workers and managed public image serving until operator review. It does not activate a listener.

After restoration, queued/leased jobs receive a new generation and lose old claim tokens. Derived reconciliation snapshots are cleared. Schema25 advances a restore generation and rejects pre-restore HTTP/blob authorization timestamps, including unused tokens absent from the backup replay table. Fresh authentication resumes after the short timestamp barrier. Signed content timestamps are not rewritten.

Review peers, account/group authorization and all changes since the backup's recovery point before activating the same identity. A backup cannot contain decisions made after it was taken. Do not run the source and restored identity simultaneously. Keep external import plans/journals in a separate operator backup scope.

## Evidence

`results/postgres-backup-restore-regression-5.log` and `results/postgres-backup-restore.json` cover a synthetic actual node: live-owner refusal, snapshot/copy, nonempty-database refusal, tamper refusal before database restoration, identity/original/library preservation, moderation tombstones, revoked group membership, private group keys, old worker/token rejection, fresh operation and independent restored inodes. `results/postgres-backup-unit.log` covers path/symlink safety and credential-free command arguments.

This is process/database recovery evidence, not host power-loss certification, proof of remote storage durability or a substitute for tested off-site/encrypted backup operations.

## Public image serving state

The managed public namespace is derived from retained private originals. Backup excludes the storage root's `public` subtree, including hard-linked image aliases and `.serving` process/control state. It therefore does not double-copy those originals or carry old process identities to a different host. Original object receipts, files and the database remain verified normally.

Restore rejects an older backup that contains files under `storage/public`; make a fresh backup with the current tool instead. After verified independent file copies and successful database restore, the target transaction clears copied external-reader holds. Those old readers cannot reference the target's newly copied inodes. Source holds and source files are untouched. The helper still never starts a server, pauses recurring/background work, and requires explicit confirmation that the source stays offline before activating the identity.

This is the offline co-located profile. It requires storage/projection writer ownership, and does not claim to quiesce or control an independently running source nginx family. Source read-only file descriptors do not become target file descriptors. Operator confirmation of source shutdown remains necessary; managed node startup/read-policy gating is implemented separately, but restore does not activate it.

Six Python safety checks and the actual PostgreSQL/node backup/restore fixture pass. The fixture includes active/creating/draining source holds, a derived public hard link and synthetic old process state; the backup/target omit that namespace, target holds are empty, source holds remain, and copied original inodes are independent. See results/public-export-backup-unit.log and public-export-backup-regression.log. No100TB backup duration, online snapshot or power-loss result is claimed.

## Audit follow-through, 2026-10-07

The restore configuration rewrite now explicitly sets `public_serving.enabled=false` when that role exists, preserving helper/nginx/bind settings for deliberate recovery and re-enable. Legacy configurations do not gain an invented role. The source configuration remains unchanged. Eight Python safety tests and the actual PostgreSQL/node migration/backup/restore fixture pass; see `results/audit-restore-policy-unit.log` and `results/audit-restore-policy-regression.log`. The fixture backs up an enabled source-role configuration while offline and confirms the restored node starts with that role disabled. This is not a claim that every broader ticket-29 recovery-state-machine requirement is finished.

The current restore helper also explicitly disables peer_discovery_enabled. Re-enabling recurring sync alone therefore cannot unexpectedly resume endpoint relocation before peer/destination policy has been reviewed. Original source configuration is unchanged. The actual reader supervisor continues to reject foreign boot/PID-namespace state; this remains an offline-review boundary, not an automatic cross-host process recovery claim.


## Schema52 restored-enrollment drill

The actual pg_dump/restore/node fixture now verifies an enabled source enrollment policy is disabled after restored startup, while preserving its hourly/lifetime limits and signup counters. A previously admitted account remains admitted; a revoked account remains blocked even when checked against an enabled policy. Source configuration is unchanged. Focused fixture,unit tests,Clippy and process/database restart pass in results/restore-enrollment-policy.log with exit0. This extends evidence without changing application behavior.

## Integrated full publisher

For WeebDex's full publisher, use the [private full-publisher wrapper and explicit restore review fence](../../weebdex-platform/docs/offline-publisher-backup.md). The original-only helper does not include native platform originals/derivatives or platform auth invalidation. Its existing behavior stays unchanged unless the integrated wrapper explicitly opts into the extra snapshot/copy/restore hooks.
