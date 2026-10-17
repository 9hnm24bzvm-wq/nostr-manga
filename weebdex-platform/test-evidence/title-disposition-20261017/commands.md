# Exact disposition qualification commands

All commands ran in `cover-write-worktree` with cached dependencies, no downloads,
Rust jobs1 and a4GiB free-disk preflight. Runtime guards use separate disposable
loopback PostgreSQL and synthetic fixture identities; no real keys or external
relays were used. The exact source and executable are pinned in
`qualification-final.json` and `runtime-source-pins.json`.

## Rust

```sh
R=/workspace/shared/nostr-manga-recovery-20261017
T=$R/toolchains
BIN=$T/rustup/toolchains/1.90.0-x86_64-unknown-linux-gnu/bin
CARGO_HOME=$T/cargo RUSTUP_HOME=$T/rustup RUSTC=$BIN/rustc \
PATH=$BIN:$PATH CARGO_TARGET_DIR=$R/chapter-cargo-target CARGO_INCREMENTAL=0 \
cargo test --manifest-path weebdex-platform/backend/Cargo.toml \
  --lib --test public_manga_writes --test catalog_feed_projection \
  --locked --offline -j1
```

## Actual owned title lifecycle

```sh
R=/workspace/shared/nostr-manga-recovery-20261017
PLATFORM_TEST_PG_BIN=$R/toolchains/debian-pg/root/usr/lib/postgresql/17/bin \
LD_LIBRARY_PATH=$R/toolchains/debian-pg/root/usr/lib/x86_64-linux-gnu \
STRFRY_BIN=$R/upload-replacement-worktree/manga-protocol-lab/vendor/strfry/strfry \
bash weebdex-platform/scripts/run-auth-tests.sh \
  $R/chapter-cargo-target/debug/deps/public_manga_writes-929f4e97d9307bf7 \
  --ignored --test-threads=1 --nocapture
```

## Finite frontend regressions

```sh
node --experimental-vm-modules --test \
  weebdex-platform/tests/ci/test_public_title_commands.mjs
node --experimental-vm-modules --test \
  weebdex-platform/tests/ci/test_public_title_commands.mjs \
  weebdex-platform/tests/ci/test_public_cover_commands.mjs \
  weebdex-platform/tests/ci/test_public_upload_request_recovery.mjs
```

## Actual composed strict semantics

Run once for each `MODE` in `1` and `0`, choosing a fresh owned output path.
No canonical frontend source or dependency files are edited by this check.

```sh
R=/workspace/shared/nostr-manga-recovery-20261017
PUBLIC_MANGADEX_CATALOGUE=$MODE \
WEEBDEX_SEMANTIC_NODE_MODULES=$R/frontend-build-worktree/weebdex-platform/.build/frontend-restore3/node_modules \
WEEBDEX_SEMANTIC_OUTPUT=$PWD/weebdex-platform/.build/FRESH_OWNED_OUTPUT \
node weebdex-platform/scripts/check-composed-frontend.mjs
```

The enabled mode initially failed on a genuine missing `creator` relationship
union member. `strict-enabled-initial-failure` retains that failure. The actual
backend DTO emits this relationship; commit80ac1b2 adds its real type. Final
strict enabled/default evidence is separately retained without suppression.

Production build and real-browser acceptance are not claimed for this successor.
