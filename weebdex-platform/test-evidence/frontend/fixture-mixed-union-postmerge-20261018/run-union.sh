#!/usr/bin/env bash
set -euo pipefail
ROOT="$PWD"
export PYTHONDONTWRITEBYTECODE=1
export PYTHONPATH="/workspace/shared/nostr-manga-recovery-20261018/finite-fixture-worktree/.build/fixture-python-recovery/owned/site-packages:$ROOT/manga-protocol-lab:$ROOT/manga-protocol-lab/bench/library-node"
export TMPDIR="$ROOT/.build/fixture-mixed-union/tmp"
export NODE_RESULTS="$ROOT/.build/fixture-mixed-union/results"
export MIXED_JPEG_SOURCE="$ROOT/manga-protocol-lab/server/tests/fixtures/real-images/met-57682.jpg"
cd manga-protocol-lab
python3 -B -m unittest -v test_mixed test_diagnostic_observer test_image_shards test_process_metrics test_diagnostic_process
