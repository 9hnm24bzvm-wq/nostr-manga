# Compiled-cache removal notice

The two compiler-only archive tarballs and current `.build-release/release/deps` were removed under the user-approved cache cleanup on 2026-10-11. Their original JSON inventories/receipts remain as historical evidence, but their old archive restore commands are no longer executable. No runtime archive or recovery checkpoint was removed.

All source/locks/vendor toolchains, runnable top-level executables, historical baseline binaries, test evidence and recovery files were preserved and hash-verified. Future builds must regenerate compiled dependencies from the desired source snapshot and its lock file with Rust1.90.0, vendored Cargo/Rustup, offline/locked release, j2, CARGO_INCREMENTAL=0. Rebuilding is not a claim of byte-identical recreation of every old test artifact.

Current-source preparation command, to be run only when a future build is authorized:
`CARGO_HOME=$PWD/vendor/cargo RUSTUP_HOME=$PWD/vendor/rustup RUSTUP_TOOLCHAIN=1.90.0 CARGO_TARGET_DIR=$PWD/.build-release CARGO_INCREMENTAL=0 vendor/cargo/bin/cargo test --release --offline --locked --manifest-path server/Cargo.toml -j2 --no-run`

Exact deletion inventories, source/lock and protected-artifact hashes are in `manga-protocol-lab/results/rebuild-mixed-prep/compiler-cache-removal-20261011/`. No rebuild or mixed workload was performed by this cleanup.
