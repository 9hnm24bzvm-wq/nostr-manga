# Signed native catalogue discussion reports

This bounded addition lets an explicitly selected signed account report the actual stored thread or an active, version-captured reply of a manga, chapter or group. It uses the existing private instance moderation tables and audit, not protocol-source comments/reports. It adds no moderator acceptance/refusal operation, cookie fallback, fabricated thread, account grant or source publication.

## Routes and authority

- GET `/api/v1/nostr/{manga|chapter|group}/{publicId}/discussion/reports/context` requires a signed request and the exact expected account/thread. Reply context also requires the actual reply ID and version.
- POST at the corresponding `/discussion/reports` path seals expected account, preferred typed target, actual stored thread, optional reply/version, enabled native reason/version and details. Unknown/duplicate/null/irrelevant command fields fail closed. Details retain native Unicode trimming and the 20,000 UTF-8-byte limit; only those exact native report POST paths receive the 131,072-byte escaped JSON body allowance.
- Native-user behavior and provenance remain separate and unchanged. The shared bounded reason reader accepts at most 1,000 complete reasons, bounds each ID/text before transfer, and checks the entire escaped response against 1 MiB. No truncation is accepted.

## Transaction lock graph

Account admission/binding and the current native `report.create` grant precede private/action context. Creation takes the actor UPDATE NOWAIT lock before resource locks, then uses the existing generic typed-target resolver: alias row locks, catalogue row locks, genuine source event/blob-generation witnesses, actual stored thread SHARE NOWAIT, optional active reply SHARE NOWAIT with exact revision, enabled native reason SHARE with exact revision, then the shared duplicate/hourly-limit report and audit transaction. Actor-upgrade or resource contention refuses rather than waiting, preventing cookie-native audit-FK lock cycles. Group source visibility uses one real stored visible chapter witness; it never invents a publisher discussion.

## Client and UI

The selected account ID/name, typed resource ID, actual stored thread, optional reply/version, actual reason/version and private details appear in explicit review before submission. Account, signer, service, namespace, route, page, target and request lifetimes remain captured. Generic discussion and report mutations mutually fence running or uncertain operations. Only an exact attested receipt or a known refusal/provably unsent request clears its own marker; reads, inspection, cancellation and mode changes cannot clear uncertain writes. Native-user existing separate behavior is preserved.

The generic UI links uncertain reports to `/reports/signed/native`, whose personal inspection implementation is separately integrated through `e0c59e5`. This branch does not duplicate or extend that inspection backend. Integration requires that existing route.

## Qualification limits

Finite synthetic transport/controller and pinned Svelte/JSDOM checks pass. They are not real signature, TCP/Postgres, native browser, build, deployment or production acceptance. Original 679 frontend files retain their recorded hashes; all changes are backend or additive integration/test/docs files. Original native-user report/discussion authority tests and profile UI are retained unchanged.

`backend/tests/public_native_discussion_reports.rs` adds actual signed HTTP, actor permission/rebind barriers, duplicate/hourly limits, bounded body, cookie isolation, native-cookie audit-FK collision, reason-version retention, atomic rollback, typed resource/thread/reply contention, source event/blob/parent/group witness withdrawal and missing generation tests. These tests and new Rust parser/router tests are written but UNEXECUTED. No Rust compile, service, native browser or publishing was performed in this resource-bounded slice. The original native-user runtime suites must still run alongside them in the integrator's isolated environment.

Detailed finite receipts and resource measurements: `test-evidence/generic-native-reporting-20261022/`.

## Separately scoped generic comment transport correction

The existing typed manga/chapter/group comment controller allowed 50,000 UTF-8 bytes, but its transport still applied the ordinary 32 KiB request and 1 MiB JSON response ceilings. A distinct correction admits only exact typed generic discussion GET/POST routes to the already-established native discussion bounds: 393,216 serialized request bytes (POST with no query, JSON request/response) and 8 MiB response bytes. GET still supports its actual pagination/order query. Both the initial response allowance and final declared/streamed-body clamp use the same narrow classification. The existing user classification and protocol report/default/global bounds remain unchanged.

Finite controls submit actual reviewed controller plans at 32 KiB and 50,000 UTF-8 bytes, including maximal Unicode and JSON escaping; reject oversized plans/envelopes and wrong method/query/ID/kind/media/sibling/source routes; accept twenty maximally escaped native comments; and reject oversized declared and streamed responses. Exact hash-pinned pre-fix production transport counterfactuals reject the otherwise admissible generic 50K request and 6 MB feed. The first maximal-feed controls revealed the second body clamp and were retained as negative evidence before the corrected final run. These are still synthetic transport checks, not runtime HTTP acceptance.
