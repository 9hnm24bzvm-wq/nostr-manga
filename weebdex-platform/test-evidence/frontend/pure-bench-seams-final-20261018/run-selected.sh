#!/usr/bin/env bash
set -euo pipefail
ROOT="$PWD"
export PYTHONDONTWRITEBYTECODE=1
export PYTHONPATH="$ROOT/weebdex-platform/.build/python-migration-approved-20261018-0556/site-packages:$ROOT/manga-protocol-lab:$ROOT/manga-protocol-lab/bench/library-node"
export NODE_RESULTS="$ROOT/weebdex-platform/.build/pure-bench-seams-final-20261018/results"
export TMPDIR="$ROOT/weebdex-platform/.build/pure-bench-seams-final-20261018/tmp"
cd manga-protocol-lab/bench/library-node
python3 -m unittest -v test_mixed_fixture_config test_wire_security
