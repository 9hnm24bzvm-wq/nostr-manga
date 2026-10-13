# Private offline full-publisher backup and restore

Tracks [issue #23](https://github.com/9hnm24bzvm-wq/nostr-manga/issues/23). This is a co-located offline recovery profile, not online/incremental backup, a deployment activator, or power-loss/off-site durability certification.

## Scope and safety

Use `manga-protocol-lab/src/platform_backup.py` through `python -m src.platform_backup`. The original-only `src.postgres_backup` command remains supported, but is insufficient for a full publisher.

The integrated helper includes both `public` and `platform` PostgreSQL schemas in the same exported snapshot/dump, canonical original storage, original projection data, native uploaded originals and derivatives, pending native filesystem journals, both independent operator/service signers, optional group keys, and both private configurations. It preserves the catalogue namespace, identity bindings, moderation and policy state, and immutable pending command manifest bytes/native inputs. Native `media_objects` inventory hashes/sizes and healthy original receipts must match actual copied files. Restored DB inventory, namespace and signing identity are checked again before usable configuration is published. File copying creates independent inodes. An enabled empty group-key root is explicitly restored as a private empty directory after validating its backed-up root; no group key or authority is fabricated.

Operational backups contain signing keys, password hashes, account data and private user content. Keep them private and encrypt them using separately reviewed operator tooling. Never commit them, upload them in GitHub Actions, or mix them into source/checkpoint/Catbox recovery archives. The public checkpoint permission does not authorize operational key/user-data disclosure. Permissions are not encryption. Only trusted backups may be restored: checksums detect accidental corruption, not provenance, and PostgreSQL dumps execute SQL.

DB connection strings and mail credentials stay in external environment files. Credential-bearing config fields/URLs are rejected; a mail credential environment-variable *name* may be preserved. Do not place credential files in the owned storage/projection roots. Immutable application binaries/UI artifacts are supplied by the matched installed release, not by this private runtime backup. The installed strfry binary must match the recorded hash; use `--backend-binary` for a relocated matching binary and `--ui-dir` for the installed matching frontend.

## Backup

Stop the integrated publisher, original listener, all native/original storage and projection writers, and any independent workers. The updated service holds `uploads_dir/.platform-owner`; the helper must acquire that existing named lock plus original `.volume-owner`, `.projection-owner` and `.command-lock` for the entire copy/snapshot. An older deployment must be started and cleanly stopped once with the updated service to initialize the native owner lock. Never replace or unlink lock files while an owner may be live.

The synchronous service entry point holds its native owner outside the Tokio runtime scope. On success or startup/drain error, runtime teardown joins already-started blocking filesystem jobs before the owner can drop. Cancelling a request/maintenance future alone is insufficient; do not replace this with background/timeout runtime teardown that can leave a late rename/write/remove job alive after the lock is released.

Use absolute normalized paths with no symlink ancestry. Native/original/projection roots must be disjoint. The destination must be new, outside every source data root, with an existing parent. Stop every cooperating writer; arbitrary privileged SQL/filesystem writers are outside this profile. The helper takes both migration advisory locks (697241113 and 697241114) and SHARE table locks in both schemas before exporting the snapshot and copying files. A bounded end-of-copy round-trip must confirm the same live exporter backend, transaction ID and repeatable-read isolation before completion; exporter death or transaction loss fails closed even after a successful pg_dump. A failed operation has no valid completion marker; do not activate or share partial output.

```sh
cd /opt/weebdex/manga-protocol-lab
# PLATFORM_DATABASE_URL: explicit host/database/user; supplied externally.
PYTHONPATH=vendor/pydeps:. python3 -m src.platform_backup --pg-bin /installed/postgresql/bin \
  backup /etc/weebdex/platform.json /private/new-publisher-backup
```

The embedded listener's serialized `storage:null` is expected: it receives the publisher's owned UploadStore. The helper normalizes that effective storage for the original backup primitives and rejects a conflicting non-null original storage config. Operator and service/holder signer identities are validated separately; they need not be the same key.

## Restore and explicit review fence

Provision a new empty database with restricted operator access. Custom schemas, tables, views, sequences, functions and types make it nonempty. Keep every source owner/signing identity offline. Use a new private filesystem destination; the helper never overwrites an existing database or directory and never starts a server.

```sh
cd /opt/weebdex/manga-protocol-lab
# PLATFORM_DATABASE_URL now points to the NEW empty destination database.
PYTHONPATH=vendor/pydeps:. python3 -m src.platform_backup --pg-bin /installed/postgresql/bin \
  restore /private/publisher-backup /private/new-publisher --source-offline \
  --backend-binary /opt/weebdex/bin/strfry --ui-dir /opt/weebdex/weebdex-platform/artifacts/frontend
```

After transactional `pg_restore`, one transaction advances the original restore/auth/worker generation, clears derived worker claims/reconciliation/public-serving holds, disables original sync schedules, and invalidates every copied platform browser session, API client, browser flow and email verification/recovery link. Unused pre-backup links are invalid too. Passwords, roles, membership, admissions and moderation decisions still reflect the recovery point and require review; decisions made after that backup cannot be recovered from it.

Both rewritten configurations are private. Use `restored-platform-config.json` for integrated ownership. `restored-config.json` is the normalized original config for offline inspection, not permission to start a second original listener/store owner. Shared native storage remains injected (`native_node.storage=null`), while the publisher's storage path points at restored originals.

The platform config sets `restore_review_required=true` and closes native registration. `serve` and `migrate` refuse before opening the DB, owners, listeners or startup workers. `check-config` and `verify` are read-only; neither clears this fence. Verify requires a compatible installed release and existing matching schema, so an older schema remains blocked until the operator has reviewed it.

Review the recovery point, passwords/account revocations, both signers, groups, peers, moderation, endpoint/network and mail policy, and exact pending publication/deletion inputs offline. Reconcile using the existing command IDs, manifest bytes, timestamps, native payload and pinned originals; do not allocate replacement commands. Only then explicitly set `restore_review_required=false` in the private platform config. This deliberate release permits `migrate` (if required by the matched release) and ordinary service startup/background work. Pending user publication still needs an authenticated exact retry, which uses the preserved command and must not duplicate its signed event.

Original automatic sync/discovery, collection/reclamation/audits, enrollment and managed public image serving stay disabled in rewritten original config until separately re-enabled. Derived original `storage/public` aliases/process state are excluded and must be rebuilt from verified retained originals. Supply DB/mail secrets externally. Never run source and restored signing identities at the same time.

## Verification boundaries and selected hosted drill

As of this implementation commit, 19 local Python filesystem/subprocess safety seams passed: eleven full-publisher checks, four unchanged original-helper regressions and four test-lifecycle projection checks. The latter include an actual on-disk supervisor run whose exact one-pass output, exit-zero and clean-after-SIGKILL receipt must project failed. Python compilation and whitespace checks passed. Subsequently prepared pinned Rust 1.90 passed locked all-target compilation, scoped `main.rs`/`runtime.rs` formatting checks and the exact cancelled-blocking-writer owner test (one passed, zero failed, clean terminal receipt) on runtime source `e34e2ba37242ca02e194fe8196d7a544df70e189`. Actual PostgreSQL dump/restore and HTTP recovery/auth/gate/retry checks remain authored but **unrun**, not a functional recovery acceptance pass.

The selected Rust drill is `protocol_bridge_http::offline_full_publisher_backup_restore_fences_auth_and_exact_retry`. It creates only isolated synthetic data, distinct operator/service keys and a second disposable restore DB. It covers actual exporter-backend termination after pg_dump with no completion marker, live-native/live-original refusal, wrong signer/nonempty/tampered targets, actual CLI snapshot/copy/restore, namespace/identity, staged native/original/derivative media, old sessions/API/flows/mail invalidation and fresh password login, unchanged pending CBOR/native pins, serve refusal, exact repeated retry/no duplicate source event, HTTP original/range/private preview/derivative reads, and independent native inodes. It is ignored in ordinary suites, so they do not claim to have run it.

On the reviewed standard public Ubuntu 24.04 hosted runner, reuse the existing pinned preparation from the CI branch, without running browser phases. Add only the exact CPython 3.12 x86-64 Coincurve wheel:

```sh
weebdex-platform/.build/hosted-python/bin/python -m pip install \
  --disable-pip-version-check --timeout 30 --retries 2 --only-binary=:all: \
  --require-hashes --no-deps -r weebdex-platform/scripts/requirements-backup-hosted.txt
export RUSTUP_TOOLCHAIN=1.90.0
export CARGO_TARGET_DIR="$PWD/weebdex-platform/.build/hosted-native"
export PLATFORM_TEST_PYTHON="$PWD/weebdex-platform/.build/hosted-python/bin/python"
# Separate exact unit seam, also included in ordinary --lib tests; require one passed test.
cargo test --locked --manifest-path weebdex-platform/backend/Cargo.toml --lib \
  runtime::owner_tests::native_owner_outlives_cancelled_blocking_writer -- --exact --test-threads=1
python3 weebdex-platform/scripts/run-backup-tests.py --deadline 1800
```

The selected runner calls the owned `run-auth-tests.sh` fixture and cargo with `--ignored --exact --test-threads=1`. It requires exactly one executed/passed test, a terminal lifecycle receipt, no deadline, no cleanup signals, no surviving group members and verified cleanup without touching prior/unrelated processes. Forced cleanup fails even when the supervisor exits zero and verifies the group is empty afterward; zero matching tests also fail. Only its closed `public-summary.json` may be an uploaded artifact. Raw test/supervisor logs, lifecycle metadata, synthetic private backup directories, runtime databases and keys remain private and excluded. The existing browser artifact validator must not be broadened to accept them.

The Coincurve SHA-256 pin comes from the [official 21.0.0 PyPI release file](https://pypi.org/project/coincurve/21.0.0/#files); [the maintainer's release history](https://ofek.dev/coincurve/history/) confirms version21 removed runtime dependencies. These helpers do not import cbor2.
