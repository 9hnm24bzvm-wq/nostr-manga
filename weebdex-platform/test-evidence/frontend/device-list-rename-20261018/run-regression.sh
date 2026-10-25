#!/usr/bin/env bash
# Run from repository root, reusing existing verified dependencies.
set -euo pipefail
R=/workspace/shared/nostr-manga-recovery-20261017
export WEEBDEX_FRONTEND_ROOT="$R/replacement-frontend-build-worktree/weebdex-platform/.build/frontend-restore3"
export WEEBDEX_BROWSER_DOM_NODE_MODULES="$R/account-library-worktree/weebdex-platform/.build/browser-dom-dependencies/node_modules"
export WEEBDEX_MIGRATION_TEST_NODE_MODULES="$WEEBDEX_BROWSER_DOM_NODE_MODULES"
export WEEBDEX_MIGRATION_TEST_NOBLE="$R/account-library-worktree/weebdex-platform/.build/local-library-noble-2.0.1/node_modules/@noble/hashes"
node --experimental-vm-modules --test --test-concurrency=2 weebdex-platform/tests/ci/test_device_list_rename_idb.mjs weebdex-platform/tests/ci/test_device_list_rename_dom.mjs weebdex-platform/tests/ci/test_uuid_library_orchestration.mjs weebdex-platform/tests/ci/test_library_identity_route_ownership.mjs weebdex-platform/tests/ci/test_library_management_lifetime.mjs weebdex-platform/tests/ci/public-canonical-ui-generation.test.mjs weebdex-platform/tests/ci/public-device-write-v2-idb.test.mjs weebdex-platform/tests/ci/public-uuid-publication.test.mjs weebdex-platform/tests/ci/test_public_device_publication_fence.mjs weebdex-platform/tests/ci/test_browser_signed_publication_clone.mjs weebdex-platform/tests/ci/test_browser_signed_publication_outcomes.mjs
WEEBDEX_TYPESCRIPT_ROOT="$WEEBDEX_FRONTEND_ROOT/node_modules/typescript" node weebdex-platform/test-evidence/frontend/device-list-rename-20261018/typecheck.cjs
