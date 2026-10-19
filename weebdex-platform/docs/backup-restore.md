# Private, quiesced database and storage recovery

This runbook is for the operator's **private runtime data**, not the public
source/checkpoint archive. GitHub/Catbox project archives deliberately exclude
runtime databases and signing keys. They cannot restore account data, publication
intents, native originals or the original module's durable state.

`scripts/private-checkpoint.py` provides bounded planning, integrity verification
and an explicitly enabled copy procedure. Its default is read-only. The finite
tests use a synthetic command stand-in. A later [real synthetic PostgreSQL17
mechanics drill](../test-evidence/deployment/private-checkpoint-mechanics-20261018/README.md)
verified actual dump/transactional restore and three tiny storage roots. That
is not a full application or production recovery qualification. Qualify the
procedure on an isolated staging installation before relying on it for recovery.

## Prerequisites and authority

- Use Linux, Python 3.10+, and already-installed official `pg_dump`, `pg_restore`,
  `psql` and libpq client tools compatible with the database version. The script
  never installs software. libpq parses the existing private connection value
  without connecting; it supports the runtime URI or libpq keyword syntax.
- Keep the existing `PLATFORM_DATABASE_URL` in the approved runtime secret
  environment. For a legacy configured `MANGA_DATABASE_URL`, pass
  `--database-env MANGA_DATABASE_URL`. Do not paste a credential into a command,
  repository, public support log or archive. Production execution requires an
  effective `sslmode=verify-full`; retain the approved trust-root files. An
  explicit ordinary database name is required. Service-file indirection is
  unsupported: supply the already-approved expanded runtime connection value.
- Run as the storage owner with access to the **whole** intended database,
  including both original-module and `platform` schemas. A partial schema dump
  is insufficient. Ensure the database server also has enough disk and lock
  capacity; the local free-space guard cannot inspect a remote server's disks.
  The maintenance identity also needs authorized visibility into Linux process
  arguments, working directories and file descriptors for the shutdown check.
  A restricted `/proc` view refuses execution; the script never elevates itself
  or changes system permissions. Preserve service-compatible ownership when
  selecting that existing operator context.
- Retain the exact verified release manifest, matching immutable binaries,
  migration source manifests and frontend artifacts independently. The backup
  records the release-manifest hash; restore requires that same file. It does
  not infer a compatible newer or older binary.
- Provision a private parent directory outside the checkout, releases, public
  UI and storage roots. The new bundle and restored roots are mode 0700; files
  are 0600, owned by the invoking operator. Do not run as a public web user or
  change ownership/permissions of a live installation through this procedure.
  Verify any parent ACLs, remote filesystem behavior and encryption policy.
- Keep existing operator/service/group signing keys and runtime configuration
  separately in the approved private secret backup. Their configured paths must
  not overlap a checkpointed storage root. This script reads no key contents,
  copies no key/configuration files and creates no identities. Restore must
  reuse the same admitted identities and authorized volume/group bindings.
- The configured layout must have three distinct physical local roots:
  `uploads_dir`, `protocol_publication.storage.path` (or the explicit original
  node's `storage.path`), and `native_node.backend.data_dir`. Supply
  `--node-config /absolute/private/node.json` for an independently configured
  original node. A read-only client of a remote original node must obtain that
  node's separately authorized backup; this script refuses a missing local root.
  Symlinks, hardlinked files and special files are refused. Both input runtime
  configuration files must remain outside all copied roots; overlap is refused
  before a dump or output creation. Unknown secrets placed among original bytes
  cannot be recognized automatically: review those roots independently.

## 1. Establish a consistent, stopped checkpoint

Before executing a backup, have the process manager gracefully stop the platform
and original listeners, wait for accepted requests and owned workers to finish,
and verify normal process exit. Stop **every** other writer to this database and
these volumes, including relay/import/maintenance jobs, separate clients,
administrators and remote jobs. Do not force-stop an unresolved publication and
assume it completed. Retain its immutable intent and exact receipt for later
reconciliation.

`--quiesced` is the operator's explicit assertion of all-writer quiescence. The
script can additionally check numeric loopback listeners are closed and Linux
process arguments, current directories and file descriptors no longer reference
the storage roots. Those checks cannot prove the absence of remote writers or
all possible database connections. An uninspectable process or ambiguous listener
refuses execution. The PostgreSQL server stays available for the dump.

With all writers stopped, one complete database snapshot followed by exact
copies of all three unchanged roots represents the same application checkpoint.
The script hashes/scans storage before and after the copy, checks file identity
during copying, and checks shutdown again before writing the completion marker.
Keep writers stopped until completion or refusal has been inspected.

From the release's `weebdex-platform` directory, first inspect a plan:

```sh
python3 scripts/private-checkpoint.py backup \
  --config /etc/weebdex/platform.json \
  --release-manifest /opt/weebdex/current/weebdex-platform/artifacts/release-manifest.json \
  --bundle /srv/private-weebdex-backups/checkpoint-20261018
```

The command above invokes no PostgreSQL tools, performs no service action, makes
no database connection and creates no output directory. Review the three roots,
database environment name, release hash and private destination. No DSN is
printed. After independently establishing quiescence, deliberately execute:

```sh
python3 scripts/private-checkpoint.py backup \
  --config /etc/weebdex/platform.json \
  --release-manifest /opt/weebdex/current/weebdex-platform/artifacts/release-manifest.json \
  --bundle /srv/private-weebdex-backups/checkpoint-20261018 \
  --execute --quiesced
```

This calls `pg_dump` in custom format with no owner/ACL restoration statements,
copies native bytes, original bytes and projection data, and writes a private
manifest plus the final `COMPLETE.json` marker. The bundle contains the **entire
private database**, which can include sensitive account data and authentication
state. Never upload it as public project/test evidence, even if logs are clean.
Follow the operator's approved encrypted private backup/retention policy.

## 2. Verify and retain

```sh
python3 scripts/private-checkpoint.py verify \
  --bundle /srv/private-weebdex-backups/checkpoint-20261018
```

Verification is always read-only. It checks the completion marker, database and
storage hashes, all recorded membership, path containment and private directory
mode. Missing, extra, linked or altered members refuse verification. A valid hash
proves internal integrity, **not the authenticity or safety of an untrusted
backup**. Only restore a trusted, access-controlled operator backup; PostgreSQL
restoration can execute database-defined code.

Default bounds are 1,800 seconds, 1,000,000 storage entries, 1 TiB combined dump
and storage bytes, 64 MiB manifest bytes and a 4 GiB local free-space reserve.
Use finite `--timeout` (maximum 86,400), `--max-files` (maximum 10,000,000),
`--max-bytes` and `--min-free-bytes` when reviewing a particular installation.
These are refusal ceilings, not capacity or completion-time guarantees. Hashing
and copying original bytes take real time. A partial output is retained for
inspection, never automatically deleted and never accepted as complete.

## 3. Restore into an isolated, empty target

Do not point restore at production. An authorized database operator first creates
a fresh isolated database from `template0`, with an appropriate least-privilege
restore owner and compatible extensions. Database creation, roles, tablespaces,
grants, credentials, certificates, signing keys and service-manager changes are
outside this script. `--no-owner --no-acl` deliberately leaves ownership with
the restoring role; re-establish the reviewed service grants separately before
serving. Keep the target inaccessible to application writers.

Prepare a private target runtime configuration with **new, absent** native,
original and projection paths whose private parent directories already exist.
Use the exact retained release and admitted key identities. Inject the target's
existing approved database connection value into the normal runtime environment;
never override a live service's environment as a side effect of this runbook.

```sh
python3 scripts/private-checkpoint.py restore \
  --config /etc/weebdex/restore-staging.json \
  --release-manifest /opt/weebdex/restored-release/weebdex-platform/artifacts/release-manifest.json \
  --bundle /srv/private-weebdex-backups/checkpoint-20261018
```

That verifies the bundle and prints a plan only. After reviewing the isolated
database identity, absent roots and all-writer quiescence:

```sh
python3 scripts/private-checkpoint.py restore \
  --config /etc/weebdex/restore-staging.json \
  --release-manifest /opt/weebdex/restored-release/weebdex-platform/artifacts/release-manifest.json \
  --bundle /srv/private-weebdex-backups/checkpoint-20261018 \
  --execute --quiesced --confirm-empty-database weebdex_restore_staging
```

Before creating any target root, a read-only query must return that exact database
name and zero non-system relations/routines/types/custom namespaces. This is an
additional refusal check, not permission to use an occupied target. The script
copies and verifies all storage, then explicitly connects `pg_restore` to the
same database using `--single-transaction --exit-on-error`. It never uses
`--clean`, creates/drops a database or starts an application. Secrets travel only
through the child environment, never process arguments or generated secret files.

An error, timeout or interrupted restore is **not success**. New private storage
may remain. A lost connection near commit can have an uncertain database outcome;
inspect it before taking any further action. Do not blindly retry or delete a
target based on a missing CLI receipt. The script performs no automatic rollback
of filesystem copies and no deletion of partial data.

## 4. Prove recovery before serving

Use the matching release first, without automatic migrations:

```sh
PLATFORM_CONFIG=/etc/weebdex/restore-staging.json scripts/run-platform.sh check-config
PLATFORM_CONFIG=/etc/weebdex/restore-staging.json scripts/run-platform.sh verify
```

The production platform `verify` command checks both platform migration checksums
and the original schema without starting listeners or acquiring publication
ownership. The original `manga-node` CLI has `migrate` and `serve`, not a separate
`verify` command. Do not invent a restore-time original migration or use the
database dump with an incompatible migration manifest. Qualify an intentional
upgrade on the staging copy only after the matching release verifies.

With operator approval, start the isolated configured runtime through the existing
service process manager. Verify `/readyz`, original signer/storage authority,
account reads, original signed-event visibility and exact original media hashes
(including Range/conditional reads). Check native and signed upload intent
reconciliation, reader pages, private library state and outbox/restart recovery.
Gracefully stop and restart the staging runtime once, repeat those checks and
retain sanitized source/hash/test/physical-shutdown evidence. Do not publish raw
database dumps, keys, account content or private connection values in that report.
Only an operator-reviewed successful drill supports a production recovery claim.

References: official PostgreSQL [dump procedure](https://www.postgresql.org/docs/17/app-pgdump.html),
[restore modes and transaction flags](https://www.postgresql.org/docs/17/app-pgrestore.html),
[libpq connection parsing](https://www.postgresql.org/docs/17/libpq-connect.html#LIBPQ-CONNSTRING)
and [connection environment](https://www.postgresql.org/docs/17/libpq-envars.html).
