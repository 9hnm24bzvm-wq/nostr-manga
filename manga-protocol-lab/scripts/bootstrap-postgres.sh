#!/usr/bin/env bash
# Local developer PostgreSQL toolchain. No system packages/services are modified.
set -euo pipefail
cd "$(dirname "$0")/.."
ROOT="$PWD"
mkdir -p vendor results
PG_VERSION=18.6
PG_SHA=555610c24d53e4316da5b7d3fc25c279d96856d5e0e23ee308c328c5fa881d9f
if [[ -x vendor/pgsql/bin/postgres ]] && vendor/pgsql/bin/postgres --version | grep -q "$PG_VERSION"; then
  vendor/pgsql/bin/postgres --version
  exit 0
fi
# Use installed build tools where possible. This optional local fallback is only
# for the Debian13 x86_64 environment in which it was validated.
if ! command -v bison >/dev/null || ! command -v flex >/dev/null || ! command -v m4 >/dev/null; then
  . /etc/os-release
  [[ "$ID" == debian && "$VERSION_ID" == 13 && "$(uname -m)" == x86_64 ]] || { echo 'Install bison, flex and m4 through your normal package manager first.' >&2; exit 1; }
  mkdir -p vendor/pg-build-debs vendor/pg-build-tools
  fetch_deb(){
    local package="$1" path="$2" sum="$3" target="vendor/pg-build-debs/$1.deb"
    [[ -f "$target" ]] || curl -fLsS --max-time 120 "https://deb.debian.org/debian/pool/main/$path" -o "$target"
    printf '%s  %s\n' "$sum" "$target" | sha256sum -c -
    dpkg-deb -x "$target" vendor/pg-build-tools
  }
  fetch_deb bison 'b/bison/bison_3.8.2+dfsg-1+b2_amd64.deb' ce6eb1f7aad18850e32d223a68f54bcbe756d74f138c8ec5460ad9d220f4d335
  fetch_deb flex 'f/flex/flex_2.6.4-8.2+b4_amd64.deb' 75f963765232c356d0a2fb70e780dfa9de2425aca036ec60eefb7fc98a2c905b
  fetch_deb m4 'm/m4/m4_1.4.19-8_amd64.deb' 221b3e224708c1da2a1a88b6824380593579b2254ae7d914afa14cbd5d343334
  export PATH="$ROOT/vendor/pg-build-tools/usr/bin:$PATH"
  export BISON_PKGDATADIR="$ROOT/vendor/pg-build-tools/usr/share/bison" M4="$ROOT/vendor/pg-build-tools/usr/bin/m4"
fi
[[ -f "vendor/postgresql-$PG_VERSION.tar.bz2" ]] || curl -fLsS --max-time 180 "https://ftp.postgresql.org/pub/source/v$PG_VERSION/postgresql-$PG_VERSION.tar.bz2" -o "vendor/postgresql-$PG_VERSION.tar.bz2"
printf '%s  %s\n' "$PG_SHA" "vendor/postgresql-$PG_VERSION.tar.bz2" | sha256sum -c -
[[ -d "vendor/postgresql-$PG_VERSION" ]] || tar -xjf "vendor/postgresql-$PG_VERSION.tar.bz2" -C vendor
(cd "vendor/postgresql-$PG_VERSION" && ./configure --prefix="$ROOT/vendor/pgsql" --without-icu --without-readline --with-ssl=openssl && make -j"${BUILD_JOBS:-2}" && make install) >results/postgres-build.log 2>&1
vendor/pgsql/bin/postgres --version
