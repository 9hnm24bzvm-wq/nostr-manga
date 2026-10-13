#!/usr/bin/env bash
# Executed only in the reviewed disposable standard GitHub runner.
set -euo pipefail
repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
platform="$repo/weebdex-platform"
[[ "${GITHUB_ACTIONS:-}" == true && "${RUNNER_OS:-}" == Linux ]] || { echo 'This installer is GitHub-hosted Linux only' >&2; exit 69; }
cd "$repo"
python3 -c 'import platform,sys; assert sys.version_info[:2] == (3,12) and platform.machine() == "x86_64", "Exact hashed Python wheel platform required"'
sudo apt-get update
sudo apt-get install --no-install-recommends -y build-essential libssl-dev zlib1g-dev libzstd-dev pkg-config bison flex m4 python3-venv
python3 -m venv "$platform/.build/hosted-python"
"$platform/.build/hosted-python/bin/python" -m pip install --disable-pip-version-check --timeout 30 --retries 2 --only-binary=:all: --require-hashes --no-deps -r "$platform/scripts/ci/requirements-hosted.txt"
npm ci --prefix .ci/browser --ignore-scripts --omit=optional --no-audit --no-fund --registry=https://registry.npmjs.org --cache "$platform/.cache/hosted-npm"
node "$platform/scripts/ci/browser-runtime.mjs" --verify-hosted
# This exact official Playwright version selects its bundled Chromium revision.
node .ci/browser/node_modules/playwright/cli.js install --with-deps chromium
"$platform/.build/hosted-python/bin/python" "$platform/scripts/ci/prepare-hosted-browser.py"
