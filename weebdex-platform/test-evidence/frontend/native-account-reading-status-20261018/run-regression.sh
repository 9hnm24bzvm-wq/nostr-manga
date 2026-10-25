#!/usr/bin/env bash
set -euo pipefail
: "${WEEBDEX_FRONTEND_ROOT:?Use the existing verified Svelte5.55.1 frontend stage}"
: "${WEEBDEX_BROWSER_DOM_NODE_MODULES:?Use the existing pinned jsdom27.0.1 dependency root}"
node --experimental-vm-modules --test \
 weebdex-platform/tests/ci/test_account_reading_status_client.mjs \
 weebdex-platform/tests/ci/test_account_reading_status_dom.mjs \
 weebdex-platform/tests/ci/test_account_follow_ui.mjs \
 weebdex-platform/tests/ci/test_account_follow_client.mjs
