#!/bin/sh
# Portable prospective finite recipe, not the command record of a past run.
# Run from the repository root only after explicit runtime/resource admission.
# Dependency roots are caller-supplied, reviewed, already provisioned directories.
# Do not install dependencies, dispatch browser/server/database work, or infer a pass.
set -eu
: "${WEEBDEX_FRONTEND_ROOT:?Set the reviewed existing frontend root}"
: "${WEEBDEX_BROWSER_DOM_NODE_MODULES:?Set the reviewed existing DOM dependency root}"
export WEEBDEX_FRONTEND_ROOT WEEBDEX_BROWSER_DOM_NODE_MODULES
export NODE_OPTIONS=--max-old-space-size=512
timeout 120s node --experimental-vm-modules --test --test-concurrency=1 \
 weebdex-platform/tests/ci/test_generic_native_reports.mjs \
 weebdex-platform/tests/ci/test_native_user_reports.mjs \
 weebdex-platform/tests/ci/test_signed_native_discussion.mjs \
 weebdex-platform/tests/ci/test_generic_native_report_contract.mjs \
 weebdex-platform/tests/ci/test_native_discussion_dom.mjs \
 weebdex-platform/tests/ci/test_native_user_discussion_dom.mjs \
 weebdex-platform/tests/ci/test_public_upload_request_recovery.mjs \
 weebdex-platform/tests/ci/test_signed_native_user_discussion.mjs \
 weebdex-platform/tests/ci/test_native_user_navigation.mjs \
 weebdex-platform/tests/ci/test_native_user_discussion_reader.mjs
