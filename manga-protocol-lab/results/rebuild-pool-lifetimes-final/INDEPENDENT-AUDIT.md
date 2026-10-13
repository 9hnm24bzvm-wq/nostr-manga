# Independent terminal acceptance audit

Audited 2026-10-12 02:05 UTC. **The exact 392-input pool-lifetime build passes the full functional acceptance gate.** This reviewer inspected the source changes and independently audited the operator's completed execution; no duplicate application gate was run.

Acceptance SHA256: `656aa030c83f239bde726813cefb6f6c49ffe9d3cd55e18ca1be38f9779cd6d5`. Node SHA256: `4c499f724af4db1843f54c51e47746114f777de15b446979a3e8e8a0e7a3e23a`. Terminal time: 2026-10-12 02:00:32.753093 UTC, command exit 0.

## Independent checks

- Recomputed the actual node binary, all 392 current source/build inputs, all five runner identities and every evidence hash referenced by the acceptance. All match. Before/after source maps are equal, with the exact previously reviewed three production edits and one new fixture relative to accepted 1c3.
- Parsed the raw canonical log: 58 Python, 6 packaging, 91 browser-model and 74 Rust-module passes, with two intentional module ignores. All 89 integration executables and their per-executable result lines match the declared inventory and sum to 108 passing cases, without integration failures or ignores.
- The only integration-file exclusion is the canonical runner's existing optional `audit_gaps` characterization. The new two-case `http_pool_lifetimes` fixture ran and passed. The two Rust module ignores keep their explicit meanings: crash-parent child harness and separately invoked large codec fixture.
- Recompared copied runners to the original scripts: only relocated root anchors, four Cargo `-j2` to `-j1` substitutions and delegation change. Python/package/browser commands and test/database settings remain intact. Strict all-target release Clippy with warnings denied is present in the runner; its successful log duration is 43.79 seconds. Formatting passed before Rust tests.
- The copied PostgreSQL multiprocess result exactly equals both the JSON embedded in the raw log and the acceptance: 12 writers/one unique insert, four workers/32 unique jobs, immediate database stop/restart, fail-closed outage, recovered pool and a fresh-process read. The terminal log shows fsync and synchronous commit both on.
- The owned guard reports the leader reaped, no remaining owned member, no stop action, resource abort, monitoring/cleanup error, termination signal or input drift. The separate owned-runtime receipt reports no PostgreSQL pidfile under the two exact owned fixture roots. This does not infer control over any other execution namespace.

Machine-readable recomputed evidence is in `INDEPENDENT-AUDIT.json`, SHA256 `28341472d10cdbbae5b55086a4e049554afa0c5dde20b4852190362009fc9314`.

## Correctness and qualification boundaries

The focused baseline failures and identical candidate fixture pass remain independently supported by `../rebuild-pool-lifetimes/INDEPENDENT-ACCEPTANCE-AUDIT.md`. Empty signed audit and both nonempty cover/author lists are genuine public one-connection regressions. Later audit controls unreached by baseline are candidate-only observations. The postcommit Moka release has source-level lifetime evidence and public regressions, without a fabricated causal timing red or strict red-before-implementation claim.

No pool/queue tuning, SQL change, retry change or public-export transaction/filesystem fence change is part of this patch. Audit was disabled and compatibility collections were absent in the preceding failed mixed trial; these corrections do not explain that wave. No new mixed-load run, sustained-capacity pass, rendered-browser qualification or matching recovery delivery follows from the functional gate. Prior 1c3 evidence and failed attempts remain preserved.
