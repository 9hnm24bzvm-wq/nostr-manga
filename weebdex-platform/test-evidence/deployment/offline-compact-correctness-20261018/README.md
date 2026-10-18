# Offline original compaction correctness, 2026-10-18

## Result and exact scope

The unchanged original `manga-protocol-lab/bench/library-node/test_compact.py`
executed once, with Python assertions enabled. Both synthetic histories passed:

| Completed chapters | Decoded records before | After compaction | Five exact updates |
| ---: | ---: | ---: | :--- |
| 30,000 | 31,712 | 469 | identical |
| 500,000 | 511,202 | 1,843 | identical |

The test also asserts unchanged canonical root bytes after compaction. Its
`changed()` comparison covers the exact chunk and root byte pairs for each of
five successive updates. This is in-memory codec/retention correctness. It does
not measure server throughput, SQL, WAN, real-node recovery, 500k service capacity,
or production recovery.

Run time was 5.209 seconds; sampled total owned RSS peaked at 312,688,640 bytes.
Sampled minimum host available memory was 8,963,936,256 bytes and minimum
workspace free space was 30,697,668,608 bytes. During work the named output
allocation peaked at 36,864 bytes. All retained inputs/results together are
well below the 64 MiB output cap. The monitor sampled every 50 ms; these are
observations from this correctness run, not a performance qualification.

One owned child birth was verified and reaped. Terminal cleanup was normal,
with no remaining or unconfirmed births, no forced signal and no error.
The sole runtime lease was released before the account worker's next run.

## Source and dependencies

The approved unchanged source was restored commit
`be283b664f699e8c7175f6eac0200cd22e212c8a`, tree
`59cbf855807428690c975bbbba31705aca03464d`. Before admitting correctness work,
the driver checked actual HEAD/tree and compared all 13 selected source inputs
(including every local `src/*.py`, the test, actual benchmark library and guard)
against the immutable Git blobs. It rehashed them after the run. The six actual
project imports are in `import-closure.json` and match that same pre-run ledger.
No application source was edited for this run.

The existing separately approved recovered target supplied cbor2 5.6.5 and
coincurve 21.0.0. Exact regular-file membership, absence of symlinks/special files,
and all 41 file sizes/hashes were checked before and after. The expected
inventory is anchored to reviewed SHA256
`7e7c5c88bc831111cc7583dbc2d6706ca133a9cff2d427dfb8b54f1085c2dcca`
from the fixture owner's `66bf5bf` source/evidence handoff. There was no install.
See `../../frontend/finite-fixture-portability-20261018` in the composed source tree
for the recovery provenance (that path is under `test-evidence`, not this
`deployment` directory).

Host aiohttp was 3.14.4, while the historical requirements pin is 3.13.5.
Pillow was 12.3.0, and Python was 3.12.14. Full historical requirements parity is
explicitly false. No HTTP path is exercised by this test.

## Guard and retained commands

The retained driver/launcher are newly prepared verification work, not lost
historical runtime evidence. Their absolute paths describe this restored
workspace. The executed command was:

```sh
PYTHONDONTWRITEBYTECODE=1 python3 -B \
  /workspace/shared/nostr-manga-recovery-20261018/offline-compact-evidence-20261018T1534/run-owned.py
```

Caps were 120 seconds total (90 seconds work plus at most 25 seconds of owned
cleanup), 1.5 GiB total owned RSS, at least 2 GiB host available memory, 64 MiB
outputs, and a 4 GiB workspace floor with start/live reserves. A conservative
1,181,159,424-byte memory forecast had to fit before launch.

The child waits on a private admission pipe until its parent captures the exact
PID/start tick/group/session. EOF aborts unadmitted work. Existing pidfd/birth-safe
cleanup is reused. A Python audit hook refuses attempted socket connect/bind,
subprocess and SQLite creation during the unchanged test; no such event occurred.
This is an audit-level boundary, not an OS network sandbox claim. No server,
PostgreSQL, install, deletion or external request was started.

During source-only harness preparation a missing space in one conditional was
caught by `compile()` and corrected before any test execution. Independent
pre-run review then required the admission gate, immutable source parity and
exact dependency inventory binding. Only the final reviewed driver/launcher
were executed. Their exact hashes are recorded in `result.json`.

## Evidence handling

Raw output is a synthetic two-row JSON result; stderr is empty. No config,
credential, private key, runtime database, archive or dump is included. Each
committed file except the manifest itself is covered by `manifest.json`.
Original retained raw paths remain outside Git. Earlier recovery and PG17
mechanics receipts retain their original, separate qualification scope.
