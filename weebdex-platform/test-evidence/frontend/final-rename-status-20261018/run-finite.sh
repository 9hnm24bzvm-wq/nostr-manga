#!/usr/bin/env bash
# Run from the repository root. No installs or dependency/cache copies.
set -euo pipefail
R=/workspace/shared/nostr-manga-recovery-20261017
export WEEBDEX_BROWSER_DOM_NODE_MODULES="$R/account-library-worktree/weebdex-platform/.build/browser-dom-dependencies/node_modules"
export WEEBDEX_MIGRATION_TEST_NODE_MODULES="$WEEBDEX_BROWSER_DOM_NODE_MODULES"
export WEEBDEX_MIGRATION_TEST_NOBLE="$R/account-library-worktree/weebdex-platform/.build/local-library-noble-2.0.1/node_modules/@noble/hashes"
export WEEBDEX_NATIVE_SETTINGS_SCHEMA="$PWD/weebdex-platform/contracts/native-settings-template-v1.json"
export WEEBDEX_FRONTEND_ROOT="$R/replacement-frontend-build-worktree/weebdex-platform/.build/frontend-restore3"
export WEEBDEX_READONLY_CACHE_ROOTS="[\"$WEEBDEX_FRONTEND_ROOT/node_modules\",\"$WEEBDEX_BROWSER_DOM_NODE_MODULES\",\"$R/account-library-worktree/weebdex-platform/.build/local-library-noble-2.0.1/node_modules\",\"$R/account-library-worktree/weebdex-platform/.build/local-signing-base-2.0.0/node_modules\"]"
export WEEBDEX_READONLY_AUDIT_DIR="$PWD/weebdex-platform/test-evidence/frontend/final-rename-status-20261018/cache-audit"
export NODE_OPTIONS="--require=$PWD/weebdex-platform/scripts/semantic-readonly-cache-guard.cjs"
mapfile -t tests < <(python3 -c "import json;print('\\n'.join(json.load(open('weebdex-platform/test-evidence/frontend/final-rename-status-20261018/finite-test-files.json'))))")
node --experimental-vm-modules --test --test-concurrency=4 "${tests[@]}"
