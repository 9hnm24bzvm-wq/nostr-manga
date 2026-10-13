#!/usr/bin/env bash
# Historical alias; there is one canonical current-runtime functional gate.
set -euo pipefail
cd "$(dirname "$0")/.."
exec bash scripts/test.sh "$@"
