# Original mixed read helper: controlled correctness, 2026-10-18

## Actual result

The one unchanged method
`LibraryReadHttp.test_large_and_extreme_selective_restore_rejects_corrupt_200`
from `manga-protocol-lab/bench/library-node/test_mixed_reads.py` passed:
**1 test, 0 failures, 0 errors, 0 skips**. Unittest took 1.975 seconds; the complete
owned driver took 2.326 seconds. It ran once, after source-only guard review.

The method builds real synthetic canonical library bytes for histories with
30,000 and 500,000 completed chapters. Each successful selective restore fetches
an actual Schnorr-signed head, its addressed root and three selected chunks.
The method then asks the same helper to read corrupted chunk bytes in an HTTP200
response and requires the `chunk mismatch` refusal. This verifies the actual
Python read helper over a controlled aiohttp fixture. It does not qualify the
application service, HTTP service-key proof, database, full-tree restore, WAN,
browser, throughput, capacity or production recovery.

The transparent original `Wire.call` observer recorded **nine actual HTTP200
returns**, in three `GET head / GET root / POST read` sequences. It passes all
arguments to the original implementation and returns the original object; it
does not supply synthetic client results or replace the read helper. The test's
own handlers supply the deliberately controlled server responses. No application
or original-node binary was started.

## Ownership and bounds

Exactly one ephemeral IPv4 loopback listener was created. Before the first HTTP
call, the child paused on a private rendezvous pipe. The parent verified the
listener inode belonged to its captured exact PID/start-tick/group/session birth
and matched 127.0.0.1. Only then did it acknowledge the first request. The ready
endpoint and its hash are retained. Python audit checks refuse other binds,
nonmatching connects, subprocesses and SQLite creation. These are audit-level
checks, not an OS network isolation claim.

Normal terminal cleanup verified the single port closed and the sole owned
child reaped, with no remaining/unconfirmed births, forced signals or errors.
No runtime remains. The sole local runtime lease was released after terminal
cleanup; source pinning and evidence association continued read-only.

The approved caps were 120 seconds total (90 seconds work plus at most 25 seconds
owned cleanup), 1.5 GiB total owned RSS, at least 2 GiB available host memory,
64 MiB named outputs, and a 4 GiB disk floor with start/live reserves. The
conservative 1,181,159,424-byte memory forecast was required to fit before launch.
Measured sampled peak owned RSS was **197,566,464 bytes**; minimum host available
memory was 9,054,777,344 bytes; minimum workspace free was 30,682,906,624 bytes.
The 50 ms monitor observed at most 57,344 allocated bytes in the output directory
during work; complete retained files also remain below 64 MiB. These sampled
measurements describe this correctness run only.

## Exact source and dependency association

The frozen source was clean local commit
`6c5000081092134facb8713ccea202fc37edd7b5`, tree
`9fad0a11e76c4ca2174b4d103dfc8238a0e7124f`, the exact tree already published at
`cc5b237d33ceb8a9027feaa2b64682f0826cc012`. The source owner held that path
through execution and evidence hash association.

The driver checked actual HEAD/tree and all 16 selected file contents against
that immutable commit before admission and rehashed them after. These include
the unchanged test, `mixed.py`, the actual library implementation, metrics,
every local `src/*.py`, requirements and owned guard. All ten actual project
imports belong to that pre-run ledger. The ownership helper's known hash is
checked before it is imported. The driver and launcher are newly prepared
verification work; the committed bytes are exactly the once-executed versions.

The existing approved recovered target was verified before and after: exact
41-file regular-file membership, no symlinks/special files, all sizes/hashes,
and the immutable reviewed inventory SHA256
`7e7c5c88bc831111cc7583dbc2d6706ca133a9cff2d427dfb8b54f1085c2dcca`.
It supplies cbor2 5.6.5 and coincurve 21.0.0. See
`../../frontend/finite-fixture-portability-20261018` for the separate installation
provenance. There was no new install or download.

Python was 3.12.14. Host aiohttp was **3.14.4**, while the historical requirement
is 3.13.5; Pillow was 12.3.0. Full historical requirements parity remains false.

## Reproduction and evidence

The exact retained command was:

```sh
PYTHONDONTWRITEBYTECODE=1 python3 -B \
  /workspace/shared/nostr-manga-recovery-20261018/mixed-read-correctness-evidence-20261018T1607/run-owned.py
```

The absolute paths intentionally identify this restored source/dependency
layout. No database, installation, build, deletion, public listener or external
request is part of this command. All handlers and data are synthetic.

The pre-run reviewer required first-request ownership rendezvous and refusal
checks before acknowledging it; those changes were made before any execution.
No failed or superseded runtime attempt is being omitted. The test itself and
application source were not edited.

`manifest.json` covers every committed file except itself. Raw stdout is empty;
stderr contains the actual one-method unittest result and an asyncio debug slow
callback note. HTTP observations contain synthetic public publisher/hash values,
response byte counts and diagnostic times. No config, key, credential, runtime
database, private data, body dump or archive is included. This receipt is
separate from earlier phase-specific qualification and its explicit skips.
