# Restored runtime and real-service smoke,2026-10-12

**Result: runtime restoration and bounded functional smoke passed.** No mixed-load test or Rust/application rebuild ran.

Accepted node SHA256: `4c499f724af4db1843f54c51e47746114f777de15b446979a3e8e8a0e7a3e23a`. The restored checkpoint contains387 of392 historical canonical inputs; five historical diagnostic files remain missing. Two approved packaging files changed separately and were explicitly excluded from these guards. Every other protected source and all accepted binaries were unchanged.

## Runtime restoration

|Stage|Elapsed seconds|Minimum free bytes|Minimum available-memory bytes|Result|
|---|---:|---:|---:|---|
|python-runtime|13.837|31613394944|8845787136|passed|
|postgres-runtime|149.323|31310843904|8497020928|passed|
|native-runtime|299.342|30944231424|7187582976|passed|

Python direct pins: aiohttp3.13.5,cbor2 5.6.5,coincurve21.0.0,Pillow12.3.0. PG18.6,nginx1.30.5 and pinned strfry/secp256k1/LMDB/FlatBuffers were restored from the declared official endpoints. All downloaded fixed archive checksums and Git revisions matched. Version/ELF shared-library checks succeeded. Python transitive versions are recorded and are not claimed byte-identical to the lost vendor tree.

Each build used at most two native compile jobs and a60-minute stage deadline. The copied qualified guard used24GiB sampled disk stop,1GiB available-memory stop and32MiB captured-console cap. PG also wrote its ordinary build log outside the console cap; total filesystem growth remained guarded. All three command exits were0, no abort or cleanup error occurred, and owned-process remainders were empty.

## Actual process/service checks

|Check|Elapsed seconds|Observed result|
|---|---:|---|
|ownership-clean|0.074|exit0; empty owned remainder|
|ownership-setsid|2.395|expected guard failure; anchored setsid child stopped and reaped; empty remainder|
|demo|2.870|exit0; empty owned remainder|
|nginx|1.794|exit0; empty owned remainder|

The run-specific180-second smoke guard differs from the prior guard only in the deadline and removal of its session filter. Ownership still requires the exact leader PID/start identity or the dedicated subreaper ancestry; signals use identity-checked pidfds. The original before-source,diff and both actual CLI seams are preserved. The negative test intentionally left a child in a new session, received the expected failed guard verdict, then confirmed termination/adoption/reaping. It is not an application failure.

The unchanged runtime/run-demo.sh --check was invoked through a fresh symlink-only layout to verified binaries. Its existing synthetic credentials were confined to its ephemeral local runtime and were not retained in evidence. The demo verified:
- signed series and chapter publication
- chapter discovery
- bilaterally approved synchronization
- exact original bytes self-hosted by both nodes
- public reading progress saved to both instances
- complete library recovered from second instance
- reader progress views and authenticated identity on both instances
- administrator-managed group profile signed by group identity
- closed signup on first node and scoped reader self-registration on second node

Two unchanged real-nginx tests verified prepare/activate/suspend/stop, exact206 range bytes, an open200 reader after unlink, bounded reader drain and404 in the replacement namespace. The nginx fixture uses synthetic original bytes; the two-node demo uses its generated PNG. This is not the full public-domain mixed-load corpus.

The same-launch guards recorded normal command completion and no remaining owned descendants. The demo guard additionally reaped its daemonized PostgreSQL process with wait_status0; it did not need to signal any service. The successful demo and operator cases cleaned their own ephemeral data, and the dedicated temporary root was empty afterward. This proof does not depend on a separate-exec ps view.

Fresh free space after acceptance: 30943854592bytes. Minimum smoke free space: 30874517504bytes. Minimum sampled available memory: 8767459328bytes. Per-process CPU/RSS/I/O was not collected for these short functional checks; the global resource samples are safeguards, not load attribution.

## Remaining scope

The accepted application is usable with the restored runtime. Next priorities are independently reviewed browser qualification and reconstruction/review of any needed run-specific load ownership/diagnostic helpers. The lost fast-PG helpers and raw/runtime history are not restored. No new mixed workload, source performance fix, automatic retry, full compiler rebuild or new whole-project canonical gate follows from this smoke. Rust/npm toolchain restoration remains deferred; the existing browser/public assets are directly usable.

Evidence: ../restored-runtime-20261012/PLAN.md and runtime-acceptance.json; this directory acceptance.json,guard.diff, ownership seam receipts,demo-report.json and exact nginx test log.
