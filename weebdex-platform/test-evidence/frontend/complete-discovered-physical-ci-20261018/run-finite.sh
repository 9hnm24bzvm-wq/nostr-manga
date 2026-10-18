#!/usr/bin/env bash
set -euo pipefail
R=/workspace/shared/nostr-manga-recovery-20261017
export WEEBDEX_BROWSER_DOM_NODE_MODULES="$R/account-library-worktree/weebdex-platform/.build/browser-dom-dependencies/node_modules"
export WEEBDEX_MIGRATION_TEST_NODE_MODULES="$WEEBDEX_BROWSER_DOM_NODE_MODULES"
export WEEBDEX_MIGRATION_TEST_NOBLE="$R/account-library-worktree/weebdex-platform/.build/local-library-noble-2.0.1/node_modules/@noble/hashes"
export WEEBDEX_NATIVE_SETTINGS_SCHEMA="$PWD/weebdex-platform/contracts/native-settings-template-v1.json"
export WEEBDEX_FRONTEND_ROOT="$R/replacement-frontend-build-worktree/weebdex-platform/.build/frontend-restore3"
export WEEBDEX_SELECT_SOURCE_NODE_MODULES="$WEEBDEX_FRONTEND_ROOT/node_modules"
export WEEBDEX_PLAYWRIGHT_CORE_ROOT=/opt/codex/runtimes/codex-primary-runtime/dependencies/node/node_modules/playwright-core
export WEEBDEX_READONLY_CACHE_ROOTS="[\"$WEEBDEX_FRONTEND_ROOT/node_modules\",\"$WEEBDEX_BROWSER_DOM_NODE_MODULES\",\"$R/account-library-worktree/weebdex-platform/.build/local-library-noble-2.0.1/node_modules\",\"$R/account-library-worktree/weebdex-platform/.build/local-signing-base-2.0.0/node_modules\",\"$R/frontend-build-worktree/weebdex-platform/.build/local-library-noble-2.0.1/node_modules\",\"$WEEBDEX_PLAYWRIGHT_CORE_ROOT\"]"
export WEEBDEX_READONLY_CACHE_ROOTS="$(python3 -c 'import json,os;print(json.dumps(sorted({os.path.realpath(p) for p in json.loads(os.environ["WEEBDEX_READONLY_CACHE_ROOTS"])})))')"
export WEEBDEX_READONLY_CACHE_ROOTS="$(python3 -c 'import json,os;roots=json.loads(os.environ["WEEBDEX_READONLY_CACHE_ROOTS"]);roots.append("/workspace/shared/nostr-manga-recovery-20261017/frontend-build-worktree/weebdex-platform/.build/frontend-restore3/node_modules");print(json.dumps(sorted({os.path.realpath(p) for p in roots})))')"
export WEEBDEX_READONLY_CACHE_ROOTS="$(python3 -c 'import json,sys;print(json.dumps(json.load(open(sys.argv[1]))["roots"]))' "$PWD/weebdex-platform/test-evidence/frontend/complete-discovered-physical-ci-20261018/physical-dependency-root-discovery.json")"
export WEEBDEX_READONLY_AUDIT_DIR="$PWD/weebdex-platform/test-evidence/frontend/complete-discovered-physical-ci-20261018/cache-audit/finite-final"
mkdir -p "$WEEBDEX_READONLY_AUDIT_DIR"
export NODE_OPTIONS="--require=$PWD/weebdex-platform/scripts/semantic-readonly-cache-guard.cjs"
mapfile -t tests < <(python3 -c "import json;print('\\n'.join(json.load(open('weebdex-platform/test-evidence/frontend/complete-discovered-physical-ci-20261018/finite-node-files.json'))))")
node --experimental-vm-modules --test --test-concurrency=1 "${tests[@]}"
