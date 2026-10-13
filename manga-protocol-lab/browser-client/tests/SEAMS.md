# Reconstructed client test seams

The public interfaces are local library operations, immutable outbox reads/state changes, publication/recovery operations, backup import/export, canonical codec inputs/outputs, actual DOM user actions and the build command. Tests observe returned records/events/bytes, visible controls, rejected operations, actual transport requests or published filesystem output. They do not assert current private storage keys, SQL, internal helpers, exact batch counts or cache implementation flags.

## Model coverage and external adapters

- `store`, `outbox`, `backup` and `publication` tests use real application modules with fake-indexeddb. Separate connections and reopened connections exercise transactional interleavings in that model. Frozen version-one and version-three fixtures arrange legacy persisted input; outcomes are read through current public operations. They are not browser crash/restart evidence.
- Storage faults are injected only at IndexedDB's external methods/events. A quota exception after a clear and an explicit transaction abort must both preserve previously visible data. Tests reopen and read actual public records, then perform a successful retry.
- File faults supply an external File-compatible adapter. Interrupted reads, malformed UTF-8, oversized lines, bad footers, duplicate records and stale revisions must preserve the previous library. Large-backup input never permits whole-file text reads.
- DOM tests import the actual app and operate its controls. Deferred IndexedDB events and a Worker transport adapter arrange delayed responses. Assertions cover the selected view, pagination, retry controls, forgotten instances, conflicting preference edits, rejected reconnects and local-only backup flows. No LocalLibrary/app method is replaced.
- Most lock adapters only invoke the callback. `backup-reopen.test.mjs` additionally models exclusion and `ifAvailable` at the external Web Locks dependency: a paused File read keeps an import active while another connection opens. Neither adapter certifies native browser lock scheduling or process termination. Actual Web Locks and extension interaction need browser acceptance.
- Signer fixtures compute the event hash independently using Node crypto. Late approval tests use the public signing or publication operation with external clock/signer controls and observe the absence of transmission or queued jobs.
- Codec tests include literal canonical vectors, strict malformed inputs, all protocol families, byte/count limits, maximum-width names and complete 30k/500k chapter recovery. Round trips supplement, rather than replace, independent literals.
- The service-worker test is a Cache/Fetch event adapter model. Returned offline responses and bypassed private/unrelated requests are its oracle. It is separate from a real browser's service-worker install, redirect, reload and eviction behavior.
- The build CLI test runs the real command in a temporary project. It observes unchanged prior output after missing input, next-invocation recovery from the documented interrupted-promotion filesystem state, removed obsolete assets, working vendored imports, a complete precache and deterministic repeated output. No crash hook or function mock is used.

## Deliberately separate acceptance

`npm test` is the model gate. `npm run test:backup-large` consumes appreciably more memory/time and is opt-in. Browser and HTTP/Nostr transport tests require actual current services and are not inferred from either model gate. Run `npm run build` before testing the generated service worker or serving the app.

Remaining limits include actual browser storage eviction/quota/power-loss behavior, extension signers, real multi-tab Web Locks scheduling, production load, interrupted OS directory renames and platform-specific filesystem failures. The build's portable promotion has a brief rename gap; the CLI fixture proves its next-invocation recovery contract, not crash durability.

The reconstruction logs retain failed setup attempts and genuine red states. A filename containing `green` alone is not proof: inspect the exit status/results (notably early `005` and `026` contain failed transitional attempts). Historical October 8 logs elsewhere in the archive are not new acceptance.

## Selected fresh red → green evidence

All paths below are under repository `results/rebuild-client/`. Subsequent aggregate gates must also pass; a focused fix alone is not final acceptance.

| Observable failure                                    | Actual red                        | Focused green                       |
| ----------------------------------------------------- | --------------------------------- | ----------------------------------- |
| Delayed publisher recreates removed snapshot          | 004-outbox-remove-red.log         | 006-checkpoint-one-green.log        |
| Progress reads return immutable byte inventory        | 008-bounded-outbox-red.log        | 010-bounded-outbox-green.log        |
| Forget while signer is pending creates late job       | 012-forget-sign-red.log           | 013-forget-sign-green.log           |
| Old public owner clock is reset after identity switch | 018-owner-clock-red.log           | 019-owner-clock-green.log           |
| Wide valid list names exceed a chunk byte budget      | 036-wide-codec-red.log            | 037-wide-codec-green.log            |
| Failed build modifies prior public output             | 038-build-preservation-red.log    | 039-build-preservation-green.log    |
| Interrupted promotion leaves public absent next time  | 040-build-recovery-red.log        | 041-build-recovery-green.log        |
| Synchronous storage failure empties local library     | 043-store-quota-red.log           | 044-store-quota-green.log           |
| Forget during Worker preparation still publishes      | 045-forget-preparation-red.log    | 046-forget-preparation-green.log    |
| Older tab publishes to a forgotten instance           | 048-stale-tab-selection-red.log   | 049-stale-tab-selection-green.log   |
| Stale checkbox recreates a forgotten instance         | 050-stale-preference-edit-red.log | 051-stale-preference-edit-green.log |
| Aborted snapshot read reports success                 | 059-read-abort-red.log            | 060-read-abort-green.log            |
| An asset rename reuses the old cache identity         | 078-build-asset-identity-red.log  | 080-build-asset-identity-green.log  |

The bound-page getAll change is a behavior-preserving refactor under existing public pagination tests, not a claim of a new functional red. The original large-run outcome is recorded in `056-backup-large-status.json`; its successful import is not a completed round trip. The final large run records source digests in `068-backup-large.log`.

## Capacity and abandoned-import supplement

`local-capacity.test.mjs` arranges a modeled near/over-2M population through the external IndexedDB upgrade-count response. It preserves actual transaction serialization and asserts public save/update/delete/reopen/replace/import outcomes plus the caller-visible capacity diagnostic, never a private stored counter. An ordinary-sized legacy fixture separately derives the count from actual rows and checks list/membership deletion. Aborted and concurrent same-key writes must not consume extra capacity. These checks do not allocate two million stored records or qualify their performance.

`backup-reopen.test.mjs` models external quota occupancy after a file disconnect and failed cleanup. Reopen must enable a subsequent local save without changing prior records, must leave occupancy alone without Web Locks, and must not destroy any of the 1,500 records belonging to an active importer. The quota model records committed external storage effects; there is no private staging-table outcome assertion. Native killed-tab recovery remains unverified by this model.

`schema-upgrade.test.mjs` holds an actual older IndexedDB-model connection to verify blocked-open rejection and retry. A cooperating old connection's admitted transaction and separated pending object bytes must survive migration. Existing outbox tests cover embedded version-one bytes. Browser `versionchange` scheduling and power-loss durability remain separate acceptance.

The bounded `exportRescue` interface shares the paged exporter, emits a distinct non-importable header, retains exact records, and rejects concurrent edits. Its fixture uses a modeled over-budget diagnosis with real ordinary rows; this is not an actual multi-million-record rescue run. See `../README.md` for manual partition/sink requirements and remaining UI gaps.

Fresh red/green and the combined build-first 70-check gate are in `../../results/rebuild-local-storage-parity/acceptance.json`. The >256 MiB backup fixture was updated for the additive public capacity diagnostic, but was not rerun for this supplement.

## Recovery review and scalar-input supplement

`list-name-ui.test.mjs` checks the real HTML UTF-16 allowance against a literal 128-emoji name, submits that name through the actual form, and verifies that 129 ASCII scalars are rejected without changing records. Its initial failure is a DOM attribute-contract oracle. JSDOM assignment and supported browser fill/paste are not evidence of native typing truncation.

`recovery-ui.test.mjs` operates the actual app with signed external service/head/object HTTP responses. A deferred chunk allows real reconnect, forget, selection and second-connection preference actions before recovery finishes. Public records and visible confirmation/notice text are the oracles. Preview approval/decline, preference change at confirmation, a pending reconnect, local edits, and external commit abort followed by retry are covered. No app method, module state or private metadata is mocked or asserted. Native signer-extension/browser scheduling remains separate acceptance.

The new model gate has 81 checks under build `72e656def51db26f`; exact red/green and current hashes are in `../../results/rebuild-client-ui-parity/acceptance.json`. Earlier 70-check evidence remains scoped to its prior build.

## Durable family-statistics supplement

`statistics.test.mjs` preserves the public `counts()` result when the external IndexedDB adapter refuses repeated range/object-store counting after initialization. That causal red demonstrates the removed dependency; it does not measure browser performance or assert a private stored counter. Native baseline timings are separately recorded in `../../results/rebuild-browser/native-counts-baseline.json`.

Real ordinary-sized schema-3/4 fixtures verify every family, revision, and pending object bytes through the current public library/outbox interfaces. Interrupted upgrade, updates, absent deletion, list/orphan-membership cascades, two-connection conservation, deletion rollback, both replacement formats' abort/retry, mixed-family streamed import, empty replacement, and exported backup round trips are observed through public records/counts. Capacity adapters still model a large population at the external upgrade-count seam; actual two-million-record admission remains unqualified. Build `e4f4862bd75c26bb` passes 91 checks, including the original 81; the final gate is recorded in `../../results/rebuild-family-statistics/acceptance.json`. The first combined attempt retained in `008-full-client-gate.log` failed because the existing pending-reset race paused a range count no longer used by rendering. That fixture now pauses the external paged read instead and retains the same visible no-duplication oracle.
