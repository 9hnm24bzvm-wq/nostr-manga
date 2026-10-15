# Browser CI correction recovery record

The execution environment was replaced at approximately 2026-10-15 04:35 UTC. This document reconstructs facts already observed before replacement. It is not a new test receipt, does not substitute for missing raw artifacts, and does not claim the full browser suite passed.

## Source and commits

The isolated branch was `fix/browser-ci-38671869087`, based on `81c3ed1d3360b4a6039baecfebcdc708fe3a8e4b` in `browser-ci-six-failures-worktree`. These local commits existed and their commit commands completed before replacement; availability in a remote repository is unverified:

- `4073aef` Snapshot all relay rows at checkbox selection before IndexedDB handoff
- `191adc7` Defer actor status read admission until authority-keyed Svelte flush
- `d5e7010` Normalize unknown native publication year before editing a draft
- `c56699a` Scope editorial contributor search to its labeled field
- `d986baa` Measure compact request concurrency from complete native transport intervals
- `680f1cec3137176a2e4c5ad7e8c15fb7c954bbb6` Run instance-local ordinary publication journeys in explicit native-only fixtures

No independent GitHub push was made. The six commits and raw evidence are currently unavailable on the replacement filesystem.

## Causes, corrections, and observed tests

### Upload: fixture authority mismatch

The ordinary upload journey asserted an instance-local receipt, but its combined fixture configured original-protocol publication. A newly registered native account had no operator-provisioned `platform.protocol_identity_bindings` row, so the actual native commit correctly returned403. No permission bypass was appropriate.

The hosted runner now selects explicit native-only fixtures for `ordinary-a` and `ordinary-b`; all signed and other batches retain combined mode. Runtime `--native-only` rejects provisioner/key/hash/self-enrollment/second-recipient/UI-regression/PG-fault/Ubunchu combined flags. Native readiness rejects every protocol authority field and cleanup requires exactly platform/mail/postgres ports. Existing defaults stay unchanged.

Observed real native-only upload browser run `upload-ci-native-0409`: all9 steps passed, including exact receipt, dirty metadata Revert, Edit/Replace, and current-document navigation. The product generation was f0ee87ef8015, whose product source matched81c3.

28 hosted-runner Python tests passed, including seven actual invalid CLI combinations that exited2 before creating a fixture. The existing bridge positive is in `backend/tests/protocol_bridge_http.rs`, test `accepted_upload_rollback_recovers_source_media_covers_revocation_and_embedded_owner_lifecycle`; it explicitly binds the owner then verifies actual signed1002 and1001. A new intentional pre-binding403 regression was written but not compiled/committed before the reset; see the final section.

### Editorial: search selector and unknown-year form validation

The author search selected the global AppHeader Search control. A new strict field-scoped helper chooses `label[for=authors]` or `label[for=artists]`, its parent, then exact `Search...`; unknown families and absent/duplicate controls fail.

After this revealed the next genuine defect, actual DOM diagnostics showed hidden number input `#year`, min1900/max2100, held native unknown-year sentinel0. Browser constraint validation stopped Save; no PUT occurred. A single exact-hash/count1 overlay on `draft/manga/MangaEdit.svelte` changes only:

    year: snapshot.year ?? null!,

to:

    year: snapshot.year === 0 ? null! : snapshot.year ?? null!,

The original file hash was `e1f8b3a97bc414eab974258ee1e529fa0a7df5b0841573f778868770e44b3782`. Undefined/null/zero become empty; valid positive years and original bounds remain unchanged. A composed actual save-builder regression proves a title-only unknown-year edit omits year rather than saving0, and an explicit positive year change persists.

Fixed-generation actual browser `editorial-ci-fixed-0430`: all4 steps and23 granular checks passed, zero page errors. It covered author/artist/cover creation; submit/reject/revise/resubmit; local publication; repeated lock/unlock. Two unknown-year tests passed. Editorial controls/diagnostic/ordinary-contract tests passed; together with compact observer tests the group had64 passing tests. The ordinary-contract fingerprint retained all98 older assertions and explicitly accounted for a pre-existing81c3 welcome assertion, rather than dropping one.

### Signed recipients: reactive Proxy crossed IndexedDB

On the actual two-destination checkbox, unchanged rows from `relays.map(...)` remained Svelte proxies. IndexedDB failed with the actual visible alert: `Failed to execute 'put' on 'IDBObjectStore': #<Object> could not be cloned.` The second checkbox did not persist; a valid signed11999 job retained only one recipient.

The exact checkbox callback now maps `$state.snapshot(relays)` instead of `relays`, retaining the same revision/CAS and review invalidation. No automatic recipient-list rebinding was added. A separate missing-signer assertion was aligned with the current truthful UI copy, `No compatible NIP-07 browser signer was found.`

27 signed seams passed, including actual pinned Svelte compiler/runtime Proxy and actual original outbox/fake-IndexedDB structured-clone tests: original two-row callback fails; snapshot callback captures both reviewed identities before await; later reactive mutation does not rebind; stale revision rejects.

Fixed-generation actual browser `signed-recipients-ci-fixed-0431`: all6 steps and38 checks passed, zero page errors. Both original two-snapshot accepted/job-retained checks passed; two genuine original recipients accepted one exact reviewed signed event. Exact-body retry retained its approved destination, selection changes discarded review, and mutual peer revocation never granted browser accounts federation authority.

### Compact feed: observer delivery order was mistaken for transport overlap

The real original observation retained pages1..6, all200, without duplicates, but event maximum2. Complete Playwright native timings proved one request ended before the next began; the next request event could arrive before the prior requestfinished event.

The corrected gate retains the original exact duplicate predicate, full505 unique membership, bounded burst/resume, terminal, Back, and resize assertions. It uses complete native intervals for concurrency: `startTime` is Unix epoch milliseconds; `responseEnd` is milliseconds relative to that start. Every start must be at least the preceding start+responseEnd. No tolerance is used; even0.001ms real overlap fails. Missing, negative, nonfinite, relative/mixed-clock, failed, incomplete, truncated, or mismatched observations fail closed. Event maximum remains an unchanged diagnostic.

Nine observer seams passed, including genuine overlap and delayed observer delivery. Existing compact fixture46 and classification511 tests passed. Actual corrected-driver browser `ui12-compact-feed-ci-fixed-0414` on unchanged product passed; fixed-generation `ui12-compact-feed-ci-fixed-0432` also passed its full505-row phase with zero page errors.

### Actor follow: disposed controller admitted an unnecessary read

Real stack/timing traces showed one Session.set synchronously notified an old Group/User controller. It started a GET; 5–8ms later the actual Comments.Provider authority-key flush disposed/remounted the child, and its replacement started another. The harness correctly held the first actual204; the second settled and exposed Unfollow early. It was a genuine redundant old-lifecycle admission, not a bad Follow expectation.

The Session callback retains synchronous unknown/reset behavior, captures the generation after reset, and schedules only Session-triggered refresh admission in a Promise microtask. It refreshes only when not disposed and the exact generation is still current. Manual refresh actions and existing owner/authority guards remain intact.

54 existing owner/readiness seams plus a real pinned Svelte5.55.1 compiled mount/key/effect-flush/Session lifecycle test passed. The lifecycle test executes the original follow controller, mutation owner and authority registry, with finite explicit read/auth stand-ins. It proves anonymous no-read, old disposal before admission, one replacement read, same-actor coalescing, A→B→A replacement admission, and late completion after unmount without DOM writes.

Fixed-generation actual browser `ui14-actor-follow-ci-fixed-0430`: all15 steps and79 checks passed, zero page errors. Both Group and User finite observations were: current route; one header; Follow disabled; Unfollow/Retry/Block/Unblock absent; one GET; zero POST/DELETE/PUT; prerequisite actor; genuine native204; delivery pending.

### Home private403: unresolved observer capture

No product correction or weakened assertion was committed. On fixed-generation real browser `ui14-home-failures-ci-fixed-diag-0431`, the private401 step passed. The restricted recovery flow passed actual logout, real request form/mail, native303 redemption, recovery-only capability without identity/preferences/follows, direct private403, current widget403 and Retry feedback. The original single-read check still failed because its observer rows stayed empty.

Finite actual fetch diagnostics proved exactly one matching personal-feed GET, genuine completed403, and correct rendered error/Retry. At failure the observer reported requests1, rows[]. Thus no product replay was observed. A second diagnostic driver added exact Playwright response-listener/body-parse event tracing and unchanged predicates, but the environment was replaced before its terminal result could be read. Root cause remains unproven.

A prior full current-product home run also encountered a newer public429 saturation assertion failure (`Actual native request permits were not saturated`); no performance claim or relaxation was made. The actual real15-second PG outage/recovery portion and cookie401 passed in that earlier run. The PG performance gate was not rerun. The old signed-c infrastructure RuntimeError occurred before phase records with unretained raw evidence; no cause was invented.

## Build and runtime boundaries

The fixed product frontend was built once successfully with the same bounded `serial-terser` resource profile from reviewed commitc89d890: terser unchanged, maxWorkers1, reportCompressedSizefalse, Node heap2048MB. The full c89 commit was not cherry-picked because it depended on newer public-API plugin context; only its identical two profile/evidence hunks were transplanted privately and remained uncommitted. A first standard build had been killed by SIGKILL; neither failure was reported as a passing generation.

The source-bound QA stage used sourcecommit680f1ce, unchanged f0ee backend binary and provisioner. Index SHA256:

    461fc32e5899add0e759ed9a47e58a4d0bed299627c61957b45fd15f77a6c693

Complete fixture manifest SHA256:

    a85bafcdf4bb5a5841c3f80a043d361b4319769709ba25937a99cf361401b0e7

Backend:

    51a1c7d59f43c5e65eda8f14bb1b9f3f76457d811e57b341e0776d594928c51a

Provisioner:

    2cc4b3f843ef69138c7da53e9540a1007af520fdf31711464a50ad85abdbec59

The first materialization scan found100 nested executable symlinks after copying external package-directory links. The shell still started the successful build; no shared dependency cache was written. All100 remaining links were subsequently materialized and the private tree then had zero symlinks. The distinction was reported contemporaneously.

The relocated combined fixture correctly rejected the reused provisioner's compile-time root guard. That failed attempt cleaned every owned process/key/port successfully. The successful combined retry retained the original immutable provisioner's exact root, supplied the new hash-bound frontend snapshot and private reviewed source-bound drivers, and did not relax guards. All actual Chromium launches explicitly enabled its OS sandbox and kept exact HTTP/WS origin and deny-proxy isolation. No user preview interaction was performed.

Before replacement, the earlier two reproduction fixtures had complete verified-stopped cleanup receipts (exact5 and3 ports, keys absent, zero remaining owned processes). Their aggregate lifecycle was false because intentionally reproduced browser failures were retained. At replacement, two newer fixtures had not yet reached final cleanup: native48745 and combined51383. Their post-reset lifecycle is interrupted/unverified; absence of the old environment is not a fabricated cleanup receipt.

## Replacement verification

At04:35 the shell workspace became empty placeholders and usage fell from roughly28GB to84MB. All old worktrees, source stages, temporary logs, helper files and `.git` data were absent. CUA state and variables reset; its previous Terminal37748739 and all old tabs were absent. Fresh inventory had Chromium27262979 with only newtab2. A new recovery-only Terminal35651587 visibly ran `ls /workspace/shared`, showing only `downloads`. No old independent cloud filesystem was found. No delete, recovery overwrite, unapproved rebuild or independent upload was attempted.

## Uncommitted security regression to reconstruct and test

Immediately after `configure_publication(cfg.clone())` in the existing real protocol-bridge test, before inserting `protocol_identity_bindings`, the draft changes rebuilt the actual Router and:

1. Created a valid covered native draft, submitted its exact version, asserted actual Publish403/`forbidden`, and reloaded it to prove the version unchanged and no protocol_event_id.
2. Began a real upload against the pre-existing public native title; uploaded a valid generated PNG; asserted actual Commit403/`forbidden` with the original payload.
3. Reloaded the retained upload to prove its same ID, `is_committed:false`, and exactly one retained file.
4. Queried the actual original event table, requiring zero kind1001/1002 events before any binding.
5. Canceled that retained upload through native DELETE204.
6. Left the existing explicit binding and all subsequent genuine signed1002/1001 positive assertions unchanged.

This test edit was not compiled or committed before replacement. The available shared build guidance, which must be revalidated after recovery, was normal default test profile, no RUSTFLAGS or CARGO_PROFILE_TEST overrides, CARGO_INCREMENTAL0, locked/offline, jobs2, target `rust-tools/target-743619e`. No expensive benchmark was requested or started.
