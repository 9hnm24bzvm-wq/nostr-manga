#!/usr/bin/env bash
set -euo pipefail
cd manga-protocol-lab/bench/library-node
env -u MIXED_DIAGNOSTIC_TEST_DB -u MIXED_DIAGNOSTIC_OWNED_RUNTIME python3 -m unittest -v test_process_metrics test_diagnostic_process test_scheduler_probe test_pg_diagnostics
