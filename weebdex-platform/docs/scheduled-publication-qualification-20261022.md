# Scheduled publication qualification, 2026-10-22

## Result and exact scope

The isolated backend candidate at `e883ca85f5798202895b0b7046cb347436256bdb` passed **13 unique actual HTTP fixtures**, **162 library tests** (two existing ignored entries) and **one upload unit test**. The upload binary initially reported its 13 HTTP entries ignored; all were subsequently invoked individually against fresh disposable PostgreSQL fixtures. Production source last changed at `2648305`; `1c20b29` and `e883ca8` are reviewed fixture corrections, not production changes.

Machine-readable records are in `../test-evidence/scheduled-publication-20261022/qualification.json` and `source-and-dependency-association.json`. All 481 captured source/helper files were hash-checked again after execution. The offline cache association verifies 258 required registry packages against their locked archives and every unpacked file. The 39 uncached target-specific packages were not required by this build. This is not a claim that every possible target was compiled.

## Actual acceptance coverage

The exact test names and individual cleanup results are in the JSON matrix. They cover:

- Private pre-due originals, negative original-media GET/Range/export controls and real immediate-publication positive controls; session expiry, cancellation, account rebinding and exact full signed URL account fencing.
- Real DB-clock deadlines, immutable captured retry configuration, bounded pre-command infrastructure retry, conservative post-command uncertainty and application recreation.
- Actual competing claim/cancel transactions, stale lease CAS, derivative-inclusive private quota, old local chapter readability and both native-edit/worker lock orders.
- Native begin-edit expiry ordering with retained media and unauthorized/wrong-version rollback.
- Exact durable command provenance, two schedules sharing a source-less parent, own accepted-parent recovery, immutable parent fingerprints, legacy NULL provenance refusal and retained ambiguous source/media pins.
- Cover removal/replacement and unrelated parent edits blocking rather than silently substituting content.
- Existing immediate upload/page replacement contracts and original authority-lock/FK compatibility.

No fixture success is inferred from a source-only review or a mocked state transition.

## Build and execution association

The owned offline command was:

`cargo test --locked --offline -j2 --manifest-path weebdex-platform/backend/Cargo.toml --lib --test public_uploads --no-run --message-format=json`

Rust and Cargo were both 1.90.0. Each real HTTP case used the exact hash-verified `public_uploads` executable with `scripts/run-auth-tests.sh`, `--ignored --test-threads=1 <full_test_name> --exact`. There was no blanket ignored-helper invocation.

Executable SHA-256 values:

- Main: `490cc4d0d2939204992764e59a9846a35c782a81583e073f49d22b4480357ec9`
- Library test: `ca21951257722271fde87364c4c7799825762631dac5ba7f1fbe1f8dd7d71238`
- Upload test: `6e37b3643d3778d45b835f4254fee5f16a838ca6d23878a1c0e352a8d8a5c415`

All successful outer owners and fixture owners were verified stopped, unforced, with zero unknown births, zero tracking/cleanup errors, closed ports and stdout EOF. Each successful private database was archived losslessly with complete member/hash/mode verification before its stopped data directory was reclaimed. Runtime databases, credentials, private fixture manifests and raw HTTP logs are excluded from project evidence.

## Ownership correction and retained failures

The first two passing HTTP cases used the original lifecycle helper. The remaining eleven used separately reviewed helper commit `4288bb422e8ea69ec9e7afc8be4f6281e786b85a`, SHA-256 `1ef70dc4cebee9da6190c3ba5e5d5c4328c895136b6a1b7025ad9b6f675f91ac`, without changing product source or resource guards.

One interrupted attempt captured an exact census zombie disappearing before registration. The correction recognizes only an initial exact zombie followed by fresh absence as terminal-observed. Live, reused, unreadable or otherwise ambiguous identities still fail. Thirteen focused finite controls, including actual fork/reap and adopted descendants, passed; the old-source positive control failed as expected. None of the successful HTTP executions actually encountered this terminal-observed branch. Earlier ownership-uncertain attempts remain unqualified, including attempts without enough diagnostic information to attribute a cause.

Earlier fixture failures are retained rather than overwritten. Reviewed corrections enabled the existing real original projection for positive controls, supplied the required native Origin/grant, used distinct immutable parent manifests and staged lock contention after the durable-start transaction actually committed. Original status, byte/hash, ordering and authority assertions were retained.

## Resource and artifact preservation

Launch and live disk floors remained 4 GiB + 256 MiB and 4 GiB + 128 MiB respectively. No guard was lowered. The owner retained its existing process, memory, timeout, evidence-growth and cleanup limits.

All five current outputs, including reusable rlib/rmeta, were protected by full gzip reconstruction/hash/size verification and durable restore maps. After qualification, the three executable byte sequences were retired recoverably under the coordinated composed-build plan, including both main hardlinks: 224,344,336 bytes. Reusable rlib/rmeta and dependencies remained unchanged. Future output restoration must verify hashes and modes and must not blindly replace newer Cargo output. This record is not evidence that a later merged executable has passed these tests.

## Explicit limits and next gates

- “Restart” here means application state/pool/router/publisher recreation in the same OS process, not a fresh-process restart, power-loss or crash proof.
- The configured two-second real-clock retry test proves the immutable configured cutoff and changing configuration does not rewrite it. It does not claim 24 hours of elapsed execution.
- Lease acceleration exercises actual CAS, not five minutes of elapsed time.
- The unavailable-source control uses an injected owned-source visibility transition, not a full deletion/moderation journey.
- UTC interpretation, a schedule-accepted 202 receipt and bounded scheduling limits are documented instance extensions, not strict upstream response parity. Schedule acceptance is never reader success.
- Main catalogue/account-closure semantic composition requires fresh source review, actual joint races and a newly associated backend build. The separate UI candidate has finite/component evidence, not yet composed browser-to-backend acceptance.
- No hosted publication, deployment, real credentials or actual user scheduled release was performed.
