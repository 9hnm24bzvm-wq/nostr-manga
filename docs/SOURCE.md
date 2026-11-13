# Source and dependency boundary

## Provenance

Originally extracted from `manga-protocol-lab/server` at repository source commit `2e1ccdb5d78c2377c832cfef56d26e1cdf3c58d2` in [9hnm24bzvm-wq/nostr-manga](https://github.com/9hnm24bzvm-wq/nostr-manga). Production Rust modules and all migrations are preserved. The optional `src/serving_supervisor.py` comes from the protocol project's production operator code.

The extraction removes test-only Rust modules, development dependencies, demos, benchmark examples, test workers and all integration fixtures. It relocates the lockfile to a root Cargo workspace and adds standalone documentation. It does not split off a wire-only subset or rewrite protocol semantics.

The parent repository's main and historical branches remain separate. This branch's current tree excludes the WeebDex backend/frontend, browser client, application deployment scripts, historical reports, build artifacts, installed toolchains, runtime state and secrets. General migration/import/backup CLIs from the larger project are not included. The node retains its native database migration and offline key commands.

This update derives the production core from the November 13 reconstruction based on main commit `09c7eb861d935e386c6424c06dc5fc9820681833`. It adds native projection, platform filesystem/private-file adapters and guarded HTTP range handling. Windows/macOS compilation and PostgreSQL runtime behavior remain unverified.

## Rust dependencies

`server/Cargo.toml` lists the direct production dependencies, and `Cargo.lock` pins the full resolved dependency graph. There are no path dependencies on the WeebDex platform.

- Wire formats and signatures: serde, serde_json, ciborium, SHA-256, secp256k1, hex, base64 and language-tags.
- HTTP, WebSocket and asynchronous execution: Axum, Tokio, futures-util, reqwest, async-trait and Rustls.
- Durable state and bounded derived caches: tokio-postgres, deadpool-postgres, tokio-postgres-rustls and Moka.
- Federation and original-image handling: Negentropy, image and flate2.
- Error reporting, tracing, randomness, zeroization, URLs and process interfaces: anyhow, thiserror, tracing, tracing-subscriber, rand, zeroize, url and libc.

These dependencies support the full protocol implementation. A smaller wire-codec-only crate would omit durable publication, admission, storage, synchronization and relay behavior.

## External runtime components

- PostgreSQL is authoritative. The recovered development environment used PostgreSQL 18.6; compatibility with every other PostgreSQL release is not asserted.
- The default projection runs in Rust/PostgreSQL without a relay subprocess. Legacy strfry is an explicit Linux-only private derived event projection. The source was integrated against upstream commit `4cd3cf64850caf47dda46c2a2abbbf3525a64d10` from [hoytech/strfry](https://github.com/hoytech/strfry). Follow that project's build and license instructions. The wrapper also invokes GNU coreutils `timeout` for bounded backend commands.
- Python 3 and NGINX are needed only for optional Linux-only managed public original-image serving. The included supervisor uses the Python standard library. The recovered environment used NGINX 1.30.6; select and review an appropriate maintained deployment version.

No bootstrap script automatically installs these services or changes host security settings.
