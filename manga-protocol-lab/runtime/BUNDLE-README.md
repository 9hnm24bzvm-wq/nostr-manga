# Runnable backend checkpoint26 (schema52)

This Linux x86_64 package contains the Rust/PostgreSQL/strfry backend and a local demonstration. It includes optional closed-default reader enrollment, public accounts/lists/progress, managed group profiles and retry-safe publication commands. The peer-allowlisted NIP42 WebSocket interface, WSS proxy checks and retryable database-failure handling are included. Website/UI and production capacity are not certified.

## Run the local demonstration

Use a compatible Linux x86_64 machine, as a normal user rather than root, with Python3 installed:

```sh
cd manga-backend-mvp
sha256sum -c SHA256SUMS
./run-demo.sh
```

For an automated run that removes only its successfully checked temporary data:

```sh
./run-demo.sh --check
```

The demo uses public synthetic signing keys and loopback addresses only. Never expose it to the Internet. It explicitly opens self-registration on the second demonstration instance to exercise signup; the first instance and the production configuration example keep it disabled. The temporary database password stays local. Interrupted/failed runs retain their temporary data/log path for inspection.

No compiler, Docker, package installation or outbound network is used by the demo. PostgreSQL18.6, strfry and application binaries are included. These binaries were tested on Debian13 x86_64 with glibc2.41; compatible OpenSSL3, zlib, zstd and libstdc++ runtime libraries are also needed. Other environments should build from source. The launcher checks missing libraries and refuses root.

## Included backend features

- Signed immutable original-backed series/chapters, metadata, covers and discovery
- Independent account/publisher/peer permissions and private one-off submission review
- Optional bounded reader self-registration, disabled by default, with durable revocation fences
- Verified original transfer, protected retention, storage repair and safe staged deletion
- Approved-peer HTTP replication, read-only NIP42 WebSocket events, durable jobs and filtered reconciliation
- Public signed libraries/lists/progress, profiles, comments, replies and reactions
- Multi-relay library delivery, retry state and cold-device recovery
- Instance-managed group keys, scoped member roles, public profiles and private actor audit
- Stable group-command timestamps and shared durable duplicate publication acknowledgment
- Signed service discovery, remembered key continuity and approved-peer relocation
- MangaDex-shaped read interfaces and mapped signed reader/publishing/group workflows
- Reviewed export migration, aliases, backup/restore and offline group-key recovery

The group-lifecycle choice is profile clearing and operator disabling only. No group-deletion endpoint removes keys, roles or published content. PostgreSQL with deadpool-postgres/tokio-postgres and Moka is the current server stack; it does not use SQLx or SQLite.

## Configure an instance deliberately

Use your own database, service-key file and administrator public key; never reuse fixture keys. Replace every placeholder in `node.example.json`. Provision the private service key through your own secure key-management workflow and keep the database DSN in `MANGA_DATABASE_URL`, outside source control.

```sh
./bin/manga-node migrate node.json
./bin/manga-node serve node.json
```

The service key and administrator key are distinct roles. NIP98 protects administrative/account HTTP writes; originals use scoped blob authorization. Non-loopback database connections require verified TLS. The example binds privately and leaves registration disabled. Production TLS, process supervision, admission policies and filesystem isolation need operator review. Optional managed nginx serving is implemented in the source but nginx is not bundled or enabled by this runtime.

Start with `docs/API-INDEX.md`, `docs/reader-enrollment.md`, `docs/mangadex-compatibility.md` and `docs/RELEASE-20261008.md`. Routes are not permission grants; configured roles and current policy still apply. Registration grants only personal-account rights and cannot undo an operator denial.

## Verification and limits

The source passes71 functional result groups,89 library unit tests (one optional stress test skipped),120 Python tests,Clippy and separate-process/PostgreSQL restart checks. Optimized build,enhanced two-instance demo and extracted-source library/binary checks pass. Functional recovery includes30000/500000-chapter libraries and new original-backed group publication after the founder node stops. Real TLS-proxy and database-error/recovery fixtures pass. Evidence is under `evidence/`; build metadata and hashes are in `BUILD.json` and `SHA256SUMS`.

The demo exercises only a subset of the backend. No unchanged third-party MangaDex client, independent security audit, production deployment,100TB corpus or target monthly traffic is certified. Further performance work remains paused under the feature-first instruction. Earlier load experiments are historical evidence with their own limits.

## Source and third-party notices

Application source is under `source/`; upstream strfry build sources are in `runtime-source.tar.gz`. Pinned build instructions and third-party notices are included under `licenses/`. This packaging step assigns no new license to the user's application code. Source dependency/build prerequisites are separate from running the bundled demonstration.
