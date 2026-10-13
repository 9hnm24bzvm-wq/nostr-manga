# Local-first browser library client

This is a functional reference client and reusable browser modules for the manga backend. It is not the final MangaDex-style reader-site design.

## Default behavior

- Chapters,reading progress,series status,lists and membership save in IndexedDB on this device.
- No account,key,signer or relay is needed for local use.
- Local saves do not call a relay. Remembering a relay or reconnecting a signer never enables automatic publication.
- Local IndexedDB data is not encrypted and can be read by code served from this app origin. Use a trusted/self-hosted client origin.
- Export/import transfers a plain version-two JSONL local backup. Version-one JSON imports remain supported up to 256 MiB. Version-two imports/exports admit up to 2 million records and 4 GiB, with 64 KiB maximum lines and I/O parts. Keep it private. Browser storage can be cleared or evicted; persistence requests are optional and not a backup guarantee.

## Optional publishing

Connect a compatible NIP07 signer,verify/remember selected instance service keys,and explicitly approve a public snapshot. No private key is requested or stored in the client. No login or signature is attempted at startup.

Publication exposes all four protocol families,including reading history,progress and list names under the selected key. It may replace that key's existing public library. Previously published copies cannot be recalled by cancelling local sending or deleting local data. New local edits stay local until a new snapshot is explicitly approved.

A durable IndexedDB outbox keeps the signed snapshot,objects and per-instance outcomes. Retry requires explicit action and fresh HTTP authorization. The immutable publication event stays the same. Retry restages the graph because relay staging leases may have expired. Cancellation is sticky across competing writes and is checked after signing before transmission; an already-sent request cannot be recalled.

An instance may reject admission,disable signup,restrict CORS or refuse storage. Failures never undo the local save. The optional admission request is a separate user action. No keys are generated silently.

Public recovery verifies the service key,event,root/chunk hashes,canonical encoding,bucket placement and record counts. Replacement requires a preview confirmation and an unchanged local revision. Local mutations serialize through IndexedDB; publication operations use Web Locks. Native remote-signer/NIP46 and browser-signer installation are not supplied.

## Run

The prebuilt public/ directory has no remote scripts,fonts,analytics or runtime package downloads. Serve it over HTTPS or loopback HTTP:

```sh
# Run from browser-client/ in the source, or the extracted manga-local-browser-client/ folder
python3 -m http.server 8085 --bind 127.0.0.1 --directory public
```

Open http://127.0.0.1:8085 in a modern browser. Opening index.html as file:// is unsupported because modules,IndexedDB origins and service workers need a proper origin. Avoid private browsing for durable data. Changing the app origin gives it a different browser storage namespace; export/import is the explicit local transfer path.

For cross-origin publishing,set the backend's cors_origins to the exact client origin through the operator's reviewed configuration. Never expose raw strfry. The selected backend must advertise and implement this project's HTTP library routes; an arbitrary Nostr relay without the manga wrapper is insufficient.

To rebuild pinned vendored modules:

```sh
cd browser-client
npm ci --ignore-scripts --registry=https://registry.npmjs.org
npm run build
npm test
```

Node is a build/test prerequisite,not a production browser runtime requirement. Build copies pinned noble-curves/hashes modules and their licenses into public/vendor; no CDN is needed. fake-indexeddb is test-only. The application itself is plain JavaScript and browser APIs.

## Module API

- src/store.js: LocalLibrary.open,write,list,remove,counts,snapshot,replace and durable outbox operations
- src/codec.js: typed records,canonical CBOR,hash bucket partitioning,root/chunk graph validation
- src/nostr.js: exact signer binding,service identity pinning,signed HTTP response verification
- src/publishing.js: explicit snapshot queue,recipient retries/cancellation,public recovery
- src/backup.js: bounded JSONL import/export, staging, revision-fenced atomic replacement
- src/outbox.js: immutable signed jobs/bytes, bounded delivery batches, preference cancellation and revision fences
- src/indexeddb.js: transaction completion, abort and cursor lifetime adapter (internal)
- src/worker.js: snapshot/graph preparation away from the page's main thread

### Migration from earlier browser-module return shapes

`job(id)`, `jobs()` and `queueSnapshot(...)` return job metadata with `objectCount`, without an embedded `objects` array. To read the approved immutable bytes, call `jobBatch(id, 0)`, process its `objects`, and continue with the returned `next` cursor until an empty batch. Each read is bounded to 64 objects/4 MiB; missing or cancelled jobs reject. Do not regenerate queued bytes from the current local library.

`saveJob(job)` now updates only an existing job's recipient progress (`cursor`, `state`, `message`, `projected`) and sticky cancellation. It rejects missing/deleted jobs and changed recipient origins/service keys. Changed event, revision or object bytes are not persisted. It is not an upsert: create new signed publications through explicit-consent `queueSnapshot`. Instance preferences use `setRelays`/`relayPreferences`; the arbitrary `setMeta` writer was removed.

Client integer fields are restricted to JavaScript safe unsigned integers; chapter page is u32. This is narrower than the protocol's i64 timestamp/position ceiling. The client rejects larger values rather than rounding them.

## Build publication and recovery

Build creates a fresh `.public-build-*` staging directory and completes all source/vendor copies before touching `public`. Removed assets cannot survive in the new service-worker manifest. Canonical asset paths, byte lengths, contents and build logic all contribute to the cache identity. Missing or unreadable build inputs leave the previous output intact. The output contains current source modules, pinned vendor modules/licenses, index, styles and a generated worker; hand-edited files in `public` are not build inputs and are not preserved.

Promotion uses two same-filesystem renames. The old output is retained at `.public-previous-<UUID>` until the new output has been promoted; ordinary promotion errors attempt rollback. There is a brief gap during which `public` is absent. This is not a power-loss-atomic directory swap and simultaneous build invocations are not a supported deployment strategy.

If a previous invocation was interrupted in that gap, the next invocation restores the sole recognized retained directory before reading new inputs. If several retained directories exist and `public` is absent, it stops for an operator to choose the intended build. It never guesses or deletes those ambiguous backups. An interrupted staging directory may remain unused; do not serve it as published output. Do not publish the client source folder itself: serve only `public`.

## Verification boundary

This working tree is reconstructed from the recovered October 8 source. Earlier archive test claims do not certify these changes. The current evidence is under `results/rebuild-client/` in the repository; `082-final-full-model-gate.log` records 42 passing model checks including 30k/500k chapter graphs after build `17e72bdd1b58d37c`; `083-final-format-check.log` records the formatting check. Later focused checks and gates are recorded separately rather than rewriting that log.

`npm test` uses the actual public library/backup/publication operations, actual app DOM controls in JSDOM, independent codec/signature fixtures, external IndexedDB/File/signer/time adapters and the actual build CLI. It covers abort-safe replacement, interrupted file I/O, legacy database migration, immutable outbox byte continuation, stale signing/preference races, cancelled jobs, ordering clocks, late rendering and signer deadlines. These models do not certify a browser engine, actual Web Locks fairness/mutual exclusion, extension prompts, disk-full recovery or power-loss durability.

`npm run test:backup-large` is a separate resource-heavy 500,000-list backup test with a file larger than 256 MiB. It checks actual bounded File slices and Blob parts, imported public counts and every exported row. Running it is distinct from the codec 500k test. The final reconstruction run passed in `068-backup-large.log`: 500,000 lists, 294,500,060 input bytes, all exported rows verified, 98.0 seconds and a highest sampled RSS of 2,411 MiB in this Node/IndexedDB model. A result is accepted only when its log reaches the final `passed` record and the process exits successfully.

`tests/local-browser.cjs` is an optional actual Chromium/Playwright driver for offline reload, multi-tab local updates, file download/import restoration and layout. `tests/backend.mjs` exercises actual HTTP/Nostr transport against a running Rust/PostgreSQL/strfry node. Neither is part of `npm test`; both require their own current execution evidence. Historical hosting/access approvals do not authorize publishing a new static copy or changing sharing.

See [tests/SEAMS.md](tests/SEAMS.md) for what the model oracles prove and what remains outside their boundary. No target throughput, complete reader website or production-readiness certification is claimed.

## Rendered-browser verification, October 10

Final build `17e72bdd1b58d37c` was exercised in the existing cloud Chromium through supported UI controls, with synthetic data only. Local chapter completion/progress, reload persistence, series status, list creation/membership, actual backup download and confirmed restore, restored-data reload, live cross-tab updates, and the no-signer publication guard passed. Stopping the local origin server still allowed cached reload and local save/reload. Evidence: `../results/rebuild-browser/acceptance.json` and associated captured UI states.

This does not establish real signer-extension publishing, mobile behavior, full network-offline emulation or production traffic capacity. The earlier direct headless Chromium startup failure did not prevent this supported existing-browser route.

## Local record capacity and old-database recovery

New records are admitted up to 2,000,000 total across all four families. Updating an existing key and deleting records still work at the limit. The check and total update share the records/revision transaction, including concurrent tabs; deleting a list also releases its removed membership records. A save does not scan the whole library. Version-one replacement and version-two streamed import reset the total atomically with their accepted records.

Schema version 5 derives the four family counts once from existing records and stores them with their total as one coherent value. Normal `counts()` reads that value and the revision in one transaction; it does not recount record ranges on every UI refresh. Insertions, deletions (including list membership cascades), replacement and streamed import update statistics atomically with their records and revision. Existing-key updates preserve the counts, and transaction aborts preserve the previous complete state. Migration from schema 4 ignores the old total and derives all counts from the actual records; it never truncates an over-budget library.

This change follows an actual desktop-browser diagnostic on the prior `72e656def51db26f` build: three 500k-library count reads took approximately 2.1 seconds each, while 50-row page reads took 2.2–4.9 ms. The host was also compiling, so those observations are not isolated capacity measurements. Build `e4f4862bd75c26bb` passes 91 model checks, build and formatting; its native upgrade and timing comparison remain separate acceptance. See `../results/rebuild-family-statistics/acceptance.json` and `../results/rebuild-browser/native-counts-baseline.json`.

The `counts()` interface is unchanged: family counts plus `revision`, `total`, `recordLimit` and `overBudget`. New keys are refused until space is available. Normal export refuses an over-budget library with recovery guidance instead of silently truncating it. Direct writes to private IndexedDB stores are unsupported. An older application build cannot reopen a database upgraded to schema 5; use the current build, or export through the current supported backup interface before intentionally changing applications.

An older tab that handles `versionchange` closes its connection after already-started transactions finish. A noncooperating connection causes `LocalLibrary.open()` to reject with close/reload guidance, also displayed by the normal app startup error notice. Close older tabs and reload. An issued IndexedDB upgrade cannot be canceled: it may finish once the blocking connection closes, and the rejected opener then closes its eventual connection. The upgrade preserves both version-one embedded and version-three/four separated queued publication bytes.

Opening a library opportunistically reclaims abandoned backup staging only while holding the same per-database Web Lock as import. A live importer, missing Web Locks or a lock/storage failure causes reclamation to be skipped; existing records remain readable. No timeout is treated as proof that another tab died. If a failed cleanup leaves storage occupied, reopen after the importer has finished and storage is available.

### Streaming rescue for an existing over-budget library

The module operation `exportRescue(store)` from `src/backup.js` yields newline-terminated strings from bounded 500-record pages, at most 64 KiB per line. A consumer must await each write to its own streaming sink rather than collecting the lines into an array or Blob. It is revision-fenced: abandon an incomplete output and retry if local records change, or if the sink fails. There is no whole-library materialization or total rescue archive size cap inside this iterator.

Its header is `{"format":"manga-local-library-rescue","version":1,...}`; record lines remain `[family,record]`, and the final `{"end":N}` counts all emitted records. This is deliberately **not** a supported-size backup: normal import refuses it. Keep the full rescue archive private and intact. To recover, review and stream its records into separately named empty `LocalLibrary` instances through `write(family,record)`, choosing an explicit partition of no more than 2,000,000 records per library. Keep related list/membership and series records together where needed, retain the exact IDs/values, and use ordinary `backupBlob`/`importBackup` for each supported-size result. There is no automatic merge, partition policy or permission to discard the source archive.

The reference UI does not yet provide a streaming rescue download or partition wizard. An operator/programmatic sink is needed for this exceptional legacy case. Normal UI backup still constructs a Blob and snapshot/publication preparation still materializes the library; their memory/responsiveness limits remain separate from bounded I/O and admission.

Fresh storage evidence is `../results/rebuild-local-storage-parity/acceptance.json`: model capacity boundaries use an explicitly modeled IndexedDB count, not two million allocated records. Actual small legacy rows verify count derivation; real model transactions cover racing writes, aborts, replacement and upgrade/reopen. External File/quota/Web Lock adapters cover abandoned occupancy and a concurrently active importer. Build `17e9c89ef2b0da1d` passed the combined 70-check model gate and formatting. No new native-browser, actual quota/power-loss or >256 MiB backup acceptance is implied.

## Recovery review and Unicode UI supplement

List names accept 1–128 Unicode scalar values and at most 512 UTF-8 bytes, without control characters. The input allows 256 UTF-16 units so supplementary-plane characters can use the full scalar allowance; the normalizer remains authoritative when saving. Names are not normalized or rewritten.

A public recovery preview now names the exact owner, instance origin, pinned service key, signed event ID and timestamp before asking to replace local data. Reconnecting the signer or changing saved instance preferences invalidates an in-flight review. Preference revision is checked again inside the same replacement transaction as the local record revision, including changes from another tab. `LocalLibrary.replace` accepts an optional fourth `expectedRelayRevision` argument for this caller-reviewed fence. Existing three-argument local replacement callers retain their behavior.

Reconnect cannot change the identity during the short replacement commit; cancel/error/success releases the operation's controls. A failed or declined review never starts automatic recovery or publication. If another tab changed saved instances, reload the page to display its latest choices before reviewing again.

Build `72e656def51db26f` passed 81 model checks in `../results/rebuild-client-ui-parity/009-full-client-gate.log`. The prior 70-check build remains historical. These model tests use actual DOM controls with signed external peer responses; they do not certify a real signer extension or native network UI. Supported browser fill/paste on the old build did not reproduce maxlength truncation, so the model's HTML-contract red must not be described as a native-browser failure.

Final native cloud Chrome acceptance for build `72e656def51db26f` preserved prior synthetic records, saved and reloaded an exact 128-emoji list name (512 UTF-8 bytes), and rejected a 129-ASCII-scalar name with `Invalid list name` and no added list. Evidence: `../results/rebuild-browser/final-parity-ui.json`. Recovery races remain model-tested; this browser pass did not exercise a real signer extension, remote publication, native capacity pressure, mobile or power loss.
