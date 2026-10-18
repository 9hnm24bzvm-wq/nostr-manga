#!/usr/bin/env bash
set -euo pipefail
ROOT="$PWD"
export PYTHONDONTWRITEBYTECODE=1
export PYTHONPATH="/workspace/shared/nostr-manga-recovery-20261018/finite-fixture-worktree/.build/fixture-python-recovery/owned/site-packages:$ROOT/manga-protocol-lab:$ROOT/manga-protocol-lab/bench/library-node"
export TMPDIR="$ROOT/.build/mixed-import-followon/tmp"
export NODE_RESULTS="$ROOT/.build/mixed-import-followon/results"
unset MIXED_DIAGNOSTIC_TEST_DB MIXED_DIAGNOSTIC_OWNED_RUNTIME
cd manga-protocol-lab
python3 -B -m unittest -v test_disk_guard test_head_verification
