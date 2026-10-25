# nostr-manga

Rust/PostgreSQL manga protocol node with an integrated WeebDex web frontend. Start here to build and run it; the linked operator documents cover deployment and recovery in more detail.

## Choose the source and operating mode

These instructions describe review candidate `5841776663880f275dae0d4ac0e666c5e3acdc62`, not the older source-preservation baseline on `main`. The reviewed changes are **not merged into main**. To reproduce this candidate:

```sh
git clone --branch review/upload-observer-finalization-20261025 \
  https://github.com/9hnm24bzvm-wq/nostr-manga.git
cd nostr-manga
git checkout --detach 5841776663880f275dae0d4ac0e666c5e3acdc62
export REPO="$PWD"
```

Use a path without spaces for the native dependency builds. Create your own branch before editing. Keep these directories adjacent:

- `manga-protocol-lab/`: original protocol server, migrations, browser library and tools.
- `weebdex-platform/`: integrated backend, preserved frontend, additive UI integration, deployment templates and tests. Its Cargo manifest depends on `../../manga-protocol-lab/server`.
- `checkpoint/`: historical source/artifact recovery records. `platform-planning/` contains historical plans, not live issue state.

Choose one mode:

- **Local native-only development:** the web app and native platform database, without a signer, strfry or original protocol schema. Useful for UI/native development; protocol-dependent operations fail closed. It is not the full publishing system.
- **Integrated publisher:** both schemas, original storage, strfry projection, an admitted publisher and an embedded original-node listener. Required for complete protocol-backed publishing.
- **Production protocol reader:** explicitly limited `protocol_read_only` configuration, both schemas, and a separately operated self-hosted original-serving node. It is not a publisher or a maintenance-free standalone static site.

## Prerequisites and pinned dependencies

The documented native workflow targets **Linux**. Dependency restoration was exercised on Debian 13 x86_64; hosted browser work used Ubuntu 22.04. Native Windows/macOS and other architectures are not qualified by these instructions. Linux/WSL or another platform still needs its own validation. Run as an ordinary user, not root.

- Git, Bash, GNU coreutils (`timeout`), util-linux (`flock`), Python 3.12+ with pip, C/C++ compiler, make, Perl, tar, curl, pkg-config, zlib/OpenSSL/PCRE2/zstd development headers; bison, flex and m4 for PostgreSQL. The PostgreSQL helper's missing-tool fallback is Debian 13 x86_64 only.
- Rust **1.90.0**; both Rust manifests require 1.90. Use the tracked Cargo lockfiles.
- Node **24.19.0** (the hosted pin), Yarn Classic **1.22.22**. The release script checks only Node 22.12+, which is a minimum, not an exact reproducibility pin. Have Corepack available, or use the pinned Yarn alternative below.
- PostgreSQL **18.6** is the current local bootstrap pin. An existing PostgreSQL installation requires compatible server/client tools and independent qualification; historical PostgreSQL 17 tests are not an all-version guarantee.
- [bootstrap.sh](manga-protocol-lab/scripts/bootstrap.sh) pins strfry `4cd3cf64850caf47dda46c2a2abbbf3525a64d10` and recursive submodules, secp256k1 0.6.0, LMDB 0.9.33, FlatBuffers 25.2.10, CMake 3.31.6, and nginx **1.30.6**. Its current pins supersede older dated build notes. Python direct dependencies are in [requirements.txt](manga-protocol-lab/requirements.txt).
- The integrated UI additionally requires exact `@noble/curves`/`@noble/hashes` **2.0.1** and `@scure/base` **2.0.0**, restored separately below. Frontend Yarn installation alone is insufficient.

Dependency downloads need access to their official upstreams/registries. Toolchains, caches, binaries, runtime databases and secrets are not included in Git. These are locked build procedures, not a clean-machine or bitwise-reproducibility guarantee.

## 1. Install dependencies and build

Commands use the same shell and `REPO` from above. The bootstrap downloads/builds project-local dependencies, without sudo or installing a system service. Read [build details](manga-protocol-lab/docs/build.md) for manual alternatives and historical limitations.

```sh
cd "$REPO/manga-protocol-lab"
bash scripts/bootstrap.sh --dependencies-only
# Optional if you already have a qualified PostgreSQL installation:
bash scripts/bootstrap-postgres.sh

export CARGO_HOME="$PWD/vendor/cargo"
export RUSTUP_HOME="$PWD/vendor/rustup"
export RUSTUP_TOOLCHAIN=1.90.0
export PATH="$CARGO_HOME/bin:$PATH"
# Absolute and shared by the two builds; keep it set for build-release.sh.
export CARGO_TARGET_DIR="$REPO/.build/cargo-target"

cd "$REPO/weebdex-platform/frontend"
COREPACK_ENABLE_AUTO_PIN=0 corepack yarn@1.22.22 install \
  --frozen-lockfile --ignore-scripts --cache-folder ../.cache/yarn
# If Corepack is unavailable, use this INSTEAD of the preceding command:
# npm exec --yes --package=yarn@1.22.22 -- yarn install \
#   --frozen-lockfile --ignore-scripts --cache-folder ../.cache/yarn
node node_modules/@sveltejs/kit/svelte-kit.js sync

cd "$REPO/weebdex-platform"
export WEEBDEX_FRONTEND_ROOT="$REPO/weebdex-platform/frontend"
mkdir -p .build test-evidence/frontend/noble-restore
python3 scripts/restore-local-library-noble.py

# 0 = native/default UI; 1 = public UUID /api/v1 and signed-upload UI.
export PUBLIC_MANGADEX_CATALOGUE=0
unset WEEBDEX_QA_UPLOAD_REFUSALS WEEBDEX_QA_BUILD_RESOURCE_PROFILE
bash scripts/build-release.sh

# Also build the original migration/standalone-node CLI for integrated mode:
cargo build --locked --release \
  --manifest-path "$REPO/manga-protocol-lab/server/Cargo.toml" --bin manga-node
```

The signer dependency helper also restores `@scure/base`; run it once on a fresh checkout. It refuses existing/uncertain stages. After an interruption, inspect its provenance and partial stage rather than deleting it or blindly rerunning. The release script does not install dependencies.

Outputs are `weebdex-platform/artifacts/backend/weebdex-platform`, `artifacts/frontend/`, and `artifacts/release-manifest.json`. The original CLI is `$CARGO_TARGET_DIR/release/manga-node`. Serve the generated UI through the Rust application, not `file://`, raw `yarn dev`, or the supplied frontend's upstream-oriented `.env`/build commands. To iterate locally, rebuild through the integration wrapper and restart the backend; no integrated hot-reload workflow is claimed.

All `PUBLIC_*` values are compiled into the UI. The release wrapper pins API/auth/media to same-origin `/api`, `/auth`, `/covers`, `/data`, `/uploads`, `/proxy`, `/og-image`; `PUBLIC_MANGADEX_CATALOGUE=1` selects the additional `/api/v1` integration. Change the flag **before building**, not just at runtime. It does not grant Nostr authority or enable registration. Never enable private QA refusal hooks, fixture diagnostics or fixture provisioners in a deployed service. See [frontend preservation](weebdex-platform/docs/frontend-preservation.md).

## 2. Local native-only development

This optional example creates a **new disposable**, Unix-socket-only PostgreSQL cluster outside the checkout. It uses peer authentication and your development database owner for migration and serving; this is deliberately **not** the production role model. Do not reuse it for personal/production data. Use a short state path without spaces (it is used in socket/connection options). If using an existing database, skip cluster creation, set `PGBIN` to its matching tools instead of the vendor path, and supply its approved private connection.

```sh
export PGBIN="$REPO/manga-protocol-lab/vendor/pgsql/bin"
export DEV_STATE="$HOME/.local/share/nostr-manga-dev"
umask 077
mkdir -p "$DEV_STATE"
# First initialization only; do not run initdb against an existing cluster.
"$PGBIN/initdb" -D "$DEV_STATE/postgres" --auth-local=peer --auth-host=scram-sha-256 \
  --no-locale --encoding=UTF8
"$PGBIN/pg_ctl" -D "$DEV_STATE/postgres" \
  -l "$DEV_STATE/postgres.log" -o "-h '' -k $DEV_STATE -p 55432" -w start
"$PGBIN/createdb" -h "$DEV_STATE" -p 55432 -U "$(id -un)" weebdex_dev
export PLATFORM_DATABASE_URL="host=$DEV_STATE port=55432 dbname=weebdex_dev user=$(id -un)"
export PLATFORM_CONFIG="$DEV_STATE/platform.json"
python3 - <<'PY'
import json, os
from pathlib import Path
state = Path(os.environ['DEV_STATE'])
config = {
    'production': False, 'protocol_read_only': True,
    'bind': '127.0.0.1:8080', 'public_origin': 'http://127.0.0.1:8080',
    'ui_dir': str(Path(os.environ['REPO']) / 'weebdex-platform/artifacts/frontend'),
    'uploads_dir': str(state / 'uploads'),
    'database_plaintext_local': True, 'registration_enabled': False
}
Path(os.environ['PLATFORM_CONFIG']).write_text(json.dumps(config, indent=2) + '\n')
PY
cd "$REPO/weebdex-platform"
bash scripts/run-platform.sh migrate
bash scripts/run-platform.sh check-config
bash scripts/run-platform.sh verify
bash scripts/run-platform.sh serve
```

Open **http://127.0.0.1:8080**, matching `public_origin` exactly. The launcher changes directory to `weebdex-platform`, so use absolute config/storage paths. Registration is intentionally disabled and no sample accounts/content are seeded. Without original schema/publication, protocol features remain unavailable, even if their UI is built. Local TCP plaintext must use a literal loopback address such as `127.0.0.1`, not `localhost`; Unix sockets as above are also accepted.

`Ctrl-C` gracefully stops the app. Restart PostgreSQL with the same `pg_ctl ... start` command if stopped, re-export the paths/connection, and run `check-config`, `verify`, `serve`; **do not rerun initdb/createdb**. Stop this development cluster only after the app exits:

```sh
"$PGBIN/pg_ctl" -D "$DEV_STATE/postgres" -m fast -w stop
```

## 3. Integrated deployment and production preparation

Start from [platform.publisher.example.json](weebdex-platform/deployment/platform.publisher.example.json), or the explicitly limited [read-only example](weebdex-platform/deployment/platform.read-only.example.json). They are templates, not runnable credentials. Privately provision:

1. A database containing both original and platform schemas, with a dedicated migration/preparation owner and a separate restricted runtime role. Production requires certificate-verified TLS and `database_plaintext_local: false`.
2. Absolute, separate paths for immutable UI, native uploads, original content, strfry projection and secrets. The publisher template expects `/opt/weebdex/bin/strfry`; deliberately install the pinned binary there or change that path. Keep raw strfry private. Keys must be private files (0600 or stricter), never inside UI/storage archives.
3. Existing valid operator/service key files, the already admitted publisher public key, storage owner/holder and any required group bindings. Replace every `.invalid` domain and `REPLACE_*` identity. Startup does not create keys, admit a publisher, grant group roles or create a native site administrator.
4. Exact HTTPS public origins, DNS/certificates and a real mail provider, including when registration is disabled. Configure `auth_mail` and inject its named bearer-token environment variable privately. The [mail contract](weebdex-platform/backend/src/domains/auth/README.md) is a JSON HTTP provider, not an SMTP URL; no separate Ory/Kratos server is needed.
5. For the public/signed UI, set `PUBLIC_MANGADEX_CATALOGUE=1` and rerun only `bash scripts/build-release.sh` after dependency restoration. Provision original account enrollment separately if desired; `registration_enabled` only controls native site signup.

**Fresh publisher gate:** original migrations create schema, not admission. If there is no admitted original instance/publisher yet, stop before platform `serve` and provision that authority through the [original operator workflow](manga-protocol-lab/docs/publisher-authority.md) and [API index](manga-protocol-lab/docs/API-INDEX.md). The platform checks publication authority before it opens its embedded listener. A filled JSON template and an empty migrated database are not enough.

The publisher embeds an original listener on `127.0.0.1:8081`; the platform uses `127.0.0.1:8080`. Do **not** also start a standalone original process on that same address/storage. A read-only deployment instead needs its separately operated original node and same-origin proxy for originals. Use the reviewed [nginx example](weebdex-platform/deployment/nginx.example.conf) for public ports 80/443 and [systemd example](weebdex-platform/deployment/weebdex.service.example), adjusting paths and identity. Neither template installs itself. Preserve exact signed URL/method/body, upload body limits, WebSocket handling and reviewed proxy trust; do not expose the loopback application by changing its bind to `0.0.0.0`.

### Prepare once per supported schema/policy upgrade, then run unprivileged

Stop listeners/writers before preparation. Supply a matching original `NodeConfig` as `/etc/weebdex/node.json` (see [node.example.json](manga-protocol-lab/runtime/node.example.json); for embedded mode it must agree with `protocol_publication.native_node`). Database connections are **environment variables**, not JSON fields: `MANGA_DATABASE_URL` for the original CLI; `PLATFORM_DATABASE_URL` for the platform (legacy fallback: `MANGA_DATABASE_URL`). Inject credentials through your secret manager/service environment, not command arguments, source files or logs.

```sh
# Original migration role's MANGA_DATABASE_URL is privately injected:
"$CARGO_TARGET_DIR/release/manga-node" migrate /etc/weebdex/node.json

cd "$REPO/weebdex-platform"
export PLATFORM_CONFIG=/etc/weebdex/platform.json
# Platform preparation owner's PLATFORM_DATABASE_URL is privately injected:
bash scripts/run-platform.sh migrate

# Now replace that connection with the RESTRICTED runtime role's connection:
bash scripts/run-platform.sh check-config
bash scripts/run-platform.sh verify
bash scripts/run-platform.sh serve
```

**Do not execute the last three commands with the migration owner's credentials in production.** `check-config` checks config and built UI without opening the database/signer. `migrate` explicitly applies platform migrations and installs/attests visibility policy; original migrations remain separate. `verify` and `serve` only verify schema and the exact installed policy, and fail closed rather than repairing it. Switching a local database from native-only to original-backed mode also requires explicit preparation before runtime startup.

The runtime role needs only required schema usage, application DML/sequences and function execution, with SELECT-only migration histories and policy attestation. It must not own schema objects, create schemas, be superuser, or assume the preparation role. There is no automatic role/grant setup script. Follow the detailed [preparation and runtime-role requirements](weebdex-platform/docs/production-readiness.md#release-and-runtime-entrypoints), including staged upgrade/rollback restrictions. A restored checkpoint is different: **verify it as-is with its matching release; never migrate merely to make restore checks pass**.

## 4. First account and upload

For an instance intended to accept users, deliberately enable `registration_enabled`, configure working mail, restart, and visit `/auth/register`. Native signup creates an ordinary, initially unverified account, not an administrator. Use the site's verification flow and received link before protected native writes; recovery/login and email verification do not establish publishing authority.

For signed workflows, the registration setup card or Settings → Public identity lets a user select/create/import a local key or connect a NIP-07 signer, back it up, and separately review/join the instance. Admission must be enabled/authorized by the original node. Private keys stay client-side; session-only keys are forgotten on reload/lock. Site-password recovery cannot recover them. See [account onboarding](weebdex-platform/docs/account-onboarding.md) and [publisher authority](manga-protocol-lab/docs/publisher-authority.md).

With the appropriate verified native permissions or admitted signed identity/binding, create a title at `/create/title` (subject to its review policy), or open an existing title and choose Upload. Select images you have rights to publish, review metadata/page order, then explicitly submit. `/upload` exposes active/recoverable work. Native protocol-backed publication additionally needs the configured operator/storage authority; a working signup alone is insufficient. Do not use fixture account provisioners as an administrator bootstrap, bypass verification in SQL, or blindly repeat a timed-out publication: inspect its existing session/receipt first.

## 5. Checks, shutdown and troubleshooting

From a second terminal in local development:

```sh
curl --fail http://127.0.0.1:8080/livez
curl --fail http://127.0.0.1:8080/healthz
curl --fail http://127.0.0.1:8080/readyz
```

`/livez` is process liveness; `/healthz` (also `/api/health`) checks database reachability; `/readyz` reports startup/readiness and withdraws on drain. In production use the configured public HTTPS origin through the actual proxy. A 200 readiness response is not a complete end-to-end or capacity test. API/media errors must remain errors, not SPA HTML.

- **No binary/UI fallback:** build the release first; check `PLATFORM_BIN` (optional launcher override) and `ui_dir`. `check-config` requires the real integrated Svelte bootstrap, not a placeholder index.
- **Missing Noble/Scure or frontend imports:** complete both dependency restores and explicit SvelteKit sync; use the integration wrapper, not the untouched upstream build.
- **Database/TLS failure:** check the private connection, role, socket/port and trust roots. Do not turn off production TLS to silence a failure.
- **Schema/policy missing or stale:** stop and review release/database compatibility; use the preparation role only for an authorized fresh install/upgrade. Never make `verify` auto-repair or alter a restored checkpoint.
- **403/signup/email/protocol unavailable:** check exact origin, registration/mail, verification, selected signer and existing admission/permissions; UI flags are not authorization.
- **Upload 413/timeouts:** compare the exact nginx upload/replacement locations and body limits with the runtime. Preserve the session and reconcile uncertain outcomes rather than resubmitting different inputs.
- **Empty catalogue:** a fresh database is empty. This repository does not automatically scrape/download manga or supply a production dataset.

Use SIGINT/SIGTERM or your installed service manager to stop; allow readiness withdrawal and request/worker drain. The example grace is 210 seconds and systemd timeout 225 seconds; the manager timeout must remain longer. A timeout/forced exit is not proof of committed writes. Restart the same verified release/config after reconciling unresolved work. Never run multiple owners against the same original storage/projection by accident. See [shutdown ownership](weebdex-platform/docs/production-readiness.md#service-shutdown-ownership).

## Backup, verification limits and project status

Before real data: follow the private [backup/restore runbook](weebdex-platform/docs/backup-restore.md) and [full application recovery qualification](weebdex-platform/docs/full-application-restore.md). Stop **all** writers, retain the whole database plus all native/original/projection roots, exact release and separately protected keys/configuration. Restore only into an isolated empty target with workers paused. The new restore fence revokes restored sessions/API clients/tokens, advances original authentication state and disables resumable schedules/exports before success. A fence failure can leave live credentials in the isolated database: keep owners offline and the target inaccessible; do not bypass it or retry blindly. Public source archives cannot restore private runtime data.

This README update was checked against source/CLI/config contracts only; it did not run installs, builds, databases or deployment. Historical candidate023/Build19 receipts remain in [recovery records](checkpoint/RECOVERY-README.md) and [native runtime evidence](weebdex-platform/test-evidence/final/http-system-acceptance-current.md), but do not certify later source changes. The current candidate has a successful [selected-scope Chrome 157 run](https://github.com/9hnm24bzvm-wq/nostr-manga/actions/runs/40157408654); this is not a complete-suite pass. Earlier hosted attempts and failures remain in [hosted browser evidence](weebdex-platform/docs/hosted-browser-e2e.md). Full clean-machine, production, two-role database, restored-application and target-capacity qualification remain separate requirements. No production-readiness, complete feature coverage or performance guarantee is made.

No credentials, private signing keys, runtime databases, sessions, dependency caches or manga datasets belong in Git or public checkpoints. Synthetic test keys must never become production authority. Existing notices are retained; no project-wide license or third-party frontend redistribution rights are inferred. Review [frontend provenance](weebdex-platform/docs/frontend-preservation.md) before redistribution.

GitHub is the canonical source and issue tracker. Keep incremental changes on separate branches and open draft pull requests; never fabricate old history or force-rewrite `main` to publish a checkpoint.
