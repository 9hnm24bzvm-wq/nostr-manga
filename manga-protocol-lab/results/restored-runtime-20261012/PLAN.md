# Runtime-first restoration, 2026-10-12

## Scope and priority

Root authorized project-local runtime restoration at21:05UTC. Preserve the accepted
`build-artifacts/linux-x86_64/manga-node` SHA256
`4c499f724af4db1843f54c51e47746114f777de15b446979a3e8e8a0e7a3e23a`.
This checkpoint restored1262 archive members, including387 of392 historical canonical
inputs. Five historical diagnostic sources are absent. The new fast-PG helpers, private
runtime databases and previous raw traffic are not restored by this work.

Priority: pinned Python, PostgreSQL18.6, pinned strfry prerequisites/runtime, then nginx.
Existing browser/public assets can be served directly without npm or rebuilding them.
Use the existing accepted application binary for subsequent authorized runtime/browser
smoke. Full compiler restoration is deferred until a code change or full source rebuild
requires it. No mixed workload is authorized by this plan.

## Observed environment

Debian13.6 x86_64; Python3.12.14/pip26.2.1; Node24.19.0/npm11.9.0; GCC/G++14.2.0;
GNU make/git/perl/tar/timeout/curl/pkg-config available. OpenSSL3.5.7, zlib1.3.1,
PCRE2 10.46 and zstd development headers/shared libraries present.9 reported CPUs;
about8.8GB MemAvailable at inspection, which is host-visible rather than an exclusive
allocation. Initial workspace free31,656,316,928 bytes.

Absent: vendor/, PG server/client tools, strfry, nginx, CMake, bison/flex/m4,
Rust/rustup/Cargo, npm node_modules and compiled Cargo cache. System aiohttp3.14.4
differs from the required3.13.5; cbor2/coincurve are absent. System Pillow12.3.0 matches.
The four manifest-selected public-domain JPEGs are present at recorded sizes. The
restored archive verification, rather than these size checks alone, establishes bytes.

## Sources and pins

| Component | Official source and pin |
|---|---|
| Python | PyPI: aiohttp3.13.5, cbor2 5.6.5, coincurve21.0.0, Pillow12.3.0; requirements.txt SHA18f532a573b6fa96abcb0ad8e706d5a5ec9a1ccf3ca7557d280556f5136c5028 |
| CMake | PyPI cmake3.31.6 |
| PostgreSQL | https://ftp.postgresql.org/pub/source/v18.6/postgresql-18.6.tar.bz2,22,632,314 bytes; SHA555610c24d53e4316da5b7d3fc25c279d96856d5e0e23ee308c328c5fa881d9f |
| bison | Official Debian bison_3.8.2+dfsg-1+b2_amd64.deb; SHAce6eb1f7aad18850e32d223a68f54bcbe756d74f138c8ec5460ad9d220f4d335 |
| flex | Official Debian flex_2.6.4-8.2+b4_amd64.deb; SHA75f963765232c356d0a2fb70e780dfa9de2425aca036ec60eefb7fc98a2c905b |
| m4 | Official Debian m4_1.4.19-8_amd64.deb; SHA221b3e224708c1da2a1a88b6824380593579b2254ae7d914afa14cbd5d343334 |
| strfry | https://github.com/hoytech/strfry.git @4cd3cf64850caf47dda46c2a2abbbf3525a64d10, recursive upstream gitlinks |
| secp256k1 | https://github.com/bitcoin-core/secp256k1.git @0cdc758a56360bf58a851fe91085a327ec97685a |
| LMDB | https://github.com/LMDB/lmdb.git @3a29a24777c82a0165de813ae696a5068b5add30 |
| FlatBuffers | https://github.com/google/flatbuffers.git @1c514626e83c20fffa8557e75641848e1e15cd5e |
| nginx | https://nginx.org/download/nginx-1.30.5.tar.gz,1,328,600 bytes; SHA6c20565aa2325cb82216ae804f4a4ff1875179014759a381c42ddc8e11c4906d |

PG's official checksum text was read and matched the restored script. nginx's official
[download page](https://nginx.org/en/download.html) lists1.30.5; downloaded bytes must
still match the restored checksum before extraction. The PostgreSQL
[official source index](https://ftp.postgresql.org/pub/source/v18.6/) lists the exact
archive. The [pinned strfry source](https://github.com/hoytech/strfry/tree/4cd3cf64850caf47dda46c2a2abbbf3525a64d10)
requires C++20 and the native dependencies being restored here.

Python direct versions are pinned; transitive Python distributions/platform wheels are
not completely hash-locked by the recovered requirements. Record actual installed
distribution versions and the install logs. Do not describe this as a hermetic recreation
of the lost vendor tree. Git object pins bind source revisions; record recursive submodule
pins after checkout. No alternate hosts or mirrors after an access denial.

Normal-exec HEAD probes returned200 for PyPI, PG, Debian bison, nginx, strfry GitHub,
sh.rustup.rs, the Rust1.90 manifest checksum, crates.io sparse index and npm registry.
This verifies metadata reachability only. Bulk downloads and every recursive submodule
remain independently fallible; retain failures and stop/report actual policy denials.

## Exact runtime commands

Each stage runs through the restored guarded-command helper copied into this result
directory. Only its disk-stop constant is raised from2GiB to24GiB. It retains the
tested PID/start/pidfd/subreaper cleanup,60-minute stage deadline,1GiB available-memory
stop,32MiB captured console and8MiB telemetry caps. Native compilation uses at most
two jobs. The24GiB floor bounds currently available filesystem growth to roughly5.9GB;
it is sampled protection, not a hard overshoot/OOM guarantee. PostgreSQL also writes
its historical results/postgres-build.log, which is covered by disk monitoring rather
than the captured-console cap. No database/service is launched during these builds.

```sh
python3 results/restored-runtime-20261012/guarded-command.py python-runtime bash scripts/bootstrap.sh --python-only
python3 results/restored-runtime-20261012/guarded-command.py postgres-runtime bash scripts/bootstrap-postgres.sh
python3 results/restored-runtime-20261012/guarded-command.py native-runtime bash results/restored-runtime-20261012/restore-native-runtime.sh
```

The native-only command repeats the restored bootstrap's pinned checkout/build blocks
without its Rust setup or application compilation. CMake and Python install under vendor;
Debian build-tool packages are checksum-verified and extracted locally with dpkg-deb.
No package maintainer scripts, sudo, global installs, system services or shell-profile
changes are used. nginx keeps the historical1.28.0 path as a symlink to actual1.30.5.

Before/after guards protect all present canonical inputs and accepted binaries except
the two explicitly authorized concurrent packaging edits: scripts/package.py and
scripts/tests/test_package_cli.py. Both exceptions and the five missing historical
inputs are recorded in restored-source-protection.json. Source drift elsewhere fails
the stage. Fresh stage records are never overwritten.

After runtime build success, verify executable versions, recursive Git pins, no missing
shared libraries and unchanged accepted node. A separate authorized disposable PG/node
smoke must confirm migration, signed HTTP and owned same-context cleanup before load.
The mixed fixture already accepts:

```sh
MIXED_NODE_BIN="$PWD/build-artifacts/linux-x86_64/manga-node"
MIXED_EXPECTED_NODE_SHA256=4c499f724af4db1843f54c51e47746114f777de15b446979a3e8e8a0e7a3e23a
MIXED_PG_BIN_DIR="$PWD/vendor/pgsql/bin"
MIXED_STRFRY_BIN="$PWD/vendor/strfry/strfry"
MIXED_NGINX_BIN="$PWD/vendor/nginx-1.30.5/objs/nginx"
```

These assignments are future configuration, not authorization to execute traffic.

## Deferred compiler/model-test restoration

Rust1.90.0 minimal+rustfmt+clippy from sh.rustup.rs/static.rust-lang.org, using local
CARGO_HOME/RUSTUP_HOME and --no-modify-path. Fresh installer bytes must be inspected
and hashed before execution; the recovered installer URL is not itself a frozen hash.
The official channel-rust-1.90.0.toml checksum read today is
489c19f20d331765ab2835661eb546de90f6446a107a8db83045e7371e45cae2.
Cargo.lock SHAd7cdff3f56961c3e64ea986c65bfbc71ddb36d9202f4dad7dd6747c0a00e3503
binds registry crate checksums. Fetch with --locked first, then use --offline --locked
release builds in one explicit target directory, CARGO_INCREMENTAL=0, initially-j1.
Do not overwrite the accepted packaged executable or claim the rebuilt hash in advance.

For model/build checks only: npm ci --ignore-scripts against package-lock.json
SHAd8acbe16c2334d627d02c9c4d7f43d336746d04897e478349013b6cb6b9719c4;
direct packages @noble/curves2.0.1, @noble/hashes2.0.1, fake-indexeddb6.2.4,
jsdom27.0.1, prettier3.6.2. Native browser operation of restored public assets does not
require this step. Do not rebuild/replace those assets merely to serve them.

Planning estimates, not measurements: runtime download/build10–30 minutes and1–3GB
working disk if sources remain reachable; cold compiler/toolchain plus full Rust gate
30–90 minutes and an additional4–8GB working allowance. Prior compiled dependency
caches were approximately1.6–2.4GB, excluding toolchains/native sources/test runtimes.
Actual stage clocks, bytes and resource minima decide whether to continue. Recheck the
future mixed-load9,436,910,336-byte preflight and4GiB abort floor only after the missing
run-specific ownership/diagnostic helpers have been rebuilt and independently qualified.
