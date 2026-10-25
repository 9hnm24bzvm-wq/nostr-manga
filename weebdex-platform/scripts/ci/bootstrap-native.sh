#!/usr/bin/env bash
# The same reviewed native pins as scripts/bootstrap.sh; no unrelated nginx,
# application, original test suite or system service is started here.
set -euo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
cd "$root/manga-protocol-lab"
export RUSTUP_TOOLCHAIN=1.90.0
rustup toolchain install 1.90.0 --profile minimal --no-self-update
cmake="$root/weebdex-platform/.build/hosted-python/bin/cmake"
checkout() {
  local url="$1" directory="$2" revision="$3"
  [[ ! -e "$directory" ]] || { echo 'Refusing a previously populated native stage' >&2; exit 1; }
  git clone --no-checkout "$url" "$directory"
  git -C "$directory" checkout --detach "$revision"
  [[ "$(git -C "$directory" rev-parse HEAD)" == "$revision" ]]
  git -C "$directory" diff --quiet HEAD
}
mkdir -p vendor
checkout https://github.com/hoytech/strfry.git vendor/strfry 4cd3cf64850caf47dda46c2a2abbbf3525a64d10
git -C vendor/strfry submodule update --init --recursive
checkout https://github.com/bitcoin-core/secp256k1.git vendor/secp256k1 0cdc758a56360bf58a851fe91085a327ec97685a
checkout https://github.com/LMDB/lmdb.git vendor/lmdb 3a29a24777c82a0165de813ae696a5068b5add30
checkout https://github.com/google/flatbuffers.git vendor/flatbuffers 1c514626e83c20fffa8557e75641848e1e15cd5e
"$cmake" -S vendor/secp256k1 -B vendor/secp256k1/build -DBUILD_SHARED_LIBS=OFF -DSECP256K1_ENABLE_MODULE_SCHNORRSIG=ON -DSECP256K1_BUILD_TESTS=OFF -DSECP256K1_BUILD_BENCHMARK=OFF
"$cmake" --build vendor/secp256k1/build -j2
make -C vendor/lmdb/libraries/liblmdb -j2 liblmdb.a
"$cmake" -S vendor/flatbuffers -B vendor/flatbuffers/build -DFLATBUFFERS_BUILD_TESTS=OFF
"$cmake" --build vendor/flatbuffers/build --target flatc -j2
export APPS='dbutils relay mesh'
(cd vendor/strfry && mkdir -p build && golpe/gen-config.pl && golpe/gen-main.cpp.pl)
PATH="$PWD/vendor/flatbuffers/build:$PATH" make -C vendor/strfry -j2 NPROC=2 OPT=-O1 \
  XCXXFLAGS="-I$PWD/vendor/secp256k1/include -I$PWD/vendor/lmdb/libraries/liblmdb -I$PWD/vendor/flatbuffers/include" \
  XLDFLAGS="-L$PWD/vendor/secp256k1/build/lib -L$PWD/vendor/lmdb/libraries/liblmdb"
bash scripts/bootstrap-postgres.sh
test -x vendor/strfry/strfry
vendor/pgsql/bin/postgres --version | grep -F 18.6
