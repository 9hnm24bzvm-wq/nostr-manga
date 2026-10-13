# Staging-accounting scaling repair

## Measured problem

The former per-upload staging-quota query summed the caller's blob ownership records while excluding hosted references. It returned the correct number but repeatedly walked retained history. In the actual SQLite schema, a warm lookup over 200,000 retained-image metadata rows took a median 465 ms even though the staged result was zero. An additional owner-first index did not fix that workload because the existing owner still matched every retained row.

That offline fixture occupied 231,469,056 bytes of SQLite storage for 200,000 image metadata rows and 5,000 chapter rows. It did not contain the historical image files. It excludes additional manifest/series blob records, social/library scale, backup/WAL headroom and filesystem metadata. It is not a 100-TB capacity measurement.

## Implemented change

The database maintains a hosted-reference count per blob and staged-byte totals per owner in the same SQLite transactions as ownership/reference/state changes. The charged predicate is unchanged: a claimed blob counts toward staging only when no hosted event references it. Shared retained images stop counting once, and become staged again only after their last hosted reference goes away. Size corrections, duplicate claims, owner deletion, event deletion and rollback have explicit checks.

Quota lookup is now a keyed counter read rather than a scan of retained ownership history. A partial expiry index covers only unretained blobs for pressure/expiry selection. Physical deletion still also checks the original hosted-reference predicate. Global physical-byte initialization no longer submits a full history sum on every normal startup.

## Evidence

An actual authenticated HTTP upload comparison cloned the same 200,000-row metadata history for both builds. Seven small synthetic uploads per case measured a median 1,086.8 ms before and 2.90 ms after. The cached staged total exactly matched the original predicate at 119 bytes after those uploads. This is a warm, local, debug-build accounting-path measurement, not general image-ingestion throughput or WAN capacity. Historical media files were deliberately absent; only the newly uploaded test blobs were real.

The candidate's one-time migration took about 4.76 seconds in that fixture; a subsequent restart was about 0.10 seconds, at the harness's polling resolution. Larger migrations need their own time/free-space planning. Raw binary hashes, trials and scope are retained in results/accounting-http-benchmark.json and staging-query-scale.json.

The server passed the full deep suite with 75 Rust tests and 91 Python tests. Six accounting-model tests use the same trigger definitions as the server, including 5,000 seeded transactions with rollback and truth checks against the original predicate. A separate 40-cycle SIGKILL/restart test checked global/staging byte counters and retained-reference counts after every restart. These are not power-loss or arbitrary-corruption proofs. The older hour-long soak predates this repair.

## Migration and remaining limits

The derived-accounting migration is transactional and marked with an internal version. It reconstructs older databases, can rebuild missing derived structures, and rejects a newer unsupported accounting version. Keep an offline backup before upgrading; downgrading an older prototype binary onto this changed schema is not supported. The migration changes internal database layout, not signed event or manifest bytes.

Counters are derived state, not proof original files still exist. Integrity checks, preservation policy and repair remain separate. Counter correctness is established for tested application mutations and transaction rollback; arbitrary external edits can still require an accounting audit/rebuild. This is not an atomic multi-worker download-reservation system, nor does it remove the metadata mutex or the cost of validating unique images.

To reproduce the comparison, first run prototype/tests/staging_query_scale.py, then run accounting_http_benchmark.py with ACCOUNTING_BEFORE_BIN pointing to a built pre-counter checkpoint and ACCOUNTING_AFTER_BIN to the candidate. The archive does not silently substitute current code as the old baseline.
