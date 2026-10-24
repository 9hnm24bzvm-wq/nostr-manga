# Signed native moderator decisions

## Separate authority

The additive `/reports/moderation` route opens only after an explicit user action and uses the selected signed identity's captured, bound native account. It does not call native cookie APIs. `/reports/signed/native` remains an owner-scoped, read-only inspector; possessing a report or using that inspector never grants moderation.

Queue/context/decision routes live at `/api/v1/nostr/moderation/reports`. Every read captures `expectedAccountId`; every decision carries that account, exact report/reason versions, a target revision digest, complete immutable context digest, action, exact notes, and expected native effect. The server independently revalidates the current admission, service, binding, active account, moderator role and `report.update` grant in its transaction. Role or account changes cannot use the browser login as fallback.

## Native transaction and effect preview

The sealed `ReportReviewActor` capability is implemented only by the original `SessionPrincipal` and genuine `BoundAccountWrite`. The extracted `review_report` core preserves native terminal waiting-only transitions, native audits, and native effect semantics. Personal read capabilities do not implement it.

Native lock ordering is preserved: title-removal target before report; group claim report before claimant before group; other reports before actual target. Target/report/reason/claimant lock acquisition is NOWAIT. Transaction lock waits are capped at 250 ms and statements at 5 s. The active-account fence also remains in the native shared core. Reciprocal rows fail boundedly rather than waiting indefinitely.

The preview captures actual native target state/version/updated time, the exact group claimant and leader state, chapter's manga parent, reply's actual thread and its native resource parent, and a chapter-parented discussion's manga grandparent. Group-member writers share the group-row lock. Source visibility reuses the existing `native_discussion` event/blob fence and its bounded publication membership witness; it locks source events and blob generations against withdrawal/quarantine. Imported groups require a currently visible, locked chapter/manga witness.

Generic acceptance only changes report status/notes. Group-claim acceptance grants native group leadership, requiring `scanlation_group.update`, current eligible claimant, unclaimed unlocked group, and exact submitted/current version. Chapter removal requires `chapter.remove`; manga removal requires `manga.delete`. These are native instance soft-hides. They do not publish or rewrite original protocol events, request source deletion, or claim peer copies were deleted.

The shared core's actual effect must exactly equal the confirmed preview before commit. The precise receipt identifies account, method, path, raw request hash, report old/new status/version, context hash and actual effect.

## Bounds and confidentiality

Queue pages expose at most five fixed summaries. A six-row lookahead produces canonical JSON continuation scoped to account/category/status and ordered by microsecond `created_at` plus C-collated native ID. There is no offset ceiling. Cursors are at most 512 UTF-8 bytes and reject malformed dates, duplicate/extra/missing fields, noncanonical encoding or changed scope. Concurrent insertions ahead of a cursor do not shift older pages; filters are live snapshots, not a promise of frozen historical membership.

Exact detail displays one native target, not `report_json` expansion. Text is SQL-bounded before projection: report details/notes 20,000 bytes, reason/target labels 8,000 bytes, body 50,000 bytes, claimant name 128 bytes; identifiers 128 bytes. Oversized required content fails closed rather than silently truncating. Unavailable targets expose bounded historical identity/revision only. Complete context/response is at most 1 MiB. Exact decision POSTs allow at most 131,072 wire bytes so 20,000 JSON-escaped native note bytes fit. Other transport classifiers retain their former limits; new moderator exception rejects any query marker, wrong type/method, unknown path or malformed native ID.

All moderation success and error responses, including outer request-boundary errors, use `private, no-store` and vary on Authorization and Cookie. The client verifies these headers before installing private snapshots or accepting decisive receipts. It never renders raw signer/server error text or untrusted HTML. Route, signer, close and unmount changes abort captured reads and erase private UI state.

## Uncertain outcome

A per-account/service/identity/report marker is persisted before dispatch and read back before transport. It is removed only for a precisely verified receipt and postflight account check, a trustworthy definitive 4xx refusal, or a transport-proven unsent attempt. Network failures, 5xx, malformed/wrong receipts, post-dispatch abort and changed postflight authority retain the marker. Reloads and remounts never silently reconcile or retry a decision. Only its digest is stored, not private notes. The UI directs the operator to reconcile uncertainty without offering a force-repeat button.

## Qualification and explicit limits

Local finite execution on 2026-10-22:
- Controller: 57/57 actual TypeScript/controller tests with synthetic signed-account/transport primitives.
- Mounted route: 13/13 actual Svelte 5.55.1 + JSDOM cases, including the frozen old native cookie gate, explicit signer load/review/confirmation, route/signer/unmount cancellation, uncertain reopen and keyset navigation.
- Transport: 23/23 actual request transport tests with finite in-memory signing/fetch primitives, including escaped 20,000 notes, exact/over 131,072 cap, hostile paths/query/types, and old-source failure before signing.

These tests do not prove real Nostr signatures, network/HTTP behavior, Rust compilation, database lock scheduling, a built browser application or production deployment. The synthetic-only ignored `backend/tests/public_moderator_reports.rs` is authored for the isolated PostgreSQL auth runner and awaits the shared heavy execution slot. It covers real request signing, current role/grant/admission/account checks, target/reason/effect freshness, private errors, queue timestamp ties and >10,005 older reports, source withdrawal/quarantine/grandparent locks, and native group/chapter/manga effects/audits. No real user was moderated.

The supplied frontend tree remains byte-for-byte unchanged (`c90f53f5018f8eda8613ccff97bf4a209b0e33f0`). New routes/components are additive integrations. The existing personal-inspection files are unchanged. Transport association metadata is comparison-only and preserves the older raw/QA source hashes, diagnostic evidence and pins. It is never a runtime/build substitution or a claim that historical browser/HTTP/timing receipts cover the successor.

Finite Node commands (existing shared dependencies, no installation):

```sh
node --experimental-vm-modules --test tests/ci/test_signed_moderator_reports.mjs
WEEBDEX_FRONTEND_ROOT=/path/to/existing/frontend WEEBDEX_BROWSER_DOM_NODE_MODULES=/path/to/existing/dom/node_modules node --conditions=browser --experimental-vm-modules --test tests/ci/test_signed_moderator_reports_dom.mjs
node --experimental-vm-modules --test tests/ci/test_signed_moderator_transport.mjs
```

The mounted harness creates only bounded generated modules and removes its owned generation directory. Evidence logs are under `tests/evidence/signed-moderator-decisions-20261022/`.

Export portability closure: controller 58/58 and mounted 13/13 passed after replacing test-time Git calls with an independently SHA-pinned 173-byte exact predecessor layout and source witnesses. The fixture records the original commit/blob and preserves the unchanged trusted 676-file attachment manifest plus the three pre-existing reconstruction files. Runtime tests verify all 679 current frontend files, exact membership and all three personal-inspection files, and reject corrupted/unknown fixture bytes. No Git fallback, skip, reconstructed old markup or current-source substitution is used. The prior 57-test log remains a separate, truthful earlier result; newer logs use `*-no-git.log` names.


## Native-control parity successor (controller/mounted qualified; backend pending)

This successor adds exact native object and reason ID filters and ascending or descending queue order. Omitted order normalizes to descending. Canonical cursors now carry account, category, status, object, reason and normalized order, and are capped at 1,024 UTF-8 bytes. The SQL and client use the same strict direction-aware `(created_at microseconds, C-collated native ID)` keyset, with five summaries plus one lookahead. Changing a filter or order starts again; direct report lookup cannot combine queue filters. IDs remain native bounded ASCII identifiers, not fabricated public UUIDs.

Explicit Delete Report uses a separate `intent=delete` detail context and the existing signed POST path. A current moderator with `report.delete` can open that minimal context by exact ID even without `report.update`; the rich queue and accept/refuse review remain independently update-gated. The deletion context exposes only report ID, category, target/reason identities, status, revision, undeleted state, and captured account authority/consequence. It never reads target content, source visibility, claimant, parents, report details, notes or submitter. An already reviewed report or a report whose target no longer exists remains deletable. Neither current target nor reason version is a deletion prerequisite because the native operation does not depend on either.

The shared native `delete_report` transaction preserves original behavior: set the report deletion/update timestamps, increment its revision, preserve status, body, notes and reviewer, and append exactly one equal-status native `delete` audit with the existing empty effect. It does not delete a report row, target, source event or peer copy. The signed consequence object is a precise explanation of this report-only mutation, not a replacement native audit effect. Receipt verification requires matching raw request hash/method/path/account, report ID and old/new versions, preserved old/new status, deletion flag, immutable context/consequence and authority revision.

Authority is captured from the bound actor row already locked in the request transaction: exact account identity, version and normalized role/grant digest. Service/admission and binding locks, the active-account fence and the native report lock remain in the same transaction. Deletion adds no target locks. The additive shared `observeAccount()` method reuses existing exact signer/service/catalogue account-response validation, adds no grants and leaves existing `account`/`verify` behavior unchanged. It returns immutable current facts and checks freshness again after the response. Moderator deletion compares those facts before dispatch and after the receipt. Role/grant sorting uses UTF-8 byte order on both sides, including non-ASCII values.

Deletion shares the existing per-account/service/identity/report uncertainty key with accept/refuse. After awaited authority checks, the controller rechecks current state and pending marker immediately before installing its own marker, without an intervening await. It clears a marker only when the stored digest still belongs to its own attempt. This is a same-session concurrency safeguard, not a cross-tab atomic-storage claim. No reload reconciles an uncertain mutation automatically.

The authored successor tests cover report deletion in every native status; orphaned and source-hidden targets; target-lock independence; delete-only moderator positive and wrong-role/grant negative cases; authority/report drift; strict cross-action separation; exact native audit and retained content; same-session cross-controller fence races; wrong-owner markers; malformed or absent private receipts; second-response provenance swaps; Unicode authority ordering; full filter/cursor scope; and ascending/descending timestamp ties. Tests are synthetic only and must be executed under explicit resource admission. No result counts above qualify this successor until separate evidence is recorded.

The exact shared-scope predecessor is retained under `tests/fixtures/signed-moderator-authority/`, with independently pinned predecessor and successor hashes in the noGit test. These bytes are comparison-only source witnesses, never runtime substitutes. The original frontend attachment manifest and the three personal-inspection source files remain unchanged.


### Separate successor execution evidence

On 2026-10-22, exact source `aab3c0308159b0835ce2b2218ae28557bf9fdf43` passed 111/111 serial controller/shared-account-observation/parity controls (0.891 seconds) and 16/16 actual Svelte mounted controls (1.354 seconds), with no skipped tests. The competing-controller barrier, marker ownership, swapped provenance, UTF-8 ordering and explicit deletion-only review/cancel/confirm transitions ran against the actual successor controllers and component. Existing Svelte 5.55.1, TypeScript 5.9.3 and JSDOM 27.0.1 dependencies were resolved from retained qualification metadata, with no installation. Exact commands, dependency paths, bounds, elapsed times and complete output are retained under `tests/evidence/signed-moderator-parity-20261022/`.

Each batch had a 30-second wall bound and 65,536-byte output bound. Both exited zero without reaching either bound; mounted owned generation directories were absent at completion. A temporary root-only symlink supplied the unchanged original frontend tree for its 679-file witness and was removed after execution; no source or fixture module was replaced with predecessor code. The sparse source and Git metadata remained below the admitted 3 MiB budget.

These successor results are synthetic transport/controller and actual mounted-component evidence only. They do not qualify the authored Rust test, actual signed HTTP authorization, PostgreSQL lock scheduling/native audit behavior, a built browser application, or deployment. No real report or content was deleted. Backend compilation and the isolated synthetic PostgreSQL test remain pending and must use separate resource admission.
