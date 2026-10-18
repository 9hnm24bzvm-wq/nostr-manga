#!/usr/bin/env bash
set -euo pipefail
: "${WEEBDEX_FRONTEND_ROOT:?Use the existing verified Svelte5.55.1 stage}"
: "${WEEBDEX_BROWSER_DOM_NODE_MODULES:?Use the existing pinned JSDOM27.0.1 root}"
node --experimental-vm-modules --test \
 weebdex-platform/tests/ci/test_public_popularity.mjs \
 weebdex-platform/tests/ci/test_popularity_analytics_callers.mjs \
 weebdex-platform/tests/ci/test_popularity_dom.mjs \
 weebdex-platform/tests/ci/public-api-recovery.test.mjs \
 weebdex-platform/tests/ci/test_account_title_composition.mjs \
 weebdex-platform/tests/ci/test_public_rating_build_seam.mjs
