#!/usr/bin/env bash
# Executed only in the reviewed disposable standard GitHub runner.
set -euo pipefail
repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
platform="$repo/weebdex-platform"
[[ "${GITHUB_ACTIONS:-}" == true && "${RUNNER_OS:-}" == Linux ]] || { echo 'This installer is GitHub-hosted Linux only' >&2; exit 69; }
cd "$repo"
case "${BROWSER_RUNTIME_PROFILE:-bundled}" in bundled|cft-157) ;; *) echo 'Unreviewed browser runtime profile' >&2; exit 69 ;; esac
python3 -c 'import platform,sys; assert sys.version_info[:2] == (3,12) and platform.machine() == "x86_64", "Exact hashed Python wheel platform required"'
sudo apt-get update
sudo apt-get install --no-install-recommends -y build-essential libssl-dev zlib1g-dev libzstd-dev pkg-config bison flex m4 python3-venv
python3 -m venv "$platform/.build/hosted-python"
"$platform/.build/hosted-python/bin/python" -m pip install --disable-pip-version-check --timeout 30 --retries 2 --only-binary=:all: --require-hashes --no-deps -r "$platform/scripts/ci/requirements-hosted.txt"
npm ci --prefix .ci/browser --ignore-scripts --omit=optional --no-audit --no-fund --registry=https://registry.npmjs.org --cache "$platform/.cache/hosted-npm"
BROWSER_RUNTIME_PROFILE=bundled node "$platform/scripts/ci/browser-runtime.mjs" --verify-hosted
if [[ "${BROWSER_RUNTIME_PROFILE:-bundled}" == cft-157 ]]; then
  # Existing qualified driver lock and acquisition code; no lifecycle scripts.
  npm ci --prefix .ci/native-upload-counterfactual --ignore-scripts --omit=optional --no-audit --no-fund --registry=https://registry.npmjs.org --cache "$platform/.cache/hosted-npm"
  python3 -B "$platform/scripts/ci/verify-chrome157-runtime.py" --driver-only
  node .ci/native-upload-counterfactual/node_modules/playwright/cli.js install-deps chromium
  node "$platform/scripts/ci/acquire-native-upload-browsers.mjs" "$platform/.build/hosted-browser/cft-acquisition"
  python3 -B "$platform/scripts/ci/verify-chrome157-runtime.py" --archives-only
  python3 -B "$platform/scripts/ci/extract-native-upload-browsers.py" "$platform/.build/hosted-browser/cft-acquisition"
else
  # This exact official Playwright version selects its bundled Chromium revision.
  node .ci/browser/node_modules/playwright/cli.js install --with-deps chromium
fi
"$platform/.build/hosted-python/bin/python" "$platform/scripts/ci/prepare-hosted-browser.py"
