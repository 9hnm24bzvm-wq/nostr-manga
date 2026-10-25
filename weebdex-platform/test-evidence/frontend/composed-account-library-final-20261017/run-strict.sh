#!/usr/bin/env bash
# Run from weebdex-platform only after the frontend build slot is released.
# Existing dependencies are referenced through symlinks, never copied/installed.
set -euo pipefail
R=/workspace/shared/nostr-manga-recovery-20261017
export WEEBDEX_SEMANTIC_NODE_MODULES="$R/replacement-frontend-build-worktree/weebdex-platform/.build/frontend-restore3/node_modules"
export WEEBDEX_READONLY_CACHE_ROOTS="[\"$WEEBDEX_SEMANTIC_NODE_MODULES\",\"$R/account-library-worktree/weebdex-platform/.build/browser-dom-dependencies/node_modules\",\"$R/account-library-worktree/weebdex-platform/.build/local-library-noble-2.0.1/node_modules\",\"$R/account-library-worktree/weebdex-platform/.build/local-signing-base-2.0.0/node_modules\"]"
export WEEBDEX_READONLY_AUDIT_DIR="$PWD/test-evidence/frontend/composed-account-library-final-20261017/cache-audit"
export NODE_OPTIONS="--require=$PWD/scripts/semantic-readonly-cache-guard.cjs"
for gate in 0 1; do
 available=$(df -B1 /workspace/shared | awk 'NR==2{print $4}')
 [ "$available" -ge 4399824896 ] || { echo "4 GiB reserve guard: $available available"; exit 3; }
 WEEBDEX_SEMANTIC_OUTPUT="$PWD/.build/composed-account-final-strict-$gate-20261017-2300" PUBLIC_MANGADEX_CATALOGUE=$gate node scripts/check-composed-frontend.mjs > "test-evidence/frontend/composed-account-library-final-20261017/strict-$gate.log" 2>&1
 tail -3 "test-evidence/frontend/composed-account-library-final-20261017/strict-$gate.log"
done
