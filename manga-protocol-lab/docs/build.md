> Current reconstruction: see [rebuild/TOOLCHAIN.md](rebuild/TOOLCHAIN.md) for October10 dependency restoration. The canonical current gate is `bash scripts/test.sh`. Earlier dated results below are historical and do not validate the reconstructed tree.

# Reproduce on a Linux research machine

Prerequisites: GNU coreutils (including timeout), Python 3.12+, C++ compiler, make, Perl, Git, zlib/OpenSSL development headers, and a Rust toolchain. This was built with Rust 1.90.0, GCC 14.2, and CMake 3.31.6. The harness expects the paths below. Review official upstream licenses before redistribution.

## Project-local bootstrap

The convenience script below installs Python/build tools and Rust under vendor, checks out the pinned native sources, builds them and builds the selected PostgreSQL server. It uses official project/registry sources and does not use sudo, install a system service or modify shell profiles. System compiler, Perl, zlib/OpenSSL development headers, Git and GNU coreutils timeout must already be available. Use a project path without spaces for the upstream native builds.

```sh
bash scripts/bootstrap.sh
bash scripts/bootstrap-postgres.sh
CARGO_TARGET_DIR="$PWD/prototype/target" bash scripts/test-postgres.sh
```

For a prepared environment, ./scripts/bootstrap.sh --check performs read-only revision/version checks. That check and shell syntax validation passed on the research computer. The complete bootstrap script has not been independently run on a second clean machine. Direct Python versions are pinned in requirements.txt; this is not a fully hermetic lock of every platform/transitive build dependency.

## Python and Rust

```sh
mkdir -p vendor
python3 -m pip install --target vendor/pydeps cbor2==5.6.5 coincurve==21.0.0 aiohttp==3.13.5 Pillow==12.3.0
python3 -m pip install --target vendor/buildtools cmake==3.31.6
# Install Rust 1.90.0 from the official rustup distribution, or use an existing toolchain.
# The original workspace uses vendor/cargo and vendor/rustup without changing the shell profile.
export CARGO_HOME="$PWD/vendor/cargo" RUSTUP_HOME="$PWD/vendor/rustup"
export PATH="$CARGO_HOME/bin:$PATH"
CARGO_TARGET_DIR="$PWD/prototype/target" cargo build --locked --manifest-path server/Cargo.toml -j2
```

The canonical functional runner is scripts/test.sh; deep-test.sh delegates to it. scripts/test-postgres.sh is the narrower current PostgreSQL/strfry integration gate. Keep CARGO_TARGET_DIR explicit to avoid accumulating separate build caches.

## Pinned native sources

- https://github.com/hoytech/strfry : 4cd3cf64850caf47dda46c2a2abbbf3525a64d10; recursive submodules at that commit.
- https://github.com/bitcoin-core/secp256k1 : v0.6.0, 0cdc758a56360bf58a851fe91085a327ec97685a.
- https://github.com/LMDB/lmdb : LMDB_0.9.33, 3a29a24777c82a0165de813ae696a5068b5add30.
- https://github.com/google/flatbuffers : v25.2.10, 1c514626e83c20fffa8557e75641848e1e15cd5e.
- https://nginx.org/download/nginx-1.30.5.tar.gz : research build, not a statement about latest secure production release.

Clone these under vendor/strfry, vendor/secp256k1, vendor/lmdb, vendor/flatbuffers, respectively. Verify the commit IDs, then initialize strfry submodules with git submodule update --init --recursive.

```sh
CMAKE="$PWD/vendor/buildtools/cmake/data/bin/cmake"
"$CMAKE" -S vendor/secp256k1 -B vendor/secp256k1/build \
  -DBUILD_SHARED_LIBS=OFF -DSECP256K1_ENABLE_MODULE_SCHNORRSIG=ON \
  -DSECP256K1_BUILD_TESTS=OFF -DSECP256K1_BUILD_BENCHMARK=OFF
"$CMAKE" --build vendor/secp256k1/build -j2
make -C vendor/lmdb/libraries/liblmdb -j2 liblmdb.a
"$CMAKE" -S vendor/flatbuffers -B vendor/flatbuffers/build -DFLATBUFFERS_BUILD_TESTS=OFF
"$CMAKE" --build vendor/flatbuffers/build --target flatc -j2
export APPS='dbutils relay mesh'
(cd vendor/strfry && mkdir -p build && golpe/gen-config.pl && golpe/gen-main.cpp.pl)
PATH="$PWD/vendor/flatbuffers/build:$PATH" make -C vendor/strfry -j2 NPROC=2 OPT='-O1' \
  XCXXFLAGS="-I$PWD/vendor/secp256k1/include -I$PWD/vendor/lmdb/libraries/liblmdb -I$PWD/vendor/flatbuffers/include" \
  XLDFLAGS="-L$PWD/vendor/secp256k1/build/lib -L$PWD/vendor/lmdb/libraries/liblmdb"
```

Extract nginx under vendor/nginx-1.30.5:

```sh
(cd vendor/nginx-1.30.5 && ./configure --prefix="$PWD/local" \
  --with-http_auth_request_module --with-http_ssl_module --with-threads && make -j2)
./scripts/test.sh
```

The test generator creates its own loopback-only nginx and instance configurations, using unprivileged ports. No root service installation is needed. Run one integration/serving test at a time because the harness uses fixed ports 19201/19202 and 19301/19302.

## Historical reproduction notes (not current acceptance)

The dependencies and prototype were actually built in the research workspace and the integration tests were run there. These condensed clean-machine instructions have not yet been independently executed in a second fresh machine. Network access, platform libraries and upstream build prerequisites can vary.

A later fresh-extraction bootstrap attempt installed the Python dependencies but was blocked by the execution environment’s network policy while fetching Rust components from static.rust-lang.org. No alternate route was attempted. The full bootstrap remains unverified; this does not invalidate tests using the existing prepared dependencies. See results/clean-bootstrap-status.json.

## October7 restored-workspace verification

The user-supplied v43 source was byte-verified and restored into a fresh workspace. Missing dependencies were rebuilt from the pinned official sources. The fresh run exposed a missing strfry build-directory creation, now fixed. The script also avoids reinstalling already matching Python/build dependencies and bounds pip retry time. `results/restored-bootstrap-idempotent.log` records the corrected bootstrap completing and building the PostgreSQL server. This was a repaired fresh-environment restoration followed by an idempotent pass, not an untouched single-pass clean-machine proof.


## Browser client and combined test prerequisites

The local-first client ships prebuilt under browser-client/public. Rebuilding it and running the combined real-backend client fixture require Node20.19+ and pinned npm dependencies. Before scripts/test-postgres.sh, run `cd browser-client && npm ci --ignore-scripts && npm run build`. Browser clients do not require Node at runtime. The IndexedDB test model is not a rendered-browser substitute; see browser-client/README.md for current verification limits.


## October 10 recovery toolchain

See `docs/rebuild/TOOLCHAIN.md` for the current recovery evidence and deviations. `scripts/bootstrap.sh --python-only` prepares only direct Python dependencies. `scripts/bootstrap.sh --dependencies-only` prepares Python, Rust, and native dependencies while leaving application compilation to a separate step. The recovered fixtures' historical `vendor/nginx-1.28.0` path is a compatibility symlink to the actual nginx 1.30.5 build. PostgreSQL native build defaults to two jobs.
