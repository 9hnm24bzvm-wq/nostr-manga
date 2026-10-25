#!/usr/bin/env bash
# Project-local dependencies only. No sudo, system service or shell-profile changes.
set -euo pipefail
cd "$(dirname "$0")/.."
ROOT="$PWD"
case "$ROOT" in *" "*) echo "Use a project path without spaces for the native upstream builds." >&2; exit 1;; esac
export CARGO_HOME="$ROOT/vendor/cargo"
export RUSTUP_HOME="$ROOT/vendor/rustup"
export RUSTUP_TOOLCHAIN=1.90.0
CARGO="$CARGO_HOME/bin/cargo"
RUSTUP="$CARGO_HOME/bin/rustup"

verify_repo() {
  local dir="$1" expected="$2"
  test "$(git -C "$dir" rev-parse HEAD)" = "$expected" || { echo "Unexpected revision: $dir" >&2; exit 1; }
  git -C "$dir" diff --quiet HEAD || { echo "Tracked local changes in $dir; refusing to overwrite." >&2; exit 1; }
}
# BEGIN NGINX COMPATIBILITY VERIFY
verify_nginx_compatibility_link() {
  if [[ ! -L vendor/nginx-1.28.0 ]] || [[ "$(readlink vendor/nginx-1.28.0)" != nginx-1.30.6 ]]; then
    echo "Nginx compatibility link is not the current pinned build; refusing to execute it." >&2
    return 1
  fi
}
# END NGINX COMPATIBILITY VERIFY
verify_all() {
  timeout --version | grep -q "GNU coreutils"
  verify_repo vendor/strfry 4cd3cf64850caf47dda46c2a2abbbf3525a64d10
  verify_repo vendor/secp256k1 0cdc758a56360bf58a851fe91085a327ec97685a
  verify_repo vendor/lmdb 3a29a24777c82a0165de813ae696a5068b5add30
  verify_repo vendor/flatbuffers 1c514626e83c20fffa8557e75641848e1e15cd5e
  test -x vendor/strfry/strfry
  test -x vendor/nginx-1.30.6/objs/nginx
  test "$(vendor/nginx-1.30.6/objs/nginx -v 2>&1)" = "nginx version: nginx/1.30.6"
  verify_nginx_compatibility_link
  test "$(vendor/nginx-1.28.0/objs/nginx -v 2>&1)" = "nginx version: nginx/1.30.6"
  "$RUSTUP" toolchain list | grep -q '^1.90.0'
  "$CARGO" --version
  PYTHONPATH="$ROOT/vendor/pydeps" python3 - <<'PY'
from importlib.metadata import version
for package,expected in [('aiohttp','3.13.5'),('cbor2','5.6.5'),('coincurve','21.0.0'),('Pillow','12.3.0')]:
    actual=version(package)
    assert actual==expected,(package,actual,expected)
    print(package,actual)
PY
}
if [[ "${1:-}" == "--check" ]]; then
  verify_all
  echo "Existing dependency revisions and direct Python package versions verified. No fresh-machine build claim."
  exit 0
fi
case "${1:-}" in ""|--python-only|--dependencies-only) ;; *) echo "Usage: $0 [--check|--python-only|--dependencies-only]" >&2; exit 1;; esac
command -v python3 >/dev/null || { echo "Missing prerequisite: python3" >&2; exit 1; }
mkdir -p vendor
download() {
  python3 - "$1" "$2" <<'PY'
import pathlib,sys,urllib.request
url,dest=sys.argv[1:]
with urllib.request.urlopen(url,timeout=120) as response:
    data=response.read(128*1024*1024+1)
if len(data)>128*1024*1024:raise SystemExit("download budget exceeded")
pathlib.Path(dest).write_bytes(data)
PY
}
if ! PYTHONPATH="$ROOT/vendor/pydeps" python3 -c 'from importlib.metadata import version; assert all(version(p)==v for p,v in [("aiohttp","3.13.5"),("cbor2","5.6.5"),("coincurve","21.0.0"),("Pillow","12.3.0")])' 2>/dev/null; then
  python3 -m pip install --timeout 30 --retries 2 --upgrade --target vendor/pydeps -r requirements.txt
fi
if [[ "${1:-}" == "--python-only" ]]; then
  echo "Python dependencies ready in vendor/pydeps."
  exit 0
fi
for command in git python3 make g++ perl tar timeout; do command -v "$command" >/dev/null || { echo "Missing prerequisite: $command" >&2; exit 1; }; done
if ! test -x vendor/buildtools/cmake/data/bin/cmake || ! vendor/buildtools/cmake/data/bin/cmake --version | grep -q '3.31.6'; then
  python3 -m pip install --timeout 30 --retries 2 --upgrade --target vendor/buildtools cmake==3.31.6
fi
if [[ ! -x "$RUSTUP" ]]; then
  download https://sh.rustup.rs vendor/rustup-init.sh
  sh vendor/rustup-init.sh -y --no-modify-path --profile minimal --default-toolchain 1.90.0
fi
if ! "$RUSTUP" toolchain list | grep -q '^1.90.0-'; then
  "$RUSTUP" toolchain install 1.90.0 --profile minimal --no-self-update
fi
for component in rustfmt clippy; do
  if ! "$RUSTUP" component list --toolchain 1.90.0 --installed | grep -q "^${component}-"; then
    "$RUSTUP" component add "$component" --toolchain 1.90.0
  fi
done
checkout() {
  local url="$1" dir="$2" revision="$3"
  if [[ ! -e "$dir" ]]; then
    git clone --no-checkout "$url" "$dir"
    git -C "$dir" checkout --detach "$revision"
  fi
  verify_repo "$dir" "$revision"
}
checkout https://github.com/hoytech/strfry.git vendor/strfry 4cd3cf64850caf47dda46c2a2abbbf3525a64d10
git -C vendor/strfry submodule update --init --recursive
checkout https://github.com/bitcoin-core/secp256k1.git vendor/secp256k1 0cdc758a56360bf58a851fe91085a327ec97685a
checkout https://github.com/LMDB/lmdb.git vendor/lmdb 3a29a24777c82a0165de813ae696a5068b5add30
checkout https://github.com/google/flatbuffers.git vendor/flatbuffers 1c514626e83c20fffa8557e75641848e1e15cd5e
CMAKE="$ROOT/vendor/buildtools/cmake/data/bin/cmake"
"$CMAKE" -S vendor/secp256k1 -B vendor/secp256k1/build -DBUILD_SHARED_LIBS=OFF -DSECP256K1_ENABLE_MODULE_SCHNORRSIG=ON -DSECP256K1_BUILD_TESTS=OFF -DSECP256K1_BUILD_BENCHMARK=OFF
"$CMAKE" --build vendor/secp256k1/build -j2
make -C vendor/lmdb/libraries/liblmdb -j2 liblmdb.a
"$CMAKE" -S vendor/flatbuffers -B vendor/flatbuffers/build -DFLATBUFFERS_BUILD_TESTS=OFF
"$CMAKE" --build vendor/flatbuffers/build --target flatc -j2
export APPS='dbutils relay mesh'
(cd vendor/strfry && mkdir -p build && golpe/gen-config.pl && golpe/gen-main.cpp.pl)
PATH="$ROOT/vendor/flatbuffers/build:$PATH" make -C vendor/strfry -j2 NPROC=2 OPT='-O1' \
  XCXXFLAGS="-I$ROOT/vendor/secp256k1/include -I$ROOT/vendor/lmdb/libraries/liblmdb -I$ROOT/vendor/flatbuffers/include" \
  XLDFLAGS="-L$ROOT/vendor/secp256k1/build/lib -L$ROOT/vendor/lmdb/libraries/liblmdb"
if [[ ! -f vendor/nginx-1.30.6.tar.gz ]]; then
  download https://nginx.org/download/nginx-1.30.6.tar.gz vendor/nginx-1.30.6.tar.gz
fi
echo '3cf46f5ed7504d460125866b7bea99fb357f31d887e2e63b35503310e9b8b6c8  vendor/nginx-1.30.6.tar.gz' | sha256sum -c -
if [[ ! -d vendor/nginx-1.30.6 ]]; then tar -xzf vendor/nginx-1.30.6.tar.gz -C vendor; fi
(cd vendor/nginx-1.30.6 && ./configure --prefix="$PWD/local" --with-http_auth_request_module --with-http_ssl_module --with-threads && make -j2)
# Compatibility path used by the recovered fixture, still serving nginx 1.30.6.
# BEGIN NGINX COMPATIBILITY LINK
if [[ -L vendor/nginx-1.28.0 ]]; then
  case "$(readlink vendor/nginx-1.28.0)" in
    nginx-1.30.5|nginx-1.30.6) ln -sfn nginx-1.30.6 vendor/nginx-1.28.0 ;;
    *) echo "Unexpected nginx compatibility link; refusing to replace it." >&2; exit 1 ;;
  esac
elif [[ ! -e vendor/nginx-1.28.0 ]]; then
  ln -s nginx-1.30.6 vendor/nginx-1.28.0
else
  echo "Existing nginx compatibility directory/file; refusing to replace it." >&2
  exit 1
fi
# END NGINX COMPATIBILITY LINK
if [[ "${1:-}" != "--dependencies-only" ]]; then
CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$ROOT/prototype/target}" "$CARGO" build --locked --manifest-path server/Cargo.toml -j2
fi
verify_all
if [[ "${1:-}" == "--dependencies-only" ]]; then
  echo "Dependencies ready; application build skipped."
else
  echo "Dependencies and PostgreSQL server built. Run bash scripts/bootstrap-postgres.sh, then bash scripts/test-postgres.sh. This is a loopback research build, not a public deployment."
fi
