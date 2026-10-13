#!/usr/bin/env bash
# No automatic migrations, key creation, dependency downloads, or public deploy.
set -euo pipefail
root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
action="${1:-serve}"
case "$action" in check-config|verify|migrate|serve) ;; *) printf 'Usage: %s [check-config|verify|migrate|serve]\n' "$0" >&2; exit 64 ;; esac
[[ $# -le 1 ]] || { printf 'Unexpected arguments\n' >&2; exit 64; }
: "${PLATFORM_CONFIG:?Set PLATFORM_CONFIG to the provisioned runtime JSON file}"
binary="${PLATFORM_BIN:-$root/artifacts/backend/weebdex-platform}"
[[ -x "$binary" ]] || { printf 'Platform binary unavailable; build the locked release first\n' >&2; exit 69; }
if [[ "$action" != check-config && -z "${PLATFORM_DATABASE_URL:-${MANGA_DATABASE_URL:-}}" ]]; then
  printf 'Set PLATFORM_DATABASE_URL through the runtime secret environment\n' >&2; exit 78
fi
cd "$root"
exec "$binary" "$action"
