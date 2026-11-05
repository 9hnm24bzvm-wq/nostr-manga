# MangaPub core

The standalone Rust protocol implementation extracted from WeebDex's `manga-node` package. This branch contains the functioning protocol node and its reusable Rust library, not the WeebDex website or platform backend.

The original crate name, `manga-node` (imported as `manga_node`), is preserved for API compatibility. “MangaPub” names this extracted protocol implementation; it does not imply a standardized or formally allocated Nostr protocol.

## Included

- Signed Nostr event verification, exact HTTP authentication binding, Blossom scopes, and bounded CBOR chapter, series, and public-library codecs.
- Publication and publisher admission, complete-original retention, storage accounting, guarded reads, moderation, groups, and public-library lifecycle.
- Approved-peer federation, identity and endpoint discovery, reconciliation, durable synchronization, and private strfry projection.
- PostgreSQL persistence and all 52 checksum-guarded migrations.
- The `manga-node` service, `manga-key-tool` operator utility, and optional managed-image-serving supervisor.

There are no tests, test fixtures, test helpers, test evidence, demos, website assets, runtime databases, credentials, installed dependencies, or compiled binaries in this branch's current tree. Existing repository history is retained. The small `platform_bridge` module is a read-only protocol visibility interface for downstream consumers; it does not depend on the WeebDex platform.

## Build

Install Rust 1.90 or later with a C compiler and linker, then:

```sh
cargo build --locked --release
```

The library and both production executables are built. Dependencies are recorded in `server/Cargo.toml` and pinned by the root `Cargo.lock`. Building does not require a running database or relay. Running a complete node requires PostgreSQL and strfry. Optional managed original-image serving also needs Python 3 and NGINX.

## Use as a Rust dependency

```toml
[dependencies]
manga-node = { git = "https://github.com/9hnm24bzvm-wq/nostr-manga.git", branch = "mangapub-core" }
```

For a reproducible consumer build, replace `branch` with a reviewed commit `rev`. Cargo finds the `server` package in this workspace. The package intentionally retains `publish = false`: this Git branch is the publication destination, not crates.io.

```rust
use manga_node::{auth::Event, manifest};

fn inspect_event(json: &[u8]) -> Result<Event, String> {
    let event: Event = serde_json::from_slice(json).map_err(|e| e.to_string())?;
    event.verify()?;
    Ok(event)
}
```

A consumer using this JSON example must declare its own `serde_json` dependency. Signature verification alone does not grant admission, validate a publication's complete object graph, or establish that an instance retains its images. Use the node's publication/storage workflows for those guarantees.

- [Run a node](docs/RUNNING.md)
- [Protocol and architecture](docs/PROTOCOL.md)
- [Dependencies and source boundary](docs/SOURCE.md)
- [License status](LICENSE-STATUS.md)

This is experimental software. Publication does not certify production security, availability, storage durability, or traffic capacity.
