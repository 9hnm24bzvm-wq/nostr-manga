#!/usr/bin/env bash
# Run from weebdex-platform only after the frontend build slot is released.
# Existing dependencies are referenced through symlinks, never copied/installed.
set -euo pipefail
R=/workspace/shared/nostr-manga-recovery-20261017
export WEEBDEX_SEMANTIC_NODE_MODULES="$R/replacement-frontend-build-worktree/weebdex-platform/.build/frontend-restore3/node_modules"
export WEEBDEX_READONLY_CACHE_ROOTS="[\"$WEEBDEX_SEMANTIC_NODE_MODULES\",\"$R/account-library-worktree/weebdex-platform/.build/browser-dom-dependencies/node_modules\",\"$R/account-library-worktree/weebdex-platform/.build/local-library-noble-2.0.1/node_modules\",\"$R/account-library-worktree/weebdex-platform/.build/local-signing-base-2.0.0/node_modules\"]"
export WEEBDEX_READONLY_CACHE_ROOTS="$(python3 -c 'import json,os;roots=json.loads(os.environ["WEEBDEX_READONLY_CACHE_ROOTS"]);roots.append("/workspace/shared/nostr-manga-recovery-20261017/frontend-build-worktree/weebdex-platform/.build/frontend-restore3/node_modules");print(json.dumps(sorted({os.path.realpath(p) for p in roots})))')"
export WEEBDEX_READONLY_AUDIT_DIR="$PWD/test-evidence/frontend/final-discovered-ci-20261018/cache-audit/strict-physical"
mkdir -p "$WEEBDEX_READONLY_AUDIT_DIR"
export NODE_OPTIONS="--max-old-space-size=2048 --require=$PWD/scripts/semantic-readonly-cache-guard.cjs"
for gate in 0 1; do
 available=$(df -B1 /workspace/shared | awk 'NR==2{print $4}')
 [ "$available" -ge 4563402752 ] || { echo "4 GiB +256MiB start guard: $available available"; exit 3; }
 WEEBDEX_SEMANTIC_OUTPUT="$PWD/.build/final-discovered-strict-physical-$gate-20261018-0321" PUBLIC_MANGADEX_CATALOGUE=$gate node scripts/check-composed-frontend.mjs > "test-evidence/frontend/final-discovered-ci-20261018/strict-$gate.log" 2>&1
 tail -3 "test-evidence/frontend/final-discovered-ci-20261018/strict-$gate.log"
done
