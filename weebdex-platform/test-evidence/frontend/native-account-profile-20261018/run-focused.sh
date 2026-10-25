#!/usr/bin/env bash
set -euo pipefail
: "${WEEBDEX_FRONTEND_ROOT:?existing verified frontend stage}"
: "${WEEBDEX_BROWSER_DOM_NODE_MODULES:?existing pinned jsdom27.0.1 root}"
export WEEBDEX_NATIVE_SETTINGS_SCHEMA="$PWD/weebdex-platform/contracts/native-settings-template-v1.json"
node --experimental-vm-modules --test --test-concurrency=1 \
 weebdex-platform/tests/ci/test_account_profile_client.mjs \
 weebdex-platform/tests/ci/test_account_profile_dom.mjs \
 weebdex-platform/tests/ci/public-api-recovery.test.mjs \
 weebdex-platform/tests/ci/test_account_title_composition.mjs \
 weebdex-platform/tests/ci/test_public_rating_build_seam.mjs \
 weebdex-platform/tests/ci/test_account_following_reader.mjs \
 weebdex-platform/tests/ci/test_account_following_ui.mjs \
 weebdex-platform/tests/ci/test_user_directory.mjs \
 weebdex-platform/tests/ci/test_account_settings_client.mjs
