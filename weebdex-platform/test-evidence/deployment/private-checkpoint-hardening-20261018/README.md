# Private checkpoint hardening finite evidence

Audit source is restored be283; qualified source is d5ed829. Two new refusal
regressions intentionally failed against the original script: both input runtime
configurations could be copied from storage roots. The corrected script rejects
that overlap before tool/output creation. A same-size original change during the
synthetic dump also refuses completion.

Final execution passes 24 cases:21 filesystem/command-boundary tests and 3 wrapper/
source lifecycle contracts. PostgreSQL commands and the platform wrapper target
are explicit stand-ins; the system libpq parser handles only synthetic DSNs
without connecting. Actual-PG diagnostic execution is not claimed: five finite
methods pass and twelve real PostgreSQL 18 methods explicitly skip. One temporary
boundary-fixture quoting SyntaxError was repaired before the final run; it was
not a product-script failure.

Commands (TMPDIR was a retained private workspace directory):

- `python3 tests/ci/test_private_checkpoint.py -v`
- `env -u MIXED_DIAGNOSTIC_TEST_DB -u MIXED_DIAGNOSTIC_OWNED_RUNTIME PYTHONPATH=bench/library-node:. python3 -m unittest discover -s bench/library-node -p test_pg_diagnostics.py -v`
- `bash -n scripts/run-platform.sh` and `git diff --check`

The earlier actual nginx 8-case source association matches the current unchanged
supervisor, but no nginx/application lifecycle was rerun here. The exact full-app
restore gap and proposed PG18/30k/500k budgets are documented separately; no heavy
work is authorized by the plan. All generated fixture/config/dump stand-ins stay
outside Git, retained for inspection. Public files contain only synthetic test
names, source hashes, counts and boundary outcome logs. No actual database,
account records, signing key, connection values or runtime bundle is included.
