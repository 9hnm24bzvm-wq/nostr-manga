# Recovered project-local toolchain (2026-10-10)

## Scope and provenance

Dependencies are restored only under this project's `vendor/` directory using official upstream release endpoints, upstream Git repositories, or PyPI. No global package install, sudo, system service, account change, or shell-profile modification is used. Native builds use at most two jobs per build. Runtime source reconstruction and integration tests are separate work.

Observed host tools: Debian GNU/Linux 13, GCC 14.2.0, Python 3.12.14, Node 24.19.0. Node is provided by the existing environment rather than a new local install.

Installed local tools: Rust 1.90.0 (official rustup distribution), CMake 3.31.6 (PyPI), PostgreSQL 18.6 (official archive SHA256 `555610c24d53e4316da5b7d3fc25c279d96856d5e0e23ee308c328c5fa881d9f`). Missing PostgreSQL build tools are extracted from the script's checksum-pinned official Debian packages; their package installation scripts are not executed.

The rustup shell installer was retrieved from https://sh.rustup.rs and inspected before execution: it selects the official static.rust-lang.org rustup binary by host architecture and passes the local install settings. `--no-modify-path`, project-local CARGO_HOME/RUSTUP_HOME, and minimal profile are used.

## Intentional recovery changes

- `scripts/bootstrap.sh` now supports `--python-only`, which installs/checks the direct Python requirements and exits without any Rust/native build.
- `--dependencies-only` performs the native dependency build and verification but skips compiling application source, permitting concurrent source recovery.
- nginx 1.28.0 is replaced by 1.30.5 from https://nginx.org/download/nginx-1.30.5.tar.gz, confirmed listed by the official download page. Archive SHA256: `6c20565aa2325cb82216ae804f4a4ff1875179014759a381c42ddc8e11c4906d`. A compatibility symlink at `vendor/nginx-1.28.0` points to the actual 1.30.5 directory for historical fixture paths; it does not supply the old version.
- PostgreSQL's default build parallelism is reduced from four to two jobs.
- Rust bootstrap now skips already installed toolchain/components and suppresses rustup self-update when installation is required; an unnecessary self-update endpoint had failed during recovery.
- Pinned strfry, secp256k1, LMDB and FlatBuffers revisions remain those documented in `docs/build.md` and checked by bootstrap.

## Environment

From the project root:

```sh
export CARGO_HOME="$PWD/vendor/cargo"
export RUSTUP_HOME="$PWD/vendor/rustup"
export RUSTUP_TOOLCHAIN=1.90.0
export PATH="$CARGO_HOME/bin:$PWD/vendor/pgsql/bin:$PATH"
export CARGO_TARGET_DIR="$PWD/prototype/target"
export PYTHONPATH="$PWD/vendor/pydeps${PYTHONPATH:+:$PYTHONPATH}"
```

Native paths: `vendor/strfry/strfry`, `vendor/nginx-1.30.5/objs/nginx`, and `vendor/pgsql/bin/`.

## Verification status

Rust 1.90.0, PostgreSQL 18.6, nginx 1.30.5, pinned strfry, and Python direct requirements are installed. Version commands and strfry `--help` pass; `ldd` reports no missing strfry shared libraries. The final `--dependencies-only` bootstrap completed successfully (application compilation intentionally skipped), followed by a successful public `--check` and `--python-only` run. `results/rebuild/bootstrap-dependencies.log` and `results/rebuild/bootstrap-postgres.log` record the operations; detailed PostgreSQL build output is in `results/postgres-build.log`. `bash scripts/bootstrap.sh --python-only` has exited successfully and is recorded in `results/rebuild/python-only.log`. No full integration test success is claimed by this document.


### Recovery execution notes

The first dependency run was interrupted after the running bootstrap file was edited; this is an execution mistake, not evidence of a clean single-pass bootstrap. The script was syntax-checked and restarted from its stable contents. All logs are retained.

The first recursive strfry checkout hit an HTTP403 on its required official `https://github.com/hoytech/templar.git` submodule, and a subsequent session poll reported a cancelled approval review. Work on that action paused. After the parent supplied explicit user authorization evidence, the same official `git submodule update --init external/templar` action succeeded and checked out commit `05554eea58a15d461e3385507c1c3fd1bfd05fa3`. No alternate host or route was used. Evidence: `results/rebuild/templar-authorized-retry.log`. Remaining bootstrap work resumed into `results/rebuild/bootstrap-dependencies-final.log`.


The next recursive dependency checkout also encountered HTTP403 at the official hoytech/uWebSockets repository. With the same explicit rebuild authorization, the retry found a partial clone without a checked-out HEAD. Fetching the exact pinned revision `45816de84b51b9f217fd0242fecd003e0d6a48c6` from its existing official origin succeeded, and it was checked out. Recursive submodule normalization then succeeded. Two incomplete initial clone worktrees (`hoytech-cpp` and `session-token-cpp`) contained staged missing files and were restored from their already-downloaded pinned HEADs. `results/rebuild/strfry-submodule-pins.log` records the final recursive revisions. No upstream tracked source code was edited.


Final successful evidence:

- `results/rebuild/bootstrap-success-attempt.log`: completed dependency bootstrap with app compilation skipped.
- `results/rebuild/bootstrap-check.log`: public CLI revision/version check, exit 0.
- `results/rebuild/python-only.log`: public Python-only command, exit 0.
- `results/rebuild/toolchain-versions.log`: observed versions and source archive hashes.
- `results/rebuild/strfry-help.log`: executable help smoke test, exit 0.
- `results/rebuild/strfry-submodule-pins.log`: recursive dependency pins.
- Shell syntax validation: `bash -n scripts/bootstrap.sh scripts/bootstrap-postgres.sh`, exit 0.

This is a repaired restoration with documented interruptions and resumes. It is not a claim that the original bootstrap works untouched on every fresh machine. Full application integration verification belongs to the other rebuild reports.
